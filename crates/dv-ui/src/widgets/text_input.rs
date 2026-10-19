//! Single-line text input widget: browser-style shortcuts, clipboard, mouse selection, drawing.

use std::time::{Duration, Instant};

use vello::kurbo::Rect;

use super::text_edit::Edit;
use crate::anim::Spring;
use crate::clipboard;
use crate::draw::Cx;
use crate::input::{Key, Mods};
use crate::text::Font;
use crate::theme::colors::*;
use crate::theme::metrics::{FONT_MD, R_BUTTON};
use crate::theme::motion::{HOVER_DAMPING, HOVER_STIFFNESS};

const BLINK_PERIOD: f32 = 1.1;
const MULTI_CLICK: Duration = Duration::from_millis(400);
/// Horizontal text padding inside the field.
pub const PAD_X: f64 = 14.0;

pub struct TextInput {
    edit: Edit,
    max_chars: usize,
    show_counter: bool,
    pub focused: bool,
    pub dragging: bool,
    focus: Spring,
    blink: f32,
    last_press: Option<(Instant, usize)>,
    clicks: u8,
    /// Font size of the text (px).
    size: f32,
}

impl TextInput {
    pub fn new(max_chars: usize, show_counter: bool) -> Self {
        Self {
            edit: Edit::new(max_chars),
            max_chars,
            show_counter,
            focused: false,
            dragging: false,
            focus: Spring::new(0.0, HOVER_STIFFNESS, HOVER_DAMPING),
            blink: 0.0,
            last_press: None,
            clicks: 0,
            size: FONT_MD,
        }
    }

    pub fn text(&self) -> &str {
        self.edit.text()
    }

    pub fn select_all(&mut self) {
        self.edit.select_all();
    }

    pub fn is_blank(&self) -> bool {
        self.edit.text().trim().is_empty()
    }

    pub fn insert(&mut self, s: &str) {
        self.edit.type_text(s);
        self.blink = 0.0;
    }

    fn copy(&self) {
        if self.edit.has_selection() {
            clipboard::set_text(self.edit.selected());
        }
    }

    fn cut(&mut self) {
        if let Some(s) = self.edit.cut() {
            clipboard::set_text(&s);
        }
    }

    fn paste(&mut self) {
        if let Some(s) = clipboard::get_text() {
            self.edit.paste(&s);
        }
    }

    /// Browser-style shortcuts: Ctrl+A/C/X/V/Z/Y, Ctrl+Insert, Shift+Insert, Shift+Delete,
    /// plus caret movement and deletion (see `Edit::key`).
    pub fn key(&mut self, key: Key, mods: Mods) {
        match key {
            Key::Copy => self.copy(),
            Key::Cut => self.cut(),
            Key::Paste => self.paste(),
            Key::Undo => self.edit.undo(),
            Key::Redo => self.edit.redo(),
            Key::Insert if mods.ctrl => self.copy(),
            Key::Insert if mods.shift => self.paste(),
            Key::Delete if mods.shift && self.edit.has_selection() => self.cut(),
            other => self.edit.key(other, mods),
        }
        self.blink = 0.0;
    }

    /// Character index nearest to `x` (logical px relative to the field's left edge).
    pub fn index_at(&self, font: &Font, x: f64) -> usize {
        let target = x - PAD_X;
        let chars: Vec<char> = self.edit.text().chars().collect();
        let mut prev = 0.0;
        for i in 1..=chars.len() {
            let prefix: String = chars[..i].iter().collect();
            let w = font.layout(&prefix, self.size).1 as f64;
            if target < (prev + w) / 2.0 {
                return i - 1;
            }
            prev = w;
        }
        chars.len()
    }

    /// Mouse press: click places the caret (Shift extends), double-click selects a word,
    /// triple-click selects everything.
    pub fn press_at(&mut self, font: &Font, x: f64, extend: bool) {
        let idx = self.index_at(font, x);
        let repeat = self
            .last_press
            .is_some_and(|(t, i)| t.elapsed() < MULTI_CLICK && i == idx);
        self.clicks = if repeat { (self.clicks + 1).min(3) } else { 1 };
        self.last_press = Some((Instant::now(), idx));
        self.focused = true;
        self.dragging = self.clicks == 1;
        match self.clicks {
            1 => self.edit.move_to(idx, extend),
            2 => self.edit.select_word_at(idx),
            _ => self.edit.select_all(),
        }
        self.blink = 0.0;
    }

