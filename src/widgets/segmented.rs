//! Segmented control (2+ options) with a sliding accent pill.

use vello::kurbo::{Point, Rect};

use crate::anim::Spring;
use crate::draw::{Cx, mix};
use crate::icons::Icon;
use crate::theme::colors::*;
use crate::theme::metrics::{FONT_UI, R_BUTTON};
use crate::theme::motion::*;

pub struct Segment<'a> {
    pub icon: Option<Icon>,
    pub label: &'a str,
}

pub struct Segmented {
    pill: Spring,
    hover: Vec<Spring>,
}

/// Index of the segment under `p` when `rect` is split into `count` equal parts.
pub fn segment_at(rect: Rect, count: usize, p: Point) -> Option<usize> {
    if count == 0 || !rect.contains(p) {
        return None;
    }
    let i = ((p.x - rect.x0) / rect.width() * count as f64) as usize;
    Some(i.min(count - 1))
}

impl Segmented {
    pub fn new(selected: usize, count: usize) -> Self {
        Self {
            pill: Spring::new(selected as f32, MOVE_STIFFNESS, MOVE_DAMPING),
            hover: (0..count)
                .map(|_| Spring::new(0.0, HOVER_STIFFNESS, HOVER_DAMPING))
                .collect(),
        }
    }

    pub fn select(&mut self, index: usize) {
        self.pill.target = index as f32;
    }

    /// Returns true while an animation is running.
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

    pub fn draw(&self, cx: &mut Cx, rect: Rect, options: &[Segment]) {
        let n = options.len().max(1) as f64;
        let seg_w = (rect.width() - 8.0) / n;
        cx.fill(rect, R_BUTTON + 2.0, hex(bg_raised()));
        cx.stroke(rect, R_BUTTON + 2.0, 1.0, alpha(overlay(), 0.10));

        let px = rect.x0 + 4.0 + self.pill.pos as f64 * seg_w;
        let pill = Rect::new(px, rect.y0 + 4.0, px + seg_w, rect.y1 - 4.0);
        cx.fill(pill, R_BUTTON, hex(accent()));

        for (i, opt) in options.iter().enumerate() {
            let cell = Rect::new(
                rect.x0 + 4.0 + i as f64 * seg_w,
                rect.y0 + 4.0,
                rect.x0 + 4.0 + (i + 1) as f64 * seg_w,
                rect.y1 - 4.0,
            );
            let h = self.hover[i].pos.clamp(0.0, 1.0);
            let on = (1.0 - (self.pill.pos - i as f32).abs()).clamp(0.0, 1.0);
            let base = mix(text_secondary(), text_primary(), h);
            let color = mix_between(base, hex(WHITE), on);
            let icon_w = if opt.icon.is_some() { 22.0 } else { 0.0 };
            let text_w = cx.text_width(opt.label, FONT_UI);
            let mut x = cell.center().x - (text_w + icon_w) / 2.0;
            if let Some(icon) = opt.icon {
                cx.icon(icon, x, cell.center().y - 8.0, 16.0, color);
                x += icon_w;
            }
            cx.text(opt.label, FONT_UI, x, cell.center().y + 4.3, color);
        }
    }
}

/// Blend of two colors, `t` in 0..=1.
fn mix_between(a: vello::peniko::Color, b: vello::peniko::Color, t: f32) -> vello::peniko::Color {
    let (x, y) = (a.components, b.components);
    let l = |i: usize| x[i] * (1.0 - t) + y[i] * t;
    vello::peniko::Color::new([l(0), l(1), l(2), l(3)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segments_split_the_rect_evenly() {
        let r = Rect::new(0.0, 0.0, 200.0, 40.0);
        assert_eq!(segment_at(r, 2, Point::new(10.0, 10.0)), Some(0));
        assert_eq!(segment_at(r, 2, Point::new(150.0, 10.0)), Some(1));
        assert_eq!(segment_at(r, 3, Point::new(199.0, 10.0)), Some(2));
        assert_eq!(segment_at(r, 2, Point::new(250.0, 10.0)), None);
        assert_eq!(segment_at(r, 0, Point::new(10.0, 10.0)), None);
    }
}
