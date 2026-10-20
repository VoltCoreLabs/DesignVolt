//! A list that opens under a button and lets the user pick one entry.

use dv_core::{EffectKind, StrokeStyle, Tiling};
use dv_ui::draw::Cx;
use dv_ui::icons::Icon;
use dv_ui::theme::colors::*;
use dv_ui::theme::metrics::*;
use kurbo::{Point, Rect};

use super::color_fmt::Mode;
use super::paint_layout::Slot;
use super::paint_ui::{Look, effect_name, effects_look, looks, panel, style_name};
use super::picker_layout::{mode_rect, pat_tiling_rect};
use super::state::Editor;
use crate::i18n::{Msg, tr};

pub const ITEM_H: f64 = 28.0;
const PAD: f64 = 4.0;
const MIN_W: f64 = 150.0;

/// What the chosen entry changes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Target {
    StrokeStyle,
    ColorMode,
    Tiling,
    AddEffect,
    EffectType(usize),
}

pub struct Dropdown {
    pub target: Target,
    pub anchor: Rect,
    pub labels: Vec<&'static str>,
    /// The entry that is the current value, marked in the list.
    pub current: Option<usize>,
}

/// Where the list goes: under the button (above it when there is no room), as wide as the
/// button or at least `MIN_W`, kept inside the window.
pub fn list_rect(anchor: Rect, n: usize, win: (f64, f64)) -> Rect {
    let w = anchor.width().max(MIN_W);
    let h = n as f64 * ITEM_H + 2.0 * PAD;
    let mut x0 = anchor.x0;
    if x0 + w > win.0 - 8.0 {
        x0 = (anchor.x1 - w).max(8.0);
    }
    let below = anchor.y1 + 4.0;
    let y0 = if below + h <= win.1 - 8.0 {
        below
    } else {
        (anchor.y0 - 4.0 - h).max(8.0)
    };
    Rect::new(x0, y0, x0 + w, y0 + h)
}

pub fn item_rect(list: Rect, i: usize) -> Rect {
    let y = list.y0 + PAD + i as f64 * ITEM_H;
    Rect::new(list.x0 + PAD, y, list.x1 - PAD, y + ITEM_H)
}

pub fn item_at(list: Rect, n: usize, p: Point) -> Option<usize> {
    (0..n).find(|i| item_rect(list, *i).contains(p))
}

fn tiling_name(t: Tiling) -> &'static str {
    tr(match t {
        Tiling::Grid => Msg::TilingGrid,
        Tiling::BrickRows => Msg::TilingRows,
        Tiling::BrickColumns => Msg::TilingColumns,
    })
}

impl Editor {
    fn win(&self) -> (f64, f64) {
        (self.win.w, self.win.h)
    }

    /// Opens the list of `target` under `anchor`.
    pub(super) fn open_dropdown(&mut self, target: Target, anchor: Rect) {
        let (labels, current): (Vec<&'static str>, Option<usize>) = match target {
            Target::StrokeStyle => {
                let cur = match looks(self).1 {
                    Look::One(s) => StrokeStyle::ALL.iter().position(|x| *x == s.style),
                    _ => None,
                };
                (
                    StrokeStyle::ALL.iter().map(|s| style_name(*s)).collect(),
                    cur,
                )
            }
            Target::ColorMode => (
                Mode::ALL.iter().map(|m| m.label()).collect(),
                Mode::ALL.iter().position(|m| *m == self.picker_mode),
            ),
            Target::Tiling => {
                let cur = self
                    .current_pattern()
                    .and_then(|p| Tiling::ALL.iter().position(|t| *t == p.tiling));
                (Tiling::ALL.iter().map(|t| tiling_name(*t)).collect(), cur)
            }
            Target::AddEffect => (
                EffectKind::ALL.iter().map(|k| effect_name(*k)).collect(),
                None,
            ),
            Target::EffectType(i) => {
                let cur = match effects_look(self) {
                    Look::One(l) => l
                        .get(i)
                        .and_then(|e| EffectKind::ALL.iter().position(|k| *k == e.kind)),
                    _ => None,
                };
                (
                    EffectKind::ALL.iter().map(|k| effect_name(*k)).collect(),
                    cur,
                )
            }
        };
        self.dropdown = Some(Dropdown {
            target,
            anchor,
            labels,
            current,
        });
    }

