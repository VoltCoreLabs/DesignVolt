//! Reversible commands. `Document::apply` performs a command and returns its inverse, which is
//! all that undo/redo needs. Every change of a document goes through a command.

use crate::document::{Document, Effect, Geometry, Node, NodeId, Stroke, UserGuide};
use crate::paint::Paint;

/// Property changes; `None` leaves the property as it is.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Props {
    pub name: Option<String>,
    pub fill: Option<Option<Paint>>,
    pub stroke: Option<Option<Stroke>>,
    pub radius: Option<f64>,
    pub opacity: Option<f64>,
    pub effects: Option<Vec<Effect>>,
    pub extra_fills: Option<Vec<Paint>>,
    pub visible: Option<bool>,
    pub locked: Option<bool>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// Adds a childless node with a pre-allocated id (`Document::alloc_id`).
    Create {
        id: NodeId,
        parent: Option<NodeId>,
        index: usize,
        node: Node,
    },
    /// Removes a node together with its descendants.
    Delete {
        id: NodeId,
    },
    /// Puts a removed subtree back (root first); the inverse of `Delete`.
    Restore {
        nodes: Vec<(NodeId, Node)>,
        parent: Option<NodeId>,
        index: usize,
    },
    SetGeometry {
        id: NodeId,
        geom: Geometry,
    },
    SetProps {
        id: NodeId,
        props: Props,
    },
    /// Moves a node to `index` among its siblings (z-order).
    Reorder {
        id: NodeId,
        index: usize,
    },
    /// Moves a node (with its subtree) under another parent, at `index` among its children.
    Reparent {
        id: NodeId,
        parent: Option<NodeId>,
        index: usize,
    },
    /// Replaces the page's user guides (rulers).
    SetGuides(Vec<UserGuide>),
    /// Inserts a page at `index` and makes it the current one.
    InsertPage {
        index: usize,
        page: Box<crate::document::Page>,
    },
    /// Removes a page (never the last one); the neighbour becomes current.
    RemovePage {
        index: usize,
    },
    RenamePage {
        index: usize,
        name: String,
    },
    /// Stores an image file in the page under `id` (an image fill refers to it).
    AddImage {
        id: u64,
        bytes: crate::image::ImageBytes,
    },
    RemoveImage {
        id: u64,
    },
    /// Moves the page at `from` so that it ends up at `to`.
    MovePage {
        from: usize,
        to: usize,
    },
    /// Several commands that undo as one.
    Batch(Vec<Command>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandError {
    MissingNode(NodeId),
    DuplicateNode(NodeId),
    InvalidParent(NodeId),
    EmptyRestore,
    /// A page index is out of range, or the last page would be removed.
    InvalidPage(usize),
    MissingImage(u64),
}

impl Document {
    /// Applies `cmd` and returns the command that reverts it.
    pub fn apply(&mut self, cmd: &Command) -> Result<Command, CommandError> {
        match cmd {
            Command::Create {
                id,
                parent,
                index,
                node,
            } => self.create(*id, *parent, *index, node),
            Command::Delete { id } => self.delete(*id),
            Command::Restore {
                nodes,
                parent,
                index,
            } => self.restore(nodes, *parent, *index),
            Command::SetGeometry { id, geom } => {
                let n = self
                    .page_mut()
                    .nodes
                    .get_mut(id)
                    .ok_or(CommandError::MissingNode(*id))?;
                let old = std::mem::replace(&mut n.geom, *geom);
                Ok(Command::SetGeometry { id: *id, geom: old })
            }
            Command::SetProps { id, props } => self.set_props(*id, props),
            Command::Reorder { id, index } => self.reorder(*id, *index),
            Command::Reparent { id, parent, index } => self.reparent(*id, *parent, *index),
            Command::SetGuides(guides) => {
                let old = std::mem::replace(&mut self.page_mut().guides, guides.clone());
                Ok(Command::SetGuides(old))
            }
            Command::InsertPage { index, page } => self.insert_page(*index, page),
            Command::RemovePage { index } => self.remove_page(*index),
            Command::RenamePage { index, name } => {
                let page = self
                    .pages_mut()
                    .get_mut(*index)
                    .ok_or(CommandError::InvalidPage(*index))?;
                let old = std::mem::replace(&mut page.name, name.clone());
                Ok(Command::RenamePage {
                    index: *index,
                    name: old,
                })
            }
            Command::MovePage { from, to } => self.move_page(*from, *to),
            Command::AddImage { id, bytes } => {
                self.page_mut().images.insert(*id, bytes.clone());
                Ok(Command::RemoveImage { id: *id })
            }
            Command::RemoveImage { id } => {
                let bytes = self
                    .page_mut()
                    .images
                    .remove(id)
                    .ok_or(CommandError::MissingImage(*id))?;
                Ok(Command::AddImage { id: *id, bytes })
            }
            Command::Batch(cmds) => self.batch(cmds),
        }
    }

    fn insert_page(
        &mut self,
        index: usize,
        page: &crate::document::Page,
    ) -> Result<Command, CommandError> {
        let len = self.pages().len();
        if index > len {
            return Err(CommandError::InvalidPage(index));
        }
        for id in page.draw_order() {
            self.reserve_id(id);
        }
        self.pages_mut().insert(index, page.clone());
        self.set_current_raw(index);
        Ok(Command::RemovePage { index })
    }

    fn remove_page(&mut self, index: usize) -> Result<Command, CommandError> {
        let len = self.pages().len();
        if index >= len || len == 1 {
            return Err(CommandError::InvalidPage(index));
        }
        let cur = self.current_page();
        let page = self.pages_mut().remove(index);
        let next = if cur > index {
            cur - 1
        } else {
            cur.min(len - 2)
        };
        self.set_current_raw(next);
        Ok(Command::InsertPage {
            index,
            page: Box::new(page),
        })
    }

    fn move_page(&mut self, from: usize, to: usize) -> Result<Command, CommandError> {
        let len = self.pages().len();
        if from >= len || to >= len {
            return Err(CommandError::InvalidPage(from.max(to)));
        }
        let cur = self.current_page();
        let page = self.pages_mut().remove(from);
        self.pages_mut().insert(to, page);
        let next = if cur == from {
            to
        } else if from < cur && cur <= to {
            cur - 1
        } else if to <= cur && cur < from {
            cur + 1
        } else {
            cur
        };
        self.set_current_raw(next);
        Ok(Command::MovePage { from: to, to: from })
    }

    fn create(
        &mut self,
        id: NodeId,
        parent: Option<NodeId>,
        index: usize,
        node: &Node,
    ) -> Result<Command, CommandError> {
        if self.page().contains(id) {
            return Err(CommandError::DuplicateNode(id));
        }
        if let Some(p) = parent
            && !self.page().contains(p)
        {
            return Err(CommandError::InvalidParent(p));
        }
        self.reserve_id(id);
        let mut node = node.clone();
        node.parent = parent;
        node.children.clear();
        let page = self.page_mut();
        page.nodes.insert(id, node);
        let list = siblings_mut(page, parent);
        list.insert(index.min(list.len()), id);
        Ok(Command::Delete { id })
    }

    fn delete(&mut self, id: NodeId) -> Result<Command, CommandError> {
        if !self.page().contains(id) {
            return Err(CommandError::MissingNode(id));
        }
        let ids = self.page().subtree(id);
        let parent = self.page().nodes[&id].parent;
        let page = self.page_mut();
        let list = siblings_mut(page, parent);
        let index = list.iter().position(|x| *x == id).unwrap_or(0);
        list.remove(index);
        let nodes = ids
            .into_iter()
            .filter_map(|i| page.nodes.remove(&i).map(|n| (i, n)))
            .collect();
        Ok(Command::Restore {
            nodes,
            parent,
            index,
        })
    }

    fn restore(
        &mut self,
        nodes: &[(NodeId, Node)],
        parent: Option<NodeId>,
        index: usize,
    ) -> Result<Command, CommandError> {
        let Some((root, _)) = nodes.first() else {
            return Err(CommandError::EmptyRestore);
        };
        if let Some((dup, _)) = nodes.iter().find(|(i, _)| self.page().contains(*i)) {
            return Err(CommandError::DuplicateNode(*dup));
        }
        if let Some(p) = parent
            && !self.page().contains(p)
        {
            return Err(CommandError::InvalidParent(p));
        }
        for (id, _) in nodes {
            self.reserve_id(*id);
        }
        let page = self.page_mut();
        for (id, node) in nodes {
            page.nodes.insert(*id, node.clone());
        }
        let list = siblings_mut(page, parent);
        list.insert(index.min(list.len()), *root);
        Ok(Command::Delete { id: *root })
    }

    fn set_props(&mut self, id: NodeId, props: &Props) -> Result<Command, CommandError> {
        let n = self
            .page_mut()
            .nodes
            .get_mut(&id)
            .ok_or(CommandError::MissingNode(id))?;
        let mut old = Props::default();
        if let Some(v) = &props.name {
            old.name = Some(std::mem::replace(&mut n.name, v.clone()));
        }
        if let Some(v) = &props.fill {
            old.fill = Some(std::mem::replace(&mut n.fill, v.clone()));
        }
        if let Some(v) = props.stroke {
            old.stroke = Some(std::mem::replace(&mut n.stroke, v));
        }
        if let Some(v) = props.opacity {
            old.opacity = Some(std::mem::replace(&mut n.opacity, v));
        }
        if let Some(v) = &props.extra_fills {
            old.extra_fills = Some(std::mem::replace(&mut n.extra_fills, v.clone()));
        }
        if let Some(v) = &props.effects {
            old.effects = Some(std::mem::replace(&mut n.effects, v.clone()));
        }
        if let Some(v) = props.radius {
            old.radius = Some(std::mem::replace(&mut n.radius, v));
        }
        if let Some(v) = props.visible {
            old.visible = Some(std::mem::replace(&mut n.visible, v));
        }
        if let Some(v) = props.locked {
            old.locked = Some(std::mem::replace(&mut n.locked, v));
        }
        Ok(Command::SetProps { id, props: old })
    }

    fn reorder(&mut self, id: NodeId, index: usize) -> Result<Command, CommandError> {
        let parent = self
            .page()
            .node(id)
            .ok_or(CommandError::MissingNode(id))?
            .parent;
        let list = siblings_mut(self.page_mut(), parent);
        let old = list.iter().position(|x| *x == id).unwrap_or(0);
        list.remove(old);
        list.insert(index.min(list.len()), id);
        Ok(Command::Reorder { id, index: old })
    }

    fn reparent(
        &mut self,
        id: NodeId,
        parent: Option<NodeId>,
        index: usize,
    ) -> Result<Command, CommandError> {
        let old_parent = self
            .page()
            .node(id)
            .ok_or(CommandError::MissingNode(id))?
            .parent;
        if let Some(p) = parent {
            if !self.page().contains(p) {
                return Err(CommandError::InvalidParent(p));
            }
            if self.page().is_inside(p, id) {
                return Err(CommandError::InvalidParent(p));
            }
        }
        let page = self.page_mut();
        let old_list = siblings_mut(page, old_parent);
        let old_index = old_list.iter().position(|x| *x == id).unwrap_or(0);
        old_list.remove(old_index);
        let list = siblings_mut(page, parent);
        list.insert(index.min(list.len()), id);
        if let Some(n) = page.nodes.get_mut(&id) {
            n.parent = parent;
        }
        Ok(Command::Reparent {
            id,
            parent: old_parent,
            index: old_index,
        })
    }

    fn batch(&mut self, cmds: &[Command]) -> Result<Command, CommandError> {
        let mut inverses = Vec::with_capacity(cmds.len());
        for cmd in cmds {
            match self.apply(cmd) {
                Ok(inv) => inverses.push(inv),
                Err(e) => {
                    // Roll back what was already applied so a failed batch changes nothing.
                    for inv in inverses.iter().rev() {
                        let _ = self.apply(inv);
                    }
                    return Err(e);
                }
            }
        }
        inverses.reverse();
        Ok(Command::Batch(inverses))
    }
}

fn siblings_mut(page: &mut crate::document::Page, parent: Option<NodeId>) -> &mut Vec<NodeId> {
    match parent {
        None => &mut page.roots,
        Some(p) => &mut page.nodes.get_mut(&p).expect("parent exists").children,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::NodeKind;

    fn rect(name: &str, x: f64) -> Node {
        Node::new(
            NodeKind::Rectangle,
            name,
            Geometry {
                x,
                y: 0.0,
                w: 10.0,
                h: 10.0,
                rotation: 0.0,
            },
        )
    }

    fn create(doc: &mut Document, name: &str, parent: Option<NodeId>) -> NodeId {
        let id = doc.alloc_id();
        let index = doc.page().siblings(parent).len();
        doc.apply(&Command::Create {
            id,
            parent,
            index,
            node: rect(name, 0.0),
        })
        .unwrap();
        id
    }

    #[test]
    fn create_and_delete_roundtrip_keep_ids_and_order() {
        let mut doc = Document::default();
        let a = create(&mut doc, "a", None);
        let b = create(&mut doc, "b", None);
        let c = create(&mut doc, "c", None);
        let inv = doc.apply(&Command::Delete { id: b }).unwrap();
        assert_eq!(doc.page().roots(), [a, c]);
        doc.apply(&inv).unwrap();
        assert_eq!(doc.page().roots(), [a, b, c]);
        assert_eq!(doc.page().node(b).unwrap().name, "b");
    }

    #[test]
    fn deleting_a_frame_removes_and_restores_its_subtree() {
        let mut doc = Document::default();
        let f = create(&mut doc, "frame", None);
        let k1 = create(&mut doc, "k1", Some(f));
        let k2 = create(&mut doc, "k2", Some(f));
        let g = create(&mut doc, "grand", Some(k1));
        assert_eq!(doc.page().len(), 4);
        let inv = doc.apply(&Command::Delete { id: f }).unwrap();
        assert!(doc.page().is_empty());
        doc.apply(&inv).unwrap();
        assert_eq!(doc.page().len(), 4);
        assert_eq!(doc.page().siblings(Some(f)), [k1, k2]);
        assert_eq!(doc.page().node(g).unwrap().parent, Some(k1));
        assert_eq!(doc.page().draw_order(), [f, k1, g, k2]);
    }

    #[test]
    fn set_geometry_and_props_are_reversible() {
        let mut doc = Document::default();
        let a = create(&mut doc, "a", None);
        let geom = Geometry {
            x: 5.0,
            y: 6.0,
            w: 7.0,
            h: 8.0,
            rotation: 30.0,
        };
        let inv = doc.apply(&Command::SetGeometry { id: a, geom }).unwrap();
        assert_eq!(doc.page().node(a).unwrap().geom, geom);
        doc.apply(&inv).unwrap();
        assert_eq!(doc.page().node(a).unwrap().geom.x, 0.0);

        let props = Props {
            name: Some("renamed".into()),
            fill: Some(None),
            radius: Some(4.0),
            locked: Some(true),
            ..Props::default()
        };
        let inv = doc.apply(&Command::SetProps { id: a, props }).unwrap();
        let n = doc.page().node(a).unwrap();
        assert_eq!(
            (n.name.as_str(), n.fill.clone(), n.radius, n.locked),
            ("renamed", None, 4.0, true)
        );
        doc.apply(&inv).unwrap();
        let n = doc.page().node(a).unwrap();
        assert_eq!((n.name.as_str(), n.radius, n.locked), ("a", 0.0, false));
        assert!(n.fill.is_some());
    }

    #[test]
    fn reorder_changes_z_order_and_back() {
        let mut doc = Document::default();
        let a = create(&mut doc, "a", None);
        let b = create(&mut doc, "b", None);
        let c = create(&mut doc, "c", None);
        let inv = doc.apply(&Command::Reorder { id: a, index: 99 }).unwrap();
        assert_eq!(doc.page().roots(), [b, c, a]);
        doc.apply(&inv).unwrap();
        assert_eq!(doc.page().roots(), [a, b, c]);
    }

    #[test]
    fn reparent_moves_between_parents_undoes_and_rejects_cycles() {
        let mut doc = Document::default();
        let f = create(&mut doc, "f", None);
        let g = create(&mut doc, "g", Some(f));
        let a = create(&mut doc, "a", None);
        let inv = doc
            .apply(&Command::Reparent {
                id: a,
                parent: Some(f),
                index: 0,
            })
            .unwrap();
        assert_eq!(doc.page().roots(), [f]);
        assert_eq!(doc.page().siblings(Some(f)), [a, g]);
        assert_eq!(doc.page().node(a).unwrap().parent, Some(f));
        doc.apply(&inv).unwrap();
        assert_eq!(doc.page().roots(), [f, a]);
        assert_eq!(doc.page().node(a).unwrap().parent, None);
        let bad = Command::Reparent {
            id: f,
            parent: Some(g),
            index: 0,
        };
        assert_eq!(doc.apply(&bad), Err(CommandError::InvalidParent(g)));
        assert_eq!(doc.page().roots(), [f, a]);
    }

    #[test]
    fn failed_batch_changes_nothing() {
        let mut doc = Document::default();
        let a = create(&mut doc, "a", None);
        let before = doc.page().node(a).unwrap().geom;
        let bad = Command::Batch(vec![
            Command::SetGeometry {
                id: a,
                geom: Geometry { x: 99.0, ..before },
            },
            Command::Delete { id: NodeId(12345) },
        ]);
        assert_eq!(
            doc.apply(&bad),
            Err(CommandError::MissingNode(NodeId(12345)))
        );
        assert_eq!(doc.page().node(a).unwrap().geom, before);
    }

    #[test]
    fn set_guides_is_reversible() {
        let mut doc = Document::default();
        let g = vec![UserGuide {
            vertical: true,
            pos: 5.0,
        }];
        let inv = doc.apply(&Command::SetGuides(g.clone())).unwrap();
        assert_eq!(doc.page().guides(), &g[..]);
        doc.apply(&inv).unwrap();
        assert!(doc.page().guides().is_empty());
    }

    #[test]
    fn batch_undoes_in_reverse_order() {
        let mut doc = Document::default();
        let a = create(&mut doc, "a", None);
        let one = Geometry {
            x: 1.0,
            ..Geometry::default()
        };
        let two = Geometry {
            x: 2.0,
            ..Geometry::default()
        };
        let inv = doc
            .apply(&Command::Batch(vec![
                Command::SetGeometry { id: a, geom: one },
                Command::SetGeometry { id: a, geom: two },
            ]))
            .unwrap();
        assert_eq!(doc.page().node(a).unwrap().geom.x, 2.0);
        doc.apply(&inv).unwrap();
        assert_eq!(doc.page().node(a).unwrap().geom.x, 0.0);
    }

    #[test]
    fn invalid_commands_are_rejected() {
        let mut doc = Document::default();
        let a = create(&mut doc, "a", None);
        let dup = Command::Create {
            id: a,
            parent: None,
            index: 0,
            node: rect("x", 0.0),
        };
        assert_eq!(doc.apply(&dup), Err(CommandError::DuplicateNode(a)));
        let orphan = Command::Create {
            id: doc.alloc_id(),
            parent: Some(NodeId(999)),
            index: 0,
            node: rect("x", 0.0),
        };
        assert_eq!(
            doc.apply(&orphan),
            Err(CommandError::InvalidParent(NodeId(999)))
        );
        assert_eq!(
            doc.apply(&Command::Restore {
                nodes: vec![],
                parent: None,
                index: 0
            }),
            Err(CommandError::EmptyRestore)
        );
    }

    #[test]
    fn geo_tops_expand_groups_and_outer_group_finds_the_root_one() {
        let mut doc = Document::default();
        let mk = |doc: &mut Document, kind, parent| {
            let id = doc.alloc_id();
            let index = doc.page().siblings(parent).len();
            let node = Node::new(kind, "n", Geometry::default());
            doc.apply(&Command::Create {
                id,
                parent,
                index,
                node,
            })
            .unwrap();
            id
        };
        let g = mk(&mut doc, NodeKind::Group, None);
        let inner = mk(&mut doc, NodeKind::Group, Some(g));
        let a = mk(&mut doc, NodeKind::Rectangle, Some(inner));
        let b = mk(&mut doc, NodeKind::Rectangle, Some(g));
        let c = mk(&mut doc, NodeKind::Rectangle, None);
        let page = doc.page();
        assert_eq!(page.geo_tops(&[g, c]), vec![a, b, c]);
        assert_eq!(page.geo_tops(&[a]), vec![a]);
        assert_eq!(page.outer_group(a), g);
        assert_eq!(page.outer_group(c), c);
    }

    #[test]
    fn without_descendants_drops_nodes_inside_listed_nodes() {
        let mut doc = Document::default();
        let f = create(&mut doc, "f", None);
        let k = create(&mut doc, "k", Some(f));
        let g = create(&mut doc, "g", Some(k));
        let other = create(&mut doc, "o", None);
        let page = doc.page();
        assert_eq!(page.without_descendants(&[f, k, g, other]), vec![f, other]);
        assert_eq!(page.without_descendants(&[k, g]), vec![k]);
        assert_eq!(page.without_descendants(&[g]), vec![g]);
    }

    #[test]
    fn revision_grows_and_ids_are_never_reused() {
        let mut doc = Document::default();
        let r0 = doc.revision();
        let a = create(&mut doc, "a", None);
        assert!(doc.revision() > r0);
        let inv = doc.apply(&Command::Delete { id: a }).unwrap();
        let b = doc.alloc_id();
        assert_ne!(a, b);
        doc.apply(&inv).unwrap();
        assert!(doc.page().contains(a));
    }
}
