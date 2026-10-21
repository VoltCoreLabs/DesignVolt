//! Rulers along the canvas edges and the guide lines dragged out of them (Shift+R shows them).

use dv_core::{Command, UserGuide};
use dv_render::Viewport;
use dv_ui::draw::Cx;
use dv_ui::theme::colors::*;
use kurbo::{Point, Rect};

use super::interact::Interaction;
use super::state::Editor;
use crate::input::Cursor;

/// Thickness of a ruler (px).
pub(super) const RULER_PX: f64 = 22.0;
/// A guide is grabbed within this distance (px).
const GRAB_PX: f64 = 3.0;
const TICK_FONT: f32 = 9.0;
/// Labelled ticks are at least this far apart (px).
const MIN_LABEL_GAP: f64 = 64.0;

/// A guide being dragged: a new one (`index` is `None`) or an existing one.
#[derive(Clone, Copy, Debug)]
pub struct GuideDrag {
    pub vertical: bool,
    pub index: Option<usize>,
    /// Page-space position under the pointer.
    pub pos: f64,
}

/// The step between labelled ticks (page units): 1, 2, 5, 10, ... with `MIN_LABEL_GAP` px on screen.
pub(super) fn ruler_step(zoom: f64) -> f64 {
    let want = MIN_LABEL_GAP / zoom.max(1e-9);
    let mut mag = 10f64.powf(want.log10().floor());
    for m in [1.0, 2.0, 5.0, 10.0] {
        if mag * m >= want {
            return mag * m;
        }
    }
    mag *= 10.0;
    mag
}

impl Editor {
    /// Which ruler the canvas point is on: `Some(true)` is the left one (makes a vertical guide).
    fn ruler_at(&self, cp: Point) -> Option<bool> {
        if !self.rulers {
            return None;
        }
        match (cp.x < RULER_PX, cp.y < RULER_PX) {
            (true, false) => Some(true),
            (false, true) => Some(false),
            _ => None,
        }
    }

    /// The existing guide under the canvas point, if any.
    fn guide_at(&self, cp: Point) -> Option<usize> {
        if !self.rulers || cp.x < RULER_PX || cp.y < RULER_PX {
            return None;
        }
        let s = self.session.borrow();
        s.exec.doc().page().guides().iter().position(|g| {
            let p = s.vp.to_screen(if g.vertical {
                Point::new(g.pos, 0.0)
            } else {
                Point::new(0.0, g.pos)
            });
            (if g.vertical { p.x - cp.x } else { p.y - cp.y }).abs() <= GRAB_PX
        })
    }

    pub(super) fn toggle_rulers(&mut self) {
        self.rulers = !self.rulers;
    }

    /// Starts dragging a new guide out of a ruler or an existing one; false if neither is hit.
    pub(super) fn press_guides(&mut self) -> bool {
        let (cp, world) = (self.canvas_point(), self.world_mouse());
        let drag = if let Some(vertical) = self.ruler_at(cp) {
            GuideDrag {
                vertical,
                index: None,
                pos: if vertical { world.x } else { world.y }.round(),
            }
        } else if let Some(i) = self.guide_at(cp) {
            let g = self.session.borrow().exec.doc().page().guides()[i];
            GuideDrag {
                vertical: g.vertical,
                index: Some(i),
                pos: g.pos,
            }
        } else {
            return false;
        };
        self.interaction = Some(Interaction::Guide(drag));
        true
    }

    pub(super) fn drag_guide(&self, d: GuideDrag) -> GuideDrag {
        let w = self.world_mouse();
        GuideDrag {
            pos: if d.vertical { w.x } else { w.y }.round(),
            ..d
        }
    }

    /// The pointer is back on the ruler it came from (or left the canvas): drop the guide.
    fn guide_dropped_off(&self, d: GuideDrag) -> bool {
        let cp = self.canvas_point();
        let c = self.layout().canvas;
        let own_ruler = if d.vertical { cp.x } else { cp.y } < RULER_PX;
        own_ruler || cp.x > c.width() || cp.y > c.height()
    }

    pub(super) fn finish_guide(&mut self, d: GuideDrag) {
        let mut list = self.session.borrow().exec.doc().page().guides().to_vec();
        let off = self.guide_dropped_off(d);
        match d.index {
            Some(i) if off => {
                list.remove(i);
            }
            Some(i) => list[i].pos = d.pos,
            None if off => return,
            None => list.push(UserGuide {
                vertical: d.vertical,
                pos: d.pos,
            }),
        }
        let _ = self
            .session
            .borrow_mut()
            .exec
            .execute("Guide", Command::SetGuides(list));
    }

    pub(super) fn guide_cursor(&self) -> Option<Cursor> {
        let cp = self.canvas_point();
        let vertical = match &self.interaction {
            Some(Interaction::Guide(d)) => d.vertical,
            Some(_) => return None,
            None => match self.ruler_at(cp) {
                Some(v) => v,
                None => {
                    let i = self.guide_at(cp)?;
                    self.session.borrow().exec.doc().page().guides()[i].vertical
                }
            },
        };
        Some(if vertical {
            Cursor::ResizeEw
        } else {
            Cursor::ResizeNs
        })
    }

