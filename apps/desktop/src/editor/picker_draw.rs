//! Drawing the color picker popup.

use dv_core::{Color, Gradient, GradientKind, Paint, Tiling};
use dv_ui::draw::Cx;
use dv_ui::icons::Icon;
use dv_ui::theme::colors::*;
use dv_ui::theme::metrics::*;
use kurbo::Rect;
use vello::peniko::Color as VColor;

use super::color_fmt::components;
use super::paint_ui::paint_box;
use super::picker::Picker;
use super::picker_edit::{PickEdit, PickField};
use super::picker_grad::{field_value, tab_kind, tab_of};
use super::picker_layout::*;
use super::picker_pattern::field_value as pat_value;
use super::state::Editor;
use crate::i18n::{Lang, Msg, lang, tr};

const KNOB: f64 = 6.0;

fn vcolor(c: Color) -> VColor {
    VColor::from_rgba8(c.r, c.g, c.b, c.a)
}

fn knob(cx: &mut Cx, x: f64, y: f64, c: Color) {
    cx.circle(x, y, KNOB + 1.5, VColor::from_rgba8(255, 255, 255, 255));
    cx.circle(x, y, KNOB - 0.5, vcolor(Color { a: 255, ..c }));
}

fn value_box(cx: &mut Cx, r: Rect, text: &str, suffix: &str) {
    cx.fill(r, R_FIELD, hex(bg_app()));
    let shown = cx.fit(text, FONT_UI, (r.width() - 12.0).max(10.0));
    cx.text(
        &shown,
        FONT_UI,
        r.x0 + 6.0,
        r.y0 + 18.0,
        hex(text_primary()),
    );
    if !suffix.is_empty() {
        let w = cx.text_width(suffix, FONT_SM);
        cx.text(
            suffix,
            FONT_SM,
            r.x1 - 6.0 - w,
            r.y0 + 18.0,
            hex(text_disabled()),
        );
    }
}

fn edit_box(cx: &mut Cx, r: Rect, e: &PickEdit) {
    cx.fill(r, R_FIELD, hex(bg_app()));
    e.input.draw_inline(cx, r, r.x0 + 6.0, r.y0 + 18.0);
    cx.stroke(r, R_FIELD, 1.5, hex(accent()));
}

/// Short label of a gradient parameter.
fn label(f: GField) -> &'static str {
    let ru = lang() == Lang::Ru;
    match f {
        GField::Pos => {
            if ru {
                "Поз"
            } else {
                "Pos"
            }
        }
        GField::Angle => {
            if ru {
                "Угол"
            } else {
                "Ang"
            }
        }
        GField::X => "X",
        GField::Y => "Y",
        GField::Scale => {
            if ru {
                "Масш"
            } else {
                "Scl"
            }
        }
    }
}

fn pat_label(i: usize) -> &'static str {
    let ru = lang() == Lang::Ru;
    match i {
        0 => {
            if ru {
                "Масштаб"
            } else {
                "Scale"
            }
        }
        1 => {
            if ru {
                "Отступ X"
            } else {
                "Gap X"
            }
        }
        2 => {
            if ru {
                "Отступ Y"
            } else {
                "Gap Y"
            }
        }
        _ => {
            if ru {
                "Угол"
            } else {
                "Angle"
            }
        }
    }
}

fn suffix(f: GField) -> &'static str {
    if f == GField::Angle { "°" } else { "%" }
}

impl Editor {
    pub(super) fn draw_picker(&self, cx: &mut Cx) {
        let Some(p) = &self.picker else { return };
        let shape = self.picker_shape(p.slot);
        let popup = self.popup(p.slot);
        cx.shadow(popup, R_CARD, alpha(0x000000, 0.25), 16.0, 8.0);
        cx.fill(popup, R_CARD, hex(bg_raised()));
        cx.stroke(popup, R_CARD, 1.0, alpha(overlay(), 0.12));
        if self.pick_source && !shape.pattern {
            cx.text(
                tr(Msg::PickObject),
                FONT_UI,
                popup.x0 + 4.0,
                popup.y0 - 8.0,
                hex(accent()),
            );
        }
        let gradient = self.current_gradient();
        if shape.tabs {
            self.draw_tabs(cx, popup, shape, p);
        }
        if shape.pattern {
            self.draw_pattern(cx, popup, shape);
            return;
        }
        if let Some(g) = &gradient {
            draw_stops(cx, popup, shape, g, p.stop);
        }
        draw_surfaces(cx, popup, shape, p);
        self.draw_controls(cx, popup, shape, p.color());
        if let Some(g) = &gradient {
            self.draw_params(cx, popup, shape, g, p.stop);
        }
        self.draw_recent(cx, popup, shape, p.color());
    }

