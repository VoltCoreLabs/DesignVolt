//! Continuing a mouse interaction while the pointer moves: pan, marquee, move, resize, draw.

use dv_core::{Geometry, NodeId};
use dv_geom::{parent_delta, snap_edges, snap_rect, snap_to_lines, to_parent_space};
use kurbo::{Point, Rect};

use super::handles::{
    Handle, angle_between, normalize_angle, resize_rect, rotate_point, snap_angle,
};
use super::interact::{DRAG_START_PX, Interaction, ResizeOneDrag, drawn_rect};
use super::ops;

/// Shift snaps rotation to multiples of this angle (degrees).
const ROTATE_SNAP: f64 = 15.0;
/// Objects snap to guides closer than this (screen px).
const SNAP_PX: f64 = 6.0;
/// At most this many nearby objects are considered for snapping.
const SNAP_CANDIDATES: usize = 2000;
use super::state::Editor;

/// What a move drag needs: where it started, the original geometry and what it can snap to.
struct MoveDrag<'a> {
    start: Point,
    originals: &'a [(NodeId, Geometry)],
    key: u64,
    bounds: Rect,
    snap_to: &'a [Rect],
}

/// What a resize drag needs.
struct ResizeDrag<'a> {
    handle: Handle,
    orig: Rect,
    originals: &'a [(NodeId, Geometry, Rect)],
    key: u64,
    snap_to: &'a [Rect],
}

impl Editor {
    /// Continues the running interaction after the pointer moved.
    pub(super) fn drag_interaction(&mut self) {
        let Some(inter) = self.interaction.take() else {
            return;
        };
        let (cp, world, shift) = (self.canvas_point(), self.world_mouse(), self.mods.shift);
        let next = match inter {
            Interaction::Pan {
                from,
                pan0,
                by_middle,
            } => {
                self.session.borrow_mut().vp.pan = pan0 + (cp - from);
                Interaction::Pan {
                    from,
                    pan0,
                    by_middle,
                }
            }
            Interaction::Marquee {
                start,
                base,
                additive,
            } => {
                self.update_marquee(start, world, &base, additive);
                Interaction::Marquee {
                    start,
                    base,
                    additive,
                }
            }
            Interaction::Move {
                start,
                start_screen,
                originals,
                key,
                moved,
                deselect_on_click,
                bounds,
                snap_to,
            } => {
                let moved = moved || (cp - start_screen).hypot() >= DRAG_START_PX;
                if moved {
                    let args = MoveDrag {
                        start,
                        originals: &originals,
                        key,
                        bounds,
                        snap_to: &snap_to,
                    };
                    self.apply_move(world, shift, args);
                }
                Interaction::Move {
                    start,
                    start_screen,
                    originals,
                    key,
                    moved,
                    deselect_on_click,
                    bounds,
                    snap_to,
                }
            }
            Interaction::Resize {
                handle,
                orig,
                originals,
                key,
                snap_to,
            } => {
                let args = ResizeDrag {
                    handle,
                    orig,
                    originals: &originals,
                    key,
                    snap_to: &snap_to,
                };
                self.apply_resize(world, shift, args);
                Interaction::Resize {
                    handle,
                    orig,
                    originals,
                    key,
                    snap_to,
                }
            }
            Interaction::ResizeOne(d) => {
                self.apply_resize_one(world, shift, d);
                Interaction::ResizeOne(d)
            }
            Interaction::Rotate {
                center,
                start,
                originals,
                key,
                ..
            } => {
                let delta = self.apply_rotate(center, start, world, shift, &originals, key);
                Interaction::Rotate {
                    center,
                    start,
                    originals,
                    key,
                    delta,
                }
            }
            Interaction::PickColor => {
                self.drag_picker();
                Interaction::PickColor
            }
            Interaction::Guide(d) => Interaction::Guide(self.drag_guide(d)),
            Interaction::GradHandle(d) => {
                self.drag_grad_handle(d);
                Interaction::GradHandle(d)
            }
            Interaction::LayerDrag { start, moved, id } => Interaction::LayerDrag {
                start,
                id,
                moved: moved || (self.mouse - start).hypot() >= super::layer_drag::DRAG_START_PX,
            },
            Interaction::Create {
                kind,
                start,
                start_screen,
                parent,
                current,
            } => {
                let current = if current.is_some() || (cp - start_screen).hypot() >= DRAG_START_PX {
                    Some(drawn_rect(start, world, shift))
                } else {
                    None
                };
                Interaction::Create {
                    kind,
                    start,
                    start_screen,
                    parent,
                    current,
                }
            }
        };
        self.interaction = Some(next);
    }

