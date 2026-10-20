//! Mouse interactions on the canvas: panning, selecting, moving, resizing, drawing shapes.

use dv_core::{Geometry, NodeId, NodeKind};
use dv_geom::{selection_bounds, to_parent_space, world_bounds};
use dv_ui::input::Mods;
use kurbo::{Point, Rect, Vec2};

use super::canvas::frame_label_at;
use super::handles::{Handle, MIN_SIZE, handle_at_quad, rotate_zone_at_quad};
use super::ops;
use super::panels;
use super::state::{Editor, Tool};
use crate::i18n::{Msg, tr};
use crate::input::Cursor;

/// A drag shorter than this (screen px) is still a click.
pub(super) const DRAG_START_PX: f64 = 3.0;
/// Wheel sensitivity of the zoom (per logical px of scroll).
const ZOOM_PER_PX: f64 = 0.0016;
/// A drag of a resize handle of a single rotated shape.
#[derive(Clone, Copy)]
pub struct ResizeOneDrag {
    pub handle: Handle,
    pub id: NodeId,
    /// Geometry and page transform of the shape when the drag started.
    pub geom: Geometry,
    pub world: kurbo::Affine,
    pub key: u64,
}

pub enum Interaction {
    Pan {
        from: Point,
        pan0: Vec2,
        /// Started with the middle button, so only its release ends the pan.
        by_middle: bool,
    },
    /// Dragging in the color picker.
    PickColor,
    /// Dragging selected layers by their rows in the layers panel.
    LayerDrag {
        start: Point,
        moved: bool,
        /// The row that was pressed: a plain click on it selects only it.
        id: NodeId,
    },
    Marquee {
        start: Point,
        base: Vec<NodeId>,
        additive: bool,
    },
    Move {
        start: Point,
        start_screen: Point,
        originals: Vec<(NodeId, Geometry)>,
        key: u64,
        /// The pointer has moved far enough for this to be a drag, not a click.
        moved: bool,
        /// Shift+click on a selected shape removes it from the selection, unless it was dragged.
        deselect_on_click: Option<NodeId>,
        /// Selection bounds when the drag started (page space) and what it can snap to.
        bounds: Rect,
        snap_to: Vec<Rect>,
    },
    Resize {
        handle: Handle,
        orig: Rect,
        originals: Vec<(NodeId, Geometry, Rect)>,
        snap_to: Vec<Rect>,
        key: u64,
    },
    /// One rotated shape resized along its own axes.
    ResizeOne(ResizeOneDrag),
    Rotate {
        /// Rotation center (page space): the middle of the selection.
        center: Point,
        start: Point,
        /// Node, original geometry and original center (page space).
        originals: Vec<(NodeId, Geometry, Point)>,
        key: u64,
        /// The angle applied so far (degrees).
        delta: f64,
    },
    Create {
        kind: NodeKind,
        start: Point,
        start_screen: Point,
        parent: Option<NodeId>,
        /// The rectangle being drawn (page space); `None` until the pointer has moved.
        current: Option<Rect>,
    },
}

impl Editor {
    pub(super) fn press(&mut self, mods: Mods) {
        self.refresh_index();
        if self.press_picker() {
            return;
        }
        if self.press_in_paint_edit() {
            return;
        }
        if let Some(hit) = self.paint_hit() {
            self.paint_click(hit);
            return;
        }
        if self.press_in_rename() {
            return;
        }
        if self.press_in_prop_edit() {
            return;
        }
        if let Some(i) = self.field_hit() {
            self.start_prop_edit(i);
            return;
        }
        if let Some(action) = self.action_hit() {
            self.run_action(action);
            return;
        }
        let l = self.layout();
        if let Some(i) = self.toolbar.hit(&l, self.mouse.x, self.mouse.y) {
            if let Some(tool) = Tool::from_index(i) {
                self.set_tool(tool);
            }
            return;
        }
        if l.left.contains(self.mouse) {
            self.press_layers(mods);
        } else if self.over_canvas() {
            self.press_canvas(mods);
        }
    }

