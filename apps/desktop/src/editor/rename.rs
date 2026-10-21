//! Renaming a layer in place (double-click on its row).

use std::time::{Duration, Instant};

use dv_core::NodeId;
use dv_ui::input::{Key, Mods};
use dv_ui::theme::metrics::FONT_UI;
use dv_ui::widgets::text_input::{PAD_X, TextInput};

use super::ops_layer;
use super::state::Editor;

/// Longest layer name.
const MAX_CHARS: usize = 64;
const DOUBLE_CLICK: Duration = Duration::from_millis(400);

/// A name being typed.
pub struct Rename {
    pub id: NodeId,
    /// Set when a page (not a layer) is being renamed; `id` is unused then.
    pub page: Option<usize>,
    pub input: TextInput,
    /// Left edge of the text on screen, to place the caret by a click.
    pub text_x: f64,
}

/// A name from typed text: trimmed, `None` when nothing is left.
pub fn clean_name(s: &str) -> Option<String> {
    let t = s.trim();
    (!t.is_empty()).then(|| t.to_string())
}

impl Editor {
    /// Remembers a click on a row; true when it is the second one on the same row.
    pub(super) fn is_row_double_click(&mut self, id: NodeId) -> bool {
        let now = Instant::now();
        let double = self
            .last_row_click
            .is_some_and(|(prev, t)| prev == id && now.duration_since(t) < DOUBLE_CLICK);
        self.last_row_click = if double { None } else { Some((id, now)) };
        double
    }

    pub(super) fn start_rename(&mut self, id: NodeId, text_x: f64) {
        let name = {
            let s = self.session.borrow();
            s.exec.doc().page().node(id).map(|n| n.name.clone())
        };
        let Some(name) = name else { return };
        let mut input = TextInput::new(MAX_CHARS, false).with_font_size(FONT_UI);
        input.insert(&name);
        input.select_all();
        input.focused = true;
        self.rename = Some(Rename {
            id,
            page: None,
            input,
            text_x,
        });
    }

    pub(super) fn commit_rename(&mut self) {
        let Some(r) = self.rename.take() else { return };
        if let Some(name) = clean_name(r.input.text()) {
            let mut s = self.session.borrow_mut();
            match r.page {
                Some(i) => {
                    let cmd = dv_core::Command::RenamePage { index: i, name };
                    let _ = s.exec.execute("Rename page", cmd);
                }
                None => ops_layer::set_name(&mut s.exec, r.id, name),
            }
        }
    }

    /// A press while renaming: inside the text it moves the caret (returns true), anywhere
    /// else it applies the name.
    pub(super) fn press_in_rename(&mut self) -> bool {
        let Some(r) = &self.rename else { return false };
        let on_row = match r.page {
            Some(i) => super::pages::row_rect(self.layout().left, i).contains(self.mouse),
            None => {
                let left = self.layers_left();
                (0..self.layer_rows(super::panels::max_layer_rows(left)).len())
                    .any(|i| super::panels::layer_row_rect(left, i).contains(self.mouse))
            }
        };
        if on_row && self.mouse.x >= r.text_x {
            let x = self.mouse.x - r.text_x + PAD_X;
            let (font, shift) = (&self.assets.font, self.mods.shift);
            if let Some(r) = self.rename.as_mut() {
                r.input.press_at(font, x, shift);
            }
            return true;
        }
        self.commit_rename();
        false
    }

    pub(super) fn rename_text(&mut self, s: &str) {
        if let Some(r) = self.rename.as_mut() {
            r.input.insert(&s.replace(['\n', '\r'], ""));
        }
    }

    pub(super) fn rename_key(&mut self, key: Key, mods: Mods) {
        match key {
            Key::Enter => self.commit_rename(),
            Key::Escape => self.rename = None,
            Key::Char(_) | Key::Space | Key::Tab => {}
            other => {
                if let Some(r) = self.rename.as_mut() {
                    r.input.key(other, mods);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_trimmed_and_never_empty() {
        assert_eq!(clean_name("  Button "), Some("Button".to_string()));
        assert_eq!(clean_name("   "), None);
        assert_eq!(clean_name(""), None);
    }
}
