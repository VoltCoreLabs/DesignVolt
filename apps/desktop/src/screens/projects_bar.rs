//! Selection bar: a glass strip at the bottom with the selection count and bulk actions.

use vello::kurbo::Rect;

use super::projects::{Projects, SIDEBAR_W};
use super::projects_menu::Action;
use crate::i18n::selected_count;
use dv_ui::draw::Cx;
use dv_ui::icons::Icon;
use dv_ui::theme::colors::*;
use dv_ui::theme::metrics::*;

const BAR_H: f64 = 52.0;
const BTN_H: f64 = 36.0;
const BTN_PAD: f64 = 14.0;
const GAP: f64 = 6.0;

pub struct BarLayout {
    pub rect: Rect,
    pub buttons: Vec<(Action, Rect)>,
    pub close: Rect,
    pub count_x: f64,
}

impl Projects {
    /// Geometry of the selection bar; `None` when nothing is selected.
    pub fn bar_layout(&self) -> Option<BarLayout> {
        if self.selected.is_empty() {
            return None;
        }
        let font = &self.assets.font;
        let count = selected_count(self.selected.len() as u64);
        let count_w = font.layout(&count, FONT_UI).1 as f64;
        let actions = self.actions_for(&self.selected);
        let widths: Vec<f64> = actions
            .iter()
            .map(|a| {
                let confirm = *a == Action::DeleteForever && self.bar_confirm;
                let (_, label, _) = self.action_look(*a, &self.selected, confirm);
                font.layout(label, FONT_UI).1 as f64 + 26.0 + 2.0 * BTN_PAD
            })
            .collect();
        let total = 16.0
            + count_w
            + 16.0
            + widths.iter().sum::<f64>()
            + GAP * widths.len() as f64
            + BTN_H
            + 10.0;
        let area = Rect::new(SIDEBAR_W, 0.0, self.vp.w, self.vp.h);
        let x0 = area.center().x - total / 2.0;
        let y0 = self.vp.h - 24.0 - BAR_H;
        let rect = Rect::new(x0, y0, x0 + total, y0 + BAR_H);
        let mut x = x0 + 16.0 + count_w + 16.0;
        let by = y0 + (BAR_H - BTN_H) / 2.0;
        let mut buttons = Vec::new();
        for (a, w) in actions.into_iter().zip(widths) {
            buttons.push((a, Rect::new(x, by, x + w, by + BTN_H)));
            x += w + GAP;
        }
        let close = Rect::new(x, by, x + BTN_H, by + BTN_H);
        Some(BarLayout {
            rect,
            buttons,
            close,
            count_x: x0 + 16.0,
        })
    }

    pub(super) fn paint_bar(&self, cx: &mut Cx) {
        let Some(bar) = self.bar_layout() else { return };
        cx.glass(bar.rect, R_MODAL);
        let count = selected_count(self.selected.len() as u64);
        cx.text(
            &count,
            FONT_UI,
            bar.count_x,
            bar.rect.center().y + 4.3,
            hex(text_primary()),
        );
        for (action, r) in &bar.buttons {
            let confirm = *action == Action::DeleteForever && self.bar_confirm;
            let (icon, label, destructive) = self.action_look(*action, &self.selected, confirm);
            if self.at(*r) {
                let tint = if destructive { danger() } else { bg_hover() };
                cx.fill(
                    *r,
                    R_BUTTON,
                    alpha(tint, if destructive { 0.14 } else { 0.9 }),
                );
            }
            let color = if destructive {
                hex(danger())
            } else {
                hex(text_primary())
            };
            cx.icon(icon, r.x0 + BTN_PAD, r.center().y - 8.0, 16.0, color);
            cx.text(
                label,
                FONT_UI,
                r.x0 + BTN_PAD + 24.0,
                r.center().y + 4.3,
                color,
            );
        }
        if self.at(bar.close) {
            cx.fill(bar.close, R_BUTTON, alpha(bg_hover(), 0.9));
        }
        let c = bar.close.center();
        cx.icon(
            Icon::Close,
            c.x - 7.0,
            c.y - 7.0,
            14.0,
            hex(text_secondary()),
        );
    }
}
