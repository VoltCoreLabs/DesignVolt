//! Window-independent input events and the `Screen` interface the app drives.

use std::rc::Rc;

use vello::Scene;

use crate::icons::Icons;
use crate::text::Font;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    Backspace,
    Delete,
    Left,
    Right,
    Home,
    End,
    Enter,
    Escape,
    SelectAll,
}

/// Shared read-only resources.
#[derive(Clone, Copy, Default, Debug)]
pub struct Mods {
    pub ctrl: bool,
    pub shift: bool,
}

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
        Self { w: 1180.0, h: 760.0, scale: 1.0 }
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
}

pub trait Screen {
    fn set_viewport(&mut self, w: u32, h: u32, scale: f64);
    /// Advances animations; returns true while another frame is needed.
    fn update(&mut self, dt: f32) -> bool;
    fn draw(&self, scene: &mut Scene);
    /// Positions are physical pixels.
    fn mouse_moved(&mut self, x: f64, y: f64);
    fn mouse_left(&mut self);
    fn click(&mut self);
    fn scroll(&mut self, _dy: f64) {}
    fn key(&mut self, _key: Key, _mods: Mods) {}
    fn mouse_up(&mut self) {}
    fn text(&mut self, _s: &str) {}
    fn pointer_hand(&self) -> bool;
    fn take_transition(&mut self) -> Option<Transition>;
}

pub type SharedAssets = Rc<Assets>;
