//! Fill and stroke sections of the right panel: layout, what they show, drawing, hit-testing.

use dv_core::{Color, Stroke};
use dv_ui::draw::Cx;
use dv_ui::icons::Icon;
use dv_ui::theme::colors::*;
use dv_ui::theme::metrics::*;
use kurbo::{Point, Rect};
use vello::peniko::Color as VColor;

use super::panels::{FILL_TOP, ROW, SEC_HEAD};
use super::state::Editor;
use crate::i18n::{Msg, tr};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Slot {
    Fill,
    Stroke,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PField {
    Hex,
    Alpha,
    Width,
}

/// What a click in the sections hits.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hit {
    Add(Slot),
    Remove(Slot),
    Swatch(Slot),
    Field(Slot, PField),
}

/// What the selection has in one slot.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Look<T> {
    None,
    Mixed,
    One(T),
}

pub const STROKE_TOP: f64 = FILL_TOP + SEC_HEAD + ROW + 4.0;
const SWATCH: f64 = 24.0;
const HEX_W: f64 = 76.0;
const ALPHA_W: f64 = 52.0;
const WIDTH_W: f64 = 44.0;
const BTN: f64 = 22.0;
const GAP_X: f64 = 6.0;
const TEXT_X: f64 = 8.0;

fn top_of(slot: Slot) -> f64 {
    match slot {
        Slot::Fill => FILL_TOP,
        Slot::Stroke => STROKE_TOP,
    }
}

fn row_rect(right: Rect, slot: Slot) -> Rect {
    let y = right.y0 + top_of(slot) + SEC_HEAD;
    Rect::new(right.x0 + 13.0, y, right.x1 - 12.0, y + FIELD_H)
}

pub fn add_rect(right: Rect, slot: Slot) -> Rect {
    let y = right.y0 + top_of(slot) + 5.0;
    Rect::new(right.x1 - 12.0 - BTN, y, right.x1 - 12.0, y + BTN)
}

pub fn remove_rect(right: Rect, slot: Slot) -> Rect {
    let r = row_rect(right, slot);
    let y = r.y0 + (FIELD_H - BTN) / 2.0;
    Rect::new(r.x1 - BTN, y, r.x1, y + BTN)
}

pub fn swatch_rect(right: Rect, slot: Slot) -> Rect {
    let r = row_rect(right, slot);
    let y = r.y0 + (FIELD_H - SWATCH) / 2.0;
    Rect::new(r.x0, y, r.x0 + SWATCH, y + SWATCH)
}

pub fn field_rect(right: Rect, slot: Slot, f: PField) -> Rect {
    let r = row_rect(right, slot);
    let hex_x = r.x0 + SWATCH + 8.0;
    let (x, w) = match f {
        PField::Hex => (hex_x, HEX_W),
        PField::Alpha => (hex_x + HEX_W + GAP_X, ALPHA_W),
        PField::Width => (hex_x + HEX_W + GAP_X + ALPHA_W + GAP_X, WIDTH_W),
    };
    Rect::new(x, r.y0, x + w, r.y1)
}

/// What the selected shapes have as fill and stroke.
pub fn looks(ed: &Editor) -> (Look<Color>, Look<Stroke>) {
    let s = ed.session.borrow();
    let page = s.exec.doc().page();
    let nodes: Vec<_> = page
        .geo_tops(&ed.selection)
        .into_iter()
        .filter_map(|id| page.node(id))
        .collect();
    let fill = fold(nodes.iter().map(|n| n.fill));
    let stroke = fold(nodes.iter().map(|n| n.stroke));
    (fill, stroke)
}

fn fold<T: PartialEq + Copy>(mut it: impl Iterator<Item = Option<T>>) -> Look<T> {
    let Some(first) = it.next() else {
        return Look::None;
    };
    if it.any(|x| x != first) {
        return Look::Mixed;
    }
    first.map_or(Look::None, Look::One)
}

impl Editor {
    /// The sections' control under the pointer.
    pub(super) fn paint_hit(&self) -> Option<Hit> {
        let right = self.layout().right;
        if self.selection.is_empty() || !right.contains(self.mouse) {
            return None;
        }
        let (fill, stroke) = looks(self);
        for (slot, has) in [
            (Slot::Fill, fill != Look::None),
            (Slot::Stroke, stroke != Look::None),
        ] {
            let at = |r: Rect| r.contains(self.mouse);
            if !has {
                if at(add_rect(right, slot)) {
                    return Some(Hit::Add(slot));
                }
                continue;
            }
            if at(remove_rect(right, slot)) {
                return Some(Hit::Remove(slot));
            }
            if at(swatch_rect(right, slot)) {
                return Some(Hit::Swatch(slot));
            }
            let fields = [PField::Hex, PField::Alpha, PField::Width];
            let n = if slot == Slot::Stroke { 3 } else { 2 };
            for f in &fields[..n] {
                if at(field_rect(right, slot, *f)) {
                    return Some(Hit::Field(slot, *f));
                }
            }
        }
        None
    }
}

