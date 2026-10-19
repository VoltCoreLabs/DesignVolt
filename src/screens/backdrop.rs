//! Shared decorative background: dark gradient with two soft accent glows.

use vello::kurbo::Rect;

use crate::draw::Cx;
use crate::input::Viewport;
use crate::theme::colors::*;

pub fn draw(cx: &mut Cx, vp: Viewport) {
    let full = Rect::new(0.0, 0.0, vp.w, vp.h);
    cx.fill_grad(full, 0.0, hex(BG_APP), hex(0x16122B));
    let glow = |cx: &mut Cx, x: f64, y: f64, r: f64, c: u32, a: f32| {
        let rect = Rect::new(x - r, y - r, x + r, y + r);
        cx.shadow(rect, r, alpha(c, a), r * 0.9, 0.0);
    };
    glow(cx, vp.w * 0.15, vp.h * 0.1, 140.0, ACCENT_FROM, 0.22);
    glow(cx, vp.w * 0.88, vp.h * 0.92, 160.0, ACCENT_TO, 0.18);
}
