//! Resize handles of the selection box: positions, hit-testing and the resize math.

use kurbo::{Point, Rect};

use crate::input::Cursor;

/// Drawn size of a handle (px).
pub const HANDLE_PX: f64 = 8.0;
/// Extra px around a handle that still count as a hit.
const HIT_SLOP: f64 = 4.0;
/// Nodes never get smaller than this (page units).
pub const MIN_SIZE: f64 = 1.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Handle {
    Nw,
    N,
    Ne,
    E,
    Se,
    S,
    Sw,
    W,
}

impl Handle {
    pub const ALL: [Handle; 8] = [
        Handle::Nw,
        Handle::N,
        Handle::Ne,
        Handle::E,
        Handle::Se,
        Handle::S,
        Handle::Sw,
        Handle::W,
    ];

    fn left(self) -> bool {
        matches!(self, Handle::Nw | Handle::W | Handle::Sw)
    }

    fn right(self) -> bool {
        matches!(self, Handle::Ne | Handle::E | Handle::Se)
    }

    fn top(self) -> bool {
        matches!(self, Handle::Nw | Handle::N | Handle::Ne)
    }

    fn bottom(self) -> bool {
        matches!(self, Handle::Sw | Handle::S | Handle::Se)
    }

    /// Where the handle sits on `r`.
    pub fn position(self, r: Rect) -> Point {
        let x = if self.left() {
            r.x0
        } else if self.right() {
            r.x1
        } else {
            r.center().x
        };
        let y = if self.top() {
            r.y0
        } else if self.bottom() {
            r.y1
        } else {
            r.center().y
        };
        Point::new(x, y)
    }

    pub fn cursor(self) -> Cursor {
        match self {
            Handle::Nw | Handle::Se => Cursor::ResizeNwse,
            Handle::Ne | Handle::Sw => Cursor::ResizeNesw,
            Handle::N | Handle::S => Cursor::ResizeNs,
            Handle::E | Handle::W => Cursor::ResizeEw,
        }
    }
}

/// The handle under the screen point `p` for a selection box `bounds` (screen space).
pub fn handle_at(bounds: Rect, p: Point) -> Option<Handle> {
    let reach = HANDLE_PX / 2.0 + HIT_SLOP;
    // Corners first: on tiny boxes they win over the edge handles.
    let order = [
        Handle::Nw,
        Handle::Ne,
        Handle::Se,
        Handle::Sw,
        Handle::N,
        Handle::E,
        Handle::S,
        Handle::W,
    ];
    order.into_iter().find(|h| {
        let c = h.position(bounds);
        (p.x - c.x).abs() <= reach && (p.y - c.y).abs() <= reach
    })
}

/// New selection rectangle when `handle` is dragged to `cursor` (all page space).
/// The result is normalized (never negative) and at least `MIN_SIZE` big; with `keep_aspect`
/// the original proportions are kept.
pub fn resize_rect(handle: Handle, orig: Rect, cursor: Point, keep_aspect: bool) -> Rect {
    let (mut x0, mut y0, mut x1, mut y1) = (orig.x0, orig.y0, orig.x1, orig.y1);
    if handle.left() {
        x0 = cursor.x;
    }
    if handle.right() {
        x1 = cursor.x;
    }
    if handle.top() {
        y0 = cursor.y;
    }
    if handle.bottom() {
        y1 = cursor.y;
    }
    let mut r = Rect::new(x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1));
    if keep_aspect && orig.width() > 0.0 && orig.height() > 0.0 {
        let ratio = orig.width() / orig.height();
        let (w, h) = (r.width(), r.height());
        // Follow whichever side changed more (relative to the original).
        let by_width = (w / orig.width() - 1.0).abs() >= (h / orig.height() - 1.0).abs();
        let (nw, nh) = if by_width {
            (w, w / ratio)
        } else {
            (h * ratio, h)
        };
        let (nw, nh) = if matches!(handle, Handle::N | Handle::S) {
            (h * ratio, h)
        } else if matches!(handle, Handle::E | Handle::W) {
            (w, w / ratio)
        } else {
            (nw, nh)
        };
        // Keep the corner/edge opposite to the handle fixed.
        let ax = if handle.left() { orig.x1 } else { orig.x0 };
        let ay = if handle.top() { orig.y1 } else { orig.y0 };
        let (cx, cy) = (r.center().x, r.center().y);
        let (rx0, rx1) = if handle.left() {
            (ax - nw, ax)
        } else if handle.right() {
            (ax, ax + nw)
        } else {
            (cx - nw / 2.0, cx + nw / 2.0)
        };
        let (ry0, ry1) = if handle.top() {
            (ay - nh, ay)
        } else if handle.bottom() {
            (ay, ay + nh)
        } else {
            (cy - nh / 2.0, cy + nh / 2.0)
        };
        r = Rect::new(rx0, ry0, rx1, ry1);
    }
    if r.width() < MIN_SIZE {
        let grow = MIN_SIZE - r.width();
        if handle.left() {
            r.x0 -= grow;
        } else {
            r.x1 += grow;
        }
    }
    if r.height() < MIN_SIZE {
        let grow = MIN_SIZE - r.height();
        if handle.top() {
            r.y0 -= grow;
        } else {
            r.y1 += grow;
        }
    }
    r
}

/// Where `p` (inside `old`) lands when `old` is stretched into `new`.
pub fn map_point(old: Rect, new: Rect, p: Point) -> Point {
    let (sx, sy) = scale(old, new);
    Point::new(new.x0 + (p.x - old.x0) * sx, new.y0 + (p.y - old.y0) * sy)
}

