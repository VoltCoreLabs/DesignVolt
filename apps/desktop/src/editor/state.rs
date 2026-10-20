//! Editor screen: state, queries and the `Screen` implementation. Mouse interactions live in
//! `interact.rs`, keyboard shortcuts in `keys.rs`, drawing in `canvas.rs` and `panels.rs`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use dv_core::{NodeId, NodeKind};
use dv_geom::SpatialIndex;
use dv_render::PaintStats;
use dv_ui::anim::{ease_out_cubic, progress};
use dv_ui::draw::Cx;
use dv_ui::theme::colors::{bg_app, hex};
use dv_ui::theme::motion::{PANEL_INTRO, STAGGER};
use kurbo::{Affine, Point, Rect, Vec2};
use vello::Scene;

use super::interact::Interaction;
use super::layout::Layout;
use super::session::Session;
use super::toolbar::Toolbar;
use super::{canvas, panels};
use crate::input::{Cursor, Key, Mods, Screen, SharedAssets, Transition, Viewport};

/// Time after which the intro animation is over.
const INTRO_END: f32 = PANEL_INTRO + 5.0 * STAGGER + 0.1;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    Select,
    Frame,
    Rectangle,
    Ellipse,
    Hand,
}

impl Tool {
    /// Toolbar button order: select, frame, rectangle, ellipse, pen, text, hand.
    pub fn from_index(i: usize) -> Option<Tool> {
        match i {
            0 => Some(Tool::Select),
            1 => Some(Tool::Frame),
            2 => Some(Tool::Rectangle),
            3 => Some(Tool::Ellipse),
            6 => Some(Tool::Hand),
            _ => None,
        }
    }

    pub fn index(self) -> usize {
        match self {
            Tool::Select => 0,
            Tool::Frame => 1,
            Tool::Rectangle => 2,
            Tool::Ellipse => 3,
            Tool::Hand => 6,
        }
    }

    pub fn kind(self) -> Option<NodeKind> {
        match self {
            Tool::Frame => Some(NodeKind::Frame),
            Tool::Rectangle => Some(NodeKind::Rectangle),
            Tool::Ellipse => Some(NodeKind::Ellipse),
            Tool::Select | Tool::Hand => None,
        }
    }
}

/// The frame around the selection (see `Editor::selection_frame`).
pub struct SelFrame {
    /// Corners: top-left, top-right, bottom-right, bottom-left.
    pub quad: super::handles::Quad,
    /// Rotation of the frame in degrees (0 for an upright box).
    pub rotation: f64,
    /// The shape the frame belongs to when exactly one shape is selected.
    pub node: Option<NodeId>,
}

/// One line of the layers panel.
pub struct LayerRow {
    pub id: NodeId,
    pub depth: usize,
    pub name: String,
    pub kind: NodeKind,
    pub visible: bool,
    pub locked: bool,
    pub has_children: bool,
    pub collapsed: bool,
}

pub struct Editor {
    pub(super) assets: SharedAssets,
    pub(super) session: Rc<RefCell<Session>>,
    pub(super) project: String,
    pub(super) toolbar: Toolbar,
    pub(super) tool: Tool,
    pub(super) time: f32,
    pub(super) win: Viewport,
    /// Pointer position in window logical px (below the title bar).
    pub(super) mouse: Point,
    pub(super) mods: Mods,
    pub(super) index: SpatialIndex,
    pub(super) selection: Vec<NodeId>,
    pub(super) hover: Option<NodeId>,
    /// The property field of the right panel that is being typed into.
    pub(super) prop_edit: Option<super::props::PropEdit>,
    /// Guide lines shown while a drag snaps to other objects.
    pub(super) guides: Vec<dv_geom::Guide>,
    pub(super) interaction: Option<Interaction>,
    pub(super) space: bool,
    pub(super) drag_key: u64,
    pub(super) clip: super::ops_clip::Clip,
    /// A layer name being typed, and the last row click (for double-click).
    pub(super) rename: Option<super::rename::Rename>,
    pub(super) last_row_click: Option<(NodeId, std::time::Instant)>,
    /// Layers whose children are hidden in the layers panel.
    pub(super) collapsed: std::collections::HashSet<NodeId>,
    /// How long the last canvas paint took (ms) and what it drew.
    pub(super) paint_ms: Cell<f32>,
    pub(super) paint_stats: Cell<PaintStats>,
    next: Option<Transition>,
}

