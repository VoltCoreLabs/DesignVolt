//! Single-line text input: typing, caret movement, character limit, focus ring and blinking caret.

use vello::kurbo::Rect;

use crate::anim::Spring;
use crate::draw::Cx;
use crate::input::Key;
use crate::theme::colors::*;
use crate::theme::metrics::{FONT_MD, R_BUTTON};
use crate::theme::motion::{HOVER_DAMPING, HOVER_STIFFNESS};

const BLINK_PERIOD: f32 = 1.1;

pub struct TextInput {
    text: String,
    /// Caret position in characters.
    cursor: usize,
    max_chars: usize,
    placeholder: &'static str,
    show_counter: bool,
    pub focused: bool,
    focus: Spring,
    blink: f32,
}

impl TextInput {
    pub fn new(max_chars: usize, placeholder: &'static str, show_counter: bool) -> Self {
        Self {
            text: String::new(),
            cursor: 0,
            max_chars,
            placeholder,
            show_counter,
            focused: false,
            focus: Spring::new(0.0, HOVER_STIFFNESS, HOVER_DAMPING),
            blink: 0.0,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn is_blank(&self) -> bool {
        self.text.trim().is_empty()
    }

    fn byte_at(&self, chars: usize) -> usize {
        self.text.char_indices().nth(chars).map_or(self.text.len(), |(i, _)| i)
    }

    fn len(&self) -> usize {
        self.text.chars().count()
    }

    pub fn insert(&mut self, s: &str) {
        for c in s.chars().filter(|c| !c.is_control()) {
            if self.len() >= self.max_chars {
                break;
            }
            let at = self.byte_at(self.cursor);
            self.text.insert(at, c);
            self.cursor += 1;
        }
        self.blink = 0.0;
    }

    pub fn key(&mut self, key: Key) {
        match key {
            Key::Backspace if self.cursor > 0 => {
                let (a, b) = (self.byte_at(self.cursor - 1), self.byte_at(self.cursor));
                self.text.replace_range(a..b, "");
                self.cursor -= 1;
            }
            Key::Delete if self.cursor < self.len() => {
                let (a, b) = (self.byte_at(self.cursor), self.byte_at(self.cursor + 1));
                self.text.replace_range(a..b, "");
            }
            Key::Left => self.cursor = self.cursor.saturating_sub(1),
            Key::Right => self.cursor = (self.cursor + 1).min(self.len()),
            Key::Home => self.cursor = 0,
            Key::End => self.cursor = self.len(),
            _ => {}
        }
        self.blink = 0.0;
    }

    /// Returns true while an animation is running.
    pub fn update(&mut self, dt: f32) -> bool {
        self.focus.target = if self.focused { 1.0 } else { 0.0 };
        self.focus.step(dt);
        self.blink = (self.blink + dt) % BLINK_PERIOD;
        self.focused || !self.focus.settled()
    }

    pub fn draw(&self, cx: &mut Cx, r: Rect) {
        let f = self.focus.pos.clamp(0.0, 1.0);
        cx.fill(r, R_BUTTON, hex(BG_RAISED));
        cx.stroke(r, R_BUTTON, 1.0, alpha(WHITE, 0.10));
        if f > 0.0 {
            cx.stroke(r.inflate(1.0, 1.0), R_BUTTON + 1.0, 1.5, alpha(ACCENT, f));
        }
        let (x, base) = (r.x0 + 14.0, r.center().y + FONT_MD as f64 * 0.36);
        if self.text.is_empty() {
            cx.text(self.placeholder, FONT_MD, x, base, hex(TEXT_DISABLED));
        } else {
            cx.text(&self.text, FONT_MD, x, base, hex(TEXT_PRIMARY));
        }
        if self.show_counter {
            let c = format!("{}/{}", self.len(), self.max_chars);
            let w = cx.text_width(&c, 11.0);
            cx.text(&c, 11.0, r.x1 - 12.0 - w, base, hex(TEXT_DISABLED));
        }
        if self.focused && self.blink < BLINK_PERIOD / 2.0 {
            let before: String = self.text.chars().take(self.cursor).collect();
            let cx_pos = x + cx.text_width(&before, FONT_MD);
            let caret = Rect::new(cx_pos, r.y0 + 10.0, cx_pos + 1.5, r.y1 - 10.0);
            cx.fill(caret, 0.75, hex(ACCENT));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(max: usize) -> TextInput {
        TextInput::new(max, "", false)
    }

    #[test]
    fn limit_counts_characters_and_stops() {
        let mut t = input(5);
        t.insert("Привет мир");
        assert_eq!(t.text(), "Приве");
        t.insert("x");
        assert_eq!(t.text(), "Приве");
    }

    #[test]
    fn control_chars_are_ignored() {
        let mut t = input(10);
        t.insert("a\u{8}b\r\nc\t");
        assert_eq!(t.text(), "abc");
    }

    #[test]
    fn backspace_delete_and_caret_work_on_multibyte() {
        let mut t = input(10);
        t.insert("a🎨ж");
        t.key(Key::Left);
        t.key(Key::Backspace);
        assert_eq!(t.text(), "aж");
        t.key(Key::Delete);
        assert_eq!(t.text(), "a");
        t.key(Key::Home);
        t.insert("Ж");
        assert_eq!(t.text(), "Жa");
        t.key(Key::End);
        t.insert("!");
        assert_eq!(t.text(), "Жa!");
    }

    #[test]
    fn blank_detection() {
        let mut t = input(10);
        assert!(t.is_blank());
        t.insert("   ");
        assert!(t.is_blank());
        t.insert("a");
        assert!(!t.is_blank());
    }
}
