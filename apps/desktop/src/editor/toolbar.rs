//! Floating glass toolbar with a sliding active pill and hover springs.

use vello::kurbo::{Point, Rect};

use super::layout::{Layout, TOOL_COUNT, TOOL_GAP, TOOL_SIZE};
use dv_ui::anim::Spring;
use dv_ui::draw::{Cx, mix};
use dv_ui::icons::Icon;
use dv_ui::theme::colors::*;
use dv_ui::theme::metrics::{R_BUTTON, R_MODAL};
use dv_ui::theme::motion::*;

const ICONS: [Icon; TOOL_COUNT] = [
    Icon::Cursor,
    Icon::Frame,
    Icon::Square,
    Icon::Circle,
    Icon::Pen,
    Icon::Text,
    Icon::Hand,
];

pub struct Toolbar {
    pub active: usize,
    pill: Spring,
    hover: [Spring; TOOL_COUNT],
}

impl Toolbar {
    pub fn new() -> Self {
        Self {
            active: 0,
            pill: Spring::new(0.0, MOVE_STIFFNESS, MOVE_DAMPING),
            hover: std::array::from_fn(|_| Spring::new(0.0, HOVER_STIFFNESS, HOVER_DAMPING)),
        }
    }

    pub fn hit(&self, layout: &Layout, x: f64, y: f64) -> Option<usize> {
        layout
            .tools
            .iter()
            .position(|t| t.contains(Point::new(x, y)))
    }

    pub fn select(&mut self, i: usize) {
        self.active = i;
        self.pill.target = i as f32;
    }

    /// Returns true while an animation is still running.
    pub fn update(&mut self, dt: f32, hovered: Option<usize>) -> bool {
        self.pill.step(dt);
        let mut busy = !self.pill.settled();
        for (i, h) in self.hover.iter_mut().enumerate() {
            h.target = if hovered == Some(i) { 1.0 } else { 0.0 };
            h.step(dt);
            busy |= !h.settled();
        }
        busy
    }

    pub fn draw(&self, cx: &mut Cx, layout: &Layout) {
        cx.glass(layout.toolbar, R_MODAL);

        let first = layout.tools[0];
        let px = first.x0 + self.pill.pos as f64 * (TOOL_SIZE + TOOL_GAP);
        let pill = Rect::new(px, first.y0, px + TOOL_SIZE, first.y1);
        cx.fill(pill, R_BUTTON, hex(accent()));

        for (i, t) in layout.tools.iter().enumerate() {
            let h = self.hover[i].pos.clamp(0.0, 1.0);
            if self.active != i {
                cx.fill(*t, R_BUTTON, alpha(bg_hover(), h));
            }
            // Active icon is white; others fade from secondary to primary on hover.
            let near_pill = (self.pill.pos - i as f32).abs().min(1.0);
            let color = if near_pill < 0.5 {
                hex(WHITE)
            } else {
                mix(text_secondary(), text_primary(), h)
            };
            let c = t.center();
            cx.icon(ICONS[i], c.x - 9.0, c.y - 9.0, 18.0, color);
        }
    }
}