    fn update_marquee(&mut self, start: Point, world: Point, base: &[NodeId], additive: bool) {
        let rect = Rect::from_points(start, world);
        let hits: Vec<NodeId> = {
            let s = self.session.borrow();
            let page = s.exec.doc().page();
            self.index
                .in_rect(page, rect)
                .into_iter()
                .filter(|id| page.node(*id).is_some_and(|n| n.parent.is_none()))
                .chain(page.roots().iter().copied().filter(|id| {
                    page.node(*id).is_some_and(|n| {
                        n.kind == dv_core::NodeKind::Group && n.visible && !n.locked
                    }) && dv_geom::selection_bounds(page, &[*id]).is_some_and(|b| {
                        b.x0 <= rect.x1 && b.x1 >= rect.x0 && b.y0 <= rect.y1 && b.y1 >= rect.y0
                    })
                }))
                .collect()
        };
        let mut sel: Vec<NodeId> = if additive { base.to_vec() } else { Vec::new() };
        for id in hits {
            if !sel.contains(&id) {
                sel.push(id);
            }
        }
        self.selection = sel;
    }

    fn apply_move(&mut self, world: Point, shift: bool, m: MoveDrag<'_>) {
        let MoveDrag {
            start,
            originals,
            key,
            bounds,
            snap_to,
        } = m;
        let mut d = world - start;
        // With Shift the move is locked to its dominant axis.
        let y_locked = shift && d.x.abs() >= d.y.abs();
        let x_locked = shift && !y_locked;
        if y_locked {
            d.y = 0.0;
        }
        if x_locked {
            d.x = 0.0;
        }
        self.guides.clear();
        if !self.mods.ctrl {
            let threshold = SNAP_PX / self.session.borrow().vp.zoom;
            let snap = snap_rect(bounds + d, snap_to, threshold);
            if !x_locked {
                d.x += snap.dx;
            }
            if !y_locked {
                d.y += snap.dy;
            }
            self.guides = snap.guides;
            // User guides (rulers) pull where no object was close.
            let (xs, ys) = self.guide_lines();
            let (gx, gy) = snap_to_lines(bounds + d, &xs, &ys, threshold);
            if !x_locked && snap.dx == 0.0 {
                d.x += gx;
            }
            if !y_locked && snap.dy == 0.0 {
                d.y += gy;
            }
        }
        let list: Vec<(NodeId, Geometry)> = {
            let s = self.session.borrow();
            let page = s.exec.doc().page();
            originals
                .iter()
                .map(|(id, g)| {
                    let pd = parent_delta(page, *id, d);
                    (
                        *id,
                        Geometry {
                            x: g.x + pd.x,
                            y: g.y + pd.y,
                            ..*g
                        },
                    )
                })
                .collect()
        };
        ops::set_geometry(&mut self.session.borrow_mut().exec, key, "Move", &list);
    }

