//! A tooltip that fades and slides in after the pointer rests on a control. The screen passes
//! the control under the pointer (its rectangle and text) every frame; the tooltip does the
//! timing and the animation.

use vello::kurbo::{Point, Rect};

use crate::anim::ease_out_cubic;
use crate::draw::Cx;
use crate::theme::colors::*;
use crate::theme::metrics::{FONT_SM, R_FIELD};

/// Seconds the pointer must rest before the tooltip appears.
const DELAY: f32 = 0.45;
/// Seconds of the fade-in and of the (faster) fade-out.
const FADE_IN: f32 = 0.16;
const FADE_OUT: f32 = 0.08;
const PAD_X: f64 = 9.0;
const HEIGHT: f64 = 24.0;
const GAP: f64 = 8.0;
const SLIDE: f64 = 5.0;

#[derive(Default)]
pub struct Tooltip {
    rect: Rect,
    text: String,
    active: bool,
    /// Hidden by a press until the pointer moves to another control.
    dismissed: bool,
    /// Seconds the pointer has rested on the current control.
    rest: f32,
    /// 0 (hidden) to 1 (fully shown), linear in time; eased when drawn.
    shown: f32,
}

impl Tooltip {
    pub fn new() -> Self {
        Self::default()
    }

    /// `target` is the control under the pointer, if it has a tooltip. Returns true while a
    /// frame must be drawn again (the delay is running or the fade is not finished).
    pub fn update(&mut self, dt: f32, target: Option<(Rect, &str)>) -> bool {
        match target {
            Some((rect, text)) => {
                if !self.active || self.rect != rect || self.text != text {
                    // Moving between neighbouring controls shows the next tooltip at once.
                    self.rest = if self.shown > 0.5 { DELAY } else { 0.0 };
                    self.rect = rect;
                    self.text = text.to_string();
                    self.active = true;
                    self.dismissed = false;
                }
                self.rest += dt;
            }
            None => {
                self.active = false;
                self.rest = 0.0;
            }
        }
        let show = self.active && !self.dismissed && self.rest >= DELAY;
        let step = if show { dt / FADE_IN } else { -dt / FADE_OUT };
        self.shown = (self.shown + step).clamp(0.0, 1.0);
        let waiting = self.active && !self.dismissed && !show;
        waiting || (show && self.shown < 1.0) || (!show && self.shown > 0.0)
    }

    /// Where the bubble is, for a window of `bounds`: below the control, above when there is
    /// no room, kept inside the window.
    pub fn bubble(&self, width: f64, bounds: Rect) -> (Rect, bool) {
        let w = width + 2.0 * PAD_X;
        let below = self.rect.y1 + GAP + HEIGHT <= bounds.y1 - 4.0;
        let y = if below {
            self.rect.y1 + GAP
        } else {
            self.rect.y0 - GAP - HEIGHT
        };
        let x = (self.rect.center().x - w / 2.0)
            .min(bounds.x1 - w - 6.0)
            .max(bounds.x0 + 6.0);
        (Rect::new(x, y, x + w, y + HEIGHT), below)
    }

    pub fn draw(&self, cx: &mut Cx, bounds: Rect) {
        if self.shown <= 0.0 || self.text.is_empty() {
            return;
        }
        let e = ease_out_cubic(self.shown);
        let (r, below) = self.bubble(cx.text_width(&self.text, FONT_SM), bounds);
        let dy = if below { -SLIDE } else { SLIDE } * (1.0 - e as f64);
        let clip = r.inflate(24.0, 24.0);
        cx.layer(clip, e, (0.0, dy), |cx| {
            cx.shadow(r, R_FIELD, alpha(0x000000, 0.28), 12.0, 4.0);
            cx.fill(r, R_FIELD, hex(bg_raised()));
            cx.stroke(r, R_FIELD, 1.0, alpha(overlay(), 0.14));
            cx.text(
                &self.text,
                FONT_SM,
                r.x0 + PAD_X,
                r.y0 + 16.0,
                hex(text_primary()),
            );
        });
    }

    /// Hides the tooltip until the pointer leaves the control (a press on it).
    pub fn dismiss(&mut self) {
        self.dismissed = true;
    }

    /// True when `p` is over the control the tooltip belongs to.
    pub fn over(&self, p: Point) -> bool {
        self.active && self.rect.contains(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: Rect = Rect::new(100.0, 100.0, 130.0, 130.0);

    fn run(t: &mut Tooltip, secs: f32, target: Option<(Rect, &str)>) {
        for _ in 0..(secs / 0.01) as usize {
            t.update(0.01, target);
        }
    }

    #[test]
    fn it_appears_only_after_the_delay_and_fades_out_when_left() {
        let mut t = Tooltip::new();
        run(&mut t, 0.2, Some((R, "Move")));
        assert_eq!(t.shown, 0.0, "still waiting");
        run(&mut t, 0.5, Some((R, "Move")));
        assert_eq!(t.shown, 1.0);
        assert!(!t.update(0.01, Some((R, "Move"))), "idle once shown");
        run(&mut t, 0.2, None);
        assert_eq!(t.shown, 0.0);
        assert!(!t.update(0.01, None), "idle once hidden");
    }

    #[test]
    fn a_neighbour_control_shows_its_tooltip_without_waiting() {
        let mut t = Tooltip::new();
        run(&mut t, 0.7, Some((R, "Move")));
        let next = Rect::new(134.0, 100.0, 164.0, 130.0);
        t.update(0.01, Some((next, "Frame")));
        assert!(t.shown > 0.5);
        assert_eq!(t.text, "Frame");
    }

    #[test]
    fn the_bubble_flips_above_near_the_bottom_and_stays_in_the_window() {
        let mut t = Tooltip::new();
        t.update(0.0, Some((Rect::new(0.0, 700.0, 30.0, 730.0), "x")));
        let bounds = Rect::new(0.0, 0.0, 800.0, 740.0);
        let (r, below) = t.bubble(60.0, bounds);
        assert!(!below && r.y1 < 700.0);
        assert!(r.x0 >= 6.0, "kept inside on the left");
        let (r, below) = Tooltip::default().bubble(60.0, Rect::new(0.0, 0.0, 800.0, 740.0));
        assert!(below && r.y0 > 0.0);
    }
}
