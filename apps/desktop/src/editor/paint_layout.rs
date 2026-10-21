//! Layout of the Fill, Stroke and Effects sections of the right panel. A section is just its
//! title until something is added to it, then it grows by the rows it needs.

use dv_ui::theme::metrics::FIELD_H;
use kurbo::{Point, Rect};

use super::panels::{FILL_TOP, ROW, SEC_HEAD};

/// Most effects one shape shows rows for.
pub const MAX_EFFECTS: usize = 8;
/// Most fills one shape shows rows for.
pub const MAX_FILLS: usize = 8;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Slot {
    /// The `i`-th fill of the shape (0 is `Node::fill`, the rest are `extra_fills`).
    Fill(usize),
    Stroke,
    /// The `i`-th effect of the shape.
    Effect(usize),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PField {
    Hex,
    Alpha,
    Width,
    ShX,
    ShY,
    ShBlur,
    /// Fields of the stroke settings window.
    Dash,
    Gap,
    Miter,
}

/// What a click in the sections hits.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hit {
    Add(Slot),
    Remove(Slot),
    Swatch(Slot),
    Field(Slot, PField),
    /// The drop-down of the stroke style.
    Style,
    /// The gear that opens the stroke settings window.
    StrokeSettings,
    /// The drop-down that chooses what kind of effect to add.
    AddEffect,
    /// The drop-down of the kind of the `i`-th effect.
    EffectType(usize),
}

/// What the selection has in one slot.
#[derive(Clone, PartialEq, Debug)]
pub enum Look<T> {
    None,
    Mixed,
    One(T),
}

const SWATCH: f64 = 24.0;
const HEX_W: f64 = 70.0;
const ALPHA_W: f64 = 48.0;
const WIDTH_W: f64 = 62.0;
const BTN: f64 = 22.0;
pub const GAP_X: f64 = 6.0;
pub const TEXT_X: f64 = 8.0;
/// Where the value text starts in a field that has a label in front of it.
pub const LABELED_TEXT_X: f64 = 24.0;
const STYLE_W: f64 = 140.0;

/// Where the text of a field starts, from the left edge of the field.
pub fn text_x(f: PField) -> f64 {
    match f {
        PField::ShX | PField::ShY | PField::ShBlur => LABELED_TEXT_X,
        _ => TEXT_X,
    }
}

/// Positions of the sections for what the selection currently has.
#[derive(Clone, Copy, Debug)]
pub struct Panel {
    pub right: Rect,
    stroke_top: f64,
    effects_top: f64,
    /// Bit `i` is set when effect `i` is a blur (a single wide field).
    blur_mask: u32,
}

impl Panel {
    pub fn new(right: Rect, fills: usize, has_stroke: bool, blur_mask: u32) -> Panel {
        let stroke_top = FILL_TOP + SEC_HEAD + fills as f64 * ROW + 4.0;
        let effects_top = stroke_top + SEC_HEAD + if has_stroke { 2.0 * ROW } else { 0.0 } + 4.0;
        Panel {
            right,
            stroke_top,
            effects_top,
            blur_mask,
        }
    }

    pub fn top(&self, slot: Slot) -> f64 {
        match slot {
            Slot::Fill(_) => FILL_TOP,
            Slot::Stroke => self.stroke_top,
            Slot::Effect(_) => self.effects_top,
        }
    }

    /// Top of the title of a section (for its label and "+").
    pub fn title_y(&self, slot: Slot) -> f64 {
        self.right.y0 + self.top(slot)
    }

    /// The control row of a slot (for an effect: its second line with swatch and values).
    pub fn row(&self, slot: Slot) -> Rect {
        let mut y = self.right.y0 + self.top(slot) + SEC_HEAD;
        if let Slot::Fill(i) = slot {
            y += i as f64 * ROW;
        }
        if let Slot::Effect(i) = slot {
            y += i as f64 * 2.0 * ROW + ROW;
        }
        Rect::new(self.right.x0 + 13.0, y, self.right.x1 - 12.0, y + FIELD_H)
    }

    /// First line of an effect: the drop-down of its kind.
    pub fn type_rect(&self, i: usize) -> Rect {
        let r = self.row(Slot::Effect(i));
        let y = r.y0 - ROW;
        Rect::new(r.x0, y, r.x1 - BTN - GAP_X, y + FIELD_H)
    }

    pub fn add_rect(&self, slot: Slot) -> Rect {
        let y = self.right.y0 + self.top(slot) + 5.0;
        Rect::new(self.right.x1 - 12.0 - BTN, y, self.right.x1 - 12.0, y + BTN)
    }

