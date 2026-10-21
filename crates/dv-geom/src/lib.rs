//! Geometry for the document: transforms, bounds, exact hit-testing and a spatial index
//! (R-tree) for culling and picking.

#![forbid(unsafe_code)]

mod index;
mod measure;
mod snap;
mod transform;
mod vector_path;

pub use index::{Entry, SpatialIndex};
pub use measure::{Measure, measure};
pub use snap::{Axis, Edges, Guide, Snap, snap_edges, snap_rect, snap_to_lines};
pub use transform::{
    geometry_under, hit_local, local_affine, parent_delta, selection_bounds, to_parent_space,
    world_affine, world_bounds, world_corners,
};
pub use vector_path::{anchors_bounds, hit_path, vector_path};
