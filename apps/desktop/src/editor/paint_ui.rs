//! Fill, stroke and effects sections of the right panel: what they show, drawing, hit-testing.

use dv_core::{Effect, EffectKind, GradientKind, Paint, Stroke, StrokeStyle};
use dv_ui::draw::Cx;
use dv_ui::icons::Icon;
use dv_ui::theme::colors::*;
use dv_ui::theme::metrics::*;
use kurbo::Rect;
use vello::kurbo::{Affine, RoundedRect};

use super::paint_layout::{GAP_X, LABELED_TEXT_X, MAX_EFFECTS, TEXT_X};
pub use super::paint_layout::{Hit, Look, PField, Panel, Slot, text_x};
use super::state::Editor;
use crate::i18n::{Msg, tr};

/// What the selected shapes have as fill and stroke.
pub fn looks(ed: &Editor) -> (Look<Paint>, Look<Stroke>) {
    let s = ed.session.borrow();
    let page = s.exec.doc().page();
    let nodes: Vec<_> = page
        .geo_tops(&ed.selection)
        .into_iter()
        .filter_map(|id| page.node(id))
        .collect();
    let fill = fold(nodes.iter().map(|n| n.fill.clone()));
    let stroke = fold(nodes.iter().map(|n| n.stroke));
    (fill, stroke)
}

/// The effects of the selected shapes, when they all have the same list.
pub fn effects_look(ed: &Editor) -> Look<Vec<Effect>> {
    let s = ed.session.borrow();
    let page = s.exec.doc().page();
    let mut it = page
        .geo_tops(&ed.selection)
        .into_iter()
        .filter_map(|id| page.node(id))
        .map(|n| &n.effects);
    let Some(first) = it.next() else {
        return Look::None;
    };
    if it.any(|e| e != first) {
        return Look::Mixed;
    }
    if first.is_empty() {
        Look::None
    } else {
        Look::One(first.iter().take(MAX_EFFECTS).copied().collect())
    }
}

fn fold<T: PartialEq + Clone>(mut it: impl Iterator<Item = Option<T>>) -> Look<T> {
    let Some(first) = it.next() else {
        return Look::None;
    };
    if it.any(|x| x != first) {
        return Look::Mixed;
    }
    first.map_or(Look::None, Look::One)
}

/// The panel layout for what the selection has now.
pub fn panel(ed: &Editor) -> Panel {
    let (fill, stroke) = looks(ed);
    let mask = match effects_look(ed) {
        Look::One(list) => list
            .iter()
            .enumerate()
            .filter(|(_, e)| e.kind == EffectKind::LayerBlur)
            .fold(0u32, |m, (i, _)| m | (1 << i)),
        _ => 0,
    };
    Panel::new(
        ed.layout().right,
        fill != Look::None,
        stroke != Look::None,
        mask,
    )
}

/// Name of an effect kind.
pub fn effect_name(k: EffectKind) -> &'static str {
    tr(match k {
        EffectKind::DropShadow => Msg::DropShadow,
        EffectKind::InnerShadow => Msg::InnerShadow,
        EffectKind::LayerBlur => Msg::LayerBlur,
    })
}

/// Name of a stroke style.
pub fn style_name(s: StrokeStyle) -> &'static str {
    tr(match s {
        StrokeStyle::Solid => Msg::StyleSolid,
        StrokeStyle::Dashed => Msg::StyleDashed,
        StrokeStyle::Dotted => Msg::StyleDotted,
    })
}

/// Name of a gradient type for the panel.
pub fn kind_name(k: GradientKind) -> &'static str {
    tr(match k {
        GradientKind::Linear => Msg::GradLinear,
        GradientKind::Radial => Msg::GradRadial,
        GradientKind::Angular => Msg::GradAngular,
        GradientKind::Diamond => Msg::GradDiamond,
    })
}

