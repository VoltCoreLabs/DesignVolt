//! Filling shapes with solid colors and gradients.

use dv_core::{Color, Gradient, GradientKind, Paint};
use vello::Scene;
use vello::kurbo::{Affine, Point, Rect, Shape, Vec2};
use vello::peniko::{Color as VColor, Fill, Gradient as VGradient};

fn color(c: Color) -> VColor {
    VColor::from_rgba8(c.r, c.g, c.b, c.a)
}

fn stops(g: &Gradient) -> Vec<(f32, VColor)> {
    g.stops
        .iter()
        .map(|s| (s.pos as f32, color(s.color)))
        .collect()
}

/// Start and end of a linear gradient over a `w` x `h` box: a line through the center at
/// `angle` degrees that is just long enough to cover the box (times `scale`).
pub fn linear_line(g: &Gradient, w: f64, h: f64) -> (Point, Point) {
    let a = g.angle.to_radians();
    let d = Vec2::new(a.cos(), a.sin());
    let half = (w * d.x.abs() + h * d.y.abs()) / 2.0 * g.scale.max(0.001);
    let c = Point::new(g.cx * w, g.cy * h);
    (c - d * half, c + d * half)
}

/// Fills `shape` (which lies in a `w` x `h` box) with the paint.
pub fn fill_paint(
    scene: &mut Scene,
    world: Affine,
    shape: &impl Shape,
    w: f64,
    h: f64,
    paint: &Paint,
) {
    match paint {
        Paint::Solid(c) => scene.fill(Fill::NonZero, world, color(*c), None, shape),
        Paint::Gradient(g) => fill_gradient(scene, world, shape, w, h, g),
        // A pattern needs the document to draw its tile: a flat stand-in here.
        Paint::Pattern(_) => {
            scene.fill(Fill::NonZero, world, color(paint.main_color()), None, shape);
        }
    }
}

fn fill_gradient(
    scene: &mut Scene,
    world: Affine,
    shape: &impl Shape,
    w: f64,
    h: f64,
    g: &Gradient,
) {
    let c = Point::new(g.cx * w, g.cy * h);
    let s = stops(g);
    match g.kind {
        GradientKind::Linear => {
            let (a, b) = linear_line(g, w, h);
            let grad = VGradient::new_linear(a, b).with_stops(s.as_slice());
            scene.fill(Fill::NonZero, world, &grad, None, shape);
        }
        GradientKind::Radial => {
            // A circle stretched to the box, so the gradient is an ellipse in a wide box.
            let r = w.max(1e-6) / 2.0 * g.scale.max(0.001);
            let grad = VGradient::new_radial(c, r as f32).with_stops(s.as_slice());
            let brush = Affine::translate(c.to_vec2())
                * Affine::scale_non_uniform(1.0, h / w.max(1e-6))
                * Affine::translate(-c.to_vec2());
            scene.fill(Fill::NonZero, world, &grad, Some(brush), shape);
        }
        GradientKind::Angular => {
            let a = g.angle.to_radians() as f32;
            let grad =
                VGradient::new_sweep(c, a, a + std::f32::consts::TAU).with_stops(s.as_slice());
            scene.fill(Fill::NonZero, world, &grad, None, shape);
        }
        GradientKind::Diamond => fill_diamond(scene, world, shape, (w, h), c, g, &s),
    }
}

/// The gradient value of a diamond is `|x|/rx + |y|/ry`, which is a plain linear ramp inside
/// each quadrant around the center: four clipped linear gradients.
fn fill_diamond(
    scene: &mut Scene,
    world: Affine,
    shape: &impl Shape,
    (w, h): (f64, f64),
    c: Point,
    g: &Gradient,
    stops: &[(f32, VColor)],
) {
    let (rx, ry) = (w / 2.0 * g.scale.max(0.001), h / 2.0 * g.scale.max(0.001));
    let (gx, gy) = (1.0 / rx.max(1e-6), 1.0 / ry.max(1e-6));
    let len2 = gx * gx + gy * gy;
    let big = 1e7;
    for (sx, sy) in [(1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
        let end = Point::new(c.x + sx * gx / len2, c.y + sy * gy / len2);
        let grad = VGradient::new_linear(c, end).with_stops(stops);
        let quad = Rect::new(
            if sx > 0.0 { c.x } else { c.x - big },
            if sy > 0.0 { c.y } else { c.y - big },
            if sx > 0.0 { c.x + big } else { c.x },
            if sy > 0.0 { c.y + big } else { c.y },
        );
        scene.push_clip_layer(Fill::NonZero, world, &quad);
        scene.fill(Fill::NonZero, world, &grad, None, shape);
        scene.pop_layer();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grad(angle: f64) -> Gradient {
        let mut g = Gradient::from_color(GradientKind::Linear, Color::rgb(0, 0, 0));
        g.angle = angle;
        g
    }

    #[test]
    fn linear_line_spans_the_box_along_the_angle() {
        let (a, b) = linear_line(&grad(0.0), 200.0, 100.0);
        assert!((a.x - 0.0).abs() < 1e-9 && (b.x - 200.0).abs() < 1e-9);
        assert!((a.y - 50.0).abs() < 1e-9 && (b.y - 50.0).abs() < 1e-9);
        let (a, b) = linear_line(&grad(90.0), 200.0, 100.0);
        assert!((a.y - 0.0).abs() < 1e-9 && (b.y - 100.0).abs() < 1e-9);
        let mut g = grad(0.0);
        g.scale = 0.5;
        let (a, b) = linear_line(&g, 200.0, 100.0);
        assert!((b.x - a.x - 100.0).abs() < 1e-9);
    }
}
