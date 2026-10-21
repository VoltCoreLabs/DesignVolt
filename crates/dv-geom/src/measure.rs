//! Distances between two rectangles, shown while Alt is held (like Figma's measurements).

use kurbo::{Point, Rect};

/// One measured distance: a segment from `a` to `b`, `len` page units long.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measure {
    pub a: Point,
    pub b: Point,
    pub len: f64,
}

const EPS: f64 = 1e-6;

/// Distances from `from` to `to`. Disjoint on an axis: the gap between them. One inside the
/// other on an axis: the two gaps between their edges. Partial overlap: nothing on that axis.
/// Each segment sits in the middle of the shared range of the other axis.
pub fn measure(from: Rect, to: Rect) -> Vec<Measure> {
    let mut out = Vec::new();
    axis(
        from.x0,
        from.x1,
        to.x0,
        to.x1,
        (from.y0, from.y1),
        (to.y0, to.y1),
        |p, q, len| {
            out.push(Measure {
                a: Point::new(p, q),
                b: Point::new(p + len, q),
                len,
            });
        },
    );
    axis(
        from.y0,
        from.y1,
        to.y0,
        to.y1,
        (from.x0, from.x1),
        (to.x0, to.x1),
        |p, q, len| {
            out.push(Measure {
                a: Point::new(q, p),
                b: Point::new(q, p + len),
                len,
            });
        },
    );
    out
}

/// Gaps along one axis (`a0..a1` against `b0..b1`); `emit(start, cross, len)` per gap.
fn axis(
    a0: f64,
    a1: f64,
    b0: f64,
    b1: f64,
    cross_a: (f64, f64),
    cross_b: (f64, f64),
    mut emit: impl FnMut(f64, f64, f64),
) {
    let lo = cross_a.0.max(cross_b.0);
    let hi = cross_a.1.min(cross_b.1);
    let cross = if lo <= hi {
        (lo + hi) / 2.0
    } else {
        (cross_a.0 + cross_a.1) / 2.0
    };
    if a1 < b0 - EPS {
        emit(a1, cross, b0 - a1);
    } else if b1 < a0 - EPS {
        emit(b1, cross, a0 - b1);
    } else if a0 <= b0 + EPS && b1 <= a1 + EPS {
        // `b` inside `a`.
        for (s, len) in [(a0, b0 - a0), (b1, a1 - b1)] {
            if len > EPS {
                emit(s, cross, len);
            }
        }
    } else if b0 <= a0 + EPS && a1 <= b1 + EPS {
        for (s, len) in [(b0, a0 - b0), (a1, b1 - a1)] {
            if len > EPS {
                emit(s, cross, len);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x0: f64, y0: f64, x1: f64, y1: f64) -> Rect {
        Rect::new(x0, y0, x1, y1)
    }

    #[test]
    fn side_by_side_boxes_show_the_gap_at_the_shared_height() {
        // Taller box vs. a shorter one: the horizontal gap plus its two vertical offsets.
        let m = measure(r(0.0, 0.0, 100.0, 100.0), r(150.0, 20.0, 250.0, 60.0));
        assert_eq!(m.len(), 3);
        let gap = m.iter().find(|m| m.len == 50.0).unwrap();
        assert_eq!(
            (gap.a, gap.b),
            (Point::new(100.0, 40.0), Point::new(150.0, 40.0))
        );
    }

    #[test]
    fn a_box_inside_another_shows_four_gaps() {
        let m = measure(r(0.0, 0.0, 100.0, 100.0), r(10.0, 20.0, 60.0, 70.0));
        let mut lens: Vec<f64> = m.iter().map(|m| m.len).collect();
        lens.sort_by(f64::total_cmp);
        assert_eq!(lens, [10.0, 20.0, 30.0, 40.0]);
    }

    #[test]
    fn diagonal_boxes_show_both_gaps_and_partial_overlap_shows_none() {
        let m = measure(r(0.0, 0.0, 10.0, 10.0), r(30.0, 50.0, 40.0, 60.0));
        assert_eq!(m.len(), 2);
        assert!(measure(r(0.0, 0.0, 10.0, 10.0), r(5.0, 5.0, 15.0, 15.0)).is_empty());
    }

    #[test]
    fn touching_edges_have_no_gap() {
        assert!(measure(r(0.0, 0.0, 10.0, 10.0), r(10.0, 0.0, 20.0, 10.0)).is_empty());
    }
}
