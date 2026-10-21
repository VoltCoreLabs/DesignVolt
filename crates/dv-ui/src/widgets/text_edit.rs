//! Pure single-line editing model: text, caret, selection, word navigation and undo/redo.
//! No rendering or I/O, so every behavior is unit-tested.

use crate::input::{Key, Mods};

const HISTORY_LIMIT: usize = 200;

#[derive(Clone, PartialEq, Debug)]
struct Snap {
    text: String,
    cursor: usize,
    anchor: usize,
}

/// Kind of the last edit; consecutive edits of the same kind share one undo step.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Typing,
    Backspace,
    Delete,
    Other,
    Idle,
}

pub struct Edit {
    text: String,
    /// Caret position in characters.
    cursor: usize,
    /// Other end of the selection; equals `cursor` when nothing is selected.
    anchor: usize,
    max_chars: usize,
    undo: Vec<Snap>,
    redo: Vec<Snap>,
    last: Kind,
}

fn class(c: char) -> u8 {
    if c.is_alphanumeric() || c == '_' {
        0
    } else if c.is_whitespace() {
        1
    } else {
        2
    }
}

impl Edit {
    pub fn new(max_chars: usize) -> Self {
        Self {
            text: String::new(),
            cursor: 0,
            anchor: 0,
            max_chars,
            undo: Vec::new(),
            redo: Vec::new(),
            last: Kind::Idle,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn len(&self) -> usize {
        self.text.chars().count()
    }
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn range(&self) -> (usize, usize) {
        (self.cursor.min(self.anchor), self.cursor.max(self.anchor))
    }

    pub fn has_selection(&self) -> bool {
        self.cursor != self.anchor
    }

    fn byte_at(&self, chars: usize) -> usize {
        self.text
            .char_indices()
            .nth(chars)
            .map_or(self.text.len(), |(i, _)| i)
    }

    pub fn selected(&self) -> &str {
        let (a, b) = self.range();
        &self.text[self.byte_at(a)..self.byte_at(b)]
    }

    fn snap(&self) -> Snap {
        Snap {
            text: self.text.clone(),
            cursor: self.cursor,
            anchor: self.anchor,
        }
    }

    fn restore(&mut self, s: Snap) {
        self.text = s.text;
        self.cursor = s.cursor;
        self.anchor = s.anchor;
        self.last = Kind::Idle;
    }

    /// Starts an edit: saves an undo step unless it continues a run of the same kind.
    fn begin(&mut self, kind: Kind) {
        if kind == Kind::Other || kind != self.last || self.has_selection() {
            if self.undo.len() >= HISTORY_LIMIT {
                self.undo.remove(0);
            }
            self.undo.push(self.snap());
            self.redo.clear();
        }
        self.last = kind;
    }

    pub fn undo(&mut self) {
        if let Some(prev) = self.undo.pop() {
            self.redo.push(self.snap());
            self.restore(prev);
        }
    }

    pub fn redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(self.snap());
            self.restore(next);
        }
    }

    fn remove_range(&mut self, a: usize, b: usize) {
        let (ba, bb) = (self.byte_at(a), self.byte_at(b));
        self.text.replace_range(ba..bb, "");
        self.cursor = a;
        self.anchor = a;
    }

    fn insert_chars(&mut self, s: &str) {
        let (a, b) = self.range();
        if a != b {
            self.remove_range(a, b);
        }
        for c in s.chars().filter(|c| !c.is_control()) {
            if self.len() >= self.max_chars {
                break;
            }
            let at = self.byte_at(self.cursor);
            self.text.insert(at, c);
            self.cursor += 1;
        }
        self.anchor = self.cursor;
    }

    /// Typed text: replaces the selection, respects the limit.
    pub fn type_text(&mut self, s: &str) {
        self.begin(Kind::Typing);
        self.insert_chars(s);
    }

    /// Pasted text: always its own undo step.
    pub fn paste(&mut self, s: &str) {
        self.begin(Kind::Other);
        self.insert_chars(s);
    }

    /// Removes the selection and returns it (for cut); `None` when nothing is selected.
    pub fn cut(&mut self) -> Option<String> {
        if !self.has_selection() {
            return None;
        }
        self.begin(Kind::Other);
        let s = self.selected().to_string();
        let (a, b) = self.range();
        self.remove_range(a, b);
        Some(s)
    }

    pub fn select_all(&mut self) {
        self.anchor = 0;
        self.cursor = self.len();
        self.last = Kind::Idle;
    }

