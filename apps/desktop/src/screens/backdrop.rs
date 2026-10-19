//! Shared plain background of the full-page screens.

use vello::kurbo::Rect;

use crate::input::Viewport;
use dv_ui::draw::Cx;
use dv_ui::theme::colors::*;

pub fn draw(cx: &mut Cx, vp: Viewport) {
    cx.fill(Rect::new(0.0, 0.0, vp.w, vp.h), 0.0, hex(bg_app()));
}
