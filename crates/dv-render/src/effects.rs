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

/// Most samples per axis of the layer blur.
const MAX_SAMPLES: usize = 12;
const MIN_SAMPLES: usize = 4;

/// The inverse of the standard normal CDF (Abramowitz and Stegun 26.2.23, error < 5e-4).
fn normal_quantile(p: f64) -> f64 {
    let q = p.min(1.0 - p);
    let t = (-2.0 * q.ln()).sqrt();
    let x = t
        - (2.515_517 + 0.802_853 * t + 0.010_328 * t * t)
            / (1.0 + 1.432_788 * t + 0.189_269 * t * t + 0.001_308 * t * t * t);
    if p < 0.5 { -x } else { x }
}

/// Samples per axis for a blur of deviation `sigma_px` screen pixels: enough that the copies
/// overlap into a smooth result, few when the blur is small.
pub fn blur_samples(sigma_px: f64) -> usize {
    ((sigma_px / 2.5).ceil() as usize).clamp(MIN_SAMPLES, MAX_SAMPLES)
}

/// `n` offsets along one axis that carry equal weight under a Gaussian of deviation `sigma`.
pub fn blur_offsets(sigma: f64, n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| sigma * normal_quantile((i as f64 + 0.5) / n as f64))
        .collect()
}

/// Blurs anything `draw` paints (Vello only blurs a solid rounded box itself). A Gaussian is
/// separable, so the shape is drawn on a grid of horizontal and vertical offsets: every
/// column is a group of equally weighted copies added together, and the groups of all rows
/// are added again. That averages `n * n` copies with two small weights instead of one tiny
/// one, which keeps the 8-bit layers from banding.
pub fn blurred(
    scene: &mut Scene,
    world: Affine,
    clip: &Rect,
    blur: f64,
    draw: impl Fn(&mut Scene, Affine),
) {
    let sigma = (blur / 2.0).max(0.01);
    let [a, b, c, d, ..] = world.as_coeffs();
    let zoom = (a * d - b * c).abs().sqrt();
    let n = blur_samples(sigma * zoom);
    let add = BlendMode::new(Mix::Normal, Compose::Plus);
    let weight = 1.0 / n as f32;
    let offs = blur_offsets(sigma, n);
    scene.push_layer(Fill::NonZero, BlendMode::default(), 1.0, world, clip);
    for dy in &offs {
        scene.push_layer(Fill::NonZero, add, weight, world, clip);
        scene.push_layer(Fill::NonZero, BlendMode::default(), 1.0, world, clip);
        for dx in &offs {
            scene.push_layer(Fill::NonZero, add, weight, world, clip);
            draw(scene, world * Affine::translate((*dx, *dy)));
            scene.pop_layer();
        }
        scene.pop_layer();
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
    fn blur_offsets_are_symmetric_and_spread_like_the_deviation() {
        for n in [4, 7, 12] {
            let o = blur_offsets(10.0, n);
            assert_eq!(o.len(), n);
            let mean = o.iter().sum::<f64>() / n as f64;
            assert!(mean.abs() < 1e-3, "mean {mean}");
            assert!((o[0] + o[n - 1]).abs() < 1e-3, "symmetric");
            let var = o.iter().map(|x| x * x).sum::<f64>() / n as f64;
            // Equal-weight quantiles of N(0, 10^2) have a little less spread than the whole.
            assert!((0.6..=1.1).contains(&(var / 100.0)), "n {n} variance {var}");
        }
    }

    #[test]
    fn small_blurs_use_few_samples_and_large_ones_are_capped() {
        assert_eq!(blur_samples(1.0), MIN_SAMPLES);
        assert!(blur_samples(20.0) > MIN_SAMPLES);
        assert_eq!(blur_samples(1000.0), MAX_SAMPLES);
    }
}