impl Editor {
    /// The sections' control under the pointer.
    pub(super) fn paint_hit(&self) -> Option<Hit> {
        let p = panel(self);
        if self.selection.is_empty() || !p.right.contains(self.mouse) {
            return None;
        }
        let at = |r: Rect| r.contains(self.mouse);
        let (fill, stroke) = looks(self);
        if stroke != Look::None && at(p.style_rect()) {
            return Some(Hit::Style);
        }
        if stroke != Look::None && at(p.settings_rect()) {
            return Some(Hit::StrokeSettings);
        }
        for (slot, has) in [
            (Slot::Fill, fill != Look::None),
            (Slot::Stroke, stroke != Look::None),
        ] {
            if !has {
                if at(p.add_rect(slot)) {
                    return Some(Hit::Add(slot));
                }
                continue;
            }
            if at(p.remove_rect(slot)) {
                return Some(Hit::Remove(slot));
            }
            if at(p.swatch_rect(slot)) {
                return Some(Hit::Swatch(slot));
            }
            let fields: &[PField] = if slot == Slot::Fill {
                &[PField::Hex, PField::Alpha]
            } else {
                &[PField::Hex, PField::Alpha, PField::Width]
            };
            for f in fields {
                if at(p.field_rect(slot, *f)) {
                    return Some(Hit::Field(slot, *f));
                }
            }
        }
        self.effects_hit(&p)
    }

    fn effects_hit(&self, p: &Panel) -> Option<Hit> {
        let at = |r: Rect| r.contains(self.mouse);
        let slot0 = Slot::Effect(0);
        match effects_look(self) {
            Look::None => at(p.add_rect(slot0)).then_some(Hit::AddEffect),
            Look::Mixed => at(p.remove_rect(slot0)).then_some(Hit::Remove(slot0)),
            Look::One(list) => {
                if at(p.add_rect(slot0)) && list.len() < MAX_EFFECTS {
                    return Some(Hit::AddEffect);
                }
                for (i, e) in list.iter().enumerate() {
                    let s = Slot::Effect(i);
                    if at(p.remove_rect(s)) {
                        return Some(Hit::Remove(s));
                    }
                    if at(p.type_rect(i)) {
                        return Some(Hit::EffectType(i));
                    }
                    if e.is_shadow() && at(p.swatch_rect(s)) {
                        return Some(Hit::Swatch(s));
                    }
                    let fields: &[PField] = if e.is_shadow() {
                        &[PField::ShX, PField::ShY, PField::ShBlur]
                    } else {
                        &[PField::ShBlur]
                    };
                    if let Some(f) = fields.iter().find(|f| at(p.field_rect(s, **f))) {
                        return Some(Hit::Field(s, *f));
                    }
                }
                None
            }
        }
    }
}

fn icon_button(cx: &mut Cx, r: Rect, icon: Icon, hot: bool) {
    if hot {
        cx.fill(r, R_BUTTON, hex(bg_hover()));
    }
    let c = r.center();
    cx.icon(icon, c.x - 7.0, c.y - 7.0, 14.0, hex(text_secondary()));
}

pub(super) fn value_box(cx: &mut Cx, r: Rect, text: &str, suffix: &str) {
    cx.fill(r, R_FIELD, hex(bg_raised()));
    cx.text(
        text,
        FONT_UI,
        r.x0 + TEXT_X,
        r.y0 + 18.0,
        hex(text_primary()),
    );
    if !suffix.is_empty() {
        let w = cx.text_width(suffix, FONT_SM);
        cx.text(
            suffix,
            FONT_SM,
            r.x1 - 8.0 - w,
            r.y0 + 18.0,
            hex(text_disabled()),
        );
    }
}

/// A field with a short label in front, so the value never runs into it.
fn labeled_box(cx: &mut Cx, r: Rect, label: &str, value: &str) {
    cx.fill(r, R_FIELD, hex(bg_raised()));
    cx.text(
        label,
        FONT_SM,
        r.x0 + 7.0,
        r.y0 + 18.0,
        hex(text_disabled()),
    );
    let shown = cx.fit(value, FONT_UI, (r.width() - LABELED_TEXT_X - 6.0).max(8.0));
    cx.text(
        &shown,
        FONT_UI,
        r.x0 + LABELED_TEXT_X,
        r.y0 + 18.0,
        hex(text_primary()),
    );
}

