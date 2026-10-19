//! Node transforms and bounds.

use dv_core::{Geometry, Node, NodeId, NodeKind, Page};
use kurbo::{Affine, Point, Rect, Vec2};

/// Maps node-local space (origin at the node's top-left) into the parent's space; the
/// rotation is around the node's center.
pub fn local_affine(g: &Geometry) -> Affine {
    let c = Vec2::new(g.w / 2.0, g.h / 2.0);
    Affine::translate((g.x, g.y))
        * Affine::translate(c)
        * Affine::rotate(g.rotation.to_radians())
        * Affine::translate(-c)
}

/// Maps node-local space into page space (through all ancestors).
pub fn world_affine(page: &Page, id: NodeId) -> Affine {
    let mut chain = Vec::new();
    let mut cur = Some(id);
    while let Some(c) = cur {
        let Some(n) = page.node(c) else { break };
        chain.push(local_affine(&n.geom));
        cur = n.parent;
    }
    chain.iter().rev().fold(Affine::IDENTITY, |acc, a| acc * *a)
}

/// Axis-aligned bounds of the node in page space (covers rotation).
pub fn world_bounds(page: &Page, id: NodeId) -> Option<Rect> {
    let node = page.node(id)?;
    Some(bounds_of(world_affine(page, id), &node.geom))
}

pub(crate) fn bounds_of(world: Affine, g: &Geometry) -> Rect {
    let corners = [
        Point::new(0.0, 0.0),
        Point::new(g.w, 0.0),
        Point::new(g.w, g.h),
        Point::new(0.0, g.h),
    ]
    .map(|p| world * p);
    let (mut x0, mut y0) = (f64::MAX, f64::MAX);
    let (mut x1, mut y1) = (f64::MIN, f64::MIN);
    for p in corners {
        x0 = x0.min(p.x);
        y0 = y0.min(p.y);
        x1 = x1.max(p.x);
        y1 = y1.max(p.y);
    }
    Rect::new(x0, y0, x1, y1)
}

/// Whether the node-local point `p` lies inside the node's shape.
pub fn hit_local(node: &Node, p: Point) -> bool {
    let g = &node.geom;
    match node.kind {
        NodeKind::Frame | NodeKind::Rectangle => {
            p.x >= 0.0 && p.y >= 0.0 && p.x <= g.w && p.y <= g.h
        }
        NodeKind::Ellipse => {
            let (rx, ry) = (g.w / 2.0, g.h / 2.0);
            if rx <= 0.0 || ry <= 0.0 {
                return false;
            }
            let (dx, dy) = ((p.x - rx) / rx, (p.y - ry) / ry);
            dx * dx + dy * dy <= 1.0
        }
    }
}

/// Union of the bounds of `ids` (unknown ids are skipped).
pub fn selection_bounds(page: &Page, ids: &[NodeId]) -> Option<Rect> {
    ids.iter()
        .filter_map(|id| world_bounds(page, *id))
        .reduce(|a, b| a.union(b))
}
