//! Look effects: layer opacity, drop shadow and stroke styles.

use dv_core::{
    Color, EffectKind, LineCap, LineJoin, Node, NodeKind, Page, Stroke as DStroke, StrokePos,
    StrokeStyle,
};
use vello::Scene;
use vello::kurbo::{Affine, Cap, Join, Rect, Shape, Stroke};
use vello::peniko::{BlendMode, Color as VColor, Compose, Fill, Mix};

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

/// How far the effects of a node reach past its box (stroke, shadows, blur).
pub fn reach(node: &Node) -> f64 {
    node.effect_reach()
}

fn color(c: Color) -> VColor {
    VColor::from_rgba8(c.r, c.g, c.b, c.a)
}

fn corner_radius(node: &Node) -> f64 {
    let g = &node.geom;
    if node.kind == NodeKind::Ellipse {
        g.w.min(g.h) / 2.0
    } else {
        node.radius
    }
}

fn has_look(node: &Node) -> bool {
    node.kind != NodeKind::Group && (node.fill.is_some() || node.stroke.is_some())
}

/// Draws the drop shadows of a node, under its fill.
pub fn draw_drop_shadows(scene: &mut Scene, world: Affine, node: &Node) {
    if !has_look(node) {
        return;
    }
    let g = &node.geom;
    for e in node
        .effects
        .iter()
        .filter(|e| e.kind == EffectKind::DropShadow)
    {
        scene.draw_blurred_rounded_rect(
            world * Affine::translate((e.x, e.y)),
            Rect::new(0.0, 0.0, g.w, g.h),
            color(e.color),
            corner_radius(node),
            (e.blur / 2.0).max(0.01),
        );
    }
}

/// Draws the inner shadows of a node over its fill, clipped to its shape: a layer filled with
/// the shadow color from which the (offset, blurred) shape is cut out.
pub fn draw_inner_shadows(scene: &mut Scene, world: Affine, shape: &impl Shape, node: &Node) {
    if !has_look(node) {
        return;
    }
    let g = &node.geom;
    for e in node
        .effects
        .iter()
        .filter(|e| e.kind == EffectKind::InnerShadow)
    {
        let big = Rect::new(0.0, 0.0, g.w, g.h).inflate(e.blur * 4.0 + 100.0, e.blur * 4.0 + 100.0);
        scene.push_layer(Fill::NonZero, BlendMode::default(), 1.0, world, shape);
        scene.fill(Fill::NonZero, world, color(e.color), None, &big);
        let cut = BlendMode::new(Mix::Normal, Compose::DestOut);
        scene.push_layer(Fill::NonZero, cut, 1.0, world, &big);
        scene.draw_blurred_rounded_rect(
            world * Affine::translate((e.x, e.y)),
            Rect::new(0.0, 0.0, g.w, g.h),
            VColor::from_rgba8(0, 0, 0, 255),
            corner_radius(node),
            (e.blur / 2.0).max(0.01),
        );
        scene.pop_layer();
        scene.pop_layer();
    }
}

/// The layer blur of a node, if it has one (the largest radius wins).
pub fn layer_blur(node: &Node) -> Option<f64> {
    node.effects
        .iter()
        .filter(|e| e.kind == EffectKind::LayerBlur && e.blur > 0.0)
        .map(|e| e.blur)
        .reduce(f64::max)
}

/// Draws a blurred copy of the node's solid look instead of the sharp shape. Only the main
/// color of the fill is used and the shape is approximated by a rounded box.
pub fn draw_blurred(scene: &mut Scene, world: Affine, node: &Node, blur: f64) {
    let Some(fill) = &node.fill else { return };
    let g = &node.geom;
    scene.draw_blurred_rounded_rect(
        world,
        Rect::new(0.0, 0.0, g.w, g.h),
        color(fill.main_color()),
        corner_radius(node),
        (blur / 2.0).max(0.01),
    );
}

/// Samples of the layer blur approximation.
const BLUR_SAMPLES: usize = 24;

/// Offsets that spread equal weights like a 2D Gaussian of deviation `sigma`: radii are the
/// quantiles of the Gaussian, angles follow the golden angle.
pub fn blur_offsets(sigma: f64) -> Vec<(f64, f64)> {
    (0..BLUR_SAMPLES)
        .map(|i| {
            let u = (i as f64 + 0.5) / BLUR_SAMPLES as f64;
            let r = sigma * (-2.0 * (1.0 - u).ln()).sqrt();
            let a = i as f64 * 2.399_963_229_728_653;
            (r * a.cos(), r * a.sin())
        })
        .collect()
}

