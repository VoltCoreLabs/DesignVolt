//! Undo/redo on top of reversible commands. Consecutive edits with the same merge key (for
//! example the mouse moves of one drag) collapse into a single undo step.

use crate::command::{Command, CommandError};
use crate::document::Document;

const LIMIT: usize = 500;

struct Entry {
    label: &'static str,
    key: Option<u64>,
    /// The page that was current when the step ran; undo and redo switch back to it.
    page: usize,
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

    /// Switches the page being edited (not an undo step).
    pub fn set_page(&mut self, index: usize) {
        self.doc.set_current_page(index);
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
        let page = self.doc.current_page();
        let inverse = self.doc.apply(&cmd)?;
        self.redo.clear();
        if let (Some(k), Some(top)) = (key, self.undo.last_mut())
            && top.key == Some(k)
            && top.page == page
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
            page,
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
        self.doc.set_current_page(entry.page);
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
        self.doc.set_current_page(entry.page);
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

    #[test]
    fn undo_returns_to_the_page_of_the_edit() {
        use crate::document::Page;
        let mut ex = Executor::new(Document::default());
        let a = add(&mut ex, "a");
        ex.execute(
            "Add page",
            Command::InsertPage {
                index: 1,
                page: Box::new(Page::new("Page 2")),
            },
        )
        .unwrap();
        assert_eq!(ex.doc().current_page(), 1);
        let b = add(&mut ex, "b");
        ex.undo();
        assert_eq!(ex.doc().current_page(), 1);
        assert!(!ex.doc().page().contains(b));
        // Undoing the page insert goes back to the first page, which still has `a`.
        ex.undo();
        assert_eq!(ex.doc().pages().len(), 1);
        assert!(ex.doc().page().contains(a));
        // Redo the page, then switch away and undo the edit of page 1 from page 0.
        ex.redo();
        ex.redo();
        assert!(ex.doc().page().contains(b));
        ex.doc.set_current_page(0);
        ex.undo();
        assert_eq!(ex.doc().current_page(), 1);
        assert!(!ex.doc().page().contains(b));
    }

    #[test]
    fn pages_can_be_renamed_moved_and_removed() {
        use crate::document::Page;
        let mut ex = Executor::new(Document::default());
        for n in ["B", "C"] {
            let at = ex.doc().pages().len();
            ex.execute(
                "Add page",
                Command::InsertPage {
                    index: at,
                    page: Box::new(Page::new(n)),
                },
            )
            .unwrap();
        }
        let names = |ex: &Executor| {
            ex.doc()
                .pages()
                .iter()
                .map(|p| p.name.clone())
                .collect::<Vec<_>>()
        };
        ex.execute("Move", Command::MovePage { from: 2, to: 0 })
            .unwrap();
        assert_eq!(names(&ex), ["C", "Page 1", "B"]);
        assert_eq!(ex.doc().current_page(), 0, "the current page moves along");
        ex.execute(
            "Rename",
            Command::RenamePage {
                index: 1,
                name: "X".into(),
            },
        )
        .unwrap();
        ex.execute("Remove", Command::RemovePage { index: 0 })
            .unwrap();
        assert_eq!(names(&ex), ["X", "B"]);
        ex.undo();
        ex.undo();
        ex.undo();
        assert_eq!(names(&ex), ["Page 1", "B", "C"]);
        assert!(
            ex.execute("Remove", Command::RemovePage { index: 9 })
                .is_err()
        );
        let mut one = Executor::new(Document::default());
        assert!(
            one.execute("Remove", Command::RemovePage { index: 0 })
                .is_err()
        );
    }
}
