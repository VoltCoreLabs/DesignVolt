//! Look effects: layer opacity, drop shadow and stroke styles.

use dv_core::{Color, EffectKind, Node, NodeKind, Page, Stroke as DStroke, StrokeStyle};
use vello::Scene;
use vello::kurbo::{Affine, Cap, Rect, Shape, Stroke};
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
}
