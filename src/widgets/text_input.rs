//! Single-line text input: selection, shortcuts, mouse caret/drag, character limit, focus ring.

use vello::kurbo::Rect;

use crate::anim::Spring;
use crate::draw::Cx;
use crate::input::{Key, Mods};
use crate::text::Font;
use crate::theme::colors::*;
use crate::theme::metrics::{FONT_MD, R_BUTTON};
use crate::theme::motion::{HOVER_DAMPING, HOVER_STIFFNESS};

const BLINK_PERIOD: f32 = 1.1;
/// Horizontal text padding inside the field.
const PAD_X: f64 = 14.0;

pub struct TextInput {
    text: String,
    /// Caret position in characters.
    cursor: usize,
    /// Other end of the selection; equals `cursor` when nothing is selected.
    anchor: usize,
    max_chars: usize,
    placeholder: &'static str,
    show_counter: bool,
    pub focused: bool,
    pub dragging: bool,
    focus: Spring,
    blink: f32,
}

impl TextInput {
    pub fn new(max_chars: usize, placeholder: &'static str, show_counter: bool) -> Self {
        Self {
            text: String::new(),
            cursor: 0,
            anchor: 0,
            max_chars,
            placeholder,
            show_counter,
            focused: false,
            dragging: false,
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

    fn range(&self) -> (usize, usize) {
        (self.cursor.min(self.anchor), self.cursor.max(self.anchor))
    }

    pub fn selected(&self) -> &str {
        let (a, b) = self.range();
        &self.text[self.byte_at(a)..self.byte_at(b)]
    }

    fn delete_selection(&mut self) -> bool {
        let (a, b) = self.range();
        if a == b {
            return false;
        }
        let (ba, bb) = (self.byte_at(a), self.byte_at(b));
        self.text.replace_range(ba..bb, "");
        self.cursor = a;
        self.anchor = a;
        true
    }

    /// Inserts printable characters at the caret, replacing the selection, up to the limit.
    pub fn insert(&mut self, s: &str) {
        self.delete_selection();
        for c in s.chars().filter(|c| !c.is_control()) {
            if self.len() >= self.max_chars {
                break;
            }
            let at = self.byte_at(self.cursor);
            self.text.insert(at, c);
            self.cursor += 1;
        }
        self.anchor = self.cursor;
        self.blink = 0.0;
    }

    /// Removes the selection and returns it (for cut).
    pub fn cut(&mut self) -> String {
        let s = self.selected().to_string();
        self.delete_selection();
        s
    }

    pub fn select_all(&mut self) {
        self.anchor = 0;
        self.cursor = self.len();
        self.blink = 0.0;
    }

    fn move_to(&mut self, pos: usize, extend: bool) {
        self.cursor = pos.min(self.len());
        if !extend {
            self.anchor = self.cursor;
        }
    }

    /// Start of the previous word (or end of the next one when `forward`).
    fn word_target(&self, forward: bool) -> usize {
        let chars: Vec<char> = self.text.chars().collect();
        let is_word = |c: char| c.is_alphanumeric() || c == '_';
        let mut i = self.cursor;
        if forward {
            while i < chars.len() && !is_word(chars[i]) {
                i += 1;
            }
            while i < chars.len() && is_word(chars[i]) {
                i += 1;
            }
        } else {
            while i > 0 && !is_word(chars[i - 1]) {
                i -= 1;
            }
            while i > 0 && is_word(chars[i - 1]) {
                i -= 1;
            }
        }
        i
    }

    pub fn key(&mut self, key: Key, mods: Mods) {
        let (shift, ctrl) = (mods.shift, mods.ctrl);
        match key {
            Key::SelectAll => self.select_all(),
            Key::Backspace => {
                if !self.delete_selection() && self.cursor > 0 {
                    let from = if ctrl { self.word_target(false) } else { self.cursor - 1 };
                    self.anchor = from;
                    self.delete_selection();
                }
            }
            Key::Delete => {
                if !self.delete_selection() && self.cursor < self.len() {
                    let to = if ctrl { self.word_target(true) } else { self.cursor + 1 };
                    self.anchor = to;
                    self.delete_selection();
                }
            }
            Key::Left => {
                let (a, _) = self.range();
                if !shift && a != self.cursor.max(self.anchor) {
                    self.move_to(a, false);
                } else {
                    let to = if ctrl { self.word_target(false) } else { self.cursor.saturating_sub(1) };
                    self.move_to(to, shift);
                }
            }
            Key::Right => {
                let (_, b) = self.range();
                if !shift && self.cursor.min(self.anchor) != b {
                    self.move_to(b, false);
                } else {
                    let to = if ctrl { self.word_target(true) } else { self.cursor + 1 };
                    self.move_to(to, shift);
                }
            }
            Key::Home => self.move_to(0, shift),
            Key::End => self.move_to(self.len(), shift),
            _ => {}
        }
        self.blink = 0.0;
    }

    /// Character index nearest to `x` (logical px relative to the field's left edge).
    pub fn index_at(&self, font: &Font, x: f64) -> usize {
        let target = x - PAD_X;
        let mut prev = 0.0;
        for i in 1..=self.len() {
            let prefix: String = self.text.chars().take(i).collect();
            let w = font.layout(&prefix, FONT_MD).1 as f64;
            if target < (prev + w) / 2.0 {
                return i - 1;
            }
            prev = w;
        }
        self.len()
    }

    pub fn press_at(&mut self, font: &Font, x: f64, extend: bool) {
        self.focused = true;
        self.dragging = true;
        let i = self.index_at(font, x);
        self.move_to(i, extend);
        self.blink = 0.0;
    }

    pub fn drag_to(&mut self, font: &Font, x: f64) {
        if self.dragging {
            let i = self.index_at(font, x);
            self.move_to(i, true);
        }
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
        let (x, base) = (r.x0 + PAD_X, r.center().y + FONT_MD as f64 * 0.36);
        let width_of = |cx: &Cx, n: usize| {
            let s: String = self.text.chars().take(n).collect();
            cx.text_width(&s, FONT_MD)
        };

        let (a, b) = self.range();
        if self.focused && a != b {
            let sel = Rect::new(x + width_of(cx, a), r.y0 + 8.0, x + width_of(cx, b), r.y1 - 8.0);
            cx.fill(sel, 3.0, alpha(ACCENT, 0.45));
        }
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
        if self.focused && a == b && self.blink < BLINK_PERIOD / 2.0 {
            let pos = x + width_of(cx, self.cursor);
            cx.fill(Rect::new(pos, r.y0 + 10.0, pos + 1.5, r.y1 - 10.0), 0.75, hex(ACCENT));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: Mods = Mods { ctrl: false, shift: false };
    const SHIFT: Mods = Mods { ctrl: false, shift: true };
    const CTRL: Mods = Mods { ctrl: true, shift: false };

    fn input(max: usize) -> TextInput {
        TextInput::new(max, "", false)
    }

    fn with(text: &str) -> TextInput {
        let mut t = input(40);
        t.insert(text);
        t
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
        t.insert("a\u{8}b\r\nc\t\u{1}");
        assert_eq!(t.text(), "abc");
    }

    #[test]
    fn backspace_delete_and_caret_work_on_multibyte() {
        let mut t = with("a🎨ж");
        t.key(Key::Left, NONE);
        t.key(Key::Backspace, NONE);
        assert_eq!(t.text(), "aж");
        t.key(Key::Delete, NONE);
        assert_eq!(t.text(), "a");
        t.key(Key::Home, NONE);
        t.insert("Ж");
        assert_eq!(t.text(), "Жa");
        t.key(Key::End, NONE);
        t.insert("!");
        assert_eq!(t.text(), "Жa!");
    }

    #[test]
    fn select_all_then_type_replaces_everything() {
        let mut t = with("Вадим");
        t.key(Key::SelectAll, CTRL);
        assert_eq!(t.selected(), "Вадим");
        t.insert("Neo");
        assert_eq!(t.text(), "Neo");
        assert_eq!(t.selected(), "");
    }

    #[test]
    fn select_all_then_backspace_clears() {
        let mut t = with("hello");
        t.key(Key::SelectAll, CTRL);
        t.key(Key::Backspace, NONE);
        assert_eq!(t.text(), "");
    }

    #[test]
    fn shift_arrows_extend_selection_and_plain_arrows_collapse() {
        let mut t = with("abcdef");
        t.key(Key::Left, SHIFT);
        t.key(Key::Left, SHIFT);
        assert_eq!(t.selected(), "ef");
        t.key(Key::Left, NONE);
        assert_eq!(t.selected(), "");
        t.insert("X");
        assert_eq!(t.text(), "abcdXef");
    }

    #[test]
    fn shift_home_selects_to_start() {
        let mut t = with("abc def");
        t.key(Key::Home, SHIFT);
        assert_eq!(t.selected(), "abc def");
    }

    #[test]
    fn ctrl_arrows_jump_words_and_ctrl_backspace_deletes_word() {
        let mut t = with("hello big world");
        t.key(Key::Left, CTRL);
        t.key(Key::Left, CTRL);
        t.insert("_");
        assert_eq!(t.text(), "hello _big world");
        let mut t = with("one two");
        t.key(Key::Backspace, CTRL);
        assert_eq!(t.text(), "one ");
    }

    #[test]
    fn cut_returns_selection_and_removes_it() {
        let mut t = with("abcdef");
        t.key(Key::Left, SHIFT);
        t.key(Key::Left, SHIFT);
        assert_eq!(t.cut(), "ef");
        assert_eq!(t.text(), "abcd");
    }

    #[test]
    fn insert_over_selection_respects_limit() {
        let mut t = input(5);
        t.insert("abcde");
        t.key(Key::SelectAll, CTRL);
        t.insert("123456789");
        assert_eq!(t.text(), "12345");
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
