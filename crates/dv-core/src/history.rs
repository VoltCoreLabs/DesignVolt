//! Undo/redo on top of reversible commands. Consecutive edits with the same merge key (for
//! example the mouse moves of one drag) collapse into a single undo step.

use crate::command::{Command, CommandError};
use crate::document::Document;

const LIMIT: usize = 500;

struct Entry {
    label: &'static str,
    key: Option<u64>,
    undo: Command,
    redo: Command,
}

/// Owns a document and its history; all edits go through `execute*`.
pub struct Executor {
    doc: Document,
    undo: Vec<Entry>,
    redo: Vec<Entry>,
}

impl Executor {
    pub fn new(doc: Document) -> Self {
        Self {
            doc,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    pub fn doc(&self) -> &Document {
        &self.doc
    }

    /// Reserves a node id for a `Command::Create`.
    pub fn alloc_id(&mut self) -> crate::document::NodeId {
        self.doc.alloc_id()
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo_label(&self) -> Option<&'static str> {
        self.undo.last().map(|e| e.label)
    }

    /// Applies a command as one undo step.
    pub fn execute(&mut self, label: &'static str, cmd: Command) -> Result<(), CommandError> {
        self.run(label, None, cmd)
    }

    /// Like `execute`, but edits that follow each other with the same `key` share one step.
    /// The commands must set absolute values (such as `SetGeometry`), because only the last
    /// one is replayed on redo.
    pub fn execute_merged(
        &mut self,
        label: &'static str,
        key: u64,
        cmd: Command,
    ) -> Result<(), CommandError> {
        self.run(label, Some(key), cmd)
    }

    fn run(
        &mut self,
        label: &'static str,
        key: Option<u64>,
        cmd: Command,
    ) -> Result<(), CommandError> {
        let inverse = self.doc.apply(&cmd)?;
        self.redo.clear();
        if let (Some(k), Some(top)) = (key, self.undo.last_mut())
            && top.key == Some(k)
        {
            top.redo = cmd;
            return Ok(());
        }
        if self.undo.len() >= LIMIT {
            self.undo.remove(0);
        }
        self.undo.push(Entry {
            label,
            key,
            undo: inverse,
            redo: cmd,
        });
        Ok(())
    }

    /// Ends a merge run: the next edit with the same key starts a new undo step.
    pub fn break_merge(&mut self) {
        if let Some(top) = self.undo.last_mut() {
            top.key = None;
        }
    }

    pub fn undo(&mut self) -> bool {
        let Some(entry) = self.undo.pop() else {
            return false;
        };
        let ok = self.doc.apply(&entry.undo).is_ok();
        if ok {
            self.redo.push(entry);
        }
        ok
    }

    pub fn redo(&mut self) -> bool {
        let Some(entry) = self.redo.pop() else {
            return false;
        };
        let ok = self.doc.apply(&entry.redo).is_ok();
        if ok {
            self.undo.push(entry);
        }
        ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Geometry, Node, NodeKind};

    fn add(ex: &mut Executor, name: &str) -> crate::document::NodeId {
        let id = ex.alloc_id();
        let node = Node::new(NodeKind::Rectangle, name, Geometry::default());
        ex.execute(
            "Create",
            Command::Create {
                id,
                parent: None,
                index: 99,
                node,
            },
        )
        .unwrap();
        id
    }

    fn x_of(ex: &Executor, id: crate::document::NodeId) -> f64 {
        ex.doc().page().node(id).unwrap().geom.x
    }

    fn set_x(ex: &mut Executor, id: crate::document::NodeId, x: f64, key: u64) {
        let geom = Geometry {
            x,
            ..Geometry::default()
        };
        ex.execute_merged("Move", key, Command::SetGeometry { id, geom })
            .unwrap();
    }

    #[test]
    fn undo_and_redo_walk_through_edits() {
        let mut ex = Executor::new(Document::default());
        let a = add(&mut ex, "a");
        let b = add(&mut ex, "b");
        assert_eq!(ex.doc().page().roots(), [a, b]);
        assert!(ex.undo());
        assert_eq!(ex.doc().page().roots(), [a]);
        assert!(ex.undo());
        assert!(ex.doc().page().is_empty());
        assert!(!ex.undo());
        assert!(ex.redo() && ex.redo());
        assert_eq!(ex.doc().page().roots(), [a, b]);
        assert!(!ex.redo());
    }

    #[test]
    fn new_edit_clears_redo() {
        let mut ex = Executor::new(Document::default());
        add(&mut ex, "a");
        ex.undo();
        assert!(ex.can_redo());
        add(&mut ex, "b");
        assert!(!ex.can_redo());
    }

    #[test]
    fn a_drag_with_one_key_is_a_single_undo_step() {
        let mut ex = Executor::new(Document::default());
        let a = add(&mut ex, "a");
        for x in [1.0, 2.0, 3.0, 4.0] {
            set_x(&mut ex, a, x, 7);
        }
        assert_eq!(x_of(&ex, a), 4.0);
        ex.undo();
        assert_eq!(x_of(&ex, a), 0.0, "one undo reverts the whole drag");
        ex.redo();
        assert_eq!(x_of(&ex, a), 4.0, "redo replays the final position");
    }

    #[test]
    fn different_keys_or_a_broken_run_make_separate_steps() {
        let mut ex = Executor::new(Document::default());
        let a = add(&mut ex, "a");
        set_x(&mut ex, a, 1.0, 1);
        set_x(&mut ex, a, 2.0, 2);
        ex.undo();
        assert_eq!(x_of(&ex, a), 1.0);
        ex.undo();
        assert_eq!(x_of(&ex, a), 0.0);

        set_x(&mut ex, a, 5.0, 9);
        ex.break_merge();
        set_x(&mut ex, a, 6.0, 9);
        ex.undo();
        assert_eq!(x_of(&ex, a), 5.0);
    }

    #[test]
    fn failed_command_does_not_touch_history() {
        let mut ex = Executor::new(Document::default());
        let a = add(&mut ex, "a");
        let bad = Command::Delete {
            id: crate::document::NodeId(777),
        };
        assert!(ex.execute("Delete", bad).is_err());
        ex.undo();
        assert!(!ex.doc().page().contains(a));
        assert!(!ex.can_undo());
    }

    #[test]
    fn history_is_bounded() {
        let mut ex = Executor::new(Document::default());
        for i in 0..(LIMIT + 20) {
            add(&mut ex, &format!("n{i}"));
        }
        let mut steps = 0;
        while ex.undo() {
            steps += 1;
        }
        assert_eq!(steps, LIMIT);
    }

    #[test]
    fn undo_restores_deleted_subtree_with_stable_ids() {
        let mut ex = Executor::new(Document::default());
        let frame = ex.alloc_id();
        ex.execute(
            "Create",
            Command::Create {
                id: frame,
                parent: None,
                index: 0,
                node: Node::new(NodeKind::Frame, "f", Geometry::default()),
            },
        )
        .unwrap();
        let child = ex.alloc_id();
        ex.execute(
            "Create",
            Command::Create {
                id: child,
                parent: Some(frame),
                index: 0,
                node: Node::new(NodeKind::Rectangle, "c", Geometry::default()),
            },
        )
        .unwrap();
        ex.execute("Delete", Command::Delete { id: frame }).unwrap();
        assert!(ex.doc().page().is_empty());
        ex.undo();
        assert!(ex.doc().page().contains(child));
        assert_eq!(ex.doc().page().node(child).unwrap().parent, Some(frame));
    }
}
