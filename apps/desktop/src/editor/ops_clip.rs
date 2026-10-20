//! In-app clipboard for shapes: copy keeps subtree snapshots, paste creates fresh copies on top.

use std::collections::HashMap;

use dv_core::{Command, Executor, Node, NodeId};

/// Offset of each repeated paste so copies do not hide one another.
const PASTE_STEP: f64 = 16.0;

/// Copied subtrees, each in parent-first order.
#[derive(Default)]
pub struct Clip {
    trees: Vec<Vec<(NodeId, Node)>>,
    pastes: u32,
}

impl Clip {
    /// Remembers the nodes (with their subtrees); returns how many top-level shapes were copied.
    pub fn copy(&mut self, ex: &Executor, ids: &[NodeId]) -> usize {
        let page = ex.doc().page();
        self.trees = page
            .without_descendants(ids)
            .into_iter()
            .filter(|id| page.contains(*id))
            .map(|top| {
                page.subtree(top)
                    .into_iter()
                    .filter_map(|id| page.node(id).map(|n| (id, n.clone())))
                    .collect()
            })
            .collect();
        self.pastes = 0;
        self.trees.len()
    }

    /// Adds the copied shapes to the top level, above everything, as one step.
    pub fn paste(&mut self, ex: &mut Executor) -> Vec<NodeId> {
        if self.trees.is_empty() {
            return Vec::new();
        }
        self.pastes += 1;
        let shift = PASTE_STEP * f64::from(self.pastes);
        let mut cmds = Vec::new();
        let mut made = Vec::new();
        for tree in &self.trees {
            let map: HashMap<NodeId, NodeId> =
                tree.iter().map(|(old, _)| (*old, ex.alloc_id())).collect();
            for (i, (old, src)) in tree.iter().enumerate() {
                let mut node = src.clone();
                if i == 0 {
                    node.geom.x += shift;
                    node.geom.y += shift;
                }
                let parent = if i == 0 {
                    None
                } else {
                    src.parent.and_then(|p| map.get(&p).copied())
                };
                cmds.push(Command::Create {
                    id: map[old],
                    parent,
                    index: usize::MAX,
                    node,
                });
            }
            made.push(map[&tree[0].0]);
        }
        let _ = ex.execute("Paste", Command::Batch(cmds));
        made
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dv_core::{Document, Geometry, NodeKind};

    fn geom() -> Geometry {
        Geometry {
            x: 10.0,
            y: 10.0,
            w: 50.0,
            h: 50.0,
            rotation: 0.0,
        }
    }

    fn setup() -> (Executor, NodeId, NodeId) {
        let mut ex = Executor::new(Document::default());
        let f =
            super::super::ops::create(&mut ex, NodeKind::Frame, "F".into(), None, geom()).unwrap();
        let r =
            super::super::ops::create(&mut ex, NodeKind::Rectangle, "R".into(), Some(f), geom())
                .unwrap();
        (ex, f, r)
    }

    #[test]
    fn paste_makes_fresh_offset_copies_in_one_undo_step() {
        let (mut ex, f, r) = setup();
        let mut clip = Clip::default();
        assert_eq!(clip.copy(&ex, &[f, r]), 1);
        let before = ex.doc().page().len();
        let made = clip.paste(&mut ex);
        assert_eq!(made.len(), 1);
        assert_eq!(ex.doc().page().len(), before + 2);
        let copy = ex.doc().page().node(made[0]).unwrap();
        assert_eq!(copy.geom.x, 10.0 + PASTE_STEP);
        assert_eq!(copy.children.len(), 1);
        assert_ne!(copy.children[0], r);
        assert_eq!(ex.doc().page().roots().last(), Some(&made[0]));
        ex.undo();
        assert_eq!(ex.doc().page().len(), before);
    }

    #[test]
    fn repeated_paste_shifts_further_and_empty_clip_does_nothing() {
        let (mut ex, f, _) = setup();
        let mut clip = Clip::default();
        assert!(clip.paste(&mut ex).is_empty());
        clip.copy(&ex, &[f]);
        clip.paste(&mut ex);
        let second = clip.paste(&mut ex);
        assert_eq!(
            ex.doc().page().node(second[0]).unwrap().geom.x,
            10.0 + 2.0 * PASTE_STEP
        );
    }
}