    pub fn drag_to(&mut self, font: &Font, x: f64) {
        if self.dragging {
            let i = self.index_at(font, x);
            self.edit.move_to(i, true);
        }
    }

    /// Sets the font size (px) of the text; the default is the body size.
    pub fn with_font_size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    /// Draws only the editable text, the selection and the caret, with the text starting at `x`
    /// on `baseline`; the field's own background and label stay as the caller drew them.
    /// `r` gives the vertical extent of the selection and the caret.
    pub fn draw_inline(&self, cx: &mut Cx, r: Rect, x: f64, baseline: f64) {
        let text = self.edit.text();
        let width_of = |cx: &Cx, n: usize| {
            let s: String = text.chars().take(n).collect();
            cx.text_width(&s, self.size)
        };
        let (a, b) = self.edit.range();
        if self.focused && a != b {
            let sel = Rect::new(
                x + width_of(cx, a),
                r.y0 + 5.0,
                x + width_of(cx, b),
                r.y1 - 5.0,
            );
            cx.fill(sel, 2.0, alpha(accent(), 0.35));
        }
        cx.text(text, self.size, x, baseline, hex(text_primary()));
        if self.focused && a == b && self.blink < BLINK_PERIOD / 2.0 {
            let pos = x + width_of(cx, self.edit.cursor());
            cx.fill(
                Rect::new(pos, r.y0 + 6.0, pos + 1.5, r.y1 - 6.0),
                0.75,
                hex(accent()),
            );
        }
    }

    /// Returns true while an animation is running.
    pub fn update(&mut self, dt: f32) -> bool {
        self.focus.target = if self.focused { 1.0 } else { 0.0 };
        self.focus.step(dt);
        self.blink = (self.blink + dt) % BLINK_PERIOD;
        self.focused || !self.focus.settled()
    }

    /// `placeholder` is shown while the field is empty.
    pub fn draw(&self, cx: &mut Cx, r: Rect, placeholder: &str) {
        let f = self.focus.pos.clamp(0.0, 1.0);
        cx.fill(r, R_BUTTON, hex(bg_raised()));
        cx.stroke(r, R_BUTTON, 1.0, alpha(overlay(), 0.12));
        if f > 0.0 {
            cx.stroke(r.inflate(1.0, 1.0), R_BUTTON + 1.0, 1.5, alpha(accent(), f));
        }
        let (x, base) = (r.x0 + PAD_X, r.center().y + self.size as f64 * 0.36);
        let text = self.edit.text();
        let width_of = |cx: &Cx, n: usize| {
            let s: String = text.chars().take(n).collect();
            cx.text_width(&s, self.size)
        };

        let (a, b) = self.edit.range();
        if self.focused && a != b {
            let sel = Rect::new(
                x + width_of(cx, a),
                r.y0 + 8.0,
                x + width_of(cx, b),
                r.y1 - 8.0,
            );
            cx.fill(sel, 3.0, alpha(accent(), 0.35));
        }
        if text.is_empty() {
            cx.text(placeholder, self.size, x, base, hex(text_disabled()));
        } else {
            cx.text(text, self.size, x, base, hex(text_primary()));
        }
        if self.show_counter {
            let c = format!("{}/{}", self.edit.len(), self.max_chars);
            let w = cx.text_width(&c, 11.0);
            cx.text(&c, 11.0, r.x1 - 12.0 - w, base, hex(text_disabled()));
        }
        if self.focused && a == b && self.blink < BLINK_PERIOD / 2.0 {
            let pos = x + width_of(cx, self.edit.cursor());
            cx.fill(
                Rect::new(pos, r.y0 + 10.0, pos + 1.5, r.y1 - 10.0),
                0.75,
                hex(accent()),
            );
        }
    }
}
