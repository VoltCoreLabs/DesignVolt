//! Layer flags edited from the layers panel.

use dv_core::{Command, Executor, NodeId, Props};

/// Locks or unlocks a node (locked nodes cannot be picked on the canvas).
pub fn set_locked(ex: &mut Executor, id: NodeId, locked: bool) {
    let props = Props {
        locked: Some(locked),
        ..Props::default()
    };
    let _ = ex.execute("Lock", Command::SetProps { id, props });
}

/// Renames a node.
pub fn set_name(ex: &mut Executor, id: NodeId, name: String) {
    let props = Props {
        name: Some(name),
        ..Props::default()
    };
    let _ = ex.execute("Rename", Command::SetProps { id, props });
}

#[cfg(test)]
mod tests {
    use super::*;
    use dv_core::{Document, Geometry, NodeKind};

    #[test]
    fn rename_is_undoable() {
        let mut ex = Executor::new(Document::default());
        let g = Geometry {
            x: 0.0,
            y: 0.0,
            w: 10.0,
            h: 10.0,
            rotation: 0.0,
        };
        let id =
            super::super::ops::create(&mut ex, NodeKind::Rectangle, "R".into(), None, g).unwrap();
        set_name(&mut ex, id, "Button".into());
        assert_eq!(ex.doc().page().node(id).unwrap().name, "Button");
        ex.undo();
        assert_eq!(ex.doc().page().node(id).unwrap().name, "R");
    }

    #[test]
    fn lock_toggle_is_undoable() {
        let mut ex = Executor::new(Document::default());
        let g = Geometry {
            x: 0.0,
            y: 0.0,
            w: 10.0,
            h: 10.0,
            rotation: 0.0,
        };
        let id =
            super::super::ops::create(&mut ex, NodeKind::Rectangle, "R".into(), None, g).unwrap();
        set_locked(&mut ex, id, true);
        assert!(ex.doc().page().node(id).unwrap().locked);
        ex.undo();
        assert!(!ex.doc().page().node(id).unwrap().locked);
    }
}
