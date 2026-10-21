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
        NodeKind::Group => false,
        NodeKind::Path => crate::vector_path::hit_path(node, p),
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

/// The four corners of the node's own outline in page space, clockwise from the top-left of
/// the unrotated shape: the rotated rectangle the user actually sees.
pub fn world_corners(page: &Page, id: NodeId) -> Option<[Point; 4]> {
    let g = page.node(id)?.geom;
    let m = world_affine(page, id);
    Some([
        m * Point::new(0.0, 0.0),
        m * Point::new(g.w, 0.0),
        m * Point::new(g.w, g.h),
        m * Point::new(0.0, g.h),
    ])
}

/// Union of the bounds of `ids` (unknown ids are skipped).
pub fn selection_bounds(page: &Page, ids: &[NodeId]) -> Option<Rect> {
    page.geo_tops(ids)
        .iter()
        .filter_map(|id| world_bounds(page, *id))
        .reduce(|a, b| a.union(b))
}

/// Converts a page-space displacement into the coordinate system of the node's parent, so it
/// can be added to the node's `x`/`y` (matters when a parent is rotated).
pub fn parent_delta(page: &Page, id: NodeId, delta: Vec2) -> Vec2 {
    let parent = page.node(id).and_then(|n| n.parent);
    match parent {
        None => delta,
        Some(p) => {
            let inv = world_affine(page, p).inverse();
            let origin = inv * Point::ZERO;
            inv * Point::new(delta.x, delta.y) - origin
        }
    }
}

/// Converts a page-space point into the node's parent space (`None` parent = the page).
pub fn to_parent_space(page: &Page, parent: Option<NodeId>, p: Point) -> Point {
    match parent {
        None => p,
        Some(par) => world_affine(page, par).inverse() * p,
    }
}

/// Geometry that keeps node `id` exactly where it is on the page after it moves under
/// `new_parent` (`None` = the page): same position, same visual rotation.
pub fn geometry_under(page: &Page, id: NodeId, new_parent: Option<NodeId>) -> Option<Geometry> {
    let g = page.node(id)?.geom;
    let world = world_affine(page, id);
    let inv_parent = match new_parent {
        Some(p) => world_affine(page, p).inverse(),
        None => Affine::IDENTITY,
    };
    let m = inv_parent * world;
    let center = m * Point::new(g.w / 2.0, g.h / 2.0);
    let c = m.as_coeffs();
    Some(Geometry {
        x: center.x - g.w / 2.0,
        y: center.y - g.h / 2.0,
        rotation: c[1].atan2(c[0]).to_degrees(),
        ..g
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use dv_core::{Command, Document, Node};

    fn add(doc: &mut Document, parent: Option<NodeId>, g: Geometry) -> NodeId {
        let id = doc.alloc_id();
        let index = doc.page().siblings(parent).len();
        doc.apply(&Command::Create {
            id,
            parent,
            index,
            node: Node::new(NodeKind::Frame, "n", g),
        })
        .unwrap();
        id
    }

    fn g(x: f64, y: f64, w: f64, h: f64, rotation: f64) -> Geometry {
        Geometry {
            x,
            y,
            w,
            h,
            rotation,
        }
    }

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn rotation_is_around_the_center() {
        let g = g(0.0, 0.0, 10.0, 20.0, 90.0);
        let r = bounds_of(local_affine(&g), &g);
        assert!(near(r.x0, -5.0) && near(r.x1, 15.0) && near(r.y0, 5.0) && near(r.y1, 15.0));
    }

    #[test]
    fn corners_follow_the_rotated_outline() {
        let mut doc = Document::default();
        let a = add(&mut doc, None, g(0.0, 0.0, 200.0, 100.0, 90.0));
        let c = world_corners(doc.page(), a).unwrap();
        // A 200 x 100 box turned 90 degrees around its center (100, 50).
        let expect = [(150.0, -50.0), (150.0, 150.0), (50.0, 150.0), (50.0, -50.0)];
        for (p, (x, y)) in c.iter().zip(expect) {
            assert!(near(p.x, x) && near(p.y, y), "{p:?} vs ({x}, {y})");
        }
        assert!(world_corners(doc.page(), NodeId(99)).is_none());
    }
    #[test]
    fn geometry_under_keeps_the_world_position_and_rotation() {
        let mut doc = Document::default();
        let f = add(&mut doc, None, g(100.0, 50.0, 400.0, 400.0, 30.0));
        let a = add(&mut doc, None, g(200.0, 200.0, 40.0, 20.0, 10.0));
        let before = world_corners(doc.page(), a).unwrap();
        let ng = geometry_under(doc.page(), a, Some(f)).unwrap();
        assert!(near(ng.rotation, -20.0) && near(ng.w, 40.0));
        doc.apply(&dv_core::Command::Reparent {
            id: a,
            parent: Some(f),
            index: 0,
        })
        .unwrap();
        doc.apply(&dv_core::Command::SetGeometry { id: a, geom: ng })
            .unwrap();
        let after = world_corners(doc.page(), a).unwrap();
        for (p, q) in before.iter().zip(after) {
            assert!(near(p.x, q.x) && near(p.y, q.y), "{p:?} vs {q:?}");
        }
        let back = geometry_under(doc.page(), a, None).unwrap();
        assert!(near(back.x, 200.0) && near(back.y, 200.0) && near(back.rotation, 10.0));
    }

    #[test]
    fn page_level_delta_is_unchanged() {
        let mut doc = Document::default();
        let a = add(&mut doc, None, g(5.0, 5.0, 10.0, 10.0, 0.0));
        assert_eq!(
            parent_delta(doc.page(), a, Vec2::new(3.0, 4.0)),
            Vec2::new(3.0, 4.0)
        );
    }

    #[test]
    fn delta_inside_a_rotated_parent_is_rotated_back() {
        let mut doc = Document::default();
        let parent = add(&mut doc, None, g(0.0, 0.0, 100.0, 100.0, 90.0));
        let child = add(&mut doc, Some(parent), g(0.0, 0.0, 10.0, 10.0, 0.0));
        let d = parent_delta(doc.page(), child, Vec2::new(10.0, 0.0));
        // The parent is turned by 90 degrees, so a page-space move to the right is a move
        // "up" (negative y) in the parent's own space.
        assert!(near(d.x, 0.0) && near(d.y, -10.0), "{d:?}");
    }

    #[test]
    fn points_convert_into_the_parent_space() {
        let mut doc = Document::default();
        let parent = add(&mut doc, None, g(100.0, 50.0, 200.0, 200.0, 0.0));
        let p = to_parent_space(doc.page(), Some(parent), Point::new(130.0, 80.0));
        assert!(near(p.x, 30.0) && near(p.y, 30.0));
        assert_eq!(
            to_parent_space(doc.page(), None, Point::new(1.0, 2.0)),
            Point::new(1.0, 2.0)
        );
    }
}
