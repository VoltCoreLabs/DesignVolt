//! Window-independent screen interface the app drives, plus shared resources.

use std::rc::Rc;

use dv_ui::icons::Icons;
use dv_ui::text::Font;
use vello::Scene;

pub use dv_ui::input::{Key, Mods};

/// Shared read-only resources.
pub struct Assets {
    pub font: Font,
    pub icons: Icons,
}

/// Logical viewport size and DPI scale.
#[derive(Clone, Copy)]
pub struct Viewport {
    pub w: f64,
    pub h: f64,
    pub scale: f64,
}

impl Viewport {
    pub fn new() -> Self {
        Self {
            w: 1180.0,
            h: 760.0,
            scale: 1.0,
        }
    }

    /// `w`/`h` are physical pixels.
    pub fn set(&mut self, w: u32, h: u32, scale: f64) {
        self.scale = scale;
        self.w = w as f64 / scale;
        self.h = h as f64 / scale;
    }
}

/// What a screen asks the app to do next.
#[derive(Debug, PartialEq, Eq)]
pub enum Transition {
    /// The name was entered on the welcome screen.
    SetName(String),
    OpenProject(u64),
    BackToProjects,
    OpenSettings,
}

/// Mouse cursor a screen wants to show.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cursor {
    Default,
    Pointer,
    Grab,
    Grabbing,
    Crosshair,
    Text,
    Move,
    ResizeNs,
    ResizeEw,
    ResizeNesw,
    ResizeNwse,
    /// Turning a shape: the editor draws its own arrow, the system cursor is hidden.
    Rotate,
}

pub trait Screen {
    fn set_viewport(&mut self, w: u32, h: u32, scale: f64);
    /// Advances animations; returns true while another frame is needed.
    fn update(&mut self, dt: f32) -> bool;
    fn draw(&self, scene: &mut Scene);
    /// Positions are physical pixels.
    fn mouse_moved(&mut self, x: f64, y: f64);
    fn mouse_left(&mut self);
    fn click(&mut self, mods: Mods);
    fn right_click(&mut self, _mods: Mods) {}
    fn scroll(&mut self, _dy: f64) {}
    /// Mouse wheel / touchpad scroll in logical px (`dy > 0` = content moves down).
    fn wheel(&mut self, _dx: f64, dy: f64, _mods: Mods) {
        self.scroll(dy);
    }
    fn middle_button(&mut self, _pressed: bool) {}
    fn key_up(&mut self, _key: Key) {}
    fn cursor(&self) -> Cursor {
        if self.pointer_hand() {
            Cursor::Pointer
        } else {
            Cursor::Default
        }
    }
    fn key(&mut self, _key: Key, _mods: Mods) {}
    fn mouse_up(&mut self) {}
    fn text(&mut self, _s: &str) {}
    fn pointer_hand(&self) -> bool;
    fn take_transition(&mut self) -> Option<Transition>;
}

pub type SharedAssets = Rc<Assets>;