    fn press_layers(&mut self, mods: Mods) {
        let left = self.layout().left;
        let rows = self.layer_rows(panels::max_layer_rows(left));
        for (i, row) in rows.iter().enumerate() {
            let r = panels::layer_row_rect(left, i);
            if !r.contains(self.mouse) {
                continue;
            }
            if row.has_children && panels::caret_rect(r, row.depth).contains(self.mouse) {
                if !self.collapsed.remove(&row.id) {
                    self.collapsed.insert(row.id);
                }
            } else if panels::lock_rect(r).contains(self.mouse) {
                super::ops_layer::set_locked(
                    &mut self.session.borrow_mut().exec,
                    row.id,
                    !row.locked,
                );
                self.prune_selection();
            } else if panels::eye_rect(r).contains(self.mouse) {
                ops::set_visible(&mut self.session.borrow_mut().exec, row.id, !row.visible);
                self.prune_selection();
            } else if mods.shift {
                self.toggle_selected(row.id);
            } else if self.is_row_double_click(row.id) {
                self.selection = vec![row.id];
                let text_x = r.x0 + 26.0 + row.depth as f64 * 14.0 + 22.0;
                self.start_rename(row.id, text_x);
            } else {
                if !self.selection.contains(&row.id) {
                    self.selection = vec![row.id];
                }
                self.interaction = Some(Interaction::LayerDrag {
                    start: self.mouse,
                    moved: false,
                    id: row.id,
                });
            }
            return;
        }
    }

    fn press_canvas(&mut self, mods: Mods) {
        let (world, cp) = (self.world_mouse(), self.canvas_point());
        if self.space || self.tool == Tool::Hand {
            self.start_pan(false);
        } else if let Some(kind) = self.tool.kind() {
            self.start_create(kind, world, cp);
        } else {
            self.press_select(mods, world, cp);
        }
    }

    fn start_pan(&mut self, by_middle: bool) {
        let pan0 = self.session.borrow().vp.pan;
        self.interaction = Some(Interaction::Pan {
            from: self.canvas_point(),
            pan0,
            by_middle,
        });
    }

    fn start_create(&mut self, kind: NodeKind, world: Point, cp: Point) {
        let parent = {
            let s = self.session.borrow();
            self.index
                .pick_where(s.exec.doc().page(), world, |n| n.kind == NodeKind::Frame)
        };
        self.interaction = Some(Interaction::Create {
            kind,
            start: world,
            start_screen: cp,
            parent,
            current: None,
        });
    }

    /// Root frames cannot be picked by their body (like in Figma): they are selected by the
    /// title label, in the layers panel or by a marquee.
    fn pick_body(&self, world: Point) -> Option<NodeId> {
        let s = self.session.borrow();
        let page = s.exec.doc().page();
        self.index
            .pick_where(page, world, |n| {
                !(n.kind == NodeKind::Frame && n.parent.is_none())
            })
            .map(|id| page.outer_group(id))
    }

    fn press_select(&mut self, mods: Mods, world: Point, cp: Point) {
        if let Some(f) = self.selection_frame() {
            if let Some(h) = handle_at_quad(&f.quad, cp) {
                self.start_resize(h, &f);
                return;
            }
            if rotate_zone_at_quad(&f.quad, cp) {
                self.start_rotate(world);
                return;
            }
        }
        if let Some(id) = frame_label_at(self, cp) {
            let toggle = self.apply_pick(id, mods.shift);
            self.start_move(world, cp, toggle);
            return;
        }
        if !mods.shift && self.in_selection_box(world) {
            self.start_move(world, cp, None);
            return;
        }
        match self.pick_body(world) {
            Some(id) => {
                let toggle = self.apply_pick(id, mods.shift);
                self.start_move(world, cp, toggle);
            }
            None if self.selected_frame_contains(world) => self.start_move(world, cp, None),
            None => {
                if !mods.shift {
                    self.selection.clear();
                }
                self.interaction = Some(Interaction::Marquee {
                    start: world,
                    base: self.selection.clone(),
                    additive: mods.shift,
                });
            }
        }
    }

