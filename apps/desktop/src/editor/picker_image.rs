//! Image part of the color picker: the tab and the placement modes.

use dv_core::ImageMode;
use dv_ui::draw::Cx;
use dv_ui::theme::colors::*;
use dv_ui::theme::metrics::*;
use kurbo::Rect;

use super::paint_ui::value_box;
use super::picker_draw::edit_box;
use super::picker_edit::PickField;
use super::picker_layout::{
    Shape, image_box_rect, image_load_rect, image_mode_rect, image_scale_label, image_scale_rect,
};
use super::state::Editor;
use crate::i18n::{Lang, lang};

fn t(ru: &'static str, en: &'static str) -> &'static str {
    if lang() == Lang::Ru { ru } else { en }
}

fn mode_label(m: ImageMode) -> &'static str {
    match m {
        ImageMode::Fill => t("Заполнить", "Fill"),
        ImageMode::Fit => t("Вписать", "Fit"),
        ImageMode::Tile => t("Плитка", "Tile"),
        ImageMode::Crop => t("Обрезка", "Crop"),
        ImageMode::Stretch => t("Растянуть", "Stretch"),
    }
}

/// The image tab: a frame with a small landscape in it.
pub fn draw_image_tab(cx: &mut Cx, r: Rect, on: bool) {
    cx.fill(r, R_FIELD, hex(bg_app()));
    let c = r.center();
    cx.circle(c.x - 4.0, c.y - 3.0, 2.0, hex(text_secondary()));
    cx.fill(
        Rect::new(c.x - 7.0, c.y + 2.0, c.x + 7.0, c.y + 5.0),
        1.0,
        hex(text_secondary()),
    );
    let ring = if on {
        hex(accent())
    } else {
        alpha(overlay(), 0.2)
    };
    cx.stroke(r, R_FIELD, if on { 2.0 } else { 1.0 }, ring);
}

impl Editor {
    /// The image controls: a line about the image and the four placement modes.
    pub(super) fn draw_image(&self, cx: &mut Cx, popup: Rect, shape: Shape) {
        let r = image_box_rect(popup, shape);
        cx.fill(r, R_FIELD, hex(bg_app()));
        let current = self.current_image();
        let text = if current.is_some() {
            t(
                "Изображение. Перетащите файл, чтобы заменить",
                "Image. Drop a file to replace",
            )
        } else {
            t(
                "Перетащите PNG или JPEG на окно",
                "Drop a PNG or JPEG onto the window",
            )
        };
        let shown = cx.fit(text, FONT_SM, r.width() - 16.0);
        cx.text(
            &shown,
            FONT_SM,
            r.x0 + 8.0,
            r.y0 + 18.0,
            hex(text_secondary()),
        );
        let load = image_load_rect(popup, shape);
        let bg = if load.contains(self.mouse) {
            hex(bg_hover())
        } else {
            hex(bg_raised())
        };
        cx.fill(load, R_FIELD, bg);
        let label = t("Загрузить", "Upload");
        let w = cx.text_width(label, FONT_UI);
        cx.text(
            label,
            FONT_UI,
            load.center().x - w / 2.0,
            load.y0 + 18.0,
            hex(text_primary()),
        );
        let (lx, ly) = image_scale_label(popup, shape);
        cx.text(
            t("Масштаб плитки", "Tile scale"),
            FONT_UI,
            lx,
            ly,
            hex(text_secondary()),
        );
        let sr = image_scale_rect(popup, shape);
        match self
            .pick_edit
            .as_ref()
            .filter(|e| e.field == PickField::ImageScale)
        {
            Some(e) => edit_box(cx, sr, e),
            None => {
                let v = current.map_or(100.0, |i| i.scale * 100.0);
                value_box(cx, sr, &super::canvas::num(v), "%");
            }
        }
        for (k, mode) in ImageMode::ALL.into_iter().enumerate() {
            let m = image_mode_rect(popup, shape, k);
            let on = current.is_some_and(|i| i.mode == mode);
            let hot = current.is_some() && m.contains(self.mouse);
            let bg = if on {
                alpha(accent(), 0.28)
            } else if hot {
                hex(bg_hover())
            } else {
                hex(bg_app())
            };
            cx.fill(m, R_FIELD, bg);
            let fg = match (current.is_some(), on) {
                (false, _) => hex(text_disabled()),
                (true, true) => hex(text_primary()),
                (true, false) => hex(text_secondary()),
            };
            let label = cx.fit(mode_label(mode), FONT_SM, m.width() - 6.0);
            let w = cx.text_width(&label, FONT_SM);
            cx.text(&label, FONT_SM, m.center().x - w / 2.0, m.y0 + 18.0, fg);
        }
    }
}
