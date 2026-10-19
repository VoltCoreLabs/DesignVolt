//! Resizing of the borderless window: which edge the pointer is on and which cursor to show.

use winit::window::{CursorIcon, ResizeDirection};

/// Resize direction for a pointer at `(x, y)` in a window of `w`x`h` px (all physical),
/// or `None` when the pointer is farther than `border` px from every edge.
pub fn resize_direction(x: f64, y: f64, w: f64, h: f64, border: f64) -> Option<ResizeDirection> {
    if x < 0.0 || y < 0.0 || x > w || y > h {
        return None;
    }
    let (left, right) = (x < border, x > w - border);
    let (top, bottom) = (y < border, y > h - border);
    match (left, right, top, bottom) {
        (true, _, true, _) => Some(ResizeDirection::NorthWest),
        (_, true, true, _) => Some(ResizeDirection::NorthEast),
        (true, _, _, true) => Some(ResizeDirection::SouthWest),
        (_, true, _, true) => Some(ResizeDirection::SouthEast),
        (true, ..) => Some(ResizeDirection::West),
        (_, true, ..) => Some(ResizeDirection::East),
        (_, _, true, _) => Some(ResizeDirection::North),
        (_, _, _, true) => Some(ResizeDirection::South),
        _ => None,
    }
}

pub fn cursor_for(dir: ResizeDirection) -> CursorIcon {
    match dir {
        ResizeDirection::East | ResizeDirection::West => CursorIcon::EwResize,
        ResizeDirection::North | ResizeDirection::South => CursorIcon::NsResize,
        ResizeDirection::NorthEast | ResizeDirection::SouthWest => CursorIcon::NeswResize,
        ResizeDirection::NorthWest | ResizeDirection::SouthEast => CursorIcon::NwseResize,
    }
}

/// Maps a screen's cursor request to a window cursor.
pub fn icon_of(c: crate::input::Cursor) -> CursorIcon {
    use crate::input::Cursor;
    match c {
        Cursor::Default => CursorIcon::Default,
        Cursor::Pointer => CursorIcon::Pointer,
        Cursor::Grab => CursorIcon::Grab,
        Cursor::Grabbing => CursorIcon::Grabbing,
        Cursor::Crosshair => CursorIcon::Crosshair,
        Cursor::Move => CursorIcon::Move,
        Cursor::ResizeNs => CursorIcon::NsResize,
        Cursor::ResizeEw => CursorIcon::EwResize,
        Cursor::ResizeNesw => CursorIcon::NeswResize,
        Cursor::ResizeNwse => CursorIcon::NwseResize,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const B: f64 = 6.0;

    #[test]
    fn edges_and_corners() {
        let d = |x, y| resize_direction(x, y, 1000.0, 700.0, B);
        assert_eq!(d(2.0, 300.0), Some(ResizeDirection::West));
        assert_eq!(d(998.0, 300.0), Some(ResizeDirection::East));
        assert_eq!(d(500.0, 1.0), Some(ResizeDirection::North));
        assert_eq!(d(500.0, 699.0), Some(ResizeDirection::South));
        assert_eq!(d(1.0, 1.0), Some(ResizeDirection::NorthWest));
        assert_eq!(d(999.0, 1.0), Some(ResizeDirection::NorthEast));
        assert_eq!(d(1.0, 699.0), Some(ResizeDirection::SouthWest));
        assert_eq!(d(999.0, 699.0), Some(ResizeDirection::SouthEast));
    }

    #[test]
    fn interior_and_outside_have_no_direction() {
        assert_eq!(resize_direction(500.0, 300.0, 1000.0, 700.0, B), None);
        assert_eq!(resize_direction(-3.0, 300.0, 1000.0, 700.0, B), None);
    }
}
