//! R-tree over the world bounds of all visible nodes, with their drawing order.

use dv_core::{NodeId, Page};
use kurbo::{Affine, Point, Rect};
use rstar::{AABB, RTree, RTreeObject};

use crate::transform::{bounds_of, hit_local, local_affine, world_affine};

#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub id: NodeId,
    /// Position in drawing order: a bigger value is drawn later (on top).
    pub order: u32,
    pub bounds: Rect,
    /// Node space → page space (cached at build time).
    pub world: Affine,
}

impl RTreeObject for Entry {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        AABB::from_corners(
            [self.bounds.x0, self.bounds.y0],
            [self.bounds.x1, self.bounds.y1],
        )
    }
}

pub struct SpatialIndex {
    tree: RTree<Entry>,
    revision: u64,
    /// The farthest any stroke or effect reaches past a node box (page units).
    max_reach: f64,
}

impl SpatialIndex {
    /// Indexes every visible node (a hidden node hides its subtree).
    pub fn build(page: &Page, revision: u64) -> Self {
        let mut entries = Vec::with_capacity(page.len());
        let mut max_reach: f64 = 0.0;
        let mut stack: Vec<(dv_core::NodeId, Affine)> = page
            .roots()
            .iter()
            .rev()
            .map(|id| (*id, Affine::IDENTITY))
            .collect();
        while let Some((id, parent)) = stack.pop() {
            let Some(node) = page.node(id) else { continue };
            if !node.visible {
                continue;
            }
            let world = parent * local_affine(&node.geom);
            entries.push(Entry {
                id,
                order: entries.len() as u32,
                bounds: bounds_of(world, &node.geom),
                world,
            });
            max_reach = max_reach.max(node.effect_reach());
            stack.extend(node.children.iter().rev().map(|c| (*c, world)));
        }
        Self {
            tree: RTree::bulk_load(entries),
            revision,
            max_reach,
        }
    }

    /// How far strokes and effects reach past node boxes: widen a visibility query by this.
    pub fn max_reach(&self) -> f64 {
        self.max_reach
    }

    /// The document revision this index was built for.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn len(&self) -> usize {
        self.tree.size()
    }

    pub fn is_empty(&self) -> bool {
        self.tree.size() == 0
    }

    /// Entries whose bounds intersect `rect`, in no particular order.
    pub fn query(&self, rect: Rect) -> impl Iterator<Item = &Entry> {
        let env = AABB::from_corners([rect.x0, rect.y0], [rect.x1, rect.y1]);
        self.tree.locate_in_envelope_intersecting(env)
    }

    /// The topmost unlocked node whose shape contains the page-space point.
    pub fn pick(&self, page: &Page, p: Point) -> Option<NodeId> {
        let at = AABB::from_point([p.x, p.y]);
        self.tree
            .locate_in_envelope_intersecting(at)
            .filter(|e| exact_hit(page, e, p))
            .max_by_key(|e| e.order)
            .map(|e| e.id)
    }

    /// Like `pick`, but only considers nodes accepted by `accept` (for example frames only).
    pub fn pick_where(
        &self,
        page: &Page,
        p: Point,
        accept: impl Fn(&dv_core::Node) -> bool,
    ) -> Option<NodeId> {
        self.pick_where_id(page, p, |_, n| accept(n))
    }

    /// Like `pick_where`, with the candidate's id available to `accept`.
    pub fn pick_where_id(
        &self,
        page: &Page,
        p: Point,
        accept: impl Fn(NodeId, &dv_core::Node) -> bool,
    ) -> Option<NodeId> {
        let at = AABB::from_point([p.x, p.y]);
        self.tree
            .locate_in_envelope_intersecting(at)
            .filter(|e| page.node(e.id).is_some_and(|n| accept(e.id, n)) && exact_hit(page, e, p))
            .max_by_key(|e| e.order)
            .map(|e| e.id)
    }

    /// Unlocked nodes whose bounds intersect `rect` (marquee selection), bottom to top.
    pub fn in_rect(&self, page: &Page, rect: Rect) -> Vec<NodeId> {
        let mut hits: Vec<&Entry> = self
            .query(rect)
            .filter(|e| page.node(e.id).is_some_and(|n| !n.locked))
            .collect();
        hits.sort_by_key(|e| e.order);
        hits.into_iter().map(|e| e.id).collect()
    }
}

