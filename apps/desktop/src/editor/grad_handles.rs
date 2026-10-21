//! Gradient handles on the canvas: shown while the fill picker is open on a gradient of one
//! shape; dragging them edits the gradient (start, end, center, radius, angle).

use dv_core::NodeId;
use dv_render::{GradHandle, Viewport, drag_gradient_handle, gradient_handles};
use dv_ui::draw::Cx;
use dv_ui::theme::colors::*;
use kurbo::{Affine, BezPath, Point};

use super::interact::Interaction;
use super::paint_ui::Slot;
use super::state::Editor;

const RADIUS: f64 = 6.0;
const GRAB: f64 = 9.0;

/// A handle being dragged.
#[derive(Clone, Copy, Debug)]
pub struct GradDrag {
    pub handle: GradHandle,
    pub key: u64,
}

/// The shape being edited, the box -> canvas transform, its size and the handles in the box.
struct Shown {
    id: NodeId,
    to_screen: Affine,
    size: (f64, f64),
    handles: Vec<(GradHandle, Point)>,
}

impl Editor {
    fn shown_gradient(&self) -> Option<Shown> {
        if !matches!(self.picker.as_ref()?.slot, Slot::Fill(_)) {
            return None;
        }
        let g = self.current_gradient()?;
        let s = self.session.borrow();
        let page = s.exec.doc().page();
        let [id] = page.geo_tops(&self.selection)[..] else {
            return None;
        };
        let n = page.node(id)?;
        let (w, h) = (n.geom.w, n.geom.h);
        Some(Shown {
            id,
            to_screen: s.vp.transform() * dv_geom::world_affine(page, id),
            size: (w, h),
            handles: gradient_handles(&g, w, h),
        })
    }

    fn grad_handle_at(&self, cp: Point) -> Option<(NodeId, GradHandle)> {
        let sh = self.shown_gradient()?;
        // Later handles are drawn on top, so they win.
        sh.handles
            .iter()
            .rev()
            .find(|(_, p)| (sh.to_screen * *p).distance(cp) <= GRAB)
            .map(|(h, _)| (sh.id, *h))
    }

    pub(super) fn grad_handle_hover(&self) -> bool {
        self.over_canvas() && self.grad_handle_at(self.canvas_point()).is_some()
    }

    /// Starts dragging a gradient handle under the pointer; false if there is none.
    pub(super) fn press_grad_handle(&mut self) -> bool {
        let Some((_, handle)) = self.grad_handle_at(self.canvas_point()) else {
            return false;
        };
        let key = self.next_drag_key();
        self.interaction = Some(Interaction::GradHandle(GradDrag { handle, key }));
        true
    }

    pub(super) fn drag_grad_handle(&mut self, d: GradDrag) {
        let Some(sh) = self.shown_gradient() else {
            return;
        };
        let p = sh.to_screen.inverse() * self.canvas_point();
        let (w, h) = sh.size;
        self.edit_gradient(Some(d.key), |g| drag_gradient_handle(g, d.handle, p, w, h));
    }
}

pub(super) fn draw(ed: &Editor, cx: &mut Cx, _vp: &Viewport) {
    let Some(sh) = ed.shown_gradient() else {
        return;
    };
    let screen: Vec<(GradHandle, Point)> = sh
        .handles
        .iter()
        .map(|(h, p)| (*h, sh.to_screen * *p))
        .collect();
    let mut path = BezPath::new();
    let line_ends: Vec<Point> = screen
        .iter()
        .filter(|(h, _)| *h != GradHandle::Center)
        .map(|(_, p)| *p)
        .collect();
    let center = screen.iter().find(|(h, _)| *h == GradHandle::Center);
    match (line_ends.as_slice(), center) {
        ([a, b], _) if screen.iter().any(|(h, _)| *h == GradHandle::Start) => {
            path.move_to(*a);
            path.line_to(*b);
        }
        ([end], Some((_, c))) => {
            path.move_to(*c);
            path.line_to(*end);
        }
        _ => {}
    }
    cx.stroke_path(&path, 3.0, alpha(WHITE, 0.9));
    cx.stroke_path(&path, 1.0, hex(accent()));
    for (h, p) in screen {
        let r = if h == GradHandle::Center {
            RADIUS - 1.0
        } else {
            RADIUS
        };
        cx.circle(p.x, p.y, r + 1.0, hex(accent()));
        cx.circle(p.x, p.y, r - 0.5, hex(WHITE));
    }
}
