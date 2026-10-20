//! Dropping shapes into a frame (or out of it) after a drag, like in Figma.

use dv_core::{Command, Executor, NodeId, NodeKind};
use kurbo::Point;

use super::state::Editor;

/// Moves each of `ids` under `parent` (top of its children), keeping its place on the page,
/// as one undoable step. Nodes already there, or that would end up inside themselves, stay.
/// Returns whether anything moved.
pub fn reparent(ex: &mut Executor, ids: &[NodeId], parent: Option<NodeId>) -> bool {
    let page = ex.doc().page();
    let mut cmds = Vec::new();
    for id in page.without_descendants(ids) {
        let Some(node) = page.node(id) else { continue };
        if node.parent == parent || parent.is_some_and(|p| page.is_inside(p, id)) {
            continue;
        }
        let Some(geom) = dv_geom::geometry_under(page, id, parent) else {
            continue;
        };
        let index = page.siblings(parent).len() + cmds.len();
        cmds.push(Command::Reparent { id, parent, index });
        cmds.push(Command::SetGeometry { id, geom });
    }
    if cmds.is_empty() {
        return false;
    }
    ex.execute("Move into frame", Command::Batch(cmds)).is_ok()
}

impl Editor {
    /// After a move drag: puts the selection into the frame under the pointer, or onto the
    /// page when the pointer is over no frame.
    pub(super) fn drop_selection(&mut self, at: Point) {
        self.refresh_index();
        let mut s = self.session.borrow_mut();
        let page = s.exec.doc().page();
        let target = self.index.pick_where_id(page, at, |id, n| {
            n.kind == NodeKind::Frame
                && !n.locked
                && !self.selection.iter().any(|sel| page.is_inside(id, *sel))
        });
        reparent(&mut s.exec, &self.selection, target);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dv_core::{Document, Geometry};

    fn mk(ex: &mut Executor, kind: NodeKind, parent: Option<NodeId>, x: f64) -> NodeId {
        let g = Geometry {
            x,
            y: x,
            w: 20.0,
            h: 20.0,
            rotation: 0.0,
        };
        super::super::ops::create(ex, kind, "n".into(), parent, g).unwrap()
    }

    #[test]
    fn moving_in_and_out_keeps_the_page_position_and_is_one_step() {
        let mut ex = Executor::new(Document::default());
        let f = mk(&mut ex, NodeKind::Frame, None, 100.0);
        let r = mk(&mut ex, NodeKind::Rectangle, None, 130.0);
        assert!(reparent(&mut ex, &[r], Some(f)));
        let n = ex.doc().page().node(r).unwrap();
        assert_eq!((n.parent, n.geom.x, n.geom.y), (Some(f), 30.0, 30.0));
        assert!(!reparent(&mut ex, &[r], Some(f)));
        assert!(reparent(&mut ex, &[r], None));
        assert_eq!(ex.doc().page().node(r).unwrap().geom.x, 130.0);
        ex.undo();
        assert_eq!(ex.doc().page().node(r).unwrap().parent, Some(f));
        ex.undo();
        assert_eq!(ex.doc().page().node(r).unwrap().parent, None);
    }

    #[test]
    fn a_frame_cannot_enter_itself_or_its_child() {
        let mut ex = Executor::new(Document::default());
        let f = mk(&mut ex, NodeKind::Frame, None, 0.0);
        let c = mk(&mut ex, NodeKind::Frame, Some(f), 5.0);
        assert!(!reparent(&mut ex, &[f], Some(c)));
        assert!(!reparent(&mut ex, &[f], Some(f)));
    }
}