fn exact_hit(page: &Page, e: &Entry, p: Point) -> bool {
    let Some(node) = page.node(e.id) else {
        return false;
    };
    if node.locked {
        return false;
    }
    let local = world_affine(page, e.id).inverse() * p;
    hit_local(node, local)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dv_core::{Command, Document, Geometry, Node, NodeKind, Props};

    fn add(doc: &mut Document, kind: NodeKind, parent: Option<NodeId>, g: Geometry) -> NodeId {
        let id = doc.alloc_id();
        let index = doc.page().siblings(parent).len();
        doc.apply(&Command::Create {
            id,
            parent,
            index,
            node: Node::new(kind, "n", g),
        })
        .unwrap();
        id
    }

    fn g(x: f64, y: f64, w: f64, h: f64) -> Geometry {
        Geometry {
            x,
            y,
            w,
            h,
            rotation: 0.0,
        }
    }

    fn build(doc: &Document) -> SpatialIndex {
        SpatialIndex::build(doc.page(), doc.revision())
    }

    #[test]
    fn pick_returns_the_topmost_hit() {
        let mut doc = Document::default();
        let _below = add(
            &mut doc,
            NodeKind::Rectangle,
            None,
            g(0.0, 0.0, 100.0, 100.0),
        );
        let above = add(
            &mut doc,
            NodeKind::Rectangle,
            None,
            g(50.0, 50.0, 100.0, 100.0),
        );
        let idx = build(&doc);
        assert_eq!(idx.pick(doc.page(), Point::new(60.0, 60.0)), Some(above));
        assert!(idx.pick(doc.page(), Point::new(500.0, 500.0)).is_none());
    }

    #[test]
    fn ellipse_corners_do_not_hit() {
        let mut doc = Document::default();
        let e = add(&mut doc, NodeKind::Ellipse, None, g(0.0, 0.0, 100.0, 100.0));
        let idx = build(&doc);
        assert_eq!(idx.pick(doc.page(), Point::new(50.0, 50.0)), Some(e));
        assert!(
            idx.pick(doc.page(), Point::new(2.0, 2.0)).is_none(),
            "bounding-box corner"
        );
    }

    #[test]
    fn rotated_nodes_are_hit_in_their_rotated_shape() {
        let mut doc = Document::default();
        let r = add(
            &mut doc,
            NodeKind::Rectangle,
            None,
            Geometry {
                rotation: 90.0,
                ..g(0.0, 0.0, 100.0, 20.0)
            },
        );
        let idx = build(&doc);
        // Rotated about its center (50, 10): it now covers x 40..60 and y -40..60.
        assert_eq!(idx.pick(doc.page(), Point::new(50.0, -30.0)), Some(r));
        assert!(idx.pick(doc.page(), Point::new(10.0, 10.0)).is_none());
    }

    #[test]
    fn locked_and_hidden_nodes_cannot_be_picked() {
        let mut doc = Document::default();
        let a = add(&mut doc, NodeKind::Rectangle, None, g(0.0, 0.0, 50.0, 50.0));
        let b = add(&mut doc, NodeKind::Rectangle, None, g(0.0, 0.0, 50.0, 50.0));
        let lock = Props {
            locked: Some(true),
            ..Props::default()
        };
        doc.apply(&Command::SetProps { id: b, props: lock })
            .unwrap();
        assert_eq!(
            build(&doc).pick(doc.page(), Point::new(10.0, 10.0)),
            Some(a)
        );
        let hide = Props {
            visible: Some(false),
            ..Props::default()
        };
        doc.apply(&Command::SetProps { id: a, props: hide })
            .unwrap();
        let idx = build(&doc);
        assert!(idx.pick(doc.page(), Point::new(10.0, 10.0)).is_none());
        assert_eq!(idx.len(), 1, "hidden node is not indexed");
    }

    #[test]
    fn children_use_the_parent_space_and_hide_with_it() {
        let mut doc = Document::default();
        let frame = add(
            &mut doc,
            NodeKind::Frame,
            None,
            g(100.0, 100.0, 300.0, 300.0),
        );
        let child = add(
            &mut doc,
            NodeKind::Rectangle,
            Some(frame),
            g(10.0, 10.0, 50.0, 50.0),
        );
        let idx = build(&doc);
        assert_eq!(idx.pick(doc.page(), Point::new(120.0, 120.0)), Some(child));
        let b = crate::world_bounds(doc.page(), child).unwrap();
        assert_eq!((b.x0, b.y0, b.x1, b.y1), (110.0, 110.0, 160.0, 160.0));

        let hide = Props {
            visible: Some(false),
            ..Props::default()
        };
        doc.apply(&Command::SetProps {
            id: frame,
            props: hide,
        })
        .unwrap();
        assert!(
            build(&doc).is_empty(),
            "children of a hidden frame are hidden too"
        );
    }

    #[test]
    fn query_and_marquee_find_intersecting_nodes_in_z_order() {
        let mut doc = Document::default();
        let a = add(&mut doc, NodeKind::Rectangle, None, g(0.0, 0.0, 10.0, 10.0));
        let far = add(
            &mut doc,
            NodeKind::Rectangle,
            None,
            g(1000.0, 1000.0, 10.0, 10.0),
        );
        let b = add(&mut doc, NodeKind::Rectangle, None, g(5.0, 5.0, 10.0, 10.0));
        let idx = build(&doc);
        assert_eq!(idx.query(Rect::new(0.0, 0.0, 20.0, 20.0)).count(), 2);
        assert_eq!(
            idx.in_rect(doc.page(), Rect::new(0.0, 0.0, 20.0, 20.0)),
            vec![a, b]
        );
        assert_eq!(
            idx.in_rect(doc.page(), Rect::new(900.0, 900.0, 2000.0, 2000.0)),
            vec![far]
        );
    }

    #[test]
    fn selection_bounds_unions_nodes() {
        let mut doc = Document::default();
        let a = add(&mut doc, NodeKind::Rectangle, None, g(0.0, 0.0, 10.0, 10.0));
        let b = add(
            &mut doc,
            NodeKind::Rectangle,
            None,
            g(20.0, 30.0, 10.0, 10.0),
        );
        let r = crate::selection_bounds(doc.page(), &[a, b]).unwrap();
        assert_eq!((r.x0, r.y0, r.x1, r.y1), (0.0, 0.0, 30.0, 40.0));
        assert!(crate::selection_bounds(doc.page(), &[]).is_none());
    }

    #[test]
    fn index_remembers_the_revision_it_was_built_for() {
        let mut doc = Document::default();
        add(&mut doc, NodeKind::Rectangle, None, g(0.0, 0.0, 1.0, 1.0));
        let idx = build(&doc);
        assert_eq!(idx.revision(), doc.revision());
    }
}
