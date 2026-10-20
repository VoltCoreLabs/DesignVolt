//! Drawing the color picker popup.

use dv_core::Color;
use dv_ui::draw::Cx;
use dv_ui::icons::Icon;
use dv_ui::theme::colors::*;
use dv_ui::theme::metrics::*;
use kurbo::Rect;
use vello::peniko::Color as VColor;

use super::color_fmt::components;
use super::picker_edit::PickField;
use super::picker_layout::*;
use super::state::Editor;

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

impl Editor {
    pub(super) fn draw_picker(&self, cx: &mut Cx) {
        let Some(p) = &self.picker else { return };
        let recent = super::recent::get();
        let popup = self.popup(p.slot);
        cx.shadow(popup, R_CARD, alpha(0x000000, 0.25), 16.0, 8.0);
        cx.fill(popup, R_CARD, hex(bg_raised()));
        cx.stroke(popup, R_CARD, 1.0, alpha(overlay(), 0.12));
        draw_surfaces(cx, popup, p);
        self.draw_controls(cx, popup, p.color(), &recent);
    }

    fn draw_controls(&self, cx: &mut Cx, popup: Rect, color: Color, recent: &[Color]) {
        let mode = self.picker_mode;
        let eye = eyedrop_rect(popup);
        let active = self.eyedrop.is_some();
        let bg = if active {
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

        let m = mode_rect(popup);
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

        let texts = components(mode, color);
        for (i, t) in texts.iter().enumerate() {
            let r = field_rect(popup, mode, i);
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
            let r = alpha_field_rect(popup, mode);
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
        for (i, c) in recent.iter().enumerate() {
            let r = recent_rect(popup, i);
            cx.fill(r, R_FIELD, vcolor(*c));
            let ring = if *c == color {
                hex(accent())
            } else {
                alpha(overlay(), 0.2)
            };
            cx.stroke(r, R_FIELD, if *c == color { 2.0 } else { 1.0 }, ring);
        }
    }
}

fn edit_box(cx: &mut Cx, r: Rect, e: &super::picker_edit::PickEdit) {
    cx.fill(r, R_FIELD, hex(bg_app()));
    e.input.draw_inline(cx, r, r.x0 + 6.0, r.y0 + 18.0);
    cx.stroke(r, R_FIELD, 1.5, hex(accent()));
}

fn draw_surfaces(cx: &mut Cx, popup: Rect, p: &super::picker::Picker) {
    let pure_c = Color::from_hsv(p.h, 1.0, 1.0, 255);
    let sv = part_rect(popup, Part::Sv);
    let white = VColor::from_rgba8(255, 255, 255, 255);
    cx.gradient(sv, 4.0, false, &[(0.0, white), (1.0, vcolor(pure_c))]);
    let clear = VColor::from_rgba8(0, 0, 0, 0);
    let black = VColor::from_rgba8(0, 0, 0, 255);
    cx.gradient(sv, 4.0, true, &[(0.0, clear), (1.0, black)]);
    let hue = part_rect(popup, Part::Hue);
    let stops: Vec<(f32, VColor)> = (0..=6)
        .map(|i| {
            (
                i as f32 / 6.0,
                vcolor(Color::from_hsv(f64::from(i) * 60.0, 1.0, 1.0, 255)),
            )
        })
        .collect();
    cx.gradient(hue, BAR_H / 2.0, false, &stops);
    let bar = part_rect(popup, Part::Alpha);
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
