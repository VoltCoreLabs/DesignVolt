//! The color picker popup: saturation/value square, hue and opacity bars.

use dv_core::{Color, Stroke};
use dv_ui::draw::Cx;
use dv_ui::theme::colors::*;
use dv_ui::theme::metrics::*;
use kurbo::{Point, Rect};
use vello::peniko::Color as VColor;

use super::ops_paint::{DEFAULT_STROKE, edit_fill, edit_stroke};
use super::paint_ui::{Hit, Look, Slot, anchor, looks};
use super::state::Editor;

const W: f64 = 232.0;
const PAD: f64 = 12.0;
const SV_H: f64 = 150.0;
const BAR_H: f64 = 12.0;
const GAP_V: f64 = 12.0;
const H: f64 = PAD + SV_H + GAP_V + BAR_H + 10.0 + BAR_H + PAD;
const KNOB: f64 = 6.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Part {
    Sv,
    Hue,
    Alpha,
}

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
}

/// The popup rectangle: left of the right panel, level with the swatch, kept on screen.
fn popup_rect(right: Rect, slot: Slot, win_h: f64) -> Rect {
    let y = anchor(right, slot).y - 8.0;
    let y = y.min(win_h - H - 8.0).max(TOPBAR_H + 8.0);
    let x1 = right.x0 - 8.0;
    Rect::new(x1 - W, y, x1, y + H)
}

fn part_rect(popup: Rect, part: Part) -> Rect {
    let x0 = popup.x0 + PAD;
    let x1 = popup.x1 - PAD;
    let sv_y = popup.y0 + PAD;
    let hue_y = sv_y + SV_H + GAP_V;
    match part {
        Part::Sv => Rect::new(x0, sv_y, x1, sv_y + SV_H),
        Part::Hue => Rect::new(x0, hue_y, x1, hue_y + BAR_H),
        Part::Alpha => Rect::new(x0, hue_y + BAR_H + 10.0, x1, hue_y + 2.0 * BAR_H + 10.0),
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

fn vcolor(c: Color) -> VColor {
    VColor::from_rgba8(c.r, c.g, c.b, c.a)
}

impl Editor {
    fn popup(&self, slot: Slot) -> Rect {
        popup_rect(self.layout().right, slot, self.win.h)
    }

    /// Opens the picker for the slot (or closes it when it is already open for it).
    pub(super) fn toggle_picker(&mut self, slot: Slot) {
        if self.picker.as_ref().is_some_and(|p| p.slot == slot) {
            self.picker = None;
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

    /// A press while the picker is open. Returns true when it was consumed.
    pub(super) fn press_picker(&mut self) -> bool {
        let Some(slot) = self.picker.as_ref().map(|p| p.slot) else {
            return false;
        };
        let popup = self.popup(slot);
        if popup.contains(self.mouse) {
            let part = [Part::Sv, Part::Hue, Part::Alpha]
                .into_iter()
                .find(|p| part_rect(popup, *p).inflate(0.0, 4.0).contains(self.mouse));
            if let Some(part) = part {
                let key = self.next_drag_key();
                if let Some(p) = self.picker.as_mut() {
                    p.part = Some(part);
                    p.key = key;
                }
                self.drag_picker();
                self.interaction = Some(super::interact::Interaction::PickColor);
            }
            return true;
        }
        self.picker = None;
        // A click on the swatch that opened it only closes it.
        matches!(self.paint_hit(), Some(Hit::Swatch(s)) if s == slot)
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
            Some(Part::Hue) => {
                p.h = frac(part_rect(popup, Part::Hue), mouse, false) * 359.99;
            }
            Some(Part::Alpha) => {
                p.a = (frac(part_rect(popup, Part::Alpha), mouse, false) * 255.0).round() as u8;
            }
            None => return,
        }
        let (color, key, slot) = (p.color(), p.key, p.slot);
        let ex = &mut self.session.borrow_mut().exec;
        match slot {
            Slot::Fill => edit_fill(ex, &self.selection, Some(key), |_| Some(color)),
            Slot::Stroke => edit_stroke(ex, &self.selection, Some(key), |s| {
                Some(Stroke {
                    color,
                    ..s.unwrap_or(DEFAULT_STROKE)
                })
            }),
        }
    }

    pub(super) fn release_picker(&mut self) {
        if let Some(p) = self.picker.as_mut() {
            p.part = None;
        }
        self.session.borrow_mut().exec.break_merge();
    }

    pub(super) fn draw_picker(&self, cx: &mut Cx) {
        let Some(p) = &self.picker else { return };
        let popup = self.popup(p.slot);
        cx.shadow(popup, R_CARD, alpha(0x000000, 0.25), 16.0, 8.0);
        cx.fill(popup, R_CARD, hex(bg_raised()));
        cx.stroke(popup, R_CARD, 1.0, alpha(overlay(), 0.12));
        let pure = vcolor(Color::from_hsv(p.h, 1.0, 1.0, 255));
        let sv = part_rect(popup, Part::Sv);
        let white = VColor::from_rgba8(255, 255, 255, 255);
        cx.gradient(sv, 4.0, false, &[(0.0, white), (1.0, pure)]);
        let clear = VColor::from_rgba8(0, 0, 0, 0);
        let black = VColor::from_rgba8(0, 0, 0, 255);
        cx.gradient(sv, 4.0, true, &[(0.0, clear), (1.0, black)]);
        let hue = part_rect(popup, Part::Hue);
        let stops: Vec<(f32, VColor)> = (0..=6)
            .map(|i| {
                (
                    i as f32 / 6.0,
                    vcolor(Color::from_hsv(f64::from(i) * 60.0, 1.0, 1.0, 255)),
                )
            })
            .collect();
        cx.gradient(hue, BAR_H / 2.0, false, &stops);
        let bar = part_rect(popup, Part::Alpha);
        let solid = Color::from_hsv(p.h, p.s, p.v, 255);
        cx.fill(bar, BAR_H / 2.0, hex(bg_app()));
        let transparent = VColor::from_rgba8(solid.r, solid.g, solid.b, 0);
        cx.gradient(
            bar,
            BAR_H / 2.0,
            false,
            &[(0.0, transparent), (1.0, vcolor(solid))],
        );
        knob(
            cx,
            sv.x0 + p.s * sv.width(),
            sv.y0 + (1.0 - p.v) * sv.height(),
            p.color(),
        );
        let pure_c = Color::from_hsv(p.h, 1.0, 1.0, 255);
        knob(
            cx,
            hue.x0 + p.h / 359.99 * hue.width(),
            hue.center().y,
            pure_c,
        );
        knob(
            cx,
            bar.x0 + f64::from(p.a) / 255.0 * bar.width(),
            bar.center().y,
            solid,
        );
    }
}

fn knob(cx: &mut Cx, x: f64, y: f64, c: Color) {
    cx.circle(x, y, KNOB + 1.5, VColor::from_rgba8(255, 255, 255, 255));
    cx.circle(x, y, KNOB - 0.5, vcolor(Color { a: 255, ..c }));
}
