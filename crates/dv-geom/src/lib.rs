//! Geometry for the document: transforms, bounds, exact hit-testing and a spatial index
//! (R-tree) for culling and picking.

#![forbid(unsafe_code)]

mod index;
mod transform;

pub use index::{Entry, SpatialIndex};
pub use transform::{hit_local, local_affine, selection_bounds, world_affine, world_bounds};
