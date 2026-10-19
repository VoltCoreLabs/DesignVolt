//! Canvas mockup: dot grid, a frame with shapes and a selection box.

use vello::kurbo::Rect;

use crate::draw::{Cx, mix};
use crate::i18n::{Msg, tr};
use crate::theme::colors::*;

const GRID: f64 = 24.0;

pub fn draw(cx: &mut Cx, r: Rect) {
    cx.fill(r, 0.0, hex(bg_canvas()));
    cx.clip(r, |cx| {
        let dot = alpha(overlay(), 0.09);
        let mut y = r.y0 + GRID / 2.0;
        while y < r.y1 {
            let mut x = r.x0 + GRID / 2.0;
            while x < r.x1 {
                cx.circle(x, y, 1.0, dot);
                x += GRID;
            }
            y += GRID;
        }

        let c = r.center();
        let frame = Rect::new(c.x - 180.0, c.y - 150.0, c.x + 180.0, c.y + 90.0);
        cx.text(
            tr(Msg::LayerFrame),
            11.0,
            frame.x0,
            frame.y0 - 8.0,
            hex(text_secondary()),
        );
        cx.shadow(frame, 12.0, alpha(0x000000, 0.45), 24.0, 12.0);
        cx.fill(frame, 12.0, hex(WHITE));

        let rect = Rect::new(
            frame.x0 + 40.0,
            frame.y0 + 50.0,
            frame.x0 + 200.0,
            frame.y0 + 150.0,
        );
        cx.fill(rect, 16.0, hex(accent()));
        cx.circle(
            frame.x1 - 70.0,
            frame.y0 + 120.0,
            38.0,
            mix(0x34D399, 0x22D3EE, 0.5),
        );

        // Selection box with handles.
        cx.stroke(rect, 16.0, 1.5, hex(accent()));
        for (hx, hy) in [
            (rect.x0, rect.y0),
            (rect.x1, rect.y0),
            (rect.x0, rect.y1),
            (rect.x1, rect.y1),
        ] {
            let h = Rect::new(hx - 4.0, hy - 4.0, hx + 4.0, hy + 4.0);
            cx.fill(h, 2.0, hex(WHITE));
            cx.stroke(h, 2.0, 1.5, hex(accent()));
        }
    });
}
