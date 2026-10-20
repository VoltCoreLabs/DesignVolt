//! Dragging rows of the layers panel: reorder, or drop into a frame.

use dv_core::{Command, Executor, NodeId, NodeKind};
use kurbo::Rect;

use super::panels;
use super::state::Editor;

/// Pointer moves smaller than this (px) are still a click on the row.
pub const DRAG_START_PX: f64 = 4.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Zone {
    /// Above the row: on top of it in z-order.
    Above,
    /// Below the row: right under it in z-order.
    Below,
    /// Inside the frame of the row.
    Into,
}

/// Where in a row the pointer is: the edges place a shape next to the row, the middle of a
/// frame row puts it inside.
pub fn zone(row: Rect, y: f64, can_nest: bool) -> Zone {
    let t = ((y - row.y0) / row.height()).clamp(0.0, 1.0);
    if can_nest && (0.25..0.75).contains(&t) {
        Zone::Into
    } else if t < 0.5 {
        Zone::Above
    } else {
        Zone::Below
    }
}

/// Moves `ids` under `parent` so that they start at `index` of its current children, as
/// one undoable step; shapes keep their place on the page. Returns whether anything changed.
pub fn move_to(ex: &mut Executor, ids: &[NodeId], parent: Option<NodeId>, index: usize) -> bool {
    let page = ex.doc().page();
    let tops = page.without_descendants(ids);
    let moving: Vec<NodeId> = page
        .draw_order()
        .into_iter()
        .filter(|id| tops.contains(id))
        .collect();
    if moving.is_empty() || parent.is_some_and(|p| moving.iter().any(|m| page.is_inside(p, *m))) {
        return false;
    }
    let siblings = page.siblings(parent);
    let before = siblings
        .iter()
        .take(index)
        .filter(|s| moving.contains(s))
        .count();
    let at = index.saturating_sub(before);
    let rest: Vec<NodeId> = siblings
        .iter()
        .copied()
        .filter(|s| !moving.contains(s))
        .collect();
    let split = at.min(rest.len());
    let mut want: Vec<NodeId> = rest[..split].to_vec();
    want.extend(&moving);
    want.extend(&rest[split..]);
    let same_parent = moving
        .iter()
        .all(|m| page.node(*m).is_some_and(|n| n.parent == parent));
    if same_parent && want == siblings {
        return false;
    }
    let mut cmds = Vec::new();
    for (k, id) in moving.iter().enumerate() {
        let geom = dv_geom::geometry_under(page, *id, parent);
        cmds.push(Command::Reparent {
            id: *id,
            parent,
            index: split + k,
        });
        if let Some(geom) = geom {
            cmds.push(Command::SetGeometry { id: *id, geom });
        }
    }
    ex.execute("Move layer", Command::Batch(cmds)).is_ok()
}

impl Editor {
    /// The row and zone the dragged layers would be dropped on, for the current pointer.
    pub(super) fn layer_drop(&self) -> Option<(usize, Zone)> {
        let left = self.layout().left;
        let rows = self.layer_rows(panels::max_layer_rows(left));
        let i = (0..rows.len()).find(|i| panels::layer_row_rect(left, *i).contains(self.mouse))?;
        let rect = panels::layer_row_rect(left, i);
        let s = self.session.borrow();
        let page = s.exec.doc().page();
        let nest = rows[i].kind == NodeKind::Frame
            && !self
                .selection
                .iter()
                .any(|m| page.is_inside(rows[i].id, *m));
        Some((i, zone(rect, self.mouse.y, nest)))
    }

    /// Drops the selected layers where the pointer is.
    pub(super) fn finish_layer_drag(&mut self) {
        let Some((i, z)) = self.layer_drop() else {
            return;
        };
        let rows = self.layer_rows(panels::max_layer_rows(self.layout().left));
        let mut s = self.session.borrow_mut();
        let page = s.exec.doc().page();
        let Some(node) = page.node(rows[i].id) else {
            return;
        };
        let pos = page
            .siblings(node.parent)
            .iter()
            .position(|x| *x == rows[i].id)
            .unwrap_or(0);
        let (parent, index) = match z {
            Zone::Into => (Some(rows[i].id), node.children.len()),
            Zone::Above => (node.parent, pos + 1),
            Zone::Below => (node.parent, pos),
        };
        move_to(&mut s.exec, &self.selection, parent, index);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dv_core::{Document, Geometry};

    fn mk(ex: &mut Executor, kind: NodeKind, parent: Option<NodeId>) -> NodeId {
        let g = Geometry {
            x: 10.0,
            y: 10.0,
            w: 20.0,
            h: 20.0,
            rotation: 0.0,
        };
        super::super::ops::create(ex, kind, "n".into(), parent, g).unwrap()
    }

    #[test]
    fn zones_split_a_row() {
        let r = Rect::new(0.0, 0.0, 100.0, 40.0);
        assert_eq!(zone(r, 2.0, true), Zone::Above);
        assert_eq!(zone(r, 20.0, true), Zone::Into);
        assert_eq!(zone(r, 38.0, true), Zone::Below);
        assert_eq!(zone(r, 12.0, false), Zone::Above);
        assert_eq!(zone(r, 25.0, false), Zone::Below);
    }

    #[test]
    fn reorders_among_siblings_and_ignores_no_ops() {
        let mut ex = Executor::new(Document::default());
        let ids: Vec<_> = (0..4)
            .map(|_| mk(&mut ex, NodeKind::Rectangle, None))
            .collect();
        assert!(move_to(&mut ex, &[ids[0]], None, 4));
        assert_eq!(ex.doc().page().roots(), [ids[1], ids[2], ids[3], ids[0]]);
        assert!(!move_to(&mut ex, &[ids[0]], None, 4));
        assert!(!move_to(&mut ex, &[ids[0]], None, 3));
        ex.undo();
        assert_eq!(ex.doc().page().roots(), &ids[..]);
    }

    #[test]
    fn moves_into_a_frame_keeping_the_place_and_refuses_cycles() {
        let mut ex = Executor::new(Document::default());
        let f = mk(&mut ex, NodeKind::Frame, None);
        let r = mk(&mut ex, NodeKind::Rectangle, None);
        assert!(move_to(&mut ex, &[r], Some(f), 0));
        let n = ex.doc().page().node(r).unwrap();
        assert_eq!((n.parent, n.geom.x), (Some(f), 0.0));
        let c = mk(&mut ex, NodeKind::Frame, Some(f));
        assert!(!move_to(&mut ex, &[f], Some(c), 0));
    }
}
