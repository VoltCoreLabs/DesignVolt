//! Typing numbers into the property fields of the right panel (X, Y, W, H, angle, radius).

use dv_ui::input::{Key, Mods};
use dv_ui::theme::metrics::FONT_UI;
use dv_ui::widgets::text_input::{PAD_X, TextInput};

use super::ops::{self, Field};
use super::panels;
use super::state::Editor;

/// Longest number that fits a field.
const MAX_CHARS: usize = 12;

/// A field being edited.
pub struct PropEdit {
    pub field: Field,
    pub input: TextInput,
}

/// Parses what the user typed: a decimal number, `,` allowed as the separator.
pub fn parse_number(s: &str) -> Option<f64> {
    let v: f64 = s.trim().replace(',', ".").parse().ok()?;
    v.is_finite().then_some(v)
}

/// Keeps only characters that can be part of a number; `,` becomes `.`.
pub fn filter_number_chars(s: &str) -> String {
    s.chars()
        .filter_map(|c| match c {
            '0'..='9' | '.' | '-' => Some(c),
            ',' => Some('.'),
            _ => None,
        })
        .collect()
}

/// A value as shown while editing: up to three decimals, no trailing zeros.
fn edit_text(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() {
        "0".to_string()
    } else {
        s.to_string()
    }
}

impl Editor {
    /// The property field under the pointer (only when something is selected).
    pub(super) fn field_hit(&self) -> Option<usize> {
        let right = self.layout().right;
        if self.selection.is_empty() || !right.contains(self.mouse) {
            return None;
        }
        (0..Field::ALL.len()).find(|i| panels::field_rect(right, *i).contains(self.mouse))
    }

    /// Handles a press while a field is being edited. Returns true when it was consumed.
    pub(super) fn press_in_prop_edit(&mut self) -> bool {
        let Some(edit) = &self.prop_edit else {
            return false;
        };
        let current = edit.field.index();
        if self.field_hit() == Some(current) {
            let rect = panels::field_rect(self.layout().right, current);
            // The text starts `VALUE_X` into the field; the input measures from its own padding.
            let x = self.mouse.x - rect.x0 - panels::VALUE_X + PAD_X;
            let (font, shift) = (&self.assets.font, self.mods.shift);
            if let Some(edit) = self.prop_edit.as_mut() {
                edit.input.press_at(font, x, shift);
            }
            return true;
        }
        self.commit_prop_edit();
        false
    }

    pub(super) fn start_prop_edit(&mut self, i: usize) {
        let Some(field) = Field::from_index(i) else {
            return;
        };
        let text = panels::field_value(self, i)
            .map(edit_text)
            .unwrap_or_default();
        let mut input = TextInput::new(MAX_CHARS, false).with_font_size(FONT_UI);
        input.insert(&text);
        input.select_all();
        input.focused = true;
        self.prop_edit = Some(PropEdit { field, input });
    }

    /// Applies the typed value (if it is a number) and leaves the field.
    pub(super) fn commit_prop_edit(&mut self) {
        let Some(edit) = self.prop_edit.take() else {
            return;
        };
        if let Some(v) = parse_number(edit.input.text()) {
            ops::set_field(
                &mut self.session.borrow_mut().exec,
                &self.selection,
                edit.field,
                v,
            );
        }
    }

    pub(super) fn cancel_prop_edit(&mut self) {
        self.prop_edit = None;
    }

    pub(super) fn prop_text(&mut self, s: &str) {
        let filtered = filter_number_chars(s);
        if let Some(edit) = self.prop_edit.as_mut()
            && !filtered.is_empty()
        {
            edit.input.insert(&filtered);
        }
    }

    pub(super) fn prop_key(&mut self, key: Key, mods: Mods) {
        if self.prop_edit.is_none() {
            return;
        }
        match key {
            Key::Enter => self.commit_prop_edit(),
            Key::Tab => self.prop_tab(mods.shift),
            Key::Escape => self.cancel_prop_edit(),
            Key::Up => self.step_prop(if mods.shift { 10.0 } else { 1.0 }),
            Key::Down => self.step_prop(if mods.shift { -10.0 } else { -1.0 }),
            Key::Char(_) | Key::Space => {}
            other => {
                if let Some(edit) = self.prop_edit.as_mut() {
                    edit.input.key(other, mods);
                }
            }
        }
    }

    /// Tab: apply this field and move to the next (or previous with Shift).
    pub(super) fn prop_tab(&mut self, back: bool) {
        let Some(edit) = self.prop_edit.as_ref() else {
            return;
        };
        let n = Field::ALL.len();
        let i = edit.field.index();
        let next = if back { (i + n - 1) % n } else { (i + 1) % n };
        self.commit_prop_edit();
        self.start_prop_edit(next);
    }

    /// Up/Down in a field: change the value by `delta` and apply it right away.
    fn step_prop(&mut self, delta: f64) {
        let Some(edit) = self.prop_edit.as_ref() else {
            return;
        };
        let (field, i) = (edit.field, edit.field.index());
        let base = parse_number(edit.input.text()).or_else(|| panels::field_value(self, i));
        let Some(base) = base else { return };
        let value = base + delta;
        ops::set_field(
            &mut self.session.borrow_mut().exec,
            &self.selection,
            field,
            value,
        );
        let mut input = TextInput::new(MAX_CHARS, false).with_font_size(FONT_UI);
        input.insert(&edit_text(value));
        input.select_all();
        input.focused = true;
        self.prop_edit = Some(PropEdit { field, input });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_parsed_leniently_but_safely() {
        assert_eq!(parse_number("12"), Some(12.0));
        assert_eq!(parse_number(" -3.5 "), Some(-3.5));
        assert_eq!(parse_number("2,5"), Some(2.5));
        assert_eq!(parse_number(""), None);
        assert_eq!(parse_number("-"), None);
        assert_eq!(parse_number("1.2.3"), None);
        assert_eq!(parse_number("abc"), None);
        assert_eq!(parse_number("inf"), None);
        assert_eq!(parse_number("NaN"), None);
    }

    #[test]
    fn typed_characters_are_limited_to_number_characters() {
        assert_eq!(filter_number_chars("12abc.5"), "12.5");
        assert_eq!(filter_number_chars("2,5"), "2.5");
        assert_eq!(filter_number_chars("-4e3"), "-43");
        assert_eq!(filter_number_chars("текст"), "");
    }

    #[test]
    fn values_are_shown_without_noise() {
        assert_eq!(edit_text(12.0), "12");
        assert_eq!(edit_text(12.5), "12.5");
        assert_eq!(edit_text(1.0 / 3.0), "0.333");
        assert_eq!(edit_text(-0.0001), "0");
        assert_eq!(edit_text(-40.0), "-40");
    }
}
