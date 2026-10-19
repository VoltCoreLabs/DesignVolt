//! Snapping of a moved or resized rectangle to the edges and centers of other rectangles, with
//! the guide lines that show what it snapped to (like Figma's smart guides).

use kurbo::Rect;

/// Which way a guide runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    /// A vertical line at some x.
    Vertical,
    /// A horizontal line at some y.
    Horizontal,
}

/// A line to draw while snapped: at `pos` on the other axis, from `from` to `to` along it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Guide {
    pub axis: Axis,
    pub pos: f64,
    pub from: f64,
    pub to: f64,
}

/// Result of snapping a moved rectangle: the correction to apply and the guides.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snap {
    pub dx: f64,
    pub dy: f64,
    pub guides: Vec<Guide>,
}

/// Which edges of a rectangle are being dragged (resize).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Edges {
    pub left: bool,
    pub right: bool,
    pub top: bool,
    pub bottom: bool,
}

const EPS: f64 = 1e-6;

/// Lines of a rectangle on one axis: start, middle, end.
fn lines(lo: f64, hi: f64) -> [f64; 3] {
    [lo, (lo + hi) / 2.0, hi]
}

/// The smallest correction that moves one of `mine` onto one of `theirs` within `threshold`.
fn best_delta(mine: &[f64], theirs: impl Iterator<Item = f64>, threshold: f64) -> Option<f64> {
    let mut best: Option<f64> = None;
    for t in theirs {
        for m in mine {
            let d = t - m;
            if d.abs() <= threshold && best.is_none_or(|b| d.abs() < b.abs() - EPS) {
                best = Some(d);
            }
        }
    }
    best
}

/// Snaps a moving rectangle: both axes independently, to the edges and centers of `others`.
pub fn snap_rect(moving: Rect, others: &[Rect], threshold: f64) -> Snap {
    let xs = lines(moving.x0, moving.x1);
    let ys = lines(moving.y0, moving.y1);
    let dx = best_delta(
        &xs,
        others.iter().flat_map(|o| lines(o.x0, o.x1)),
        threshold,
    );
    let dy = best_delta(
        &ys,
        others.iter().flat_map(|o| lines(o.y0, o.y1)),
        threshold,
    );
    let shifted = moving + kurbo::Vec2::new(dx.unwrap_or(0.0), dy.unwrap_or(0.0));
    let mut guides = Vec::new();
    if dx.is_some() {
        guides.extend(guides_for(shifted, others, Axis::Vertical));
    }
    if dy.is_some() {
        guides.extend(guides_for(shifted, others, Axis::Horizontal));
    }
    Snap {
        dx: dx.unwrap_or(0.0),
        dy: dy.unwrap_or(0.0),
        guides,
    }
}

/// Snaps only the dragged `edges` of a resized rectangle. Returns the new rectangle.
pub fn snap_edges(rect: Rect, edges: Edges, others: &[Rect], threshold: f64) -> (Rect, Vec<Guide>) {
    let mut r = rect;
    let candidates_x: Vec<f64> = others.iter().flat_map(|o| lines(o.x0, o.x1)).collect();
    let candidates_y: Vec<f64> = others.iter().flat_map(|o| lines(o.y0, o.y1)).collect();
    let mut snapped_x = false;
    let mut snapped_y = false;
    if edges.left
        && let Some(d) = best_delta(&[r.x0], candidates_x.iter().copied(), threshold)
    {
        r.x0 += d;
        snapped_x = true;
    }
    if edges.right
        && let Some(d) = best_delta(&[r.x1], candidates_x.iter().copied(), threshold)
    {
        r.x1 += d;
        snapped_x = true;
    }
    if edges.top
        && let Some(d) = best_delta(&[r.y0], candidates_y.iter().copied(), threshold)
    {
        r.y0 += d;
        snapped_y = true;
    }
    if edges.bottom
        && let Some(d) = best_delta(&[r.y1], candidates_y.iter().copied(), threshold)
    {
        r.y1 += d;
        snapped_y = true;
    }
    let mut guides = Vec::new();
    if snapped_x {
        guides.extend(guides_for(r, others, Axis::Vertical));
    }
    if snapped_y {
        guides.extend(guides_for(r, others, Axis::Horizontal));
    }
    (r, guides)
}

