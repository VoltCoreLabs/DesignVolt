//! Motion tokens: spring parameters and durations (seconds).
//! Damping is at least critical (2 * sqrt(stiffness)), so hovers settle without overshoot.

pub const HOVER_STIFFNESS: f32 = 220.0;
pub const HOVER_DAMPING: f32 = 30.0;
pub const PRESS_STIFFNESS: f32 = 400.0;
pub const PRESS_DAMPING: f32 = 40.0;
pub const MOVE_STIFFNESS: f32 = 300.0;
pub const MOVE_DAMPING: f32 = 35.0;

pub const PANEL_INTRO: f32 = 0.45;
pub const STAGGER: f32 = 0.07;