    /// Updates the selection for a press on `id`. Returns the id to deselect on a plain click
    /// (Shift+press on an already selected shape: a drag moves it, a click removes it).
    fn apply_pick(&mut self, id: NodeId, shift: bool) -> Option<NodeId> {
        if shift {
            if self.selection.contains(&id) {
                return Some(id);
            }
            self.selection.push(id);
        } else if !self.selection.contains(&id) {
            self.selection = vec![id];
        }
        None
    }

    fn toggle_selected(&mut self, id: NodeId) {
        match self.selection.iter().position(|x| *x == id) {
            Some(p) => {
                self.selection.remove(p);
            }
            None => self.selection.push(id),
        }
    }

    /// A single selected frame whose area contains `world` (so empty frame space drags it).
    /// With several shapes selected (or a group), the whole area of the selection box drags
    /// them, not only the shapes themselves.
    fn in_selection_box(&self, world: Point) -> bool {
        let s = self.session.borrow();
        let page = s.exec.doc().page();
        page.geo_tops(&self.selection).len() > 1
            && selection_bounds(page, &self.selection).is_some_and(|b| b.contains(world))
    }

    fn selected_frame_contains(&self, world: Point) -> bool {
        let [id] = self.selection[..] else {
            return false;
        };
        let s = self.session.borrow();
        let page = s.exec.doc().page();
        page.node(id).is_some_and(|n| n.kind == NodeKind::Frame)
            && world_bounds(page, id).is_some_and(|b| b.contains(world))
    }

    fn start_move(&mut self, world: Point, cp: Point, deselect_on_click: Option<NodeId>) {
        let originals = {
            let s = self.session.borrow();
            let page = s.exec.doc().page();
            page.geo_tops(&self.selection)
                .into_iter()
                .filter_map(|id| Some((id, page.node(id)?.geom)))
                .collect()
        };
        let (bounds, snap_to) = self.drag_snap_data();
        let key = self.next_drag_key();
        self.interaction = Some(Interaction::Move {
            start: world,
            start_screen: cp,
            originals,
            key,
            moved: false,
            deselect_on_click,
            bounds,
            snap_to,
        });
    }

    fn start_resize(&mut self, handle: Handle, frame: &super::state::SelFrame) {
        if let Some(id) = frame.node
            && frame.rotation.abs() > 0.01
        {
            self.start_resize_one(handle, id);
            return;
        }
        let (orig, originals) = {
            let s = self.session.borrow();
            let page = s.exec.doc().page();
            let top = page.geo_tops(&self.selection);
            let Some(orig) = selection_bounds(page, &top) else {
                return;
            };
            let list = top
                .into_iter()
                .filter_map(|id| Some((id, page.node(id)?.geom, world_bounds(page, id)?)))
                .collect();
            (orig, list)
        };
        let key = self.next_drag_key();
        let (_, snap_to) = self.drag_snap_data();
        self.interaction = Some(Interaction::Resize {
            handle,
            orig,
            originals,
            key,
            snap_to,
        });
    }

    /// Resizing one rotated shape: the drag works in the shape's own, turned axes.
    fn start_resize_one(&mut self, handle: Handle, id: NodeId) {
        let drag = {
            let s = self.session.borrow();
            let page = s.exec.doc().page();
            let Some(n) = page.node(id) else { return };
            ResizeOneDrag {
                handle,
                id,
                geom: n.geom,
                world: dv_geom::world_affine(page, id),
                key: 0,
            }
        };
        let key = self.next_drag_key();
        self.interaction = Some(Interaction::ResizeOne(ResizeOneDrag { key, ..drag }));
    }