/// Guide lines along `axis` for every line of `r` that coincides with a line of some other
/// rectangle; each guide covers the rectangles it connects.
fn guides_for(r: Rect, others: &[Rect], axis: Axis) -> Vec<Guide> {
    let (mine, along) = match axis {
        Axis::Vertical => (lines(r.x0, r.x1), (r.y0, r.y1)),
        Axis::Horizontal => (lines(r.y0, r.y1), (r.x0, r.x1)),
    };
    let mut out: Vec<Guide> = Vec::new();
    for pos in mine {
        let mut span = along;
        let mut hit = false;
        for o in others {
            let (theirs, o_along) = match axis {
                Axis::Vertical => (lines(o.x0, o.x1), (o.y0, o.y1)),
                Axis::Horizontal => (lines(o.y0, o.y1), (o.x0, o.x1)),
            };
            if theirs.iter().any(|t| (t - pos).abs() < EPS) {
                hit = true;
                span = (span.0.min(o_along.0), span.1.max(o_along.1));
            }
        }
        let duplicate = out.iter().any(|g| (g.pos - pos).abs() < EPS);
        if hit && !duplicate {
            out.push(Guide {
                axis,
                pos,
                from: span.0,
                to: span.1,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x0: f64, y0: f64, x1: f64, y1: f64) -> Rect {
        Rect::new(x0, y0, x1, y1)
    }

    #[test]
    fn a_nearby_edge_snaps_onto_another_edge() {
        let other = r(100.0, 0.0, 200.0, 50.0);
        // The moving box's left edge is 3 units short of the other's left edge.
        let s = snap_rect(r(97.0, 200.0, 137.0, 240.0), &[other], 6.0);
        assert_eq!(s.dx, 3.0);
        assert_eq!(s.dy, 0.0, "nothing to snap vertically");
        assert_eq!(s.guides.len(), 1);
        let g = s.guides[0];
        assert_eq!((g.axis, g.pos), (Axis::Vertical, 100.0));
        assert_eq!(
            (g.from, g.to),
            (0.0, 240.0),
            "the guide connects both boxes"
        );
    }

    #[test]
    fn centers_snap_too() {
        let other = r(0.0, 0.0, 100.0, 100.0); // center 50, 50
        let s = snap_rect(r(131.0, 200.0, 171.0, 240.0), &[other], 6.0);
        // The moving center is 151: 1 unit away from the other's right edge at 100? No, 51.
        assert_eq!(s.dx, 0.0);
        let s = snap_rect(r(28.0, 200.0, 68.0, 240.0), &[other], 6.0);
        assert_eq!(s.dx, 2.0, "center 48 snaps to center 50");
    }

    #[test]
    fn nothing_snaps_beyond_the_threshold() {
        let other = r(100.0, 100.0, 200.0, 200.0);
        let s = snap_rect(r(70.0, 60.0, 110.0, 90.0), &[other], 5.0);
        assert_eq!((s.dx, s.dy), (0.0, 0.0));
        assert!(s.guides.is_empty());
    }

    #[test]
    fn the_closest_candidate_wins() {
        let a = r(100.0, 0.0, 110.0, 10.0);
        let b = r(103.0, 100.0, 113.0, 110.0);
        let s = snap_rect(r(101.0, 300.0, 121.0, 320.0), &[a, b], 6.0);
        assert_eq!(s.dx, -1.0, "snaps to a's edge at 100, not b's at 103");
    }

    #[test]
    fn both_axes_snap_independently() {
        let other = r(0.0, 0.0, 100.0, 100.0);
        let s = snap_rect(r(103.0, 98.0, 143.0, 138.0), &[other], 6.0);
        assert_eq!((s.dx, s.dy), (-3.0, 2.0));
        let axes: Vec<Axis> = s.guides.iter().map(|g| g.axis).collect();
        assert!(axes.contains(&Axis::Vertical) && axes.contains(&Axis::Horizontal));
    }

    #[test]
    fn resizing_snaps_only_the_dragged_edges() {
        let other = r(200.0, 0.0, 300.0, 100.0);
        let rect = r(0.0, 0.0, 197.0, 80.0);
        let right = Edges {
            right: true,
            ..Edges::default()
        };
        let (n, guides) = snap_edges(rect, right, &[other], 5.0);
        assert_eq!(n, r(0.0, 0.0, 200.0, 80.0));
        assert_eq!(guides.len(), 1);
        let left_only = Edges {
            left: true,
            ..Edges::default()
        };
        let (n, guides) = snap_edges(rect, left_only, &[other], 5.0);
        assert_eq!(n, rect, "the right edge was not being dragged");
        assert!(guides.is_empty());
    }

    #[test]
    fn a_corner_resize_can_snap_both_edges() {
        let other = r(200.0, 150.0, 300.0, 250.0);
        let corner = Edges {
            right: true,
            bottom: true,
            ..Edges::default()
        };
        let (n, guides) = snap_edges(r(0.0, 0.0, 198.0, 152.0), corner, &[other], 4.0);
        assert_eq!(n, r(0.0, 0.0, 200.0, 150.0));
        assert_eq!(guides.len(), 2);
    }

    #[test]
    fn no_candidates_means_no_snap() {
        let s = snap_rect(r(0.0, 0.0, 10.0, 10.0), &[], 10.0);
        assert_eq!(s, Snap::default());
        let (n, g) = snap_edges(
            r(0.0, 0.0, 10.0, 10.0),
            Edges {
                left: true,
                ..Edges::default()
            },
            &[],
            10.0,
        );
        assert_eq!(n, r(0.0, 0.0, 10.0, 10.0));
        assert!(g.is_empty());
    }
}
