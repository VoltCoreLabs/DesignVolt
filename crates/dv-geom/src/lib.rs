//! Geometry for the document: transforms, bounds, exact hit-testing and a spatial index
//! (R-tree) for culling and picking.

#![forbid(unsafe_code)]

mod index;
mod snap;
mod transform;

pub use index::{Entry, SpatialIndex};
pub use snap::{Axis, Edges, Guide, Snap, snap_edges, snap_rect};
pub use transform::{
    geometry_under, hit_local, local_affine, parent_delta, selection_bounds, to_parent_space,
    world_affine, world_bounds, world_corners,
};
