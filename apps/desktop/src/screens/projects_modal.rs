//! Confirmation dialog (modal) of the projects screen. Used for "Empty trash".

use vello::kurbo::Rect;

use super::projects::Projects;
use crate::i18n::{Msg, empty_trash_body, tr};
use dv_ui::anim::Spring;
use dv_ui::draw::Cx;
use dv_ui::icons::Icon;
use dv_ui::theme::colors::*;
use dv_ui::theme::metrics::*;
use dv_ui::theme::motion::*;
use dv_ui::widgets::button::Button;

const WIDTH: f64 = 440.0;
const PAD: f64 = 28.0;
const LINE_H: f64 = 22.0;
const BUTTON_H: f64 = 40.0;

pub struct Modal {
    /// 0 → 1 while the dialog fades in.
    pub shown: Spring,
    pub cancel: Button,
    pub confirm: Button,
}

impl Modal {
    pub fn new() -> Self {
        let mut shown = Spring::new(0.0, MOVE_STIFFNESS, MOVE_DAMPING);
        shown.target = 1.0;
        Self {
            shown,
            cancel: Button::new(),
            confirm: Button::new(),
        }
    }
}

struct ModalLayout {
    card: Rect,
    cancel: Rect,
    confirm: Rect,
    body: Vec<String>,
}

impl Projects {
    pub fn open_empty_trash_modal(&mut self) {
        self.menu = None;
        self.sort_open = false;
        self.modal = Some(Modal::new());
    }

    fn modal_layout(&self) -> ModalLayout {
        let n = self.store.borrow().lib.trash_count() as u64;
        let body = self
            .assets
            .font
            .wrap(&empty_trash_body(n), FONT_MD, WIDTH - 2.0 * PAD);
        let height = PAD + 24.0 + 14.0 + body.len() as f64 * LINE_H + 26.0 + BUTTON_H + PAD;
        let c = (self.vp.w + super::projects::SIDEBAR_W) / 2.0;
        let card = Rect::new(
            c - WIDTH / 2.0,
            (self.vp.h - height) / 2.0,
            c + WIDTH / 2.0,
            (self.vp.h + height) / 2.0,
        );
        let y = card.y1 - PAD - BUTTON_H;
        let confirm_w = 176.0;
        let confirm = Rect::new(card.x1 - PAD - confirm_w, y, card.x1 - PAD, y + BUTTON_H);
        let cancel = Rect::new(
            confirm.x0 - 12.0 - 120.0,
            y,
            confirm.x0 - 12.0,
            y + BUTTON_H,
        );
        ModalLayout {
            card,
            cancel,
            confirm,
            body,
        }
    }

    /// Advances the dialog animation; returns true while it is running.
    pub(super) fn modal_update(&mut self, dt: f32) -> bool {
        let (cancel, confirm) = {
            let l = self.modal_layout();
            (self.at(l.cancel), self.at(l.confirm))
        };
        let Some(m) = self.modal.as_mut() else {
            return false;
        };
        m.shown.step(dt);
        let a = m.cancel.update(dt, cancel);
        let b = m.confirm.update(dt, confirm);
        a || b || !m.shown.settled()
    }

    /// While a dialog is open it consumes every click.
    pub(super) fn modal_click(&mut self) -> bool {
        if self.modal.is_none() {
            return false;
        }
        let l = self.modal_layout();
        if self.at(l.confirm) {
            self.empty_trash();
        } else if self.at(l.cancel) || !self.at(l.card) {
            self.modal = None;
        }
        true
    }

    pub(super) fn modal_hand(&self) -> bool {
        self.modal.is_some() && {
            let l = self.modal_layout();
            self.at(l.cancel) || self.at(l.confirm)
        }
    }

    pub(super) fn paint_modal(&self, cx: &mut Cx) {
        let Some(m) = &self.modal else { return };
        let l = self.modal_layout();
        let t = m.shown.pos.clamp(0.0, 1.0);
        let full = Rect::new(0.0, 0.0, self.vp.w, self.vp.h);
        cx.layer(full, t, (0.0, 6.0 * (1.0 - t as f64)), |cx| {
            cx.fill(full, 0.0, alpha(0x000000, 0.32));
            cx.shadow(l.card, R_MODAL, alpha(0x000000, 0.35), 32.0, 14.0);
            cx.fill(l.card, R_MODAL, hex(bg_panel()));
            cx.stroke(l.card, R_MODAL, 1.0, alpha(overlay(), 0.12));

            let x = l.card.x0 + PAD;
            let badge = Rect::new(x, l.card.y0 + PAD, x + 24.0, l.card.y0 + PAD + 24.0);
            cx.fill(badge, 12.0, alpha(danger(), 0.14));
            cx.icon(Icon::Trash, x + 4.0, badge.y0 + 4.0, 16.0, hex(danger()));
            cx.text(
                tr(Msg::ModalEmptyTitle),
                17.0,
                x + 36.0,
                l.card.y0 + PAD + 18.0,
                hex(text_primary()),
            );
            let mut y = l.card.y0 + PAD + 24.0 + 14.0 + 15.0;
            for line in &l.body {
                cx.text(line, FONT_MD, x, y, hex(text_secondary()));
                y += LINE_H;
            }
            m.cancel.draw_secondary(cx, l.cancel, tr(Msg::Cancel), None);
            m.confirm
                .draw_danger(cx, l.confirm, tr(Msg::MenuDeleteForever), Some(Icon::Trash));
        });
    }
}
