//! Z-order operations: bring forward / send backward / to front / to back.

use std::collections::HashSet;

use dv_core::{Command, Executor, NodeId};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Order {
    Forward,
    Backward,
    Front,
    Back,
}

/// The sibling list after moving the selected entries; relative order of the selected is kept.
pub fn reordered(list: &[NodeId], sel: &HashSet<NodeId>, mode: Order) -> Vec<NodeId> {
    let mut v = list.to_vec();
    match mode {
        Order::Front | Order::Back => {
            let (s, rest): (Vec<_>, Vec<_>) = v.iter().partition(|id| sel.contains(*id));
            let (a, b) = if mode == Order::Front {
                (rest, s)
            } else {
                (s, rest)
            };
            v = a.into_iter().chain(b).copied().collect();
        }
        Order::Forward => {
            for i in (0..v.len().saturating_sub(1)).rev() {
                if sel.contains(&v[i]) && !sel.contains(&v[i + 1]) {
                    v.swap(i, i + 1);
                }
            }
        }
        Order::Backward => {
            for i in 1..v.len() {
                if sel.contains(&v[i]) && !sel.contains(&v[i - 1]) {
                    v.swap(i, i - 1);
                }
            }
        }
    }
    v
}

/// Moves the selected nodes in their sibling lists as one undoable step. Returns whether anything moved.
pub fn reorder(ex: &mut Executor, ids: &[NodeId], mode: Order) -> bool {
    let page = ex.doc().page();
    let sel: HashSet<NodeId> = ids.iter().copied().filter(|i| page.contains(*i)).collect();
    let mut parents: Vec<Option<NodeId>> = Vec::new();
    for id in &sel {
        let p = page.node(*id).and_then(|n| n.parent);
        if !parents.contains(&p) {
            parents.push(p);
        }
    }
    let mut cmds = Vec::new();
    for p in parents {
        let mut cur = page.siblings(p).to_vec();
        for (i, id) in reordered(&cur.clone(), &sel, mode).into_iter().enumerate() {
            if cur[i] != id {
                cmds.push(Command::Reorder { id, index: i });
                cur.retain(|x| *x != id);
                cur.insert(i, id);
            }
        }
    }
    if cmds.is_empty() {
        return false;
    }
    ex.execute("Reorder", Command::Batch(cmds)).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(n: &[u64]) -> Vec<NodeId> {
        n.iter().map(|i| NodeId(*i)).collect()
    }
    fn set(n: &[u64]) -> HashSet<NodeId> {
        ids(n).into_iter().collect()
    }

    #[test]
    fn forward_moves_one_step_and_stops_at_the_top() {
        let l = ids(&[1, 2, 3, 4]);
        assert_eq!(
            reordered(&l, &set(&[2]), Order::Forward),
            ids(&[1, 3, 2, 4])
        );
        assert_eq!(reordered(&l, &set(&[4]), Order::Forward), l);
        assert_eq!(reordered(&l, &set(&[3, 4]), Order::Forward), l);
        assert_eq!(
            reordered(&l, &set(&[1, 3]), Order::Forward),
            ids(&[2, 1, 4, 3])
        );
    }

    #[test]
    fn backward_mirrors_forward() {
        let l = ids(&[1, 2, 3, 4]);
        assert_eq!(
            reordered(&l, &set(&[3]), Order::Backward),
            ids(&[1, 3, 2, 4])
        );
        assert_eq!(reordered(&l, &set(&[1, 2]), Order::Backward), l);
    }

    #[test]
    fn front_and_back_keep_relative_order() {
        let l = ids(&[1, 2, 3, 4]);
        assert_eq!(
            reordered(&l, &set(&[1, 3]), Order::Front),
            ids(&[2, 4, 1, 3])
        );
        assert_eq!(
            reordered(&l, &set(&[2, 4]), Order::Back),
            ids(&[2, 4, 1, 3])
        );
    }
}