/// A button that opens a list: the current choice and a caret.
pub(super) fn drop_button(cx: &mut Cx, r: Rect, text: &str, hot: bool) {
    cx.fill(
        r,
        R_FIELD,
        if hot {
            hex(bg_hover())
        } else {
            hex(bg_raised())
        },
    );
    let shown = cx.fit(text, FONT_UI, (r.width() - 36.0).max(10.0));
    cx.text(
        &shown,
        FONT_UI,
        r.x0 + TEXT_X,
        r.y0 + 18.0,
        hex(text_primary()),
    );
    cx.icon(
        Icon::CaretDown,
        r.x1 - 20.0,
        r.y0 + 8.0,
        12.0,
        hex(text_secondary()),
    );
}

/// A rounded box filled with the paint (a color or a gradient fitted to the box).
pub fn paint_box(cx: &mut Cx, r: Rect, radius: f64, paint: &Paint) {
    let shape = RoundedRect::new(0.0, 0.0, r.width(), r.height(), radius);
    let world = cx.xf * Affine::translate((r.x0, r.y0));
    dv_render::fill_paint(cx.scene, world, &shape, r.width(), r.height(), paint);
}

fn swatch(cx: &mut Cx, r: Rect, p: &Paint) {
    paint_box(cx, r, R_FIELD, p);
    cx.stroke(r, R_FIELD, 1.0, alpha(overlay(), 0.2));
}

pub(super) fn editable(
    cx: &mut Cx,
    ed: &Editor,
    key: (Slot, PField),
    r: Rect,
    value: &str,
    suffix: &str,
) {
    match &ed.paint_edit {
        Some(e) if (e.slot, e.field) == key => {
            cx.fill(r, R_FIELD, hex(bg_raised()));
            e.input
                .draw_inline(cx, r, r.x0 + text_x(key.1), r.y0 + 18.0);
            cx.stroke(r, R_FIELD, 1.5, hex(accent()));
        }
        _ => value_box(cx, r, value, suffix),
    }
}

/// Fill or stroke: its title with the "+" button, and the row with swatch, values and "-".
fn slot(cx: &mut Cx, ed: &Editor, p: &Panel, which: Slot, color: Look<Paint>, width: Option<f64>) {
    let title = if which == Slot::Fill {
        tr(Msg::SectionFill)
    } else {
        tr(Msg::SectionStroke)
    };
    super::panels::section_title(
        cx,
        p.right.x0 + 1.0,
        p.right.x1,
        p.title_y(which),
        title,
        true,
    );
    let hot = |r: Rect| r.contains(ed.mouse);
    if color == Look::None {
        let add = p.add_rect(which);
        icon_button(cx, add, Icon::Plus, hot(add));
        return;
    }
    let remove = p.remove_rect(which);
    icon_button(cx, remove, Icon::Minimize, hot(remove));
    let hexr = p.field_rect(which, PField::Hex);
    let wide = Rect::new(hexr.x0, hexr.y0, remove.x0 - GAP_X, hexr.y1);
    match color {
        Look::One(Paint::Solid(c)) => {
            swatch(cx, p.swatch_rect(which), &Paint::Solid(c));
            let pct = format!("{:.0}", f64::from(c.a) / 255.0 * 100.0);
            editable(cx, ed, (which, PField::Hex), hexr, &c.to_hex(), "");
            let a = p.field_rect(which, PField::Alpha);
            editable(cx, ed, (which, PField::Alpha), a, &pct, "%");
            if let Some(w) = width {
                let r = p.field_rect(which, PField::Width);
                let text = super::canvas::num(w);
                editable(cx, ed, (which, PField::Width), r, &text, "px");
            }
        }
        Look::One(Paint::Gradient(g)) => {
            swatch(cx, p.swatch_rect(which), &Paint::Gradient(g.clone()));
            value_box(cx, wide, kind_name(g.kind), "");
        }
        Look::One(Paint::Pattern(_)) => {
            let sample = Paint::Pattern(dv_core::Pattern::new(dv_core::NodeId(0)));
            swatch(cx, p.swatch_rect(which), &sample);
            value_box(cx, wide, tr(Msg::PaintPattern), "");
        }
        _ => {
            cx.fill(wide, R_FIELD, hex(bg_raised()));
            let mixed = tr(Msg::Mixed);
            cx.text(
                mixed,
                FONT_UI,
                wide.x0 + TEXT_X,
                wide.y0 + 18.0,
                hex(text_secondary()),
            );
        }
    }
}