    fn draw_tabs(&self, cx: &mut Cx, popup: Rect, shape: Shape, p: &Picker) {
        let fill = self.fill_for_tabs();
        let active = tab_of(fill.as_ref());
        for i in 0..TABS {
            let r = tab_rect(popup, shape, i);
            if i == 5 {
                draw_pattern_tab(cx, r, i == active);
                continue;
            }
            let sample = match tab_kind(i) {
                None => Paint::Solid(p.color()),
                Some(k) => {
                    let mut g = Gradient::from_color(k, Color::rgb(30, 30, 34));
                    g.stops[1].color = Color::rgb(245, 245, 248);
                    Paint::Gradient(g)
                }
            };
            paint_box(cx, r, R_FIELD, &sample);
            let on = i == active;
            let ring = if on {
                hex(accent())
            } else {
                alpha(overlay(), 0.2)
            };
            cx.stroke(r, R_FIELD, if on { 2.0 } else { 1.0 }, ring);
        }
    }

    fn draw_pattern(&self, cx: &mut Cx, popup: Rect, shape: Shape) {
        let Some(pat) = self.current_pattern() else {
            return;
        };
        let r = pat_source_rect(popup, shape);
        let active = self.pick_source;
        cx.fill(
            r,
            R_FIELD,
            if active {
                alpha(accent(), 0.3)
            } else {
                hex(bg_app())
            },
        );
        let name = if active {
            tr(Msg::PickObject).to_string()
        } else {
            let s = self.session.borrow();
            s.exec
                .doc()
                .page()
                .node(pat.source)
                .map_or(String::new(), |n| n.name.clone())
        };
        let shown = cx.fit(&name, FONT_UI, r.width() - 16.0);
        cx.text(
            &shown,
            FONT_UI,
            r.x0 + 8.0,
            r.y0 + 18.0,
            hex(text_primary()),
        );
        let t = pat_tiling_rect(popup, shape);
        cx.fill(t, R_FIELD, hex(bg_app()));
        let label = match pat.tiling {
            Tiling::Grid => tr(Msg::TilingGrid),
            Tiling::BrickRows => tr(Msg::TilingRows),
            Tiling::BrickColumns => tr(Msg::TilingColumns),
        };
        cx.text(label, FONT_UI, t.x0 + 8.0, t.y0 + 18.0, hex(text_primary()));
        for (i, f) in PAT_FIELDS.iter().enumerate() {
            let fr = pat_field_rect(popup, shape, i);
            let (name, suf) = match f {
                PatField::Scale => (pat_label(0), "%"),
                PatField::GapX => (pat_label(1), "px"),
                PatField::GapY => (pat_label(2), "px"),
                PatField::Angle => (pat_label(3), "°"),
            };
            match self
                .pick_edit
                .as_ref()
                .filter(|e| e.field == PickField::Pat(*f))
            {
                Some(e) => edit_box(cx, fr, e),
                None => {
                    value_box(cx, fr, &super::canvas::num(pat_value(&pat, *f)), suf);
                    cx.text(
                        name,
                        FONT_SM,
                        fr.x0 + 6.0,
                        fr.y0 - 3.0,
                        hex(text_disabled()),
                    );
                }
            }
        }
    }

    fn draw_controls(&self, cx: &mut Cx, popup: Rect, shape: Shape, color: Color) {
        let mode = self.picker_mode;
        let eye = eyedrop_rect(popup, shape);
        let bg = if self.eyedrop.is_some() {
            alpha(accent(), 0.3)
        } else {
            hex(bg_app())
        };
        cx.fill(eye, R_FIELD, bg);
        let c = eye.center();
        cx.icon(
            Icon::Eyedropper,
            c.x - 8.0,
            c.y - 8.0,
            16.0,
            hex(text_primary()),
        );

        let m = mode_rect(popup, shape);
        cx.fill(m, R_FIELD, hex(bg_app()));
        cx.text(
            mode.label(),
            FONT_UI,
            m.x0 + 8.0,
            m.y0 + 18.0,
            hex(text_primary()),
        );
        cx.icon(
            Icon::CaretDown,
            m.x1 - 18.0,
            m.y0 + 8.0,
            12.0,
            hex(text_secondary()),
        );

        for (i, t) in components(mode, color).iter().enumerate() {
            let r = field_rect(popup, shape, mode, i);
            match self
                .pick_edit
                .as_ref()
                .filter(|e| e.field == PickField::Comp(i))
            {
                Some(e) => edit_box(cx, r, e),
                None => value_box(cx, r, t, ""),
            }
        }
        if mode.has_alpha_field() {
            let r = alpha_field_rect(popup, shape, mode);
            let pct = format!("{:.0}", f64::from(color.a) / 255.0 * 100.0);
            match self
                .pick_edit
                .as_ref()
                .filter(|e| e.field == PickField::Alpha)
            {
                Some(e) => edit_box(cx, r, e),
                None => value_box(cx, r, &pct, "%"),
            }
        }
    }