    pub fn move_to(&mut self, pos: usize, extend: bool) {
        self.cursor = pos.min(self.len());
        if !extend {
            self.anchor = self.cursor;
        }
        self.last = Kind::Idle;
    }

    /// Selects the word (or run of spaces/punctuation) around character index `idx`.
    pub fn select_word_at(&mut self, idx: usize) {
        let chars: Vec<char> = self.text.chars().collect();
        if chars.is_empty() {
            return;
        }
        let i = idx.min(chars.len() - 1);
        let k = class(chars[i]);
        let mut a = i;
        while a > 0 && class(chars[a - 1]) == k {
            a -= 1;
        }
        let mut b = i + 1;
        while b < chars.len() && class(chars[b]) == k {
            b += 1;
        }
        self.anchor = a;
        self.cursor = b;
        self.last = Kind::Idle;
    }

    /// Start of the previous word, or end of the next one when `forward`.
    fn word_target(&self, forward: bool) -> usize {
        let chars: Vec<char> = self.text.chars().collect();
        let word = |c: char| class(c) == 0;
        let mut i = self.cursor;
        if forward {
            while i < chars.len() && !word(chars[i]) {
                i += 1;
            }
            while i < chars.len() && word(chars[i]) {
                i += 1;
            }
        } else {
            while i > 0 && !word(chars[i - 1]) {
                i -= 1;
            }
            while i > 0 && word(chars[i - 1]) {
                i -= 1;
            }
        }
        i
    }

    /// Caret movement and deletion keys.
    pub fn key(&mut self, key: Key, mods: Mods) {
        let (shift, ctrl) = (mods.shift, mods.ctrl);
        match key {
            Key::Backspace => self.delete_back(ctrl),
            Key::Delete => self.delete_forward(ctrl),
            Key::Left => {
                let (a, _) = self.range();
                if self.has_selection() && !shift {
                    self.move_to(a, false);
                } else {
                    let to = if ctrl {
                        self.word_target(false)
                    } else {
                        self.cursor.saturating_sub(1)
                    };
                    self.move_to(to, shift);
                }
            }
            Key::Right => {
                let (_, b) = self.range();
                if self.has_selection() && !shift {
                    self.move_to(b, false);
                } else {
                    let to = if ctrl {
                        self.word_target(true)
                    } else {
                        self.cursor + 1
                    };
                    self.move_to(to, shift);
                }
            }
            Key::Home => self.move_to(0, shift),
            Key::End => self.move_to(self.len(), shift),
            Key::SelectAll => self.select_all(),
            _ => {}
        }
    }

    fn delete_back(&mut self, word: bool) {
        if self.has_selection() {
            self.begin(Kind::Other);
            let (a, b) = self.range();
            self.remove_range(a, b);
        } else if self.cursor > 0 {
            self.begin(Kind::Backspace);
            let from = if word {
                self.word_target(false)
            } else {
                self.cursor - 1
            };
            self.remove_range(from, self.cursor);
        }
    }

