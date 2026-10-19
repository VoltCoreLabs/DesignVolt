//! First-run screen: the user enters a display name (up to 20 characters, any format).

use vello::Scene;
use vello::kurbo::{Affine, Rect};

use super::backdrop;
use crate::anim::{ease_out_cubic, progress};
use crate::draw::Cx;
use crate::i18n::{Msg, tr};
use crate::input::{Key, Mods, Screen, SharedAssets, Transition, Viewport};
use crate::store::NAME_MAX_CHARS;
use crate::theme::colors::*;
use crate::theme::metrics::*;
use crate::theme::motion::{PANEL_INTRO, STAGGER};
use crate::widgets::button::Button;
use crate::widgets::text_input::TextInput;

const CARD_W: f64 = 440.0;
const CARD_H: f64 = 360.0;
const PAD_X: f64 = 40.0;

pub struct Welcome {
    assets: SharedAssets,
    vp: Viewport,
    time: f32,
    mouse: (f64, f64),
    input: TextInput,
    button: Button,
    next: Option<Transition>,
}

struct Layout {
    card: Rect,
    input: Rect,
    button: Rect,
}

impl Welcome {
    pub fn new(assets: SharedAssets) -> Self {
        let mut input = TextInput::new(NAME_MAX_CHARS, Msg::NamePlaceholder, true);
        input.focused = true;
        Self {
            assets,
            vp: Viewport::new(),
            time: 0.0,
            mouse: (-1.0, -1.0),
            input,
            button: Button::new(),
            next: None,
        }
    }

    fn layout(&self) -> Layout {
        let c = Rect::new(0.0, 0.0, self.vp.w, self.vp.h).center();
        let card = Rect::new(
            c.x - CARD_W / 2.0,
            c.y - CARD_H / 2.0,
            c.x + CARD_W / 2.0,
            c.y + CARD_H / 2.0,
        );
        let (x0, x1) = (card.x0 + PAD_X, card.x1 - PAD_X);
        let input = Rect::new(x0, card.y0 + 170.0, x1, card.y0 + 170.0 + 44.0);
        let button = Rect::new(x0, card.y0 + 262.0, x1, card.y0 + 262.0 + 44.0);
        Layout {
            card,
            input,
            button,
        }
    }

    fn at(&self, r: Rect) -> bool {
        r.contains(vello::kurbo::Point::new(self.mouse.0, self.mouse.1))
    }

    fn submit(&mut self) {
        if !self.input.is_blank() {
            self.button.press();
            self.next = Some(Transition::SetName(self.input.text().to_string()));
        }
    }

    fn intro(&self, i: f32) -> f32 {
        ease_out_cubic(progress(self.time, i * STAGGER, PANEL_INTRO))
    }
}

impl Screen for Welcome {
    fn set_viewport(&mut self, w: u32, h: u32, scale: f64) {
        self.vp.set(w, h, scale);
    }

    fn update(&mut self, dt: f32) -> bool {
        self.time += dt;
        let l = self.layout();
        let hover = self.at(l.button) && !self.input.is_blank();
        let a = self.input.update(dt);
        let b = self.button.update(dt, hover);
        a || b || self.time < PANEL_INTRO + 6.0 * STAGGER
    }

    fn draw(&self, scene: &mut Scene) {
        scene.reset();
        let l = self.layout();
        let mut cx = Cx {
            scene,
            font: &self.assets.font,
            icons: &self.assets.icons,
            xf: Affine::scale(self.vp.scale),
        };
        backdrop::draw(&mut cx, self.vp);

        let full = Rect::new(0.0, 0.0, self.vp.w, self.vp.h);
        let p = self.intro(0.0);
        cx.layer(full, p, (0.0, 24.0 * (1.0 - p as f64)), |cx| {
            cx.glass(l.card, R_MODAL)
        });

        let x = l.card.x0 + PAD_X;
        let p = self.intro(1.5);
        cx.layer(full, p, (0.0, 16.0 * (1.0 - p as f64)), |cx| {
            let logo = Rect::new(x, l.card.y0 + 36.0, x + 44.0, l.card.y0 + 80.0);
            cx.shadow(logo, 12.0, alpha(accent(), 0.5), 14.0, 6.0);
            cx.fill(logo, 12.0, hex(accent()));
            cx.text(
                tr(Msg::WelcomeTitle),
                20.0,
                x,
                l.card.y0 + 116.0,
                hex(text_primary()),
            );
            cx.text(
                tr(Msg::WelcomeAsk),
                FONT_MD,
                x,
                l.card.y0 + 142.0,
                hex(text_secondary()),
            );
        });

        let p = self.intro(2.5);
        cx.layer(full, p, (0.0, 16.0 * (1.0 - p as f64)), |cx| {
            self.input.draw(cx, l.input);
            let hint = tr(Msg::NameHint);
            cx.text(
                hint,
                FONT_SM,
                l.input.x0,
                l.input.y1 + 20.0,
                hex(text_disabled()),
            );
        });

        let p = self.intro(3.5);
        cx.layer(full, p, (0.0, 16.0 * (1.0 - p as f64)), |cx| {
            self.button.draw(
                cx,
                l.button,
                tr(Msg::Continue),
                None,
                !self.input.is_blank(),
            );
        });
    }

    fn mouse_moved(&mut self, x: f64, y: f64) {
        self.mouse = (x / self.vp.scale, y / self.vp.scale);
        let left = self.layout().input.x0;
        self.input.drag_to(&self.assets.font, self.mouse.0 - left);
    }

    fn mouse_left(&mut self) {
        self.mouse = (-1.0, -1.0);
    }

    fn click(&mut self, mods: Mods) {
        let l = self.layout();
        if self.at(l.button) {
            self.submit();
        } else if self.at(l.input) {
            self.input
                .press_at(&self.assets.font, self.mouse.0 - l.input.x0, mods.shift);
        } else {
            self.input.focused = false;
        }
    }

    fn mouse_up(&mut self) {
        self.input.dragging = false;
    }

    fn key(&mut self, key: Key, mods: Mods) {
        match key {
            Key::Enter => self.submit(),
            k if self.input.focused => self.input.key(k, mods),
            _ => {}
        }
    }

    fn text(&mut self, s: &str) {
        if self.input.focused {
            self.input.insert(s);
        }
    }

    fn pointer_hand(&self) -> bool {
        let l = self.layout();
        (self.at(l.button) && !self.input.is_blank()) || self.at(l.input)
    }

    fn take_transition(&mut self) -> Option<Transition> {
        self.next.take()
    }
}
