//! Handles of a gradient on the canvas: where they are and what dragging one changes.
//! Positions are in the shape's own `w` x `h` box.

use dv_core::{Gradient, GradientKind};
use vello::kurbo::{Point, Vec2};

use crate::gradient::linear_line;

/// A draggable point of a gradient.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GradHandle {
    /// Where a linear gradient starts (its first stop).
    Start,
    /// Where a linear gradient ends.
    End,
    /// The center: moves the whole gradient.
    Center,
    /// Radial and diamond: how far the gradient reaches along x.
    Radius,
    /// Angular: where the sweep starts.
    Angle,
}

/// Distance of the angle handle from the center of an angular gradient (box units).
fn angle_reach(w: f64, h: f64) -> f64 {
    (w.min(h) / 2.0).max(8.0)
}

/// The handles of `g` and their positions.
pub fn handles(g: &Gradient, w: f64, h: f64) -> Vec<(GradHandle, Point)> {
    let c = Point::new(g.cx * w, g.cy * h);
    let scale = g.scale.max(0.001);
    match g.kind {
        GradientKind::Linear => {
            let (a, b) = linear_line(g, w, h);
            vec![
                (GradHandle::Start, a),
                (GradHandle::End, b),
                (GradHandle::Center, c),
            ]
        }
        GradientKind::Radial | GradientKind::Diamond => vec![
            (GradHandle::Center, c),
            (GradHandle::Radius, Point::new(c.x + w / 2.0 * scale, c.y)),
        ],
        GradientKind::Angular => {
            let a = g.angle.to_radians();
            let r = angle_reach(w, h);
            vec![
                (GradHandle::Center, c),
                (GradHandle::Angle, c + Vec2::new(a.cos(), a.sin()) * r),
            ]
        }
    }
}

/// Moves `handle` of `g` to `p` (in the box).
pub fn drag(g: &mut Gradient, handle: GradHandle, p: Point, w: f64, h: f64) {
    let (w, h) = (w.max(1e-6), h.max(1e-6));
    let c = Point::new(g.cx * w, g.cy * h);
    match handle {
        GradHandle::Center => {
            g.cx = p.x / w;
            g.cy = p.y / h;
        }
        GradHandle::Start | GradHandle::End => {
            // The other end stays where it is.
            let (a, b) = linear_line(g, w, h);
            let (fixed, moved) = if handle == GradHandle::End {
                (a, p)
            } else {
                (b, p)
            };
            let (from, to) = if handle == GradHandle::End {
                (fixed, moved)
            } else {
                (moved, fixed)
            };
            let d = to - from;
            if d.hypot() < 1e-6 {
                return;
            }
            let u = d / d.hypot();
            let cover = w * u.x.abs() + h * u.y.abs();
            let mid = from.midpoint(to);
            g.angle = d.y.atan2(d.x).to_degrees();
            g.scale = d.hypot() / cover.max(1e-6);
            g.cx = mid.x / w;
            g.cy = mid.y / h;
        }
        GradHandle::Radius => g.scale = ((p.x - c.x).abs() / (w / 2.0)).max(0.01),
        GradHandle::Angle => {
            let d = p - c;
            if d.hypot() > 1e-6 {
                g.angle = d.y.atan2(d.x).to_degrees();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dv_core::Color;

    fn g(kind: GradientKind) -> Gradient {
        Gradient::from_color(kind, Color::rgb(0, 0, 0))
    }

    fn at(list: &[(GradHandle, Point)], h: GradHandle) -> Point {
        list.iter().find(|(k, _)| *k == h).unwrap().1
    }

    #[test]
    fn linear_handles_sit_at_the_line_ends() {
        let l = handles(&g(GradientKind::Linear), 200.0, 100.0);
        assert_eq!(at(&l, GradHandle::Start), Point::new(0.0, 50.0));
        assert_eq!(at(&l, GradHandle::End), Point::new(200.0, 50.0));
        assert_eq!(at(&l, GradHandle::Center), Point::new(100.0, 50.0));
    }

    #[test]
    fn dragging_an_end_keeps_the_other_end_in_place() {
        let mut gr = g(GradientKind::Linear);
        drag(
            &mut gr,
            GradHandle::End,
            Point::new(100.0, 100.0),
            200.0,
            100.0,
        );
        let l = handles(&gr, 200.0, 100.0);
        let (a, b) = (at(&l, GradHandle::Start), at(&l, GradHandle::End));
        assert!(
            (a.x - 0.0).abs() < 1e-9 && (a.y - 50.0).abs() < 1e-9,
            "{a:?}"
        );
        assert!(
            (b.x - 100.0).abs() < 1e-9 && (b.y - 100.0).abs() < 1e-9,
            "{b:?}"
        );
        // And the same for the start.
        drag(
            &mut gr,
            GradHandle::Start,
            Point::new(0.0, 0.0),
            200.0,
            100.0,
        );
        let l = handles(&gr, 200.0, 100.0);
        let (a, b) = (at(&l, GradHandle::Start), at(&l, GradHandle::End));
        assert!(a.distance(Point::new(0.0, 0.0)) < 1e-9, "{a:?}");
        assert!(b.distance(Point::new(100.0, 100.0)) < 1e-9, "{b:?}");
    }

    #[test]
    fn the_center_moves_the_gradient() {
        let mut gr = g(GradientKind::Radial);
        drag(
            &mut gr,
            GradHandle::Center,
            Point::new(50.0, 25.0),
            200.0,
            100.0,
        );
        assert_eq!((gr.cx, gr.cy), (0.25, 0.25));
    }

    #[test]
    fn the_radius_handle_sets_the_scale() {
        let mut gr = g(GradientKind::Radial);
        drag(
            &mut gr,
            GradHandle::Radius,
            Point::new(200.0, 50.0),
            200.0,
            100.0,
        );
        assert_eq!(
            gr.scale, 1.0,
            "from the center to the right edge covers the box"
        );
        drag(
            &mut gr,
            GradHandle::Radius,
            Point::new(150.0, 50.0),
            200.0,
            100.0,
        );
        assert_eq!(gr.scale, 0.5);
        let l = handles(&gr, 200.0, 100.0);
        assert_eq!(at(&l, GradHandle::Radius), Point::new(150.0, 50.0));
    }

    #[test]
    fn the_angle_handle_turns_the_sweep() {
        let mut gr = g(GradientKind::Angular);
        drag(
            &mut gr,
            GradHandle::Angle,
            Point::new(100.0, 100.0),
            200.0,
            100.0,
        );
        assert!((gr.angle - 90.0).abs() < 1e-9);
    }
}
