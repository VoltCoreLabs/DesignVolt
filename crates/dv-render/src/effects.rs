//! Look effects: layer opacity, drop shadow and stroke styles.

use dv_core::{Color, Node, NodeKind, Page, Stroke as DStroke, StrokeStyle};
use vello::Scene;
use vello::kurbo::{Affine, Cap, Rect, Stroke};
use vello::peniko::Color as VColor;

/// The opacity of a node together with all the groups and frames above it.
pub fn effective_opacity(page: &Page, node: &Node) -> f64 {
    let mut o = node.opacity;
    let mut cur = node.parent;
    while let Some(p) = cur {
        let Some(n) = page.node(p) else { break };
        o *= n.opacity;
        cur = n.parent;
    }
    o.clamp(0.0, 1.0)
}

/// How far the effects of a node reach past its box (stroke, shadow and its offset).
pub fn reach(node: &Node) -> f64 {
    let stroke = node.stroke.map_or(0.0, |s| s.width);
    let shadow = node
        .shadow
        .map_or(0.0, |s| s.blur * 2.0 + s.x.abs().max(s.y.abs()));
    stroke + shadow + 2.0
}

fn color(c: Color) -> VColor {
    VColor::from_rgba8(c.r, c.g, c.b, c.a)
}

/// Draws the soft shadow of a node under its fill.
pub fn draw_shadow(scene: &mut Scene, world: Affine, node: &Node) {
    let Some(sh) = node.shadow else { return };
    if node.fill.is_none() && node.stroke.is_none() || node.kind == NodeKind::Group {
        return;
    }
    let g = &node.geom;
    let radius = if node.kind == NodeKind::Ellipse {
        g.w.min(g.h) / 2.0
    } else {
        node.radius
    };
    let at = world * Affine::translate((sh.x, sh.y));
    scene.draw_blurred_rounded_rect(
        at,
        Rect::new(0.0, 0.0, g.w, g.h),
        color(sh.color),
        radius,
        (sh.blur / 2.0).max(0.01),
    );
}

/// The Vello stroke for the document stroke: width, dashes and caps.
pub fn line(s: &DStroke) -> Stroke {
    let w = s.width;
    match s.style {
        StrokeStyle::Solid => Stroke::new(w),
        StrokeStyle::Dashed => Stroke::new(w).with_dashes(0.0, [w * 3.0, w * 2.0]),
        StrokeStyle::Dotted => Stroke::new(w)
            .with_caps(Cap::Round)
            .with_dashes(0.0, [0.001, w * 2.0]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dv_core::{Command, Document, Geometry, NodeId, Props};

    #[test]
    fn opacity_multiplies_up_the_tree() {
        let mut doc = Document::default();
        let add = |doc: &mut Document, parent: Option<NodeId>| {
            let id = doc.alloc_id();
            let index = doc.page().siblings(parent).len();
            let node = Node::new(NodeKind::Frame, "n", Geometry::default());
            doc.apply(&Command::Create {
                id,
                parent,
                index,
                node,
            })
            .unwrap();
            id
        };
        let a = add(&mut doc, None);
        let b = add(&mut doc, Some(a));
        for (id, o) in [(a, 0.5), (b, 0.5)] {
            let props = Props {
                opacity: Some(o),
                ..Props::default()
            };
            doc.apply(&Command::SetProps { id, props }).unwrap();
        }
        let page = doc.page();
        assert_eq!(effective_opacity(page, page.node(b).unwrap()), 0.25);
        assert_eq!(effective_opacity(page, page.node(a).unwrap()), 0.5);
    }

    #[test]
    fn reach_covers_stroke_and_shadow() {
        let mut n = Node::new(NodeKind::Rectangle, "n", Geometry::default());
        let plain = reach(&n);
        n.shadow = Some(dv_core::Shadow {
            x: 10.0,
            y: -20.0,
            blur: 5.0,
            ..dv_core::Shadow::DEFAULT
        });
        assert!(reach(&n) >= plain + 30.0);
    }
}
