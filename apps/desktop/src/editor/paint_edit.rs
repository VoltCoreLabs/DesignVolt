//! Typing into the fill and stroke fields (hex, opacity, stroke width) and the add/remove buttons.

use dv_core::{Color, Paint, Stroke};
use dv_ui::input::{Key, Mods};
use dv_ui::theme::metrics::FONT_UI;
use dv_ui::widgets::text_input::{PAD_X, TextInput};

use super::dropdown::Target;
use super::ops_paint::{DEFAULT_FILL, DEFAULT_STROKE, edit_fill, edit_stroke};
use super::paint_ui::{Hit, Look, PField, Slot, looks, panel, text_x};
use super::props::{filter_number_chars, parse_number};
use super::state::Editor;

/// Widest stroke the width field accepts.
const MAX_WIDTH: f64 = 1000.0;

/// A field being typed into.
pub struct PaintEdit {
    pub slot: Slot,
    pub field: PField,
    pub input: TextInput,
}

/// Keeps what can be typed into a field: hex digits, whole percents, or a number.
pub fn filter(field: PField, s: &str) -> String {
    match field {
        PField::Hex => s.chars().filter(char::is_ascii_hexdigit).collect(),
        PField::ShX | PField::ShY => filter_number_chars(s),
        PField::Alpha => s.chars().filter(char::is_ascii_digit).collect(),
        PField::Width | PField::ShBlur => filter_number_chars(s).replace('-', ""),
    }
}

fn max_chars(field: PField) -> usize {
    match field {
        PField::Hex => 6,
        PField::Alpha => 3,
        PField::Width | PField::ShX | PField::ShY | PField::ShBlur => 7,
    }
}

/// Opacity percent (0..=100) as an alpha byte.
pub fn percent_to_alpha(p: f64) -> u8 {
    (p.clamp(0.0, 100.0) / 100.0 * 255.0).round() as u8
}

impl Editor {
    pub(super) fn paint_click(&mut self, hit: Hit) {
        match hit {
            Hit::Add(slot) => self.add_paint(slot),
            Hit::Remove(slot) => self.remove_paint(slot),
            Hit::Swatch(slot) => self.toggle_picker(slot),
            Hit::Field(slot, field) => self.start_paint_edit(slot, field),
            Hit::Style => self.open_dropdown_for(Target::StrokeStyle),
            Hit::AddEffect => self.open_dropdown_for(Target::AddEffect),
            Hit::EffectType(i) => self.open_dropdown_for(Target::EffectType(i)),
        }
    }

    fn add_paint(&mut self, slot: Slot) {
        let ex = &mut self.session.borrow_mut().exec;
        match slot {
            Slot::Fill => edit_fill(ex, &self.selection, None, |_| Some(DEFAULT_FILL.into())),
            Slot::Stroke => edit_stroke(ex, &self.selection, None, |_| Some(DEFAULT_STROKE)),
            Slot::Effect(_) => {}
        }
    }

    fn remove_paint(&mut self, slot: Slot) {
        if let Slot::Effect(i) = slot {
            self.remove_effect(i);
            return;
        }
        self.close_picker();
        let ex = &mut self.session.borrow_mut().exec;
        match slot {
            Slot::Fill => edit_fill(ex, &self.selection, None, |_| None),
            Slot::Stroke => edit_stroke(ex, &self.selection, None, |_| None),
            Slot::Effect(_) => {}
        }
    }

    fn start_paint_edit(&mut self, slot: Slot, field: PField) {
        if let Slot::Effect(i) = slot {
            self.start_effect_edit(i, field);
            return;
        }
        let (fill, stroke) = looks(self);
        let (color, width) = match (slot, fill, stroke) {
            (Slot::Fill, Look::One(Paint::Solid(c)), _) => (c, 0.0),
            (Slot::Stroke, _, Look::One(s)) => (s.color, s.width),
            _ => return,
        };
        let text = match field {
            PField::Hex => color.to_hex(),
            PField::Alpha => format!("{:.0}", f64::from(color.a) / 255.0 * 100.0),
            PField::Width => super::props::edit_text(width),
            _ => return,
        };
        let mut input = TextInput::new(max_chars(field), false).with_font_size(FONT_UI);
        input.insert(&text);
        input.select_all();
        input.focused = true;
        self.close_picker();
        self.paint_edit = Some(PaintEdit { slot, field, input });
    }