    fn delete_forward(&mut self, word: bool) {
        if self.has_selection() {
            self.begin(Kind::Other);
            let (a, b) = self.range();
            self.remove_range(a, b);
        } else if self.cursor < self.len() {
            self.begin(Kind::Delete);
            let to = if word {
                self.word_target(true)
            } else {
                self.cursor + 1
            };
            self.remove_range(self.cursor, to);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: Mods = Mods {
        ctrl: false,
        shift: false,
        alt: false,
    };
    const SHIFT: Mods = Mods {
        ctrl: false,
        shift: true,
        alt: false,
    };
    const CTRL: Mods = Mods {
        ctrl: true,
        shift: false,
        alt: false,
    };

    fn with(text: &str) -> Edit {
        let mut e = Edit::new(40);
        e.type_text(text);
        e
    }

    fn typed_chars(e: &mut Edit, s: &str) {
        for c in s.chars() {
            e.type_text(&c.to_string());
        }
    }

    #[test]
    fn limit_counts_characters_and_stops() {
        let mut e = Edit::new(5);
        e.type_text("Привет мир");
        assert_eq!(e.text(), "Приве");
        e.type_text("x");
        assert_eq!(e.text(), "Приве");
    }

    #[test]
    fn control_chars_are_ignored() {
        let mut e = Edit::new(10);
        e.type_text("a\u{8}b\r\nc\t\u{1}");
        assert_eq!(e.text(), "abc");
    }

    #[test]
    fn multibyte_editing() {
        let mut e = with("a🎨ж");
        e.key(Key::Left, NONE);
        e.key(Key::Backspace, NONE);
        assert_eq!(e.text(), "aж");
        e.key(Key::Delete, NONE);
        assert_eq!(e.text(), "a");
        e.key(Key::Home, NONE);
        e.type_text("Ж");
        assert_eq!(e.text(), "Жa");
    }

    #[test]
    fn select_all_and_type_replaces() {
        let mut e = with("Вадим");
        e.select_all();
        assert_eq!(e.selected(), "Вадим");
        e.type_text("Neo");
        assert_eq!(e.text(), "Neo");
    }

    #[test]
    fn shift_arrows_select_and_plain_arrows_collapse() {
        let mut e = with("abcdef");
        e.key(Key::Left, SHIFT);
        e.key(Key::Left, SHIFT);
        assert_eq!(e.selected(), "ef");
        e.key(Key::Left, NONE);
        assert_eq!(e.selected(), "");
        e.type_text("X");
        assert_eq!(e.text(), "abcdXef");
    }

    #[test]
    fn ctrl_word_navigation_and_deletion() {
        let mut e = with("hello big world");
        e.key(Key::Left, CTRL);
        e.key(Key::Left, CTRL);
        e.type_text("_");
        assert_eq!(e.text(), "hello _big world");
        let mut e = with("one two");
        e.key(Key::Backspace, CTRL);
        assert_eq!(e.text(), "one ");
        e.key(Key::Home, NONE);
        e.key(Key::Delete, CTRL);
        assert_eq!(e.text(), " ");
    }

    #[test]
    fn shift_ctrl_arrow_selects_a_word() {
        let mut e = with("alpha beta");
        e.key(
            Key::Left,
            Mods {
                ctrl: true,
                shift: true,
                alt: false,
            },
        );
        assert_eq!(e.selected(), "beta");
    }

    #[test]
    fn cut_and_paste() {
        let mut e = with("abcdef");
        e.key(Key::Left, SHIFT);
        e.key(Key::Left, SHIFT);
        assert_eq!(e.cut().as_deref(), Some("ef"));
        assert_eq!(e.text(), "abcd");
        assert_eq!(e.cut(), None);
        e.paste("XYZ");
        assert_eq!(e.text(), "abcdXYZ");
        e.paste("12345678901234567890123456789012345678901234567890");
        assert_eq!(e.len(), 40);
    }

    #[test]
    fn typing_run_is_one_undo_step() {
        let mut e = Edit::new(40);
        typed_chars(&mut e, "hello");
        e.undo();
        assert_eq!(e.text(), "");
        e.redo();
        assert_eq!(e.text(), "hello");
    }

    #[test]
    fn moving_the_caret_starts_a_new_undo_step() {
        let mut e = Edit::new(40);
        typed_chars(&mut e, "ab");
        e.key(Key::Left, NONE);
        typed_chars(&mut e, "X");
        assert_eq!(e.text(), "aXb");
        e.undo();
        assert_eq!(e.text(), "ab");
        e.undo();
        assert_eq!(e.text(), "");
    }

    #[test]
    fn undo_restores_cut_paste_and_replaced_selection() {
        let mut e = with("hello");
        e.select_all();
        e.type_text("X");
        assert_eq!(e.text(), "X");
        e.undo();
        assert_eq!(e.text(), "hello");
        e.select_all();
        e.cut();
        assert_eq!(e.text(), "");
        e.undo();
        assert_eq!(e.text(), "hello");
        e.paste("!");
        e.undo();
        assert_eq!(e.text(), "hello");
    }

    #[test]
    fn new_edit_clears_redo() {
        let mut e = Edit::new(40);
        typed_chars(&mut e, "ab");
        e.undo();
        e.type_text("c");
        e.redo();
        assert_eq!(e.text(), "c");
    }

    #[test]
    fn backspace_run_is_one_step() {
        let mut e = with("abcd");
        e.key(Key::Backspace, NONE);
        e.key(Key::Backspace, NONE);
        assert_eq!(e.text(), "ab");
        e.undo();
        assert_eq!(e.text(), "abcd");
    }

    #[test]
    fn double_click_selects_word_space_or_punctuation() {
        let mut e = with("foo, bar  baz");
        e.select_word_at(1);
        assert_eq!(e.selected(), "foo");
        e.select_word_at(3);
        assert_eq!(e.selected(), ",");
        e.select_word_at(8);
        assert_eq!(e.selected(), "  ");
        e.select_word_at(99);
        assert_eq!(e.selected(), "baz");
    }
}
