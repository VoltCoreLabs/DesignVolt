//! Editor screen layout (logical px), Figma-like: top bar, left/right panels, canvas, floating toolbar.

use vello::kurbo::Rect;

use crate::theme::metrics::{LEFT_W, RIGHT_W, TOPBAR_H};

pub const TOOL_COUNT: usize = 7;
pub const TOOL_SIZE: f64 = 36.0;
pub const TOOL_GAP: f64 = 4.0;
const TOOLBAR_PAD: f64 = 6.0;
const TOOLBAR_H: f64 = 48.0;
const TOOLBAR_MARGIN: f64 = 20.0;

pub struct Layout {
    pub topbar: Rect,
    pub left: Rect,
    pub right: Rect,
    pub canvas: Rect,
    pub toolbar: Rect,
    pub tools: [Rect; TOOL_COUNT],
}

impl Layout {
    pub fn new(w: f64, h: f64) -> Self {
        let topbar = Rect::new(0.0, 0.0, w, TOPBAR_H);
        let left = Rect::new(0.0, TOPBAR_H, LEFT_W, h);
        let right = Rect::new((w - RIGHT_W).max(LEFT_W), TOPBAR_H, w, h);
        let canvas = Rect::new(left.x1, TOPBAR_H, right.x0, h);

        let tb_w = TOOL_COUNT as f64 * TOOL_SIZE + (TOOL_COUNT as f64 - 1.0) * TOOL_GAP + 2.0 * TOOLBAR_PAD;
        let tb_x = canvas.center().x - tb_w / 2.0;
        let tb_y = canvas.y1 - TOOLBAR_MARGIN - TOOLBAR_H;
        let toolbar = Rect::new(tb_x, tb_y, tb_x + tb_w, tb_y + TOOLBAR_H);

        let mut tools = [Rect::ZERO; TOOL_COUNT];
        for (i, t) in tools.iter_mut().enumerate() {
            let x = tb_x + TOOLBAR_PAD + i as f64 * (TOOL_SIZE + TOOL_GAP);
            let y = tb_y + (TOOLBAR_H - TOOL_SIZE) / 2.0;
            *t = Rect::new(x, y, x + TOOL_SIZE, y + TOOL_SIZE);
        }
        Self { topbar, left, right, canvas, toolbar, tools }
    }
}