    /// The anchor button of a target in the current layout.
    pub(super) fn open_dropdown_for(&mut self, target: Target) {
        let p = panel(self);
        let anchor = match target {
            Target::StrokeStyle => p.style_rect(),
            Target::AddEffect => p.add_rect(Slot::Effect(0)),
            Target::EffectType(i) => p.type_rect(i),
            Target::ColorMode | Target::Tiling => {
                let slot = self.picker.as_ref().map(|k| k.slot);
                let Some(slot) = slot else { return };
                let shape = self.picker_shape(slot);
                let popup = self.popup(slot);
                if target == Target::ColorMode {
                    mode_rect(popup, shape)
                } else {
                    pat_tiling_rect(popup, shape)
                }
            }
        };
        self.open_dropdown(target, anchor);
    }

    /// A press while a list is open. Always consumed: a click outside only closes it.
    pub(super) fn press_dropdown(&mut self) -> bool {
        let Some(d) = self.dropdown.take() else {
            return false;
        };
        let list = list_rect(d.anchor, d.labels.len(), self.win());
        if let Some(i) = item_at(list, d.labels.len(), self.mouse) {
            self.choose(d.target, i);
        }
        true
    }

    fn choose(&mut self, target: Target, i: usize) {
        match target {
            Target::StrokeStyle => {
                if let Some(s) = StrokeStyle::ALL.get(i) {
                    self.set_stroke_style(*s);
                }
            }
            Target::ColorMode => {
                if let Some(m) = Mode::ALL.get(i) {
                    self.picker_mode = *m;
                    self.pick_edit = None;
                }
            }
            Target::Tiling => {
                if let Some(t) = Tiling::ALL.get(i) {
                    self.edit_pattern(|p| p.tiling = *t);
                }
            }
            Target::AddEffect => {
                if let Some(k) = EffectKind::ALL.get(i) {
                    self.add_effect(*k);
                }
            }
            Target::EffectType(n) => {
                if let Some(k) = EffectKind::ALL.get(i) {
                    self.set_effect_kind(n, *k);
                }
            }
        }
    }

    pub(super) fn draw_dropdown(&self, cx: &mut Cx) {
        let Some(d) = &self.dropdown else { return };
        let list = list_rect(d.anchor, d.labels.len(), self.win());
        cx.shadow(list, R_BUTTON, alpha(0x000000, 0.28), 14.0, 6.0);
        cx.fill(list, R_BUTTON, hex(bg_raised()));
        cx.stroke(list, R_BUTTON, 1.0, alpha(overlay(), 0.14));
        for (i, label) in d.labels.iter().enumerate() {
            let r = item_rect(list, i);
            if r.contains(self.mouse) {
                cx.fill(r, R_FIELD, alpha(accent(), 0.2));
            }
            cx.text(label, FONT_UI, r.x0 + 8.0, r.y0 + 18.0, hex(text_primary()));
            if d.current == Some(i) {
                cx.icon(Icon::Check, r.x1 - 22.0, r.y0 + 7.0, 14.0, hex(accent()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIN: (f64, f64) = (1180.0, 760.0);

    #[test]
    fn list_opens_below_and_flips_above_near_the_bottom() {
        let a = Rect::new(900.0, 100.0, 1040.0, 128.0);
        let l = list_rect(a, 3, WIN);
        assert_eq!(l.y0, a.y1 + 4.0);
        assert_eq!(l.width(), MIN_W);
        let low = Rect::new(900.0, 740.0, 1040.0, 768.0 - 8.0);
        let up = list_rect(low, 3, WIN);
        assert!(up.y1 <= low.y0);
    }

    #[test]
    fn list_stays_inside_the_window_horizontally() {
        let a = Rect::new(1150.0, 100.0, 1172.0, 122.0);
        let l = list_rect(a, 3, WIN);
        assert!(l.x1 <= WIN.0 - 8.0 + 0.1 && l.x0 >= 8.0);
    }

    #[test]
    fn points_pick_the_entry_under_them() {
        let l = list_rect(Rect::new(100.0, 100.0, 260.0, 128.0), 3, WIN);
        assert_eq!(item_at(l, 3, item_rect(l, 1).center()), Some(1));
        assert_eq!(item_at(l, 3, Point::new(0.0, 0.0)), None);
        assert!(item_rect(l, 2).y1 <= l.y1);
    }
}
