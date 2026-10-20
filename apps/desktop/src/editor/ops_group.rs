//! Grouping shapes (Ctrl+G) and ungrouping (Ctrl+Shift+G).

use dv_core::{Command, Executor, Geometry, NodeId, NodeKind};
use dv_geom::geometry_under;

use super::ops::{new_node, next_name};

/// Wraps the shapes in a new group placed where the topmost of them was. Returns the group.
pub fn group(ex: &mut Executor, ids: &[NodeId], name: &str) -> Option<NodeId> {
    let page = ex.doc().page();
    let picked = page.without_descendants(ids);
    // Bottom to top, so the stacking inside the group stays as it was.
    let tops: Vec<NodeId> = page
        .draw_order()
        .into_iter()
        .filter(|id| picked.contains(id))
        .collect();
    let last = *tops.last()?;
    let parent = page.node(last)?.parent;
    let siblings = page.siblings(parent);
    let at = siblings
        .iter()
        .position(|s| *s == last)
        .map_or(siblings.len(), |p| p + 1);
    let node = new_node(NodeKind::Group, next_name(page, name), Geometry::default());
    let gid = ex.alloc_id();
    let page = ex.doc().page();
    let mut cmds = vec![Command::Create {
        id: gid,
        parent,
        index: at,
        node,
    }];
    for (k, id) in tops.iter().enumerate() {
        // The group has no transform of its own: what is right under `parent` is right here.
        let geom = geometry_under(page, *id, parent)?;
        cmds.push(Command::Reparent {
            id: *id,
            parent: Some(gid),
            index: k,
        });
        cmds.push(Command::SetGeometry { id: *id, geom });
    }
    ex.execute("Group", Command::Batch(cmds)).ok().map(|()| gid)
}

/// Dissolves the groups among `ids`; the shapes stay where they were, in the place of the
/// group. Returns the freed shapes.
pub fn ungroup(ex: &mut Executor, ids: &[NodeId]) -> Vec<NodeId> {
    let page = ex.doc().page();
    let mut cmds = Vec::new();
    let mut freed = Vec::new();
    for gid in page.without_descendants(ids) {
        let Some(g) = page.node(gid).filter(|n| n.kind == NodeKind::Group) else {
            continue;
        };
        let at = page
            .siblings(g.parent)
            .iter()
            .position(|s| *s == gid)
            .unwrap_or(0);
        for (k, c) in g.children.iter().enumerate() {
            let Some(geom) = geometry_under(page, *c, g.parent) else {
                continue;
            };
            cmds.push(Command::Reparent {
                id: *c,
                parent: g.parent,
                index: at + 1 + k,
            });
            cmds.push(Command::SetGeometry { id: *c, geom });
            freed.push(*c);
        }
        cmds.push(Command::Delete { id: gid });
    }
    if !cmds.is_empty() {
        let _ = ex.execute("Ungroup", Command::Batch(cmds));
    }
    freed
}

#[cfg(test)]
mod tests {
    use super::*;
    use dv_core::Document;

    fn mk(ex: &mut Executor, kind: NodeKind, parent: Option<NodeId>, x: f64) -> NodeId {
        let g = Geometry {
            x,
            y: x,
            w: 10.0,
            h: 10.0,
            rotation: 0.0,
        };
        super::super::ops::create(ex, kind, "n".into(), parent, g).unwrap()
    }

    #[test]
    fn group_wraps_in_place_and_undoes_in_one_step() {
        let mut ex = Executor::new(Document::default());
        let a = mk(&mut ex, NodeKind::Rectangle, None, 0.0);
        let b = mk(&mut ex, NodeKind::Rectangle, None, 50.0);
        let c = mk(&mut ex, NodeKind::Rectangle, None, 90.0);
        let g = group(&mut ex, &[b, a], "Group").unwrap();
        let page = ex.doc().page();
        assert_eq!(page.roots(), [g, c]);
        assert_eq!(page.siblings(Some(g)), [a, b]);
        assert_eq!(page.node(g).unwrap().name, "Group 1");
        assert_eq!(page.node(b).unwrap().geom.x, 50.0);
        ex.undo();
        assert_eq!(ex.doc().page().roots(), [a, b, c]);
    }

    #[test]
    fn ungroup_frees_the_shapes_in_place_after_the_group_moved() {
        let mut ex = Executor::new(Document::default());
        let a = mk(&mut ex, NodeKind::Rectangle, None, 0.0);
        let b = mk(&mut ex, NodeKind::Rectangle, None, 50.0);
        let c = mk(&mut ex, NodeKind::Rectangle, None, 90.0);
        let g = group(&mut ex, &[a, b], "Group").unwrap();
        super::super::ops::nudge(&mut ex, &[g], 5.0, 0.0);
        let freed = ungroup(&mut ex, &[g]);
        assert_eq!(freed, [a, b]);
        let page = ex.doc().page();
        assert_eq!(page.roots(), [a, b, c]);
        assert_eq!(page.node(b).unwrap().geom.x, 55.0);
        assert!(!page.contains(g));
        ex.undo();
        assert!(ex.doc().page().contains(g));
    }

    #[test]
    fn grouping_a_shape_inside_a_frame_keeps_it_in_that_frame() {
        let mut ex = Executor::new(Document::default());
        let f = mk(&mut ex, NodeKind::Frame, None, 100.0);
        let r = mk(&mut ex, NodeKind::Rectangle, Some(f), 10.0);
        let g = group(&mut ex, &[r], "Group").unwrap();
        let page = ex.doc().page();
        assert_eq!(page.node(g).unwrap().parent, Some(f));
        assert_eq!(page.node(r).unwrap().parent, Some(g));
        assert_eq!(page.node(r).unwrap().geom.x, 10.0);
    }
}
