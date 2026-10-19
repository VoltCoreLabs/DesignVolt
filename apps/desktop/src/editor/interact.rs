//! Mouse interactions on the canvas: panning, selecting, moving, resizing, drawing shapes.

use dv_core::{Geometry, NodeId, NodeKind};
use dv_geom::{parent_delta, selection_bounds, to_parent_space, world_bounds};
use dv_ui::input::Mods;
use kurbo::{Point, Rect, Vec2};

use super::canvas::frame_label_at;
use super::handles::{Handle, MIN_SIZE, handle_at, map_point, resize_rect, scale};
use super::ops;
use super::panels;
use super::state::{Editor, Tool};
use crate::i18n::{Msg, tr};
use crate::input::Cursor;

/// A drag shorter than this (screen px) is still a click.
const DRAG_START_PX: f64 = 3.0;
/// Wheel sensitivity of the zoom (per logical px of scroll).
const ZOOM_PER_PX: f64 = 0.0016;

pub enum Interaction {
    Pan {
        from: Point,
        pan0: Vec2,
        /// Started with the middle button, so only its release ends the pan.
        by_middle: bool,
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
    },
    Resize {
        handle: Handle,
        orig: Rect,
        originals: Vec<(NodeId, Geometry, Rect)>,
        key: u64,
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
            if panels::eye_rect(r).contains(self.mouse) {
                ops::set_visible(&mut self.session.borrow_mut().exec, row.id, !row.visible);
                self.prune_selection();
            } else if mods.shift {
                self.toggle_selected(row.id);
            } else {
                self.selection = vec![row.id];
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
        self.index.pick_where(s.exec.doc().page(), world, |n| {
            !(n.kind == NodeKind::Frame && n.parent.is_none())
        })
    }

    fn press_select(&mut self, mods: Mods, world: Point, cp: Point) {
        if let Some(b) = self.selection_screen_bounds()
            && let Some(h) = handle_at(b, cp)
        {
            self.start_resize(h);
            return;
        }
        if let Some(id) = frame_label_at(self, cp) {
            let toggle = self.apply_pick(id, mods.shift);
            self.start_move(world, cp, toggle);
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
            page.without_descendants(&self.selection)
                .into_iter()
                .filter_map(|id| Some((id, page.node(id)?.geom)))
                .collect()
        };
        let key = self.next_drag_key();
        self.interaction = Some(Interaction::Move {
            start: world,
            start_screen: cp,
            originals,
            key,
            moved: false,
            deselect_on_click,
        });
    }

    fn start_resize(&mut self, handle: Handle) {
        let (orig, originals) = {
            let s = self.session.borrow();
            let page = s.exec.doc().page();
            let top = page.without_descendants(&self.selection);
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
        self.interaction = Some(Interaction::Resize {
            handle,
            orig,
            originals,
            key,
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
            } => {
                let moved = moved || (cp - start_screen).hypot() >= DRAG_START_PX;
                if moved {
                    self.apply_move(start, world, shift, &originals, key);
                }
                Interaction::Move {
                    start,
                    start_screen,
                    originals,
                    key,
                    moved,
                    deselect_on_click,
                }
            }
            Interaction::Resize {
                handle,
                orig,
                originals,
                key,
            } => {
                self.apply_resize(handle, orig, world, shift, &originals, key);
                Interaction::Resize {
                    handle,
                    orig,
                    originals,
                    key,
                }
            }
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

    fn apply_move(
        &mut self,
        start: Point,
        world: Point,
        shift: bool,
        originals: &[(NodeId, Geometry)],
        key: u64,
    ) {
        let mut d = world - start;
        if shift {
            if d.x.abs() >= d.y.abs() {
                d.y = 0.0;
            } else {
                d.x = 0.0;
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

    fn apply_resize(
        &mut self,
        handle: Handle,
        orig: Rect,
        world: Point,
        keep_aspect: bool,
        originals: &[(NodeId, Geometry, Rect)],
        key: u64,
    ) {
        let new = resize_rect(handle, orig, world, keep_aspect);
        let (sx, sy) = scale(orig, new);
        let list: Vec<(NodeId, Geometry)> = {
            let s = self.session.borrow();
            let page = s.exec.doc().page();
            originals
                .iter()
                .map(|(id, g, wb)| {
                    let (w, h) = ((g.w * sx).max(MIN_SIZE), (g.h * sy).max(MIN_SIZE));
                    let center = map_point(orig, new, wb.center());
                    let parent = page.node(*id).and_then(|n| n.parent);
                    let p = to_parent_space(page, parent, center);
                    (
                        *id,
                        Geometry {
                            x: p.x - w / 2.0,
                            y: p.y - h / 2.0,
                            w,
                            h,
                            rotation: g.rotation,
                        },
                    )
                })
                .collect()
        };
        ops::set_geometry(&mut self.session.borrow_mut().exec, key, "Resize", &list);
    }

    /// The left mouse button was released.
    pub(super) fn release(&mut self) {
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
            }
            Interaction::Resize { .. } => self.session.borrow_mut().exec.break_merge(),
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
                Interaction::Create { .. } => Cursor::Crosshair,
                Interaction::Marquee { .. } => Cursor::Default,
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
        if !self.over_canvas() {
            return Cursor::Default;
        }
        if self.space || self.tool == Tool::Hand {
            return Cursor::Grab;
        }
        if self.tool.kind().is_some() {
            return Cursor::Crosshair;
        }
        match self
            .selection_screen_bounds()
            .and_then(|b| handle_at(b, self.canvas_point()))
        {
            Some(h) => h.cursor(),
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
fn drawn_rect(start: Point, end: Point, square: bool) -> Rect {
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
