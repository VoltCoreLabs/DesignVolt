//! Editing the effects of the right panel: stroke style and the drop shadow fields.

use dv_core::{Shadow, Stroke};
use dv_ui::theme::metrics::FONT_UI;
use dv_ui::widgets::text_input::TextInput;

use super::ops_paint::{edit_shadow, edit_stroke};
use super::paint_edit::PaintEdit;
use super::paint_ui::{Look, PField, Slot, shadow_look};
use super::props::{edit_text, parse_number};
use super::state::Editor;

/// Largest blur or offset the fields accept.
const MAX_FX: f64 = 1000.0;

/// The shown value of a shadow field.
pub fn shadow_value(s: &Shadow, f: PField) -> Option<f64> {
    match f {
        PField::ShX => Some(s.x),
        PField::ShY => Some(s.y),
        PField::ShBlur => Some(s.blur),
        _ => None,
    }
}

/// The shadow with one field set (offsets and blur are kept in a sane range).
pub fn with_shadow_value(s: Shadow, f: PField, v: f64) -> Shadow {
    let v = v.clamp(-MAX_FX, MAX_FX);
    match f {
        PField::ShX => Shadow { x: v, ..s },
        PField::ShY => Shadow { y: v, ..s },
        PField::ShBlur => Shadow {
            blur: v.max(0.0),
            ..s
        },
        _ => s,
    }
}

impl Editor {
    pub(super) fn cycle_stroke_style(&mut self) {
        let ex = &mut self.session.borrow_mut().exec;
        edit_stroke(ex, &self.selection, None, |s| {
            s.map(|s| Stroke {
                style: s.style.next(),
                ..s
            })
        });
    }

    pub(super) fn start_shadow_edit(&mut self, field: PField) {
        let Look::One(s) = shadow_look(self) else {
            return;
        };
        let Some(v) = shadow_value(&s, field) else {
            return;
        };
        let mut input = TextInput::new(7, false).with_font_size(FONT_UI);
        input.insert(&edit_text(v));
        input.select_all();
        input.focused = true;
        self.close_picker();
        self.paint_edit = Some(PaintEdit {
            slot: Slot::Shadow,
            field,
            input,
        });
    }

    pub(super) fn commit_shadow_edit(&mut self, field: PField, text: &str) {
        let Some(v) = parse_number(text) else { return };
        let ex = &mut self.session.borrow_mut().exec;
        edit_shadow(ex, &self.selection, None, |s| {
            s.map(|s| with_shadow_value(s, field, v))
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shadow_fields_read_and_clamp() {
        let s = Shadow::DEFAULT;
        assert_eq!(shadow_value(&s, PField::ShY), Some(4.0));
        assert_eq!(shadow_value(&s, PField::Hex), None);
        assert_eq!(with_shadow_value(s, PField::ShBlur, -5.0).blur, 0.0);
        assert_eq!(with_shadow_value(s, PField::ShX, -2e6).x, -MAX_FX);
        assert_eq!(with_shadow_value(s, PField::ShX, 7.0).x, 7.0);
    }
}
