//! Bezier paths of pen-drawn nodes: building the curve and testing points against it.

use dv_core::{Node, Vector};
use kurbo::{BezPath, ParamCurveNearest, Point, Shape};

/// The curve of `v` scaled from the box it was drawn in to a `w` x `h` box.
pub fn vector_path(v: &Vector, w: f64, h: f64) -> BezPath {
    let sx = if v.size.0 > 0.0 { w / v.size.0 } else { 1.0 };
    let sy = if v.size.1 > 0.0 { h / v.size.1 } else { 1.0 };
    let mut path = BezPath::new();
    let Some(first) = v.anchors.first() else {
        return path;
    };
    path.move_to((first.x * sx, first.y * sy));
    let n = v.anchors.len();
    let segments = if v.closed { n } else { n - 1 };
    for i in 0..segments {
        let (a, b) = (&v.anchors[i], &v.anchors[(i + 1) % n]);
        let p1 = Point::new((a.x + a.out_x) * sx, (a.y + a.out_y) * sy);
        let p2 = Point::new((b.x + b.in_x) * sx, (b.y + b.in_y) * sy);
        let end = Point::new(b.x * sx, b.y * sy);
        if a.out_x == 0.0 && a.out_y == 0.0 && b.in_x == 0.0 && b.in_y == 0.0 {
            path.line_to(end);
        } else {
            path.curve_to(p1, p2, end);
        }
    }
    if v.closed {
        path.close_path();
    }
    path
}

/// Whether the node-local point `p` is on the path: inside its fill (when it has one) or
/// within reach of its stroke.
pub fn hit_path(node: &Node, p: Point) -> bool {
    let Some(v) = &node.vector else { return false };
    let path = vector_path(v, node.geom.w, node.geom.h);
    if node.fill.is_some() && path.contains(p) {
        return true;
    }
    let reach = node.stroke.map_or(0.0, |s| s.width / 2.0) + HIT_SLOP;
    path.segments()
        .any(|s| s.nearest(p, 1e-3).distance_sq <= reach * reach)
}

/// Extra distance around a thin line that still counts as a hit.
const HIT_SLOP: f64 = 3.0;

/// The tight box of the path with the given anchors, in the coordinates of the anchors.
pub fn anchors_bounds(v: &Vector) -> kurbo::Rect {
    vector_path(v, v.size.0, v.size.1).bounding_box()
}

#[cfg(test)]
mod tests {
    use super::*;
    use dv_core::{Anchor, Geometry, NodeKind, Stroke};

    fn line() -> Vector {
        Vector {
            anchors: vec![Anchor::corner(0.0, 0.0), Anchor::corner(100.0, 0.0)],
            closed: false,
            size: (100.0, 10.0),
        }
    }

    fn node(v: Vector, w: f64, h: f64) -> Node {
        let mut n = Node::new(
            NodeKind::Path,
            "p",
            Geometry {
                x: 0.0,
                y: 0.0,
                w,
                h,
                rotation: 0.0,
            },
        );
        n.fill = None;
        n.stroke = Some(Stroke::new(dv_core::Color::rgb(0, 0, 0), 2.0));
        n.vector = Some(v);
        n
    }

    #[test]
    fn the_path_is_scaled_with_the_box() {
        let p = vector_path(&line(), 200.0, 10.0);
        let b = p.bounding_box();
        assert_eq!((b.x1, b.y1), (200.0, 0.0));
    }

    #[test]
    fn a_stroke_is_hit_near_the_line_but_not_far_from_it() {
        let n = node(line(), 100.0, 10.0);
        assert!(hit_path(&n, Point::new(50.0, 2.0)));
        assert!(!hit_path(&n, Point::new(50.0, 8.0)));
        let wide = node(line(), 200.0, 10.0);
        assert!(
            hit_path(&wide, Point::new(150.0, 1.0)),
            "scaled with the box"
        );
    }

    #[test]
    fn a_filled_closed_path_is_hit_inside() {
        let v = Vector {
            anchors: vec![
                Anchor::corner(0.0, 0.0),
                Anchor::corner(100.0, 0.0),
                Anchor::corner(50.0, 80.0),
            ],
            closed: true,
            size: (100.0, 80.0),
        };
        let mut n = node(v, 100.0, 80.0);
        assert!(
            !hit_path(&n, Point::new(50.0, 30.0)),
            "no fill: only the line"
        );
        n.fill = Some(dv_core::Color::rgb(1, 2, 3).into());
        assert!(hit_path(&n, Point::new(50.0, 30.0)));
        assert!(!hit_path(&n, Point::new(5.0, 60.0)));
    }

    #[test]
    fn curves_use_the_handles() {
        let v = Vector {
            anchors: vec![
                Anchor::smooth(0.0, 0.0, 0.0, 50.0),
                Anchor::smooth(100.0, 0.0, 0.0, -50.0),
            ],
            closed: false,
            size: (100.0, 40.0),
        };
        let b = anchors_bounds(&v);
        assert!(b.y1 > 30.0, "the curve bulges away from the chord: {b:?}");
    }
}