    fn draw_params(&self, cx: &mut Cx, popup: Rect, shape: Shape, g: &Gradient, stop: usize) {
        for (i, f) in grad_fields(g.kind).iter().enumerate() {
            let r = gfield_rect(popup, shape, i);
            match self
                .pick_edit
                .as_ref()
                .filter(|e| e.field == PickField::Grad(*f))
            {
                Some(e) => edit_box(cx, r, e),
                None => {
                    value_box(
                        cx,
                        r,
                        &super::canvas::num(field_value(g, stop, *f)),
                        suffix(*f),
                    );
                    cx.text(
                        label(*f),
                        FONT_SM,
                        r.x0 + 6.0,
                        r.y0 - 3.0,
                        hex(text_disabled()),
                    );
                }
            }
        }
        let rm = remove_stop_rect(popup, shape);
        cx.fill(rm, R_FIELD, hex(bg_app()));
        let c = rm.center();
        cx.icon(
            Icon::Minimize,
            c.x - 7.0,
            c.y - 7.0,
            14.0,
            hex(text_secondary()),
        );
    }

    fn draw_recent(&self, cx: &mut Cx, popup: Rect, shape: Shape, color: Color) {
        for (i, c) in super::recent::get().iter().enumerate() {
            let r = recent_rect(popup, shape, i);
            cx.fill(r, R_FIELD, vcolor(*c));
            let on = *c == color;
            let ring = if on {
                hex(accent())
            } else {
                alpha(overlay(), 0.2)
            };
            cx.stroke(r, R_FIELD, if on { 2.0 } else { 1.0 }, ring);
        }
    }
}

/// The pattern tab: a small grid of dots.
fn draw_pattern_tab(cx: &mut Cx, r: Rect, on: bool) {
    cx.fill(r, R_FIELD, hex(bg_app()));
    for row in 0..2 {
        for col in 0..4 {
            let x = r.x0 + r.width() * (col as f64 + 0.5) / 4.0;
            let y = r.y0 + r.height() * (row as f64 + 0.5) / 2.0;
            cx.circle(x, y, 2.0, hex(text_secondary()));
        }
    }
    let ring = if on {
        hex(accent())
    } else {
        alpha(overlay(), 0.2)
    };
    cx.stroke(r, R_FIELD, if on { 2.0 } else { 1.0 }, ring);
}

/// The gradient line with a handle for each stop.
fn draw_stops(cx: &mut Cx, popup: Rect, shape: Shape, g: &Gradient, selected: usize) {
    let bar = stops_bar(popup, shape);
    let mut line = Gradient::from_color(GradientKind::Linear, Color::rgb(0, 0, 0));
    line.stops = g.stops.clone();
    cx.fill(bar, 4.0, hex(bg_app()));
    paint_box(cx, bar, 4.0, &Paint::Gradient(line));
    cx.stroke(bar, 4.0, 1.0, alpha(overlay(), 0.2));
    for (i, s) in g.stops.iter().enumerate() {
        let c = handle_center(popup, shape, s.pos);
        let on = i == selected;
        let ring = if on {
            hex(accent())
        } else {
            VColor::from_rgba8(255, 255, 255, 255)
        };
        cx.circle(c.x, c.y, HANDLE_R + 1.5, ring);
        cx.circle(
            c.x,
            c.y,
            HANDLE_R - 1.0,
            vcolor(Color { a: 255, ..s.color }),
        );
    }
}

fn draw_surfaces(cx: &mut Cx, popup: Rect, shape: Shape, p: &Picker) {
    let pure_c = Color::from_hsv(p.h, 1.0, 1.0, 255);
    let sv = part_rect(popup, shape, Part::Sv);
    let white = VColor::from_rgba8(255, 255, 255, 255);
    cx.gradient(sv, 4.0, false, &[(0.0, white), (1.0, vcolor(pure_c))]);
    let clear = VColor::from_rgba8(0, 0, 0, 0);
    let black = VColor::from_rgba8(0, 0, 0, 255);
    cx.gradient(sv, 4.0, true, &[(0.0, clear), (1.0, black)]);
    let hue = part_rect(popup, shape, Part::Hue);
    let stops: Vec<(f32, VColor)> = (0..=6)
        .map(|i| {
            (
                i as f32 / 6.0,
                vcolor(Color::from_hsv(f64::from(i) * 60.0, 1.0, 1.0, 255)),
            )
        })
        .collect();
    cx.gradient(hue, BAR_H / 2.0, false, &stops);
    let bar = part_rect(popup, shape, Part::Alpha);
    let solid = Color::from_hsv(p.h, p.s, p.v, 255);
    cx.fill(bar, BAR_H / 2.0, hex(bg_app()));
    let transparent = VColor::from_rgba8(solid.r, solid.g, solid.b, 0);
    cx.gradient(
        bar,
        BAR_H / 2.0,
        false,
        &[(0.0, transparent), (1.0, vcolor(solid))],
    );
    knob(
        cx,
        sv.x0 + p.s * sv.width(),
        sv.y0 + (1.0 - p.v) * sv.height(),
        p.color(),
    );
    knob(
        cx,
        hue.x0 + p.h / 359.99 * hue.width(),
        hue.center().y,
        pure_c,
    );
    knob(
        cx,
        bar.x0 + f64::from(p.a) / 255.0 * bar.width(),
        bar.center().y,
        solid,
    );
}
