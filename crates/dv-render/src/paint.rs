//! Page painting with culling (R-tree query) and level-of-detail skipping.

use dv_core::{Color, NodeKind, Page, Paint};
use dv_geom::SpatialIndex;
use vello::Scene;
use vello::kurbo::{Affine, Ellipse, Rect, RoundedRect};
use vello::peniko::{BlendMode, Color as VColor, Fill};

use crate::viewport::Viewport;

/// Nodes smaller than this on screen (px, both sides) are not drawn.
const MIN_VISIBLE_PX: f64 = 0.6;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PaintStats {
    /// Nodes whose bounds intersect the visible area.
    pub candidates: usize,
    /// Nodes actually sent to the scene.
    pub drawn: usize,
}

fn color(c: Color) -> VColor {
    VColor::from_rgba8(c.r, c.g, c.b, c.a)
}

/// Paints the visible nodes of `page` into `scene`. `size` is the canvas size in px and `base`
/// maps canvas coordinates to the scene (DPI scale, canvas offset in the window).
pub fn paint_page(
    scene: &mut Scene,
    page: &Page,
    index: &SpatialIndex,
    vp: &Viewport,
    size: (f64, f64),
    base: Affine,
) -> PaintStats {
    let mut entries: Vec<_> = index.query(vp.visible_world(size)).collect();
    entries.sort_unstable_by_key(|e| e.order);
    let mut stats = PaintStats {
        candidates: entries.len(),
        drawn: 0,
    };
    let view = base * vp.transform();
    for e in entries {
        if e.bounds.width() * vp.zoom < MIN_VISIBLE_PX
            && e.bounds.height() * vp.zoom < MIN_VISIBLE_PX
        {
            continue;
        }
        let Some(node) = page.node(e.id) else {
            continue;
        };
        let r = crate::effects::reach(node);
        let world = view * e.world;
        let opacity = crate::effects::effective_opacity(page, node);
        if opacity < 1.0 {
            let g = &node.geom;
            let clip = Rect::new(0.0, 0.0, g.w, g.h).inflate(r, r);
            scene.push_layer(
                Fill::NonZero,
                BlendMode::default(),
                opacity as f32,
                world,
                &clip,
            );
            draw_node(scene, page, node, world, true);
            scene.pop_layer();
        } else {
            draw_node(scene, page, node, world, true);
        }
        stats.drawn += 1;
    }
    stats
}

/// Draws one node (not its children) with its fill and stroke. `patterns` is off while a
/// pattern tile is drawn, so a tile never contains patterns itself.
pub(crate) fn draw_node(
    scene: &mut Scene,
    page: &Page,
    node: &dv_core::Node,
    world: Affine,
    patterns: bool,
) {
    let g = &node.geom;
    crate::effects::draw_shadow(scene, world, node);
    match node.kind {
        NodeKind::Group => {}
        NodeKind::Ellipse => {
            let shape = Ellipse::new((g.w / 2.0, g.h / 2.0), (g.w / 2.0, g.h / 2.0), 0.0);
            fill_and_stroke(scene, page, world, &shape, node, patterns);
        }
        NodeKind::Rectangle | NodeKind::Frame => {
            let rect = Rect::new(0.0, 0.0, g.w, g.h);
            let shape = RoundedRect::from_rect(rect, node.radius);
            fill_and_stroke(scene, page, world, &shape, node, patterns);
        }
    }
}

fn fill_and_stroke(
    scene: &mut Scene,
    page: &Page,
    world: Affine,
    shape: &impl vello::kurbo::Shape,
    node: &dv_core::Node,
    patterns: bool,
) {
    match &node.fill {
        Some(Paint::Pattern(p)) => {
            if patterns {
                crate::pattern::draw(scene, page, world, shape, node, p);
            }
        }
        Some(fill) => {
            crate::gradient::fill_paint(scene, world, shape, node.geom.w, node.geom.h, fill);
        }
        None => {}
    }
    if let Some(stroke) = node.stroke {
        scene.stroke(
            &crate::effects::line(&stroke),
            world,
            color(stroke.color),
            None,
            shape,
        );
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use dv_core::{Command, Document, Geometry, Node};
    use vello::kurbo::Vec2;

    fn doc_with_grid(n: usize, size: f64) -> Document {
        let mut doc = Document::default();
        for i in 0..n {
            let id = doc.alloc_id();
            let g = Geometry {
                x: (i % 10) as f64 * 100.0,
                y: (i / 10) as f64 * 100.0,
                w: size,
                h: size,
                rotation: 0.0,
            };
            doc.apply(&Command::Create {
                id,
                parent: None,
                index: i,
                node: Node::new(NodeKind::Rectangle, "r", g),
            })
            .unwrap();
        }
        doc
    }

    fn paint(doc: &Document, vp: &Viewport, size: (f64, f64)) -> PaintStats {
        let index = SpatialIndex::build(doc.page(), doc.revision());
        paint_page(
            &mut Scene::new(),
            doc.page(),
            &index,
            vp,
            size,
            Affine::IDENTITY,
        )
    }

    #[test]
    fn only_visible_nodes_are_drawn() {
        let doc = doc_with_grid(100, 50.0);
        let everything = paint(&doc, &Viewport::default().fit_all(), (1000.0, 1000.0));
        assert_eq!(everything.drawn, 100);
        let corner = paint(&doc, &Viewport::default(), (250.0, 250.0));
        assert_eq!(corner.candidates, 9, "3 x 3 nodes overlap a 250 px corner");
        assert_eq!(corner.drawn, 9);
    }

    #[test]
    fn nodes_far_outside_the_view_are_culled() {
        let doc = doc_with_grid(100, 50.0);
        let vp = Viewport {
            zoom: 1.0,
            pan: Vec2::new(-100_000.0, 0.0),
        };
        assert_eq!(paint(&doc, &vp, (800.0, 600.0)), PaintStats::default());
    }

    #[test]
    fn tiny_nodes_are_skipped_at_low_zoom() {
        let doc = doc_with_grid(100, 5.0);
        let vp = Viewport {
            zoom: 0.05,
            pan: Vec2::ZERO,
        };
        let s = paint(&doc, &vp, (1000.0, 1000.0));
        assert_eq!(s.candidates, 100);
        assert_eq!(s.drawn, 0, "5 px nodes are 0.25 px on screen");
    }

    impl Viewport {
        fn fit_all(mut self) -> Self {
            self.fit(Rect::new(0.0, 0.0, 1000.0, 1000.0), (1000.0, 1000.0), 0.0);
            self
        }
    }
}
