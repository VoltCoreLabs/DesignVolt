//! Gradient button with hover scale/glow springs and a short press pulse.

use vello::kurbo::{Affine, Rect, Vec2};

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
        let (h, p) = if enabled {
            (self.hover.pos as f64, self.press.pos as f64)
        } else {
            (0.0, 0.0)
        };
        let k = 1.0 + 0.03 * h - 0.05 * p;
        let c = r.center();
        let old = cx.xf;
        cx.xf = old
            * Affine::translate(Vec2::new(c.x, c.y))
            * Affine::scale(k)
            * Affine::translate(Vec2::new(-c.x, -c.y));

        let a = if enabled { 1.0 } else { 0.35 };
        if enabled {
            cx.shadow(
                r,
                R_BUTTON,
                alpha(accent(), (0.30 + 0.30 * h as f32).min(0.7)),
                14.0 + 8.0 * h,
                6.0,
            );
        }
        cx.fill_grad(r, R_BUTTON, alpha(accent_from(), a), alpha(accent_to(), a));
        cx.fill(r, R_BUTTON, alpha(WHITE, 0.10 * h as f32));

        let text_w = cx.text_width(label, FONT_MD);
        let icon_w = if icon.is_some() { 22.0 } else { 0.0 };
        let mut x = r.center().x - (text_w + icon_w) / 2.0;
        let color = alpha(WHITE, a);
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
        cx.xf = old;
    }
}