fn icon_button(cx: &mut Cx, r: Rect, icon: Icon, hot: bool) {
    if hot {
        cx.fill(r, R_BUTTON, hex(bg_hover()));
    }
    let c = r.center();
    cx.icon(icon, c.x - 7.0, c.y - 7.0, 14.0, hex(text_secondary()));
}

fn value_box(cx: &mut Cx, r: Rect, text: &str, suffix: &str) {
    cx.fill(r, R_FIELD, hex(bg_raised()));
    cx.text(
        text,
        FONT_UI,
        r.x0 + TEXT_X,
        r.y0 + 18.0,
        hex(text_primary()),
    );
    if !suffix.is_empty() {
        let w = cx.text_width(suffix, FONT_SM);
        cx.text(
            suffix,
            FONT_SM,
            r.x1 - 8.0 - w,
            r.y0 + 18.0,
            hex(text_disabled()),
        );
    }
}

fn swatch(cx: &mut Cx, r: Rect, c: Color) {
    cx.fill(r, R_FIELD, VColor::from_rgba8(c.r, c.g, c.b, c.a));
    cx.stroke(r, R_FIELD, 1.0, alpha(overlay(), 0.2));
}

/// One slot: its title with the "+" button, and the row with swatch, values and "-".
fn slot(cx: &mut Cx, ed: &Editor, right: Rect, which: Slot, color: Look<(Color, Option<f64>)>) {
    let (title, y) = match which {
        Slot::Fill => (tr(Msg::SectionFill), right.y0 + FILL_TOP),
        Slot::Stroke => (tr(Msg::SectionStroke), right.y0 + STROKE_TOP),
    };
    super::panels::section_title(cx, right.x0 + 1.0, right.x1, y, title, true);
    let hot = |r: Rect| r.contains(ed.mouse);
    if color == Look::None {
        let add = add_rect(right, which);
        icon_button(cx, add, Icon::Plus, hot(add));
        return;
    }
    let remove = remove_rect(right, which);
    icon_button(cx, remove, Icon::Minimize, hot(remove));
    let hexr = field_rect(right, which, PField::Hex);
    match color {
        Look::One((c, w)) => {
            swatch(cx, swatch_rect(right, which), c);
            let pct = format!("{:.0}", f64::from(c.a) / 255.0 * 100.0);
            editable(cx, ed, (which, PField::Hex), hexr, &c.to_hex(), "");
            let a = field_rect(right, which, PField::Alpha);
            editable(cx, ed, (which, PField::Alpha), a, &pct, "%");
            if let Some(w) = w {
                let r = field_rect(right, which, PField::Width);
                editable(
                    cx,
                    ed,
                    (which, PField::Width),
                    r,
                    &super::canvas::num(w),
                    "px",
                );
            }
        }
        _ => {
            let r = Rect::new(hexr.x0, hexr.y0, remove.x0 - GAP_X, hexr.y1);
            cx.fill(r, R_FIELD, hex(bg_raised()));
            let mixed = tr(Msg::Mixed);
            cx.text(
                mixed,
                FONT_UI,
                r.x0 + TEXT_X,
                r.y0 + 18.0,
                hex(text_secondary()),
            );
        }
    }
}

fn editable(cx: &mut Cx, ed: &Editor, key: (Slot, PField), r: Rect, value: &str, suffix: &str) {
    match &ed.paint_edit {
        Some(e) if (e.slot, e.field) == key => {
            cx.fill(r, R_FIELD, hex(bg_raised()));
            e.input.draw_inline(cx, r, r.x0 + TEXT_X, r.y0 + 18.0);
            cx.stroke(r, R_FIELD, 1.5, hex(accent()));
        }
        _ => value_box(cx, r, value, suffix),
    }
}

/// Draws the Fill and Stroke sections of the right panel.
pub fn sections(cx: &mut Cx, right: Rect, ed: &Editor) {
    let (fill, stroke) = looks(ed);
    let fill = match fill {
        Look::One(c) => Look::One((c, None)),
        Look::Mixed => Look::Mixed,
        Look::None => Look::None,
    };
    let stroke = match stroke {
        Look::One(s) => Look::One((s.color, Some(s.width))),
        Look::Mixed => Look::Mixed,
        Look::None => Look::None,
    };
    slot(cx, ed, right, Slot::Fill, fill);
    slot(cx, ed, right, Slot::Stroke, stroke);
}

/// A point inside the right panel's sections (used to keep the picker clear of them).
pub fn anchor(right: Rect, slot: Slot) -> Point {
    let r = swatch_rect(right, slot);
    Point::new(r.x0, r.y0)
}