    fn start_rotate(&mut self, world: Point) {
        let (center, originals) = {
            let s = self.session.borrow();
            let page = s.exec.doc().page();
            let top = page.geo_tops(&self.selection);
            let Some(bounds) = selection_bounds(page, &top) else {
                return;
            };
            let list = top
                .into_iter()
                .filter_map(|id| Some((id, page.node(id)?.geom, world_bounds(page, id)?.center())))
                .collect();
            (bounds.center(), list)
        };
        let key = self.next_drag_key();
        self.interaction = Some(Interaction::Rotate {
            center,
            start: world,
            originals,
            key,
            delta: 0.0,
        });
    }

    pub(super) fn middle(&mut self, pressed: bool) {
        if pressed {
            if self.over_canvas() {
                self.start_pan(true);
            }
        } else if matches!(
            self.interaction,
            Some(Interaction::Pan {
                by_middle: true,
                ..
            })
        ) {
            self.interaction = None;
        }
    }

    /// The left mouse button was released.
    pub(super) fn release(&mut self) {
        self.guides.clear();
        let Some(inter) = self.interaction.take() else {
            return;
        };
        match inter {
            Interaction::Pan {
                by_middle: true, ..
            } => self.interaction = Some(inter),
            Interaction::Create {
                kind,
                start,
                parent,
                current,
                ..
            } => {
                self.finish_create(kind, start, parent, current);
            }
            Interaction::Move {
                moved,
                deselect_on_click,
                ..
            } => {
                if !moved && let Some(id) = deselect_on_click {
                    self.toggle_selected(id);
                }
                self.session.borrow_mut().exec.break_merge();
                if moved {
                    self.drop_selection(self.world_mouse());
                }
            }
            Interaction::Resize { .. } | Interaction::ResizeOne(_) | Interaction::Rotate { .. } => {
                self.session.borrow_mut().exec.break_merge();
            }
            Interaction::PickColor => self.release_picker(),
            Interaction::LayerDrag { moved, id, .. } => {
                if moved {
                    self.finish_layer_drag();
                } else {
                    self.selection = vec![id];
                }
            }
            Interaction::Pan { .. } | Interaction::Marquee { .. } => {}
        }
    }

    fn finish_create(
        &mut self,
        kind: NodeKind,
        start: Point,
        parent: Option<NodeId>,
        drawn: Option<Rect>,
    ) {
        let rect = drawn.unwrap_or_else(|| {
            Rect::new(
                start.x,
                start.y,
                start.x + ops::DEFAULT_SIZE,
                start.y + ops::DEFAULT_SIZE,
            )
        });
        let created = {
            let mut s = self.session.borrow_mut();
            let page = s.exec.doc().page();
            let origin = to_parent_space(page, parent, Point::new(rect.x0, rect.y0));
            let base = match kind {
                NodeKind::Frame => tr(Msg::NameFrame),
                NodeKind::Rectangle => tr(Msg::NameRectangle),
                NodeKind::Ellipse => tr(Msg::NameEllipse),
                NodeKind::Group => tr(Msg::NameGroup),
            };
            let name = ops::next_name(page, base);
            let geom = Geometry {
                x: origin.x,
                y: origin.y,
                w: rect.width().max(MIN_SIZE),
                h: rect.height().max(MIN_SIZE),
                rotation: 0.0,
            };
            ops::create(&mut s.exec, kind, name, parent, geom)
        };
        if let Some(id) = created {
            self.selection = vec![id];
        }
        self.set_tool(Tool::Select);
    }

    pub(super) fn handle_wheel(&mut self, dx: f64, dy: f64, mods: Mods) {
        if !self.over_canvas() {
            return;
        }
        let cp = self.canvas_point();
        let mut s = self.session.borrow_mut();
        if mods.ctrl {
            s.vp.zoom_at(cp, (dy * ZOOM_PER_PX).exp());
        } else if mods.shift {
            s.vp.pan_by(Vec2::new(dy + dx, 0.0));
        } else {
            s.vp.pan_by(Vec2::new(dx, dy));
        }
    }