    /// Guide positions to snap to: vertical lines (x) and horizontal lines (y).
    pub(super) fn guide_lines(&self) -> (Vec<f64>, Vec<f64>) {
        let s = self.session.borrow();
        let g = s.exec.doc().page().guides();
        let pick = |v: bool| {
            g.iter()
                .filter(|g| g.vertical == v)
                .map(|g| g.pos)
                .collect()
        };
        if self.rulers {
            (pick(true), pick(false))
        } else {
            (Vec::new(), Vec::new())
        }
    }
}

fn line(cx: &mut Cx, r: Rect, c: u32, a: f32) {
    cx.fill(r, 0.0, alpha(c, a));
}

/// Draws the guides, then the rulers over them, in canvas screen space.
pub(super) fn draw(ed: &Editor, cx: &mut Cx, vp: &Viewport, size: (f64, f64)) {
    if !ed.rulers {
        return;
    }
    let page_guides: Vec<UserGuide> = ed.session.borrow().exec.doc().page().guides().to_vec();
    let dragging = match &ed.interaction {
        Some(Interaction::Guide(d)) => Some(*d),
        _ => None,
    };
    for (i, g) in page_guides.iter().enumerate() {
        if dragging.is_some_and(|d| d.index == Some(i)) {
            continue;
        }
        draw_guide(cx, vp, size, g.vertical, g.pos);
    }
    if let Some(d) = dragging
        && !ed.guide_dropped_off(d)
    {
        draw_guide(cx, vp, size, d.vertical, d.pos);
        let p = ed.canvas_point();
        let label = super::canvas::num(d.pos);
        let w = cx.text_width(&label, TICK_FONT + 2.0);
        let pill = Rect::new(p.x + 14.0, p.y + 14.0, p.x + 30.0 + w, p.y + 34.0);
        cx.fill(pill, 6.0, hex(accent()));
        cx.text(
            &label,
            TICK_FONT + 2.0,
            pill.x0 + 8.0,
            pill.y1 - 6.0,
            hex(WHITE),
        );
    }
    draw_rulers(cx, vp, size);
}

fn draw_guide(cx: &mut Cx, vp: &Viewport, size: (f64, f64), vertical: bool, pos: f64) {
    let p = vp.to_screen(if vertical {
        Point::new(pos, 0.0)
    } else {
        Point::new(0.0, pos)
    });
    let r = if vertical {
        Rect::new(p.x - 0.5, 0.0, p.x + 0.5, size.1)
    } else {
        Rect::new(0.0, p.y - 0.5, size.0, p.y + 0.5)
    };
    line(cx, r, accent(), 0.9);
}

fn draw_rulers(cx: &mut Cx, vp: &Viewport, size: (f64, f64)) {
    let (top, left) = (
        Rect::new(RULER_PX, 0.0, size.0, RULER_PX),
        Rect::new(0.0, RULER_PX, RULER_PX, size.1),
    );
    for r in [top, left, Rect::new(0.0, 0.0, RULER_PX, RULER_PX)] {
        cx.fill(r, 0.0, hex(bg_app()));
    }
    line(
        cx,
        Rect::new(0.0, RULER_PX - 1.0, size.0, RULER_PX),
        overlay(),
        0.25,
    );
    line(
        cx,
        Rect::new(RULER_PX - 1.0, 0.0, RULER_PX, size.1),
        overlay(),
        0.25,
    );
    let step = ruler_step(vp.zoom);
    let text = text_secondary();
    cx.clip(top, |cx| {
        let (w0, w1) = (
            vp.to_world(Point::new(0.0, 0.0)).x,
            vp.to_world(Point::new(size.0, 0.0)).x,
        );
        let mut v = (w0 / step).floor() * step;
        while v <= w1 {
            let x = vp.to_screen(Point::new(v, 0.0)).x;
            line(cx, Rect::new(x, 8.0, x + 1.0, RULER_PX), overlay(), 0.5);
            cx.text(&super::canvas::num(v), TICK_FONT, x + 3.0, 10.0, hex(text));
            let h = x + vp.zoom * step / 2.0;
            line(cx, Rect::new(h, 15.0, h + 1.0, RULER_PX), overlay(), 0.3);
            v += step;
        }
    });
    cx.clip(left, |cx| {
        let (w0, w1) = (
            vp.to_world(Point::new(0.0, 0.0)).y,
            vp.to_world(Point::new(0.0, size.1)).y,
        );
        let mut v = (w0 / step).floor() * step;
        while v <= w1 {
            let y = vp.to_screen(Point::new(0.0, v)).y;
            line(cx, Rect::new(8.0, y, RULER_PX, y + 1.0), overlay(), 0.5);
            cx.text(&super::canvas::num(v), TICK_FONT, 1.0, y - 2.0, hex(text));
            let h = y + vp.zoom * step / 2.0;
            line(cx, Rect::new(15.0, h, RULER_PX, h + 1.0), overlay(), 0.3);
            v += step;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ruler_steps_follow_a_1_2_5_sequence() {
        assert_eq!(
            ruler_step(1.0),
            100.0,
            "64 px asks for at least 64 units: 100"
        );
        assert_eq!(ruler_step(10.0), 10.0);
        assert_eq!(ruler_step(0.5), 200.0);
        assert_eq!(ruler_step(0.2), 500.0);
        assert_eq!(ruler_step(4.0), 20.0);
    }
}
