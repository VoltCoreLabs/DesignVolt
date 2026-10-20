//! Geometry of the color picker popup and what a point in it hits.

use dv_ui::theme::metrics::{FIELD_H, TOPBAR_H};
use kurbo::{Point, Rect};

use super::color_fmt::Mode;
use super::paint_ui::{Slot, anchor};

pub const W: f64 = 268.0;
const PAD: f64 = 12.0;
const SV_H: f64 = 150.0;
pub const BAR_H: f64 = 12.0;
const BAR_GAP: f64 = 10.0;
const GAP_V: f64 = 12.0;
const BTN: f64 = 28.0;
const MODE_W: f64 = 56.0;
const FIELD_GAP: f64 = 4.0;
const ALPHA_W: f64 = 48.0;
pub const SW: f64 = 20.0;
const SW_GAP: f64 = 6.0;
pub const PER_ROW: usize = 9;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Part {
    Sv,
    Hue,
    Alpha,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PHit {
    Part(Part),
    Eyedrop,
    Mode,
    Field(usize),
    AlphaField,
    Recent(usize),
}

pub fn recent_rows(n: usize) -> usize {
    n.div_ceil(PER_ROW)
}

fn bars_y() -> f64 {
    PAD + SV_H + GAP_V
}

fn fields_y() -> f64 {
    bars_y() + BAR_H * 2.0 + BAR_GAP + GAP_V
}

fn recents_y() -> f64 {
    fields_y() + FIELD_H + GAP_V
}

pub fn height(rows: usize) -> f64 {
    if rows == 0 {
        fields_y() + FIELD_H + PAD
    } else {
        recents_y() + rows as f64 * (SW + SW_GAP) - SW_GAP + PAD
    }
}

/// The popup: left of the right panel, level with the swatch, kept on screen.
pub fn popup_rect(right: Rect, slot: Slot, win_h: f64, rows: usize) -> Rect {
    let h = height(rows);
    let y = (anchor(right, slot).y - 8.0)
        .min(win_h - h - 8.0)
        .max(TOPBAR_H + 8.0);
    let x1 = right.x0 - 8.0;
    Rect::new(x1 - W, y, x1, y + h)
}

pub fn part_rect(p: Rect, part: Part) -> Rect {
    let (x0, x1) = (p.x0 + PAD, p.x1 - PAD);
    match part {
        Part::Sv => Rect::new(x0, p.y0 + PAD, x1, p.y0 + PAD + SV_H),
        Part::Hue => Rect::new(x0 + BTN + 8.0, p.y0 + bars_y(), x1, p.y0 + bars_y() + BAR_H),
        Part::Alpha => {
            let y = p.y0 + bars_y() + BAR_H + BAR_GAP;
            Rect::new(x0 + BTN + 8.0, y, x1, y + BAR_H)
        }
    }
}

pub fn eyedrop_rect(p: Rect) -> Rect {
    let y = p.y0 + bars_y() + (BAR_H * 2.0 + BAR_GAP - BTN) / 2.0;
    Rect::new(p.x0 + PAD, y, p.x0 + PAD + BTN, y + BTN)
}

pub fn mode_rect(p: Rect) -> Rect {
    let y = p.y0 + fields_y();
    Rect::new(p.x0 + PAD, y, p.x0 + PAD + MODE_W, y + FIELD_H)
}

fn field_area(p: Rect, mode: Mode) -> (f64, f64, f64, f64) {
    let x0 = p.x0 + PAD + MODE_W + 6.0;
    let right = p.x1 - PAD;
    let y = p.y0 + fields_y();
    let end = if mode.has_alpha_field() {
        right - ALPHA_W - FIELD_GAP
    } else {
        right
    };
    (x0, end, y, right)
}

pub fn field_rect(p: Rect, mode: Mode, i: usize) -> Rect {
    let (x0, end, y, _) = field_area(p, mode);
    let n = mode.fields() as f64;
    let w = (end - x0 - FIELD_GAP * (n - 1.0)) / n;
    let x = x0 + i as f64 * (w + FIELD_GAP);
    Rect::new(x, y, x + w, y + FIELD_H)
}

pub fn alpha_field_rect(p: Rect, mode: Mode) -> Rect {
    let (_, _, y, right) = field_area(p, mode);
    Rect::new(right - ALPHA_W, y, right, y + FIELD_H)
}

pub fn recent_rect(p: Rect, i: usize) -> Rect {
    let (row, col) = (i / PER_ROW, i % PER_ROW);
    let x = p.x0 + PAD + col as f64 * (SW + SW_GAP);
    let y = p.y0 + recents_y() + row as f64 * (SW + SW_GAP);
    Rect::new(x, y, x + SW, y + SW)
}

/// What the point hits inside the popup, if anything.
pub fn hit(p: Rect, at: Point, mode: Mode, recent: usize) -> Option<PHit> {
    for part in [Part::Sv, Part::Hue, Part::Alpha] {
        if part_rect(p, part).inflate(0.0, 4.0).contains(at) {
            return Some(PHit::Part(part));
        }
    }
    if eyedrop_rect(p).contains(at) {
        return Some(PHit::Eyedrop);
    }
    if mode_rect(p).contains(at) {
        return Some(PHit::Mode);
    }
    if let Some(i) = (0..mode.fields()).find(|i| field_rect(p, mode, *i).contains(at)) {
        return Some(PHit::Field(i));
    }
    if mode.has_alpha_field() && alpha_field_rect(p, mode).contains(at) {
        return Some(PHit::AlphaField);
    }
    (0..recent)
        .find(|i| recent_rect(p, *i).contains(at))
        .map(PHit::Recent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_fit_inside_the_popup_and_do_not_overlap() {
        let p = Rect::new(0.0, 0.0, W, height(2));
        for mode in Mode::ALL {
            let mut last = mode_rect(p).x1;
            for i in 0..mode.fields() {
                let r = field_rect(p, mode, i);
                assert!(r.x0 > last && r.width() > 24.0, "{mode:?} {i}");
                last = r.x1;
            }
            if mode.has_alpha_field() {
                let a = alpha_field_rect(p, mode);
                assert!(a.x0 > last && a.x1 <= W - 11.9);
            }
        }
    }

    #[test]
    fn recent_swatches_wrap_into_rows_inside_the_popup() {
        let p = Rect::new(0.0, 0.0, W, height(2));
        let last = recent_rect(p, 17);
        assert!(last.y1 < p.y1 && last.x1 < p.x1);
        assert_eq!(recent_rows(0), 0);
        assert_eq!(recent_rows(9), 1);
        assert_eq!(recent_rows(10), 2);
        assert!(height(0) < height(1));
    }

    #[test]
    fn points_map_to_controls() {
        let p = Rect::new(0.0, 0.0, W, height(1));
        let at = |r: Rect| r.center();
        assert_eq!(
            hit(p, at(part_rect(p, Part::Sv)), Mode::Hex, 3),
            Some(PHit::Part(Part::Sv))
        );
        assert_eq!(
            hit(p, at(eyedrop_rect(p)), Mode::Hex, 3),
            Some(PHit::Eyedrop)
        );
        assert_eq!(hit(p, at(mode_rect(p)), Mode::Rgb, 3), Some(PHit::Mode));
        assert_eq!(
            hit(p, at(field_rect(p, Mode::Rgb, 2)), Mode::Rgb, 3),
            Some(PHit::Field(2))
        );
        assert_eq!(
            hit(p, at(recent_rect(p, 1)), Mode::Hex, 3),
            Some(PHit::Recent(1))
        );
        assert_eq!(hit(p, at(recent_rect(p, 5)), Mode::Hex, 3), None);
    }
}
