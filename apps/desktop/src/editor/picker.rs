//! The color picker popup: state and interaction (drawing is in `picker_draw`).

use dv_core::{Color, Stroke};
use kurbo::{Point, Rect};

use super::ops_paint::{DEFAULT_STROKE, edit_fill, edit_stroke};
use super::paint_ui::{Hit, Look, Slot, looks};
use super::picker_edit::PickField;
use super::picker_layout::*;
use super::state::Editor;

pub struct Picker {
    pub slot: Slot,
    pub h: f64,
    pub s: f64,
    pub v: f64,
    pub a: u8,
    pub part: Option<Part>,
    /// Merge key of the edits of the running drag.
    pub key: u64,
}

impl Picker {
    pub fn color(&self) -> Color {
        Color::from_hsv(self.h, self.s, self.v, self.a)
    }

    /// Takes a new color; the hue stays when the color has none (gray, black, white).
    pub fn set(&mut self, c: Color) {
        let (h, s, v) = c.to_hsv();
        if s > 0.0 && v > 0.0 {
            self.h = h;
        }
        self.s = s;
        self.v = v;
        self.a = c.a;
    }
}

/// Position of `p` along `r` (0..1), horizontally or vertically.
fn frac(r: Rect, p: Point, vertical: bool) -> f64 {
    let t = if vertical {
        (p.y - r.y0) / r.height()
    } else {
        (p.x - r.x0) / r.width()
    };
    t.clamp(0.0, 1.0)
}

impl Editor {
    pub(super) fn popup(&self, slot: Slot) -> Rect {
        let rows = recent_rows(super::recent::get().len());
        popup_rect(self.layout().right, slot, self.win.h, rows)
    }

    /// Opens the picker for the slot (or closes it when it is already open for it).
    pub(super) fn toggle_picker(&mut self, slot: Slot) {
        if self.picker.as_ref().is_some_and(|p| p.slot == slot) {
            self.close_picker();
            return;
        }
        let (fill, stroke) = looks(self);
        let color = match (slot, fill, stroke) {
            (Slot::Fill, Look::One(c), _) => c,
            (Slot::Stroke, _, Look::One(s)) => s.color,
            _ => return,
        };
        let (h, s, v) = color.to_hsv();
        self.commit_paint_edit();
        self.picker = Some(Picker {
            slot,
            h,
            s,
            v,
            a: color.a,
            part: None,
            key: 0,
        });
    }

    pub(super) fn close_picker(&mut self) {
        self.picker = None;
        self.pick_edit = None;
        self.eyedrop = None;
    }

    /// Writes `color` into the slot the picker edits (one undo step per `key`).
    fn apply_slot_color(&mut self, color: Color, key: Option<u64>) {
        let Some(slot) = self.picker.as_ref().map(|p| p.slot) else {
            return;
        };
        let ex = &mut self.session.borrow_mut().exec;
        match slot {
            Slot::Fill => edit_fill(ex, &self.selection, key, |_| Some(color)),
            Slot::Stroke => edit_stroke(ex, &self.selection, key, |s| {
                Some(Stroke {
                    color,
                    ..s.unwrap_or(DEFAULT_STROKE)
                })
            }),
        }
    }

    /// Sets the edited color from outside the drag (typed value, recent swatch, eyedropper).
    pub(super) fn set_picker_color(&mut self, c: Color) {
        if let Some(p) = self.picker.as_mut() {
            p.set(c);
        }
        self.apply_slot_color(c, None);
        super::recent::push(c);
    }

    /// A press while the picker is open. Returns true when it was consumed.
    pub(super) fn press_picker(&mut self) -> bool {
        let Some(slot) = self.picker.as_ref().map(|p| p.slot) else {
            return false;
        };
        let popup = self.popup(slot);
        if self.press_in_pick_edit(popup) {
            return true;
        }
        if popup.contains(self.mouse) {
            let recent = super::recent::get();
            match hit(popup, self.mouse, self.picker_mode, recent.len()) {
                Some(PHit::Part(part)) => self.start_pick_drag(part),
                Some(PHit::Eyedrop) => {
                    self.eyedrop = if self.eyedrop.is_some() {
                        None
                    } else {
                        Some(slot)
                    };
                }
                Some(PHit::Mode) => self.picker_mode = self.picker_mode.next(),
                Some(PHit::Field(i)) => self.start_pick_edit(PickField::Comp(i)),
                Some(PHit::AlphaField) => self.start_pick_edit(PickField::Alpha),
                Some(PHit::Recent(i)) => self.set_picker_color(recent[i]),
                None => {}
            }
            return true;
        }
        if self.eyedrop.is_some() {
            if self.over_canvas() {
                self.sample_color();
            }
            return true;
        }
        self.close_picker();
        // A click on the swatch that opened it only closes it.
        matches!(self.paint_hit(), Some(Hit::Swatch(s)) if s == slot)
    }

    fn start_pick_drag(&mut self, part: Part) {
        let key = self.next_drag_key();
        if let Some(p) = self.picker.as_mut() {
            p.part = Some(part);
            p.key = key;
        }
        self.drag_picker();
        self.interaction = Some(super::interact::Interaction::PickColor);
    }

    /// Eyedropper: takes the color of the shape under the pointer.
    fn sample_color(&mut self) {
        let world = self.world_mouse();
        let color = {
            let s = self.session.borrow();
            let page = s.exec.doc().page();
            self.index
                .pick(page, world)
                .and_then(|id| page.node(id))
                .and_then(|n| n.fill.or(n.stroke.map(|s| s.color)))
        };
        if let Some(c) = color {
            self.set_picker_color(c);
        }
        self.eyedrop = None;
    }

    /// Follows the pointer while a bar or the square is dragged.
    pub(super) fn drag_picker(&mut self) {
        let Some(slot) = self.picker.as_ref().map(|p| p.slot) else {
            return;
        };
        let popup = self.popup(slot);
        let mouse = self.mouse;
        let Some(p) = self.picker.as_mut() else {
            return;
        };
        match p.part {
            Some(Part::Sv) => {
                let r = part_rect(popup, Part::Sv);
                p.s = frac(r, mouse, false);
                p.v = 1.0 - frac(r, mouse, true);
            }
            Some(Part::Hue) => p.h = frac(part_rect(popup, Part::Hue), mouse, false) * 359.99,
            Some(Part::Alpha) => {
                p.a = (frac(part_rect(popup, Part::Alpha), mouse, false) * 255.0).round() as u8;
            }
            None => return,
        }
        let (color, key) = (p.color(), p.key);
        self.apply_slot_color(color, Some(key));
    }

    pub(super) fn release_picker(&mut self) {
        let color = self.picker.as_mut().map(|p| {
            p.part = None;
            p.color()
        });
        self.session.borrow_mut().exec.break_merge();
        if let Some(c) = color {
            super::recent::push(c);
        }
    }
}