    /// Applies what was typed (when it is valid) and leaves the field.
    pub(super) fn commit_paint_edit(&mut self) {
        let Some(e) = self.paint_edit.take() else {
            return;
        };
        let text = e.input.text().to_string();
        if let Slot::Effect(i) = e.slot {
            self.commit_effect_edit(i, e.field, &text);
            return;
        }
        let ex = &mut self.session.borrow_mut().exec;
        let sel = &self.selection;
        match (e.slot, e.field) {
            (slot, PField::Hex) => {
                let Some(new) = Color::from_hex(&text) else {
                    return;
                };
                let with = move |c: Option<Color>| Color {
                    a: c.map_or(255, |c| c.a),
                    ..new
                };
                match slot {
                    Slot::Fill => edit_fill(ex, sel, None, |p| {
                        Some(Paint::Solid(with(p.as_ref().and_then(Paint::solid))))
                    }),
                    Slot::Stroke => edit_stroke(ex, sel, None, |s| {
                        let base = s.unwrap_or(DEFAULT_STROKE);
                        Some(Stroke {
                            color: with(s.map(|s| s.color)),
                            ..base
                        })
                    }),
                    Slot::Effect(_) => {}
                }
            }
            (slot, PField::Alpha) => {
                let Some(p) = parse_number(&text) else { return };
                let a = percent_to_alpha(p);
                match slot {
                    Slot::Fill => edit_fill(ex, sel, None, |p| {
                        p.map(|p| match p {
                            Paint::Solid(c) => Paint::Solid(Color { a, ..c }),
                            other => other,
                        })
                    }),
                    Slot::Stroke => edit_stroke(ex, sel, None, |s| {
                        s.map(|s| Stroke {
                            color: Color { a, ..s.color },
                            ..s
                        })
                    }),
                    Slot::Effect(_) => {}
                }
            }
            (_, PField::ShX | PField::ShY | PField::ShBlur) => {}
            (_, PField::Width) => {
                let Some(w) = parse_number(&text) else { return };
                let width = w.clamp(0.0, MAX_WIDTH);
                edit_stroke(ex, sel, None, |s| s.map(|s| Stroke { width, ..s }));
            }
        }
    }

    /// A press while a paint field is being edited: inside it moves the caret (true),
    /// elsewhere it applies the value.
    pub(super) fn press_in_paint_edit(&mut self) -> bool {
        let Some(e) = &self.paint_edit else {
            return false;
        };
        let rect = panel(self).field_rect(e.slot, e.field);
        if rect.contains(self.mouse) {
            let x = self.mouse.x - rect.x0 - text_x(e.field) + PAD_X;
            let (font, shift) = (&self.assets.font, self.mods.shift);
            if let Some(e) = self.paint_edit.as_mut() {
                e.input.press_at(font, x, shift);
            }
            return true;
        }
        self.commit_paint_edit();
        false
    }

    pub(super) fn paint_text(&mut self, s: &str) {
        if let Some(e) = self.paint_edit.as_mut() {
            let t = filter(e.field, s);
            if !t.is_empty() {
                e.input.insert(&t);
            }
        }
    }

    pub(super) fn paint_key(&mut self, key: Key, mods: Mods) {
        match key {
            Key::Enter => self.commit_paint_edit(),
            Key::Escape => self.paint_edit = None,
            Key::Char(_) | Key::Space | Key::Tab => {}
            other => {
                if let Some(e) = self.paint_edit.as_mut() {
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
    fn fields_only_take_their_own_characters() {
        assert_eq!(filter(PField::Hex, "#ff8Zz0"), "ff80");
        assert_eq!(filter(PField::Alpha, "5a0%"), "50");
        assert_eq!(filter(PField::Width, "-2,5x"), "2.5");
    }

    #[test]
    fn percent_maps_to_the_full_byte_range() {
        assert_eq!(percent_to_alpha(0.0), 0);
        assert_eq!(percent_to_alpha(100.0), 255);
        assert_eq!(percent_to_alpha(50.0), 128);
        assert_eq!(percent_to_alpha(250.0), 255);
        assert_eq!(percent_to_alpha(-5.0), 0);
    }
}