/// Horizontal and vertical stretch factors from `old` to `new`.
pub fn scale(old: Rect, new: Rect) -> (f64, f64) {
    let f = |a: f64, b: f64| if a.abs() < 1e-9 { 1.0 } else { b / a };
    (f(old.width(), new.width()), f(old.height(), new.height()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x0: f64, y0: f64, x1: f64, y1: f64) -> Rect {
        Rect::new(x0, y0, x1, y1)
    }

    fn same(a: Rect, b: Rect) -> bool {
        (a.x0 - b.x0).abs() < 1e-9
            && (a.y0 - b.y0).abs() < 1e-9
            && (a.x1 - b.x1).abs() < 1e-9
            && (a.y1 - b.y1).abs() < 1e-9
    }

    #[test]
    fn handles_sit_on_the_box() {
        let b = r(0.0, 0.0, 100.0, 50.0);
        assert_eq!(Handle::Nw.position(b), Point::new(0.0, 0.0));
        assert_eq!(Handle::Se.position(b), Point::new(100.0, 50.0));
        assert_eq!(Handle::N.position(b), Point::new(50.0, 0.0));
        assert_eq!(Handle::W.position(b), Point::new(0.0, 25.0));
    }

    #[test]
    fn hit_testing_finds_handles_and_misses_the_interior() {
        let b = r(100.0, 100.0, 300.0, 200.0);
        assert_eq!(handle_at(b, Point::new(101.0, 99.0)), Some(Handle::Nw));
        assert_eq!(handle_at(b, Point::new(200.0, 100.0)), Some(Handle::N));
        assert_eq!(handle_at(b, Point::new(300.0, 150.0)), Some(Handle::E));
        assert_eq!(handle_at(b, Point::new(200.0, 150.0)), None);
        assert_eq!(handle_at(b, Point::new(120.0, 120.0)), None);
    }

    #[test]
    fn dragging_a_side_moves_only_that_side() {
        let o = r(0.0, 0.0, 100.0, 100.0);
        assert!(same(
            resize_rect(Handle::E, o, Point::new(150.0, 70.0), false),
            r(0.0, 0.0, 150.0, 100.0)
        ));
        assert!(same(
            resize_rect(Handle::N, o, Point::new(30.0, -20.0), false),
            r(0.0, -20.0, 100.0, 100.0)
        ));
        assert!(same(
            resize_rect(Handle::Se, o, Point::new(60.0, 80.0), false),
            r(0.0, 0.0, 60.0, 80.0)
        ));
        assert!(same(
            resize_rect(Handle::Nw, o, Point::new(20.0, 10.0), false),
            r(20.0, 10.0, 100.0, 100.0)
        ));
    }

    #[test]
    fn dragging_past_the_opposite_side_flips_the_rectangle() {
        let o = r(0.0, 0.0, 100.0, 100.0);
        let n = resize_rect(Handle::E, o, Point::new(-40.0, 50.0), false);
        assert!(same(n, r(-40.0, 0.0, 0.0, 100.0)));
    }

    #[test]
    fn rectangles_never_collapse_below_the_minimum() {
        let o = r(0.0, 0.0, 100.0, 100.0);
        let n = resize_rect(Handle::E, o, Point::new(0.0, 50.0), false);
        assert!((n.width() - MIN_SIZE).abs() < 1e-9);
        let n = resize_rect(Handle::W, o, Point::new(100.0, 50.0), false);
        assert!((n.width() - MIN_SIZE).abs() < 1e-9);
        assert!((n.x1 - 100.0).abs() < 1e-9 || (n.x0 - 100.0).abs() < 1e-9);
    }

    #[test]
    fn keeping_the_aspect_ratio_scales_uniformly_around_the_fixed_corner() {
        let o = r(0.0, 0.0, 200.0, 100.0);
        let n = resize_rect(Handle::Se, o, Point::new(400.0, 130.0), true);
        assert!((n.width() / n.height() - 2.0).abs() < 1e-9);
        assert_eq!((n.x0, n.y0), (0.0, 0.0), "top-left corner stays");
        assert!(
            (n.width() - 400.0).abs() < 1e-9,
            "follows the side that changed more"
        );
        let n = resize_rect(Handle::Nw, o, Point::new(100.0, 50.0), true);
        assert_eq!((n.x1, n.y1), (200.0, 100.0), "bottom-right corner stays");
        assert!((n.width() / n.height() - 2.0).abs() < 1e-9);
    }

    #[test]
    fn mapping_stretches_points_between_rectangles() {
        let (old, new) = (r(0.0, 0.0, 100.0, 100.0), r(10.0, 20.0, 210.0, 70.0));
        assert_eq!(
            map_point(old, new, Point::new(0.0, 0.0)),
            Point::new(10.0, 20.0)
        );
        assert_eq!(
            map_point(old, new, Point::new(100.0, 100.0)),
            Point::new(210.0, 70.0)
        );
        assert_eq!(
            map_point(old, new, Point::new(50.0, 50.0)),
            Point::new(110.0, 45.0)
        );
        assert_eq!(scale(old, new), (2.0, 0.5));
        assert_eq!(
            scale(r(0.0, 0.0, 0.0, 10.0), new),
            (1.0, 5.0),
            "zero width does not divide by zero"
        );
    }

    #[test]
    fn cursors_match_the_handle_direction() {
        assert_eq!(Handle::Nw.cursor(), Cursor::ResizeNwse);
        assert_eq!(Handle::Sw.cursor(), Cursor::ResizeNesw);
        assert_eq!(Handle::S.cursor(), Cursor::ResizeNs);
        assert_eq!(Handle::W.cursor(), Cursor::ResizeEw);
    }
}