    pub(super) fn update_hover(&mut self) {
        self.refresh_index();
        self.hover =
            if self.interaction.is_none() && self.tool == Tool::Select && self.over_canvas() {
                let cp = self.canvas_point();
                frame_label_at(self, cp).or_else(|| self.pick_body(self.world_mouse()))
            } else {
                None
            };
    }

    pub(super) fn editor_cursor(&self) -> Cursor {
        if let Some(i) = &self.interaction {
            return match i {
                Interaction::Pan { .. } => Cursor::Grabbing,
                Interaction::Move { .. } => Cursor::Move,
                Interaction::Resize { handle, .. } => handle.cursor(),
                Interaction::ResizeOne(d) => {
                    let rotation = self.selection_frame().map_or(0.0, |f| f.rotation);
                    d.handle.cursor_rotated(rotation)
                }
                Interaction::Rotate { .. } => Cursor::Rotate,
                Interaction::Create { .. } => Cursor::Crosshair,
                Interaction::LayerDrag { .. }
                | Interaction::PickColor
                | Interaction::Marquee { .. } => Cursor::Default,
            };
        }
        let l = self.layout();
        if self.toolbar.hit(&l, self.mouse.x, self.mouse.y).is_some() {
            return Cursor::Pointer;
        }
        if l.left.contains(self.mouse) {
            return if self.layer_row_under_mouse() {
                Cursor::Pointer
            } else {
                Cursor::Default
            };
        }
        if self.field_hit().is_some() {
            return Cursor::Text;
        }
        if self.action_hit().is_some() {
            return Cursor::Pointer;
        }
        if !self.over_canvas() {
            return Cursor::Default;
        }
        if self.space || self.tool == Tool::Hand {
            return Cursor::Grab;
        }
        if self.tool.kind().is_some() {
            return Cursor::Crosshair;
        }
        let cp = self.canvas_point();
        match self.selection_frame() {
            Some(f) => match handle_at_quad(&f.quad, cp) {
                Some(h) => h.cursor_rotated(f.rotation),
                None if rotate_zone_at_quad(&f.quad, cp) => Cursor::Rotate,
                None => Cursor::Default,
            },
            None => Cursor::Default,
        }
    }

    fn layer_row_under_mouse(&self) -> bool {
        let left = self.layout().left;
        let rows = self.layer_rows(panels::max_layer_rows(left));
        (0..rows.len()).any(|i| panels::layer_row_rect(left, i).contains(self.mouse))
    }
}

/// The rectangle drawn from `start` to `end`; with `square` the sides are equal.
pub(super) fn drawn_rect(start: Point, end: Point, square: bool) -> Rect {
    if !square {
        return Rect::from_points(start, end);
    }
    let side = (end.x - start.x).abs().max((end.y - start.y).abs());
    let sx = if end.x >= start.x { 1.0 } else { -1.0 };
    let sy = if end.y >= start.y { 1.0 } else { -1.0 };
    Rect::from_points(start, Point::new(start.x + side * sx, start.y + side * sy))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drawn_rect_is_normalized_and_can_be_a_square() {
        let r = drawn_rect(Point::new(100.0, 100.0), Point::new(40.0, 130.0), false);
        assert_eq!((r.x0, r.y0, r.x1, r.y1), (40.0, 100.0, 100.0, 130.0));
        let s = drawn_rect(Point::new(100.0, 100.0), Point::new(40.0, 130.0), true);
        assert_eq!((s.x0, s.y0, s.x1, s.y1), (40.0, 100.0, 100.0, 160.0));
        let s = drawn_rect(Point::new(0.0, 0.0), Point::new(10.0, 50.0), true);
        assert_eq!((s.width(), s.height()), (50.0, 50.0));
    }
}
