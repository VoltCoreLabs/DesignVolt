//! Flat accent button: hover lightens the fill, press darkens it; no scaling or glow.

use vello::kurbo::Rect;

use crate::anim::Spring;
use crate::draw::Cx;
use crate::icons::Icon;
use crate::theme::colors::*;
use crate::theme::metrics::{FONT_MD, R_BUTTON};
use crate::theme::motion::*;

const PRESS_PULSE: f32 = 0.12;

pub struct Button {
    hover: Spring,
    press: Spring,
    pulse: f32,
}

impl Button {
    pub fn new() -> Self {
        Self {
            hover: Spring::new(0.0, HOVER_STIFFNESS, HOVER_DAMPING),
            press: Spring::new(0.0, PRESS_STIFFNESS, PRESS_DAMPING),
            pulse: 0.0,
        }
    }

    pub fn press(&mut self) {
        self.pulse = PRESS_PULSE;
    }

    /// Returns true while an animation is running.
    pub fn update(&mut self, dt: f32, hovered: bool) -> bool {
        self.pulse = (self.pulse - dt).max(0.0);
        self.hover.target = if hovered { 1.0 } else { 0.0 };
        self.press.target = if self.pulse > 0.0 { 1.0 } else { 0.0 };
        self.hover.step(dt);
        self.press.step(dt);
        !self.hover.settled() || !self.press.settled() || self.pulse > 0.0
    }

    pub fn draw(&self, cx: &mut Cx, r: Rect, label: &str, icon: Option<Icon>, enabled: bool) {
        self.draw_styled(cx, r, label, icon, enabled, Style::Primary);
    }

    /// Quiet button: raised background and border instead of the accent fill.
    pub fn draw_secondary(&self, cx: &mut Cx, r: Rect, label: &str, icon: Option<Icon>) {
        self.draw_styled(cx, r, label, icon, true, Style::Secondary);
    }

    /// Red button for destructive confirmations.
    pub fn draw_danger(&self, cx: &mut Cx, r: Rect, label: &str, icon: Option<Icon>) {
        self.draw_styled(cx, r, label, icon, true, Style::Danger);
    }

    fn draw_styled(
        &self,
        cx: &mut Cx,
        r: Rect,
        label: &str,
        icon: Option<Icon>,
        enabled: bool,
        style: Style,
    ) {
        let (h, p) = if enabled {
            (
                self.hover.pos.clamp(0.0, 1.0),
                self.press.pos.clamp(0.0, 1.0),
            )
        } else {
            (0.0, 0.0)
        };
        let a = if enabled { 1.0 } else { 0.35 };
        let secondary = style == Style::Secondary;
        if secondary {
            cx.fill(r, R_BUTTON, hex(bg_raised()));
            cx.fill(r, R_BUTTON, alpha(accent(), 0.10 * h));
            cx.stroke(r, R_BUTTON, 1.0, alpha(overlay(), 0.12 + 0.12 * h));
        } else {
            // Calm feedback: the fill only gets lighter on hover and darker on press.
            let base = if style == Style::Danger {
                danger()
            } else {
                accent()
            };
            cx.fill(r, R_BUTTON, alpha(base, a));
            cx.fill(r, R_BUTTON, alpha(WHITE, 0.12 * h));
            cx.fill(r, R_BUTTON, alpha(0x000000, 0.14 * p));
        }

        let text_w = cx.text_width(label, FONT_MD);
        let icon_w = if icon.is_some() { 22.0 } else { 0.0 };
        let mut x = r.center().x - (text_w + icon_w) / 2.0;
        let color = if secondary {
            hex(text_primary())
        } else {
            alpha(WHITE, a)
        };
        if let Some(i) = icon {
            cx.icon(i, x, r.center().y - 8.0, 16.0, color);
            x += icon_w;
        }
        cx.text(
            label,
            FONT_MD,
            x,
            r.center().y + FONT_MD as f64 * 0.36,
            color,
        );
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Style {
    Primary,
    Secondary,
    Danger,
}