impl Editor {
    pub fn new(assets: SharedAssets, session: Rc<RefCell<Session>>, project: String) -> Self {
        let index = {
            let s = session.borrow();
            let doc = s.exec.doc();
            SpatialIndex::build(doc.page(), doc.revision())
        };
        Self {
            assets,
            session,
            project,
            toolbar: Toolbar::new(),
            tool: Tool::Select,
            time: 0.0,
            win: Viewport::new(),
            mouse: Point::new(-1.0, -1.0),
            mods: Mods::default(),
            index,
            selection: Vec::new(),
            hover: None,
            prop_edit: None,
            guides: Vec::new(),
            interaction: None,
            space: false,
            drag_key: 0,
            clip: Default::default(),
            rename: None,
            last_row_click: None,
            collapsed: Default::default(),
            paint_ms: Cell::new(0.0),
            paint_stats: Cell::new(PaintStats::default()),
            next: None,
        }
    }

    pub(super) fn layout(&self) -> Layout {
        Layout::new(self.win.w, self.win.h)
    }

    /// Pointer position relative to the canvas top-left corner.
    pub(super) fn canvas_point(&self) -> Point {
        let c = self.layout().canvas;
        Point::new(self.mouse.x - c.x0, self.mouse.y - c.y0)
    }

    pub(super) fn over_canvas(&self) -> bool {
        let l = self.layout();
        l.canvas.contains(self.mouse) && !l.toolbar.inflate(8.0, 8.0).contains(self.mouse)
    }

    pub(super) fn world_mouse(&self) -> Point {
        self.session.borrow().vp.to_world(self.canvas_point())
    }

    pub(super) fn set_tool(&mut self, tool: Tool) {
        self.tool = tool;
        self.toolbar.select(tool.index());
        self.interaction = None;
    }

    /// Next unused id for a merged (drag) edit.
    pub(super) fn next_drag_key(&mut self) -> u64 {
        self.drag_key += 1;
        self.drag_key
    }

    /// Drops selected ids that no longer exist or are hidden.
    pub(super) fn prune_selection(&mut self) {
        let s = self.session.borrow();
        let page = s.exec.doc().page();
        self.selection
            .retain(|id| page.node(*id).is_some_and(|n| n.visible));
        if self.hover.is_some_and(|h| !page.contains(h)) {
            self.hover = None;
        }
    }

    /// Rebuilds the spatial index when the document changed since it was built.
    pub(super) fn refresh_index(&mut self) {
        let s = self.session.borrow();
        let doc = s.exec.doc();
        if self.index.revision() != doc.revision() {
            self.index = SpatialIndex::build(doc.page(), doc.revision());
        }
    }

    /// Layers panel rows, top-most first, at most `limit` of them.
    pub(super) fn layer_rows(&self, limit: usize) -> Vec<LayerRow> {
        let s = self.session.borrow();
        let page = s.exec.doc().page();
        let mut rows = Vec::new();
        let mut stack: Vec<(NodeId, usize)> = page.roots().iter().map(|id| (*id, 0)).collect();
        while let Some((id, depth)) = stack.pop() {
            if rows.len() >= limit {
                break;
            }
            let Some(n) = page.node(id) else { continue };
            let collapsed = self.collapsed.contains(&id);
            rows.push(LayerRow {
                id,
                depth,
                name: n.name.clone(),
                kind: n.kind,
                visible: n.visible,
                locked: n.locked,
                has_children: !n.children.is_empty(),
                collapsed,
            });
            if !collapsed {
                stack.extend(n.children.iter().map(|c| (*c, depth + 1)));
            }
        }
        rows
    }

    /// Selection box in canvas screen space.
    pub(super) fn selection_screen_bounds(&self) -> Option<Rect> {
        let s = self.session.borrow();
        let b = dv_geom::selection_bounds(s.exec.doc().page(), &self.selection)?;
        let (p0, p1) = (
            s.vp.to_screen(Point::new(b.x0, b.y0)),
            s.vp.to_screen(Point::new(b.x1, b.y1)),
        );
        Some(Rect::new(p0.x, p0.y, p1.x, p1.y))
    }

    /// The frame drawn around the selection, in canvas screen space. One shape gets its own
    /// (possibly rotated) outline, so the handles sit on the shape itself; several shapes
    /// share an upright box around all of them.
    pub(super) fn selection_frame(&self) -> Option<SelFrame> {
        let s = self.session.borrow();
        let page = s.exec.doc().page();
        let tops = page.without_descendants(&self.selection);
        if let [id] = tops[..] {
            let corners = dv_geom::world_corners(page, id)?;
            let quad = corners.map(|p| s.vp.to_screen(p));
            let rotation = (quad[1].y - quad[0].y)
                .atan2(quad[1].x - quad[0].x)
                .to_degrees();
            return Some(SelFrame {
                quad,
                rotation,
                node: Some(id),
            });
        }
        drop(s);
        let b = self.selection_screen_bounds()?;
        Some(SelFrame {
            quad: super::handles::rect_quad(b),
            rotation: 0.0,
            node: None,
        })
    }