    pub fn remove_rect(&self, slot: Slot) -> Rect {
        let r = match slot {
            Slot::Effect(i) => self.type_rect(i),
            _ => self.row(slot),
        };
        let right = self.right.x1 - 12.0;
        let y = r.y0 + (FIELD_H - BTN) / 2.0;
        Rect::new(right - BTN, y, right, y + BTN)
    }

    pub fn swatch_rect(&self, slot: Slot) -> Rect {
        let r = self.row(slot);
        let y = r.y0 + (FIELD_H - SWATCH) / 2.0;
        Rect::new(r.x0, y, r.x0 + SWATCH, y + SWATCH)
    }

    pub fn field_rect(&self, slot: Slot, f: PField) -> Rect {
        let r = self.row(slot);
        let hex_x = r.x0 + SWATCH + 8.0;
        if let Slot::Effect(i) = slot {
            if self.blur_mask & (1 << i) != 0 {
                return Rect::new(r.x0, r.y0, r.x1, r.y1);
            }
            let k = [PField::ShX, PField::ShY, PField::ShBlur]
                .iter()
                .position(|x| *x == f)
                .unwrap_or(0);
            let w = (r.x1 - hex_x - 2.0 * GAP_X) / 3.0;
            let x = hex_x + k as f64 * (w + GAP_X);
            return Rect::new(x, r.y0, x + w, r.y1);
        }
        let (x, w) = match f {
            PField::Hex => (hex_x, HEX_W),
            PField::Alpha => (hex_x + HEX_W + GAP_X, ALPHA_W),
            _ => (hex_x + HEX_W + GAP_X + ALPHA_W + GAP_X, WIDTH_W),
        };
        Rect::new(x, r.y0, x + w, r.y1)
    }

    /// The stroke style drop-down, on the line under the stroke row.
    pub fn style_rect(&self) -> Rect {
        let r = self.row(Slot::Stroke);
        Rect::new(r.x0, r.y0 + ROW, r.x0 + STYLE_W, r.y0 + ROW + FIELD_H)
    }

    /// The gear button next to the style drop-down.
    pub fn settings_rect(&self) -> Rect {
        let s = self.style_rect();
        Rect::new(s.x1 + GAP_X, s.y0, s.x1 + GAP_X + FIELD_H, s.y1)
    }

    /// A point of the section's swatch, to keep the picker beside it.
    pub fn anchor(&self, slot: Slot) -> Point {
        let r = self.swatch_rect(slot);
        Point::new(r.x0, r.y0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn right() -> Rect {
        Rect::new(900.0, 0.0, 1180.0, 760.0)
    }

    #[test]
    fn empty_sections_are_only_titles_and_grow_when_filled() {
        let none = Panel::new(right(), 0, false, 0);
        let all = Panel::new(right(), 1, true, 0);
        assert!(all.top(Slot::Stroke) > none.top(Slot::Stroke));
        assert!(all.top(Slot::Effect(0)) - none.top(Slot::Effect(0)) >= 3.0 * ROW);
        // Collapsed: the next section starts right below this title.
        assert!(none.top(Slot::Stroke) - none.top(Slot::Fill(0)) < SEC_HEAD + 10.0);
    }

    #[test]
    fn effect_rows_stack_and_stay_inside_the_panel() {
        let p = Panel::new(right(), 1, true, 0);
        let a = p.row(Slot::Effect(0));
        let b = p.row(Slot::Effect(1));
        assert!(b.y0 - a.y0 >= 2.0 * ROW - 0.1);
        assert!(p.type_rect(1).y1 <= b.y0);
        assert!(p.remove_rect(Slot::Effect(0)).x1 <= right().x1);
        assert!(p.type_rect(0).x1 < p.remove_rect(Slot::Effect(0)).x0);
        let last = p.row(Slot::Effect(MAX_EFFECTS - 1));
        assert!(last.y1 > right().y0);
    }

    #[test]
    fn shadow_fields_do_not_overlap_and_a_blur_gets_one_wide_field() {
        let p = Panel::new(right(), 1, true, 0b10);
        let mut last = p.swatch_rect(Slot::Effect(0)).x1;
        for f in [PField::ShX, PField::ShY, PField::ShBlur] {
            let r = p.field_rect(Slot::Effect(0), f);
            assert!(r.x0 > last && r.width() > 40.0);
            last = r.x1;
        }
        let wide = p.field_rect(Slot::Effect(1), PField::ShBlur);
        assert_eq!(wide.x0, p.row(Slot::Effect(1)).x0);
    }

    #[test]
    fn labeled_fields_leave_room_for_the_label() {
        assert!(text_x(PField::ShX) > text_x(PField::Hex));
    }
}
