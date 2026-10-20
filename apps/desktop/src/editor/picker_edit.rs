//! Typing into the value fields of the color picker.

use dv_ui::input::{Key, Mods};
use dv_ui::theme::metrics::FONT_UI;
use dv_ui::widgets::text_input::{PAD_X, TextInput};

use super::color_fmt::{Mode, components, with_component};
use super::paint_edit::percent_to_alpha;
use super::picker_grad::{field_value, set_field};
use super::picker_layout::{GField, Shape, alpha_field_rect, field_rect, gfield_rect, grad_fields};
use super::props::parse_number;
use super::state::Editor;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PickField {
    Comp(usize),
    Alpha,
    Grad(GField),
}

pub struct PickEdit {
    pub field: PickField,
    pub input: TextInput,
}

/// Keeps what a picker field can hold in `mode`.
pub fn filter(mode: Mode, field: PickField, s: &str) -> String {
    match (field, mode) {
        (PickField::Grad(_), _) => s
            .chars()
            .filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
            .collect(),
        (PickField::Alpha, _) => s.chars().filter(char::is_ascii_digit).collect(),
        (_, Mode::Hex) => s.chars().filter(char::is_ascii_hexdigit).collect(),
        (_, Mode::Css) => s.chars().filter(|c| !c.is_control()).collect(),
        _ => s
            .chars()
            .filter(|c| c.is_ascii_digit() || *c == '.')
            .collect(),
    }
}

fn max_chars(mode: Mode, field: PickField) -> usize {
    match (field, mode) {
        (PickField::Grad(_), _) => 7,
        (PickField::Alpha, _) => 3,
        (_, Mode::Hex) => 6,
        (_, Mode::Css) => 32,
        _ => 5,
    }
}

impl Editor {
    pub(super) fn start_pick_edit(&mut self, field: PickField) {
        let Some(color) = self.picker.as_ref().map(|p| p.color()) else {
            return;
        };
        let mode = self.picker_mode;
        let text = match field {
            PickField::Comp(i) => components(mode, color).get(i).cloned().unwrap_or_default(),
            PickField::Alpha => format!("{:.0}", f64::from(color.a) / 255.0 * 100.0),
            PickField::Grad(f) => {
                let stop = self.picker.as_ref().map_or(0, |p| p.stop);
                let v = self
                    .current_gradient()
                    .map_or(0.0, |g| field_value(&g, stop, f));
                super::props::edit_text(v)
            }
        };
        let mut input = TextInput::new(max_chars(mode, field), false).with_font_size(FONT_UI);
        input.insert(&text);
        input.select_all();
        input.focused = true;
        self.pick_edit = Some(PickEdit { field, input });
    }

    pub(super) fn commit_pick_edit(&mut self) {
        let Some(e) = self.pick_edit.take() else {
            return;
        };
        let Some(old) = self.picker.as_ref().map(|p| p.color()) else {
            return;
        };
        let text = e.input.text().to_string();
        if let PickField::Grad(f) = e.field {
            if let Some(v) = parse_number(&text) {
                let stop = self.picker.as_ref().map_or(0, |p| p.stop);
                self.edit_gradient(None, |g| set_field(g, stop, f, v));
                self.select_stop(stop);
            }
            return;
        }
        let new = match e.field {
            PickField::Grad(_) => None,
            PickField::Alpha => parse_number(&text).map(|v| dv_core::Color {
                a: percent_to_alpha(v),
                ..old
            }),
            PickField::Comp(i) => with_component(self.picker_mode, old, i, &text),
        };
        if let Some(c) = new {
            self.set_picker_color(c);
        }
    }

    /// A press while a picker field is typed into: inside it moves the caret (true),
    /// elsewhere it applies the value.
    pub(super) fn press_in_pick_edit(&mut self, popup: kurbo::Rect, shape: Shape) -> bool {
        let Some(e) = &self.pick_edit else {
            return false;
        };
        let mode = self.picker_mode;
        let rect = match e.field {
            PickField::Comp(i) => field_rect(popup, shape, mode, i),
            PickField::Alpha => alpha_field_rect(popup, shape, mode),
            PickField::Grad(f) => {
                let k = shape.grad.map(grad_fields).unwrap_or(&[]);
                let i = k.iter().position(|x| *x == f).unwrap_or(0);
                gfield_rect(popup, shape, i)
            }
        };
        if rect.contains(self.mouse) {
            let x = self.mouse.x - rect.x0 - 6.0 + PAD_X;
            let (font, shift) = (&self.assets.font, self.mods.shift);
            if let Some(e) = self.pick_edit.as_mut() {
                e.input.press_at(font, x, shift);
            }
            return true;
        }
        self.commit_pick_edit();
        false
    }

    pub(super) fn pick_text(&mut self, s: &str) {
        let mode = self.picker_mode;
        if let Some(e) = self.pick_edit.as_mut() {
            let t = filter(mode, e.field, s);
            if !t.is_empty() {
                e.input.insert(&t);
            }
        }
    }

    pub(super) fn pick_key(&mut self, key: Key, mods: Mods) {
        match key {
            Key::Enter => self.commit_pick_edit(),
            Key::Escape => self.pick_edit = None,
            Key::Char(_) | Key::Space | Key::Tab => {}
            other => {
                if let Some(e) = self.pick_edit.as_mut() {
                    e.input.key(other, mods);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_mode_filters_its_own_characters() {
        assert_eq!(filter(Mode::Hex, PickField::Comp(0), "#ZZ1f"), "1f");
        assert_eq!(filter(Mode::Rgb, PickField::Comp(0), "1a2.5"), "12.5");
        assert_eq!(
            filter(Mode::Css, PickField::Comp(0), "rgb(1, 2)\n"),
            "rgb(1, 2)"
        );
        assert_eq!(filter(Mode::Hsl, PickField::Alpha, "5%0"), "50");
    }
}