    fn intro(&self, index: f32) -> f32 {
        ease_out_cubic(progress(self.time, index * STAGGER, PANEL_INTRO))
    }
}

impl Screen for Editor {
    fn set_viewport(&mut self, w: u32, h: u32, scale: f64) {
        self.win.set(w, h, scale);
    }

    fn update(&mut self, dt: f32) -> bool {
        self.time += dt;
        {
            let c = self.layout().canvas;
            let mut s = self.session.borrow_mut();
            if !s.centered && c.width() > 1.0 {
                s.vp.pan = Vec2::new(c.width() / 2.0, c.height() / 2.0);
                s.centered = true;
            }
        }
        let hovered = self.toolbar.hit(&self.layout(), self.mouse.x, self.mouse.y);
        let busy = self.toolbar.update(dt, hovered);
        self.refresh_index();
        let saving = self
            .session
            .borrow_mut()
            .tick(dt, self.interaction.is_some());
        if self.selection.is_empty() {
            self.cancel_prop_edit();
        }
        let editing = self.prop_edit.as_mut().is_some_and(|e| e.input.update(dt));
        let renaming = self.rename.as_mut().is_some_and(|r| r.input.update(dt));
        busy || saving || editing || renaming || self.time < INTRO_END || self.interaction.is_some()
    }

    fn mouse_moved(&mut self, x: f64, y: f64) {
        self.mouse = Point::new(x / self.win.scale, y / self.win.scale);
        self.drag_interaction();
        self.update_hover();
    }

    fn mouse_left(&mut self) {
        self.mouse = Point::new(-1.0, -1.0);
        self.hover = None;
    }

    fn click(&mut self, mods: Mods) {
        self.mods = mods;
        self.press(mods);
    }

    fn mouse_up(&mut self) {
        self.release();
    }

    fn middle_button(&mut self, pressed: bool) {
        self.middle(pressed);
    }

    fn wheel(&mut self, dx: f64, dy: f64, mods: Mods) {
        self.handle_wheel(dx, dy, mods);
    }

    fn key(&mut self, key: Key, mods: Mods) {
        self.mods = mods;
        self.handle_key(key, mods);
    }

    fn text(&mut self, s: &str) {
        if self.rename.is_some() {
            self.rename_text(s);
            return;
        }
        self.prop_text(s);
    }

    fn key_up(&mut self, key: Key) {
        if key == Key::Space {
            self.space = false;
        }
    }

    fn pointer_hand(&self) -> bool {
        self.toolbar
            .hit(&self.layout(), self.mouse.x, self.mouse.y)
            .is_some()
    }

    fn cursor(&self) -> Cursor {
        self.editor_cursor()
    }

    fn take_transition(&mut self) -> Option<Transition> {
        self.next.take()
    }

    fn draw(&self, scene: &mut Scene) {
        scene.reset();
        let layout = self.layout();
        let mut cx = Cx {
            scene,
            font: &self.assets.font,
            icons: &self.assets.icons,
            xf: Affine::scale(self.win.scale),
        };
        let full = Rect::new(0.0, 0.0, self.win.w, self.win.h);
        cx.fill(full, 0.0, hex(bg_app()));

        let p = self.intro(0.0);
        cx.layer(layout.canvas, p, (0.0, 0.0), |cx| {
            canvas::draw(self, cx, layout.canvas)
        });
        cx.layer(layout.topbar, p, (0.0, -16.0 * (1.0 - p as f64)), |cx| {
            panels::topbar(cx, layout.topbar, self)
        });
        let p = self.intro(1.0);
        cx.layer(layout.left, p, (-24.0 * (1.0 - p as f64), 0.0), |cx| {
            panels::left_panel(cx, layout.left, self)
        });
        let p = self.intro(2.0);
        cx.layer(layout.right, p, (24.0 * (1.0 - p as f64), 0.0), |cx| {
            panels::right_panel(cx, layout.right, self)
        });
        let p = self.intro(4.0);
        let clip = layout.toolbar.inflate(40.0, 40.0);
        cx.layer(clip, p, (0.0, 24.0 * (1.0 - p as f64)), |cx| {
            self.toolbar.draw(cx, &layout)
        });
    }
}
