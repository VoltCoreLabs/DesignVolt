//! Geometry of the stroke settings window.

use dv_core::{LineJoin, StrokeStyle};
use dv_ui::theme::metrics::{FIELD_H, TOPBAR_H};
use kurbo::{Point, Rect};

use super::paint_layout::PField;

pub const W: f64 = 300.0;
const PAD: f64 = 14.0;
const HEAD_H: f64 = 44.0;
const GAP: f64 = 10.0;
pub const LABEL_W: f64 = 78.0;
const CLOSE: f64 = 24.0;
const SEG_GAP: f64 = 2.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Row {
    Style,
    /// Dash length and gap (dashed lines only).
    Dash,
    Position,
    Cap,
    Join,
    /// Miter angle (miter joins only).
    Miter,
}

pub fn rows(style: StrokeStyle, join: LineJoin) -> Vec<Row> {
    let mut v = vec![Row::Style];
    if style == StrokeStyle::Dashed {
        v.push(Row::Dash);
    }
    v.extend([Row::Position, Row::Cap, Row::Join]);
    if join == LineJoin::Miter {
        v.push(Row::Miter);
    }
    v
}

pub fn height(n: usize) -> f64 {
    HEAD_H + n as f64 * (FIELD_H + GAP) + PAD - GAP
}

/// Left of the right panel, level with the style button, kept on screen.
pub fn popup_rect(right: Rect, anchor: Point, n: usize, win_h: f64) -> Rect {
    let h = height(n);
    let y = (anchor.y - 8.0).min(win_h - h - 8.0).max(TOPBAR_H + 8.0);
    let x1 = right.x0 - 8.0;
    Rect::new(x1 - W, y, x1, y + h)
}

pub fn close_rect(p: Rect) -> Rect {
    Rect::new(
        p.x1 - PAD - CLOSE,
        p.y0 + 10.0,
        p.x1 - PAD,
        p.y0 + 10.0 + CLOSE,
    )
}

/// The control area of the `i`-th row (the label is to its left).
pub fn control_rect(p: Rect, i: usize) -> Rect {
    let y = p.y0 + HEAD_H + i as f64 * (FIELD_H + GAP);
    Rect::new(p.x0 + PAD + LABEL_W, y, p.x1 - PAD, y + FIELD_H)
}

pub fn label_x(p: Rect) -> f64 {
    p.x0 + PAD
}

/// Segment `k` of `n` in a control area.
pub fn seg_rect(c: Rect, k: usize, n: usize) -> Rect {
    let w = (c.width() - SEG_GAP * (n as f64 - 1.0)) / n as f64;
    let x = c.x0 + k as f64 * (w + SEG_GAP);
    Rect::new(x, c.y0, x + w, c.y1)
}

/// The field of the dash row: `Dash` is the left half, `Gap` the right one.
pub fn dash_field(c: Rect, f: PField) -> Rect {
    let w = (c.width() - 6.0) / 2.0;
    if f == PField::Dash {
        Rect::new(c.x0, c.y0, c.x0 + w, c.y1)
    } else {
        Rect::new(c.x1 - w, c.y0, c.x1, c.y1)
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PopHit {
    Close,
    StyleButton,
    Segment(Row, usize),
    Field(PField),
}

/// What the point hits inside the window.
pub fn hit(p: Rect, rows: &[Row], at: Point) -> Option<PopHit> {
    if close_rect(p).contains(at) {
        return Some(PopHit::Close);
    }
    for (i, row) in rows.iter().enumerate() {
        let c = control_rect(p, i);
        if !c.contains(at) {
            continue;
        }
        return match row {
            Row::Style => Some(PopHit::StyleButton),
            Row::Dash => [PField::Dash, PField::Gap]
                .into_iter()
                .find(|f| dash_field(c, *f).contains(at))
                .map(PopHit::Field),
            Row::Miter => Some(PopHit::Field(PField::Miter)),
            Row::Position | Row::Cap | Row::Join => (0..3)
                .find(|k| seg_rect(c, *k, 3).contains(at))
                .map(|k| PopHit::Segment(*row, k)),
        };
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_depend_on_style_and_join() {
        let solid = rows(StrokeStyle::Solid, LineJoin::Round);
        assert_eq!(solid, [Row::Style, Row::Position, Row::Cap, Row::Join]);
        let all = rows(StrokeStyle::Dashed, LineJoin::Miter);
        assert_eq!(all.len(), 6);
        assert!(all.contains(&Row::Dash) && all.contains(&Row::Miter));
    }

    #[test]
    fn controls_stack_inside_the_window() {
        let rows = rows(StrokeStyle::Dashed, LineJoin::Miter);
        let p = Rect::new(0.0, 0.0, W, height(rows.len()));
        let mut last = p.y0 + HEAD_H - 1.0;
        for i in 0..rows.len() {
            let c = control_rect(p, i);
            assert!(c.y0 > last && c.x1 <= p.x1 && c.width() > 120.0);
            last = c.y1;
        }
        assert!(last < p.y1);
    }

    #[test]
    fn points_map_to_controls() {
        let rows = rows(StrokeStyle::Dashed, LineJoin::Miter);
        let p = Rect::new(0.0, 0.0, W, height(rows.len()));
        let at = |i: usize| control_rect(p, i);
        assert_eq!(hit(p, &rows, close_rect(p).center()), Some(PopHit::Close));
        assert_eq!(hit(p, &rows, at(0).center()), Some(PopHit::StyleButton));
        assert_eq!(
            hit(p, &rows, dash_field(at(1), PField::Gap).center()),
            Some(PopHit::Field(PField::Gap))
        );
        assert_eq!(
            hit(p, &rows, seg_rect(at(3), 2, 3).center()),
            Some(PopHit::Segment(Row::Cap, 2))
        );
        assert_eq!(
            hit(p, &rows, at(5).center()),
            Some(PopHit::Field(PField::Miter))
        );
        assert_eq!(hit(p, &rows, Point::new(1.0, 1.0)), None);
    }
}
