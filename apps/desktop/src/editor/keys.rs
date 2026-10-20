//! Keyboard shortcuts of the editor.

use dv_core::NodeId;
use dv_ui::input::{Key, Mods};
use kurbo::{Point, Rect};

use super::interact::Interaction;
use super::ops;
use super::ops_order::Order;
use super::state::{Editor, Tool};

/// Zoom step of the keyboard shortcuts.
const ZOOM_STEP: f64 = 1.25;
/// Number of shapes of the performance test page (Ctrl+Shift+B).
pub const STRESS_COUNT: usize = 10_000;

impl Editor {
    pub(super) fn handle_key(&mut self, key: Key, mods: Mods) {
        if self.rename.is_some() {
            self.rename_key(key, mods);
            return;
        }
        if self.prop_edit.is_some() {
            self.prop_key(key, mods);
            return;
        }
        match key {
            Key::Space => self.space = true,
            Key::Escape => self.escape(),
            Key::Delete | Key::Backspace => self.delete_selection(),
            Key::Left => self.nudge_selection(-1.0, 0.0, mods),
            Key::Right => self.nudge_selection(1.0, 0.0, mods),
            Key::Up => self.nudge_selection(0.0, -1.0, mods),
            Key::Down => self.nudge_selection(0.0, 1.0, mods),
            Key::Undo => self.undo(),
            Key::Redo => self.redo(),
            Key::SelectAll => self.select_all(),
            Key::Copy => self.copy_selection(),
            Key::Cut => {
                self.copy_selection();
                self.delete_selection();
            }
            Key::Paste => self.paste(),
            Key::Char(c) => self.char_shortcut(c, mods),
            _ => {}
        }
    }

    fn char_shortcut(&mut self, c: char, mods: Mods) {
        if mods.ctrl {
            match c {
                'd' => self.duplicate_selection(),
                ']' => self.reorder_selection(if mods.shift {
                    Order::Front
                } else {
                    Order::Forward
                }),
                '[' => self.reorder_selection(if mods.shift {
                    Order::Back
                } else {
                    Order::Backward
                }),
                'b' if mods.shift => self.stress_page(),
                '=' => self.zoom_center(ZOOM_STEP),
                '-' => self.zoom_center(1.0 / ZOOM_STEP),
                '0' => self.set_zoom(1.0),
                _ => {}
            }
            return;
        }
        if mods.shift {
            match c {
                '0' => self.set_zoom(1.0),
                '1' => self.fit_all(),
                '2' => self.fit_selection(),
                _ => {}
            }
            return;
        }
        match c {
            'v' => self.set_tool(Tool::Select),
            'f' => self.set_tool(Tool::Frame),
            'r' => self.set_tool(Tool::Rectangle),
            'o' => self.set_tool(Tool::Ellipse),
            'h' => self.set_tool(Tool::Hand),
            _ => {}
        }
    }

    fn escape(&mut self) {
        if self.interaction.is_some() {
            // Cancel the running drag: whatever it did stays as a finished step.
            if matches!(
                self.interaction,
                Some(Interaction::Move { .. } | Interaction::Resize { .. })
            ) {
                self.session.borrow_mut().exec.break_merge();
            }
            self.interaction = None;
            self.guides.clear();
        } else if self.tool != Tool::Select {
            self.set_tool(Tool::Select);
        } else {
            self.selection.clear();
        }
    }

    fn delete_selection(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        ops::delete(&mut self.session.borrow_mut().exec, &self.selection);
        self.selection.clear();
        self.hover = None;
    }

    fn nudge_selection(&mut self, dx: f64, dy: f64, mods: Mods) {
        if self.selection.is_empty() {
            return;
        }
        let step = if mods.shift { 10.0 } else { 1.0 };
        ops::nudge(
            &mut self.session.borrow_mut().exec,
            &self.selection,
            dx * step,
            dy * step,
        );
    }

    fn undo(&mut self) {
        self.session.borrow_mut().exec.undo();
        self.prune_selection();
    }

    fn redo(&mut self) {
        self.session.borrow_mut().exec.redo();
        self.prune_selection();
    }

    fn select_all(&mut self) {
        let s = self.session.borrow();
        let page = s.exec.doc().page();
        self.selection = page
            .roots()
            .iter()
            .copied()
            .filter(|id| page.node(*id).is_some_and(|n| n.visible && !n.locked))
            .collect();
    }

    fn duplicate_selection(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        let copies = ops::duplicate(&mut self.session.borrow_mut().exec, &self.selection);
        if !copies.is_empty() {
            self.selection = copies;
        }
    }

    fn copy_selection(&mut self) {
        if !self.selection.is_empty() {
            self.clip.copy(&self.session.borrow().exec, &self.selection);
        }
    }

    fn paste(&mut self) {
        let made = self.clip.paste(&mut self.session.borrow_mut().exec);
        if !made.is_empty() {
            self.selection = made;
        }
    }

    fn reorder_selection(&mut self, mode: Order) {
        if !self.selection.is_empty() {
            super::ops_order::reorder(&mut self.session.borrow_mut().exec, &self.selection, mode);
        }
    }

    fn stress_page(&mut self) {
        ops::stress(&mut self.session.borrow_mut().exec, STRESS_COUNT);
        self.selection.clear();
        self.fit_all();
    }

    fn canvas_size(&self) -> (f64, f64) {
        let c = self.layout().canvas;
        (c.width(), c.height())
    }

    fn zoom_center(&mut self, factor: f64) {
        let (w, h) = self.canvas_size();
        self.session
            .borrow_mut()
            .vp
            .zoom_at(Point::new(w / 2.0, h / 2.0), factor);
    }

    fn set_zoom(&mut self, zoom: f64) {
        let (w, h) = self.canvas_size();
        self.session
            .borrow_mut()
            .vp
            .set_zoom_at(Point::new(w / 2.0, h / 2.0), zoom);
    }

    fn fit_all(&mut self) {
        let size = self.canvas_size();
        let mut s = self.session.borrow_mut();
        let page = s.exec.doc().page();
        let ids: Vec<NodeId> = page.roots().to_vec();
        if let Some(b) = dv_geom::selection_bounds(page, &ids) {
            s.vp.fit(b, size, 60.0);
        }
    }

    fn fit_selection(&mut self) {
        let size = self.canvas_size();
        let mut s = self.session.borrow_mut();
        let b: Option<Rect> = dv_geom::selection_bounds(s.exec.doc().page(), &self.selection);
        if let Some(b) = b {
            s.vp.fit(b, size, 80.0);
        }
    }
}
