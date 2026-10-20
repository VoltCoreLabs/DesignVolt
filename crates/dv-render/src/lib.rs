//! Draws a document page into a Vello scene: viewport (zoom/pan), culling through the spatial
//! index, and level-of-detail skipping of nodes that are too small to see.

#![forbid(unsafe_code)]

mod gradient;
mod grid;
mod paint;
mod viewport;

pub use gradient::{fill_paint, linear_line};
pub use grid::{GridDots, grid_dots, grid_step, paint_dots};
pub use paint::{PaintStats, paint_page};
pub use viewport::{MAX_ZOOM, MIN_ZOOM, Viewport};