fn stroke_style_look(ed: &Editor) -> Option<StrokeStyle> {
    match looks(ed).1 {
        Look::One(s) => Some(s.style),
        _ => None,
    }
}

/// The effects section: title with "+", and two lines for each effect.
fn effects(cx: &mut Cx, ed: &Editor, p: &Panel) {
    let slot0 = Slot::Effect(0);
    let title_y = p.title_y(slot0);
    super::panels::section_title(
        cx,
        p.right.x0 + 1.0,
        p.right.x1,
        title_y,
        tr(Msg::SectionEffects),
        true,
    );
    let hot = |r: Rect| r.contains(ed.mouse);
    match effects_look(ed) {
        Look::None => {
            let add = p.add_rect(slot0);
            icon_button(cx, add, Icon::Plus, hot(add));
        }
        Look::Mixed => {
            let r = p.type_rect(0);
            cx.fill(r, R_FIELD, hex(bg_raised()));
            cx.text(
                tr(Msg::Mixed),
                FONT_UI,
                r.x0 + TEXT_X,
                r.y0 + 18.0,
                hex(text_secondary()),
            );
            let rm = p.remove_rect(slot0);
            icon_button(cx, rm, Icon::Minimize, hot(rm));
        }
        Look::One(list) => {
            let add = p.add_rect(slot0);
            icon_button(cx, add, Icon::Plus, hot(add) && list.len() < MAX_EFFECTS);
            for (i, e) in list.iter().enumerate() {
                effect_row(cx, ed, p, i, e);
            }
        }
    }
}

fn effect_row(cx: &mut Cx, ed: &Editor, p: &Panel, i: usize, e: &Effect) {
    let s = Slot::Effect(i);
    let t = p.type_rect(i);
    drop_button(cx, t, effect_name(e.kind), t.contains(ed.mouse));
    let rm = p.remove_rect(s);
    icon_button(cx, rm, Icon::Minimize, rm.contains(ed.mouse));
    if e.is_shadow() {
        swatch(cx, p.swatch_rect(s), &Paint::Solid(e.color));
        let vals = [
            (PField::ShX, "X", e.x),
            (PField::ShY, "Y", e.y),
            (PField::ShBlur, "B", e.blur),
        ];
        for (f, label, v) in vals {
            effect_field(cx, ed, p, s, f, label, v);
        }
    } else {
        effect_field(cx, ed, p, s, PField::ShBlur, "B", e.blur);
    }
}

fn effect_field(cx: &mut Cx, ed: &Editor, p: &Panel, s: Slot, f: PField, label: &str, v: f64) {
    let r = p.field_rect(s, f);
    match &ed.paint_edit {
        Some(e) if (e.slot, e.field) == (s, f) => editable(cx, ed, (s, f), r, "", ""),
        _ => labeled_box(cx, r, label, &super::canvas::num(v)),
    }
}

/// Draws the Fill, Stroke and Effects sections of the right panel.
pub fn sections(cx: &mut Cx, _right: Rect, ed: &Editor) {
    let p = panel(ed);
    let (fill, stroke) = looks(ed);
    slot(cx, ed, &p, Slot::Fill, fill, None);
    let (look, width) = match stroke {
        Look::One(s) => (Look::One(Paint::Solid(s.color)), Some(s.width)),
        Look::Mixed => (Look::Mixed, None),
        Look::None => (Look::None, None),
    };
    slot(cx, ed, &p, Slot::Stroke, look, width);
    if let Some(s) = stroke_style_look(ed) {
        let r = p.style_rect();
        drop_button(cx, r, style_name(s), r.contains(ed.mouse));
        let g = p.settings_rect();
        let bg = if ed.stroke_pop || g.contains(ed.mouse) {
            hex(bg_hover())
        } else {
            hex(bg_raised())
        };
        cx.fill(g, R_FIELD, bg);
        let c = g.center();
        cx.icon(
            Icon::Gear,
            c.x - 8.0,
            c.y - 8.0,
            16.0,
            hex(text_secondary()),
        );
    }
    effects(cx, ed, &p);
}