/// Blurs anything `draw` paints: it is drawn once per sample offset, each time into a layer
/// of equal weight that is added to the others, which averages the copies. Used where Vello
/// has no blurred primitive (gradients, images, strokes).
pub fn blurred(
    scene: &mut Scene,
    world: Affine,
    clip: &Rect,
    blur: f64,
    draw: impl Fn(&mut Scene, Affine),
) {
    let add = BlendMode::new(Mix::Normal, Compose::Plus);
    let weight = 1.0 / BLUR_SAMPLES as f32;
    scene.push_layer(Fill::NonZero, BlendMode::default(), 1.0, world, clip);
    for (dx, dy) in blur_offsets((blur / 2.0).max(0.01)) {
        let at = world * Affine::translate((dx, dy));
        scene.push_layer(Fill::NonZero, add, weight, world, clip);
        draw(scene, at);
        scene.pop_layer();
    }
    scene.pop_layer();
}

/// The Vello stroke for the document stroke: width, dashes, joins and caps. Inside and
/// outside strokes are drawn twice as wide and cut to the right side by `draw_stroke`.
pub fn line(s: &DStroke) -> Stroke {
    let w = if s.position == StrokePos::Center {
        s.width
    } else {
        s.width * 2.0
    };
    let join = match s.join {
        LineJoin::Miter => Join::Miter,
        LineJoin::Round => Join::Round,
        LineJoin::Bevel => Join::Bevel,
    };
    let cap = match s.cap {
        LineCap::Butt => Cap::Butt,
        LineCap::Round => Cap::Round,
        LineCap::Square => Cap::Square,
    };
    let base = Stroke::new(w)
        .with_join(join)
        .with_miter_limit(s.miter_limit())
        .with_caps(cap);
    match s.style {
        StrokeStyle::Solid => base,
        StrokeStyle::Dashed => base.with_dashes(0.0, [s.dash.max(0.01), s.gap.max(0.0)]),
        StrokeStyle::Dotted => base
            .with_caps(Cap::Round)
            .with_dashes(0.0, [0.001, s.width.max(0.5) * 2.0]),
    }
}

/// Draws the stroke of a node on the line, inside or outside of the shape.
pub fn draw_stroke(scene: &mut Scene, world: Affine, shape: &impl Shape, st: &DStroke) {
    let brush = color(st.color);
    let style = line(st);
    match st.position {
        StrokePos::Center => scene.stroke(&style, world, brush, None, shape),
        StrokePos::Inside => {
            scene.push_layer(Fill::NonZero, BlendMode::default(), 1.0, world, shape);
            scene.stroke(&style, world, brush, None, shape);
            scene.pop_layer();
        }
        StrokePos::Outside => {
            let area = shape
                .bounding_box()
                .inflate(st.width * 2.0 + 2.0, st.width * 2.0 + 2.0);
            scene.push_layer(Fill::NonZero, BlendMode::default(), 1.0, world, &area);
            scene.stroke(&style, world, brush, None, shape);
            let cut = BlendMode::new(Mix::Normal, Compose::DestOut);
            scene.push_layer(Fill::NonZero, cut, 1.0, world, &area);
            scene.fill(
                Fill::NonZero,
                world,
                VColor::from_rgba8(0, 0, 0, 255),
                None,
                shape,
            );
            scene.pop_layer();
            scene.pop_layer();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dv_core::{Command, Document, Effect, Geometry, NodeId, Props};

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
    fn miter_limit_follows_the_angle_and_inside_strokes_are_doubled() {
        let mut st = DStroke::new(Color::rgb(0, 0, 0), 3.0);
        assert!((st.miter_limit() - 4.0).abs() < 0.01);
        st.miter_angle = 90.0;
        assert!((st.miter_limit() - 1.414).abs() < 0.01);
        assert_eq!(line(&st).width, 3.0);
        st.position = StrokePos::Inside;
        assert_eq!(line(&st).width, 6.0);
    }

    #[test]
    fn reach_covers_stroke_and_shadow() {
        let mut n = Node::new(NodeKind::Rectangle, "n", Geometry::default());
        let plain = reach(&n);
        n.effects.push(Effect {
            x: 10.0,
            y: -20.0,
            blur: 5.0,
            ..Effect::new(EffectKind::DropShadow)
        });
        assert!(reach(&n) >= plain + 30.0);
    }

    #[test]
    fn blur_offsets_are_centered_and_spread_like_the_deviation() {
        let o = blur_offsets(10.0);
        assert_eq!(o.len(), BLUR_SAMPLES);
        let n = o.len() as f64;
        let (mx, my) = o.iter().fold((0.0, 0.0), |a, p| (a.0 + p.0, a.1 + p.1));
        assert!(mx.abs() / n < 3.0 && my.abs() / n < 3.0, "mean {mx} {my}");
        let var = o.iter().map(|p| p.0 * p.0 + p.1 * p.1).sum::<f64>() / n;
        // A 2D Gaussian has E[r^2] = 2 sigma^2.
        assert!((var / 200.0 - 1.0).abs() < 0.25, "variance {var}");
        assert!(o.iter().all(|p| p.0.hypot(p.1) < 30.0));
    }
}