    fn apply_resize(&mut self, world: Point, keep_aspect: bool, r: ResizeDrag<'_>) {
        let ResizeDrag {
            handle,
            orig,
            originals,
            key,
            snap_to,
        } = r;
        let mut new = resize_rect(handle, orig, world, keep_aspect);
        self.guides.clear();
        if !keep_aspect && !self.mods.ctrl && !snap_to.is_empty() {
            let threshold = SNAP_PX / self.session.borrow().vp.zoom;
            let (snapped, guides) = snap_edges(new, handle.edges(), snap_to, threshold);
            new = snapped;
            self.guides = guides;
        }
        let list = {
            let s = self.session.borrow();
            ops::resized(s.exec.doc().page(), originals, orig, new)
        };
        ops::set_geometry(&mut self.session.borrow_mut().exec, key, "Resize", &list);
    }

    /// Resizes one rotated shape: the pointer is taken into the shape's own axes (as they were
    /// when the drag started), the opposite side stays where it is on the page.
    fn apply_resize_one(&mut self, cursor: Point, keep_aspect: bool, d: ResizeOneDrag) {
        self.guides.clear();
        let local = d.world.inverse() * cursor;
        let orig = Rect::new(0.0, 0.0, d.geom.w, d.geom.h);
        let new = resize_rect(d.handle, orig, local, keep_aspect);
        let center = d.world * new.center();
        let geom = {
            let s = self.session.borrow();
            let page = s.exec.doc().page();
            let parent = page.node(d.id).and_then(|n| n.parent);
            let c = to_parent_space(page, parent, center);
            Geometry {
                x: c.x - new.width() / 2.0,
                y: c.y - new.height() / 2.0,
                w: new.width(),
                h: new.height(),
                rotation: d.geom.rotation,
            }
        };
        ops::set_geometry(
            &mut self.session.borrow_mut().exec,
            d.key,
            "Resize",
            &[(d.id, geom)],
        );
    }

    /// Turns the selection around its center so that the pointer keeps its angle. Shift snaps
    /// to 15 degrees. Returns the angle applied.
    fn apply_rotate(
        &mut self,
        center: Point,
        start: Point,
        world: Point,
        snap: bool,
        originals: &[(NodeId, Geometry, Point)],
        key: u64,
    ) -> f64 {
        let mut delta = angle_between(center, start, world);
        if snap {
            // A single shape snaps its own angle, a group snaps the turn.
            delta = match originals {
                [(_, g, _)] => snap_angle(g.rotation + delta, ROTATE_SNAP) - g.rotation,
                _ => snap_angle(delta, ROTATE_SNAP),
            };
        }
        let list: Vec<(NodeId, Geometry)> = {
            let s = self.session.borrow();
            let page = s.exec.doc().page();
            originals
                .iter()
                .map(|(id, g, c)| {
                    let turned = rotate_point(center, *c, delta);
                    let parent = page.node(*id).and_then(|n| n.parent);
                    let p = to_parent_space(page, parent, turned);
                    let geom = Geometry {
                        x: p.x - g.w / 2.0,
                        y: p.y - g.h / 2.0,
                        rotation: normalize_angle(g.rotation + delta),
                        ..*g
                    };
                    (*id, geom)
                })
                .collect()
        };
        ops::set_geometry(&mut self.session.borrow_mut().exec, key, "Rotate", &list);
        delta
    }

    /// The selection's bounds and the rectangles a drag can snap to: everything visible nearby
    /// except the selection itself.
    pub(super) fn drag_snap_data(&self) -> (Rect, Vec<Rect>) {
        let s = self.session.borrow();
        let page = s.exec.doc().page();
        let top = page.without_descendants(&self.selection);
        let bounds = dv_geom::selection_bounds(page, &top).unwrap_or(Rect::ZERO);
        let skip: std::collections::HashSet<NodeId> = self
            .selection
            .iter()
            .flat_map(|id| page.subtree(*id))
            .collect();
        let canvas = self.layout().canvas;
        let margin = 300.0 / s.vp.zoom;
        let view =
            s.vp.visible_world((canvas.width(), canvas.height()))
                .inflate(margin, margin);
        let rects = self
            .index
            .query(view)
            .filter(|e| !skip.contains(&e.id))
            .take(SNAP_CANDIDATES)
            .map(|e| e.bounds)
            .collect();
        (bounds, rects)
    }
}
