//! Draws a document page into a Vello scene: viewport (zoom/pan), culling through the spatial
//! index, and level-of-detail skipping of nodes that are too small to see.

#![forbid(unsafe_code)]

mod effects;
mod gradient;
mod gradient_handles;
mod grid;
pub mod image;
mod paint;
mod pattern;
mod viewport;

pub use gradient::{fill_paint, linear_line};
pub use gradient_handles::{GradHandle, drag as drag_gradient_handle, handles as gradient_handles};
pub use grid::{GridDots, grid_dots, grid_step, paint_dots};
pub use paint::{PaintStats, paint_page};
pub use viewport::{MAX_ZOOM, MIN_ZOOM, Viewport};
