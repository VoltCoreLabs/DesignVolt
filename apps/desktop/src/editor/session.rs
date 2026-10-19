//! A document being edited, kept in memory for the whole app session so that leaving the
//! editor and coming back does not lose the work (saving to disk comes later, see roadmap M6).

use dv_core::{Document, Executor};
use dv_render::Viewport;

pub struct Session {
    pub exec: Executor,
    pub vp: Viewport,
    /// The page origin has been moved to the middle of the canvas once.
    pub centered: bool,
}

impl Session {
    pub fn new(name: &str) -> Self {
        Self {
            exec: Executor::new(Document::new(name)),
            vp: Viewport::default(),
            centered: false,
        }
    }
}
