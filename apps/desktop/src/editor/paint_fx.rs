//! Editing effects from the right panel: adding, removing and changing them, and the stroke style.

use dv_core::{Effect, EffectKind, Stroke, StrokeStyle};
use dv_ui::theme::metrics::FONT_UI;
use dv_ui::widgets::text_input::TextInput;

use super::ops_paint::{edit_effects, edit_stroke};
use super::paint_edit::PaintEdit;
use super::paint_ui::{Look, PField, Slot, effects_look};
use super::props::{edit_text, parse_number};
use super::state::Editor;

/// Largest blur or offset the fields accept.
const MAX_FX: f64 = 1000.0;

/// The shown value of an effect field.
pub fn effect_value(e: &Effect, f: PField) -> Option<f64> {
    match f {
        PField::ShX => Some(e.x),
        PField::ShY => Some(e.y),
        PField::ShBlur => Some(e.blur),
        _ => None,
    }
}

/// The effect with one field set (offsets and blur are kept in a sane range).
pub fn with_effect_value(e: Effect, f: PField, v: f64) -> Effect {
    let v = v.clamp(-MAX_FX, MAX_FX);
    match f {
        PField::ShX => Effect { x: v, ..e },
        PField::ShY => Effect { y: v, ..e },
        PField::ShBlur => Effect {
            blur: v.max(0.0),
            ..e
        },
        _ => e,
    }
}

/// The effect turned into another kind; a blur has no offset, a shadow gets the default one.
pub fn with_kind(e: Effect, kind: EffectKind) -> Effect {
    let base = Effect::new(kind);
    match kind {
        EffectKind::LayerBlur => Effect {
            kind,
            x: 0.0,
            y: 0.0,
            ..e
        },
        _ if e.kind == EffectKind::LayerBlur => Effect {
            blur: e.blur.max(base.blur),
            ..base
        },
        _ => Effect { kind, ..e },
    }
}

impl Editor {
    pub(super) fn set_stroke_style(&mut self, style: StrokeStyle) {
        let ex = &mut self.session.borrow_mut().exec;
        edit_stroke(ex, &self.selection, None, |s| {
            s.map(|s| Stroke { style, ..s })
        });
    }

    pub(super) fn add_effect(&mut self, kind: EffectKind) {
        let ex = &mut self.session.borrow_mut().exec;
        edit_effects(ex, &self.selection, None, |l| l.push(Effect::new(kind)));
    }

    /// Removes effect `i`; when the shapes have different effects, clears them all.
    pub(super) fn remove_effect(&mut self, i: usize) {
        let all = effects_look(self) == Look::Mixed;
        self.close_picker();
        let ex = &mut self.session.borrow_mut().exec;
        edit_effects(ex, &self.selection, None, |l| {
            if all {
                l.clear();
            } else if i < l.len() {
                l.remove(i);
            }
        });
    }

    pub(super) fn set_effect_kind(&mut self, i: usize, kind: EffectKind) {
        let ex = &mut self.session.borrow_mut().exec;
        edit_effects(ex, &self.selection, None, |l| {
            if let Some(e) = l.get_mut(i) {
                *e = with_kind(*e, kind);
            }
        });
    }

    pub(super) fn start_effect_edit(&mut self, i: usize, field: PField) {
        let Look::One(list) = effects_look(self) else {
            return;
        };
        let Some(v) = list.get(i).and_then(|e| effect_value(e, field)) else {
            return;
        };
        let mut input = TextInput::new(7, false).with_font_size(FONT_UI);
        input.insert(&edit_text(v));
        input.select_all();
        input.focused = true;
        self.close_picker();
        self.paint_edit = Some(PaintEdit {
            slot: Slot::Effect(i),
            field,
            input,
        });
    }

    pub(super) fn commit_effect_edit(&mut self, i: usize, field: PField, text: &str) {
        let Some(v) = parse_number(text) else { return };
        let ex = &mut self.session.borrow_mut().exec;
        edit_effects(ex, &self.selection, None, |l| {
            if let Some(e) = l.get_mut(i) {
                *e = with_effect_value(*e, field, v);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_fields_read_and_clamp() {
        let e = Effect::new(EffectKind::DropShadow);
        assert_eq!(effect_value(&e, PField::ShY), Some(4.0));
        assert_eq!(effect_value(&e, PField::Hex), None);
        assert_eq!(with_effect_value(e, PField::ShBlur, -5.0).blur, 0.0);
        assert_eq!(with_effect_value(e, PField::ShX, -2e6).x, -MAX_FX);
        assert_eq!(with_effect_value(e, PField::ShX, 7.0).x, 7.0);
    }

    #[test]
    fn changing_the_kind_keeps_what_still_applies() {
        let mut e = Effect::new(EffectKind::DropShadow);
        e.blur = 30.0;
        let b = with_kind(e, EffectKind::LayerBlur);
        assert_eq!(
            (b.kind, b.blur, b.x, b.y),
            (EffectKind::LayerBlur, 30.0, 0.0, 0.0)
        );
        let back = with_kind(b, EffectKind::InnerShadow);
        assert_eq!((back.kind, back.blur), (EffectKind::InnerShadow, 30.0));
        assert_eq!(back.y, Effect::new(EffectKind::InnerShadow).y);
        let inner = with_kind(e, EffectKind::InnerShadow);
        assert_eq!(
            (inner.kind, inner.blur, inner.y),
            (EffectKind::InnerShadow, 30.0, 4.0)
        );
    }
}
