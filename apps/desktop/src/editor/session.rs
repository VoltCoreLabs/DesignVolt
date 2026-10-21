//! A document being edited: the executor with its history, the view of the canvas and the file
//! it is saved to. Sessions stay in memory for the whole app run, so leaving the editor and
//! coming back is instant; changes are written to disk shortly after they are made, when the
//! editor is left and when the app closes.

use std::fs;
use std::path::PathBuf;

use dv_core::{Document, Executor, from_text, to_text};
use dv_render::Viewport;

use crate::store::write_atomic;

/// Seconds a change waits before it is saved by itself.
pub const AUTOSAVE_SECS: f32 = 1.0;

/// Seconds the view rests before it is written.
const VIEW_SAVE_SECS: f32 = 1.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SaveState {
    /// Everything is on disk.
    Saved,
    /// There are changes that are not on disk yet.
    Unsaved,
    /// The session has no file (tests).
    NoFile,
}

pub struct Session {
    pub exec: Executor,
    pub vp: Viewport,
    /// The page origin has been moved to the middle of the canvas once.
    pub centered: bool,
    /// The view each page had when it was left (by page index).
    pub views: Vec<Option<Viewport>>,
    path: Option<PathBuf>,
    saved_revision: u64,
    unsaved_for: f32,
    /// The view file as last written (or read), and how long the view has differed from it.
    view_text: String,
    view_for: f32,
}

impl Session {
    /// A session that is not backed by a file (used by tests).
    #[cfg(test)]
    pub fn new(name: &str) -> Self {
        Self::with(Document::new(name), None)
    }

    fn with(doc: Document, path: Option<PathBuf>) -> Self {
        let saved_revision = doc.revision();
        let mut s = Self {
            exec: Executor::new(doc),
            vp: Viewport::default(),
            centered: false,
            views: Vec::new(),
            path,
            saved_revision,
            unsaved_for: 0.0,
            view_text: String::new(),
            view_for: 0.0,
        };
        s.view_text = s.current_view_text();
        s
    }

    /// Opens the document stored at `path`, or starts a new one named `name` when there is none.
    /// A file that cannot be read is moved aside (`.corrupt`) instead of being overwritten.
    pub fn open(name: &str, path: PathBuf) -> Self {
        let doc = match fs::read_to_string(&path) {
            Ok(text) => match from_text(&text) {
                Ok(doc) => doc,
                Err(e) => {
                    eprintln!("could not read {}: {e}", path.display());
                    let _ = fs::rename(&path, path.with_extension("dvd.corrupt"));
                    Document::new(name)
                }
            },
            Err(_) => Document::new(name),
        };
        let mut s = Self::with(doc, Some(path));
        s.restore_view();
        s
    }

    fn view_path(&self) -> Option<PathBuf> {
        self.path.as_ref().map(|p| p.with_extension("view"))
    }

    fn current_view_text(&self) -> String {
        super::view_store::to_text(self.exec.doc().current_page(), &self.views, self.vp)
    }

    /// Puts the document back on the page and view it was closed with.
    fn restore_view(&mut self) {
        let Some(path) = self.view_path() else { return };
        let Ok(text) = fs::read_to_string(&path) else {
            return;
        };
        let saved = super::view_store::from_text(&text);
        let clean = !self.is_dirty();
        self.exec.set_page(saved.page);
        if clean {
            self.saved_revision = self.exec.doc().revision();
        }
        let cur = self.exec.doc().current_page();
        self.views = saved.views;
        if let Some(Some(v)) = self.views.get(cur) {
            self.vp = *v;
            self.centered = true;
        }
        self.view_text = self.current_view_text();
    }

    /// Writes the view file when the view changed.
    fn save_view(&mut self) {
        let Some(path) = self.view_path() else { return };
        let text = self.current_view_text();
        if text == self.view_text {
            return;
        }
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        if write_atomic(&path, &text).is_ok() {
            self.view_text = text;
        }
        self.view_for = 0.0;
    }

    pub fn state(&self) -> SaveState {
        match (&self.path, self.is_dirty()) {
            (None, _) => SaveState::NoFile,
            (Some(_), true) => SaveState::Unsaved,
            (Some(_), false) => SaveState::Saved,
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.exec.doc().revision() != self.saved_revision
    }

    /// Writes the document if it changed. Returns true when something was written.
    pub fn save(&mut self) -> bool {
        self.save_view();
        let Some(path) = &self.path else { return false };
        if !self.is_dirty() {
            return false;
        }
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        match write_atomic(path, &to_text(self.exec.doc())) {
            Ok(()) => {
                self.saved_revision = self.exec.doc().revision();
                self.unsaved_for = 0.0;
                true
            }
            Err(e) => {
                eprintln!("could not save {}: {e}", path.display());
                false
            }
        }
    }

    /// Autosave: saves once a change has been waiting for `AUTOSAVE_SECS` and the user is not in
    /// the middle of a drag (`busy`). Returns true while a save is still pending.
    pub fn tick(&mut self, dt: f32, busy: bool) -> bool {
        let view_pending = self.path.is_some() && self.current_view_text() != self.view_text;
        if view_pending {
            self.view_for += dt;
            if self.view_for >= VIEW_SAVE_SECS && !busy {
                self.save_view();
            }
        }
        let view_pending = view_pending && self.current_view_text() != self.view_text;
        if self.path.is_none() || !self.is_dirty() {
            self.unsaved_for = 0.0;
            return view_pending;
        }
        self.unsaved_for += dt;
        if self.unsaved_for >= AUTOSAVE_SECS && !busy {
            self.save();
        }
        self.is_dirty() || view_pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dv_core::{Command, Geometry, Node, NodeKind};

    fn temp(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("designvolt-session-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir.join("documents").join("1.dvd")
    }

    fn add_rect(s: &mut Session) {
        let id = s.exec.alloc_id();
        let node = Node::new(
            NodeKind::Rectangle,
            "r",
            Geometry {
                x: 1.0,
                y: 2.0,
                w: 3.0,
                h: 4.0,
                rotation: 0.0,
            },
        );
        s.exec
            .execute(
                "Create",
                Command::Create {
                    id,
                    parent: None,
                    index: 0,
                    node,
                },
            )
            .unwrap();
    }

    #[test]
    fn a_new_session_is_clean_and_a_change_makes_it_dirty() {
        let mut s = Session::open("Doc", temp("dirty"));
        assert_eq!(s.state(), SaveState::Saved);
        add_rect(&mut s);
        assert_eq!(s.state(), SaveState::Unsaved);
        assert!(s.save());
        assert_eq!(s.state(), SaveState::Saved);
        assert!(!s.save(), "nothing to write when nothing changed");
    }

    #[test]
    fn saved_documents_come_back_from_disk() {
        let path = temp("reload");
        let mut s = Session::open("Doc", path.clone());
        add_rect(&mut s);
        s.save();
        let back = Session::open("Other", path.clone());
        assert_eq!(back.exec.doc().page().len(), 1);
        assert_eq!(back.state(), SaveState::Saved);
        let _ = fs::remove_dir_all(path.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn undo_after_saving_marks_the_session_dirty_again() {
        let mut s = Session::open("Doc", temp("undo"));
        add_rect(&mut s);
        s.save();
        s.exec.undo();
        assert_eq!(s.state(), SaveState::Unsaved);
        s.save();
        let path = s.path.clone().unwrap();
        assert_eq!(Session::open("x", path).exec.doc().page().len(), 0);
    }

    #[test]
    fn autosave_waits_for_the_delay_and_for_the_pointer_to_be_free() {
        let mut s = Session::open("Doc", temp("auto"));
        add_rect(&mut s);
        assert!(s.tick(0.5, false), "still pending");
        assert_eq!(s.state(), SaveState::Unsaved);
        assert!(s.tick(0.6, true), "not while dragging");
        assert_eq!(s.state(), SaveState::Unsaved);
        assert!(!s.tick(0.1, false), "saved once the pointer is free");
        assert_eq!(s.state(), SaveState::Saved);
    }

    #[test]
    fn a_broken_file_is_moved_aside_and_a_fresh_document_starts() {
        let path = temp("broken");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "this is not a document").unwrap();
        let s = Session::open("Fresh", path.clone());
        assert!(s.exec.doc().page().is_empty());
        assert!(!path.exists());
        assert!(
            path.with_extension("dvd.corrupt").exists(),
            "the broken file is kept"
        );
        let _ = fs::remove_dir_all(path.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn sessions_without_a_file_never_save() {
        let mut s = Session::new("Test");
        add_rect(&mut s);
        assert_eq!(s.state(), SaveState::NoFile);
        assert!(!s.save());
        assert!(!s.tick(5.0, false));
    }

    #[test]
    fn the_view_and_page_come_back_after_reopening() {
        let path = temp("view");
        let mut s = Session::open("Doc", path.clone());
        s.exec
            .execute(
                "Add page",
                Command::InsertPage {
                    index: 1,
                    page: Box::new(dv_core::Page::new("Two")),
                },
            )
            .unwrap();
        s.vp = Viewport {
            zoom: 2.5,
            pan: kurbo::Vec2::new(-40.0, 17.0),
        };
        s.save();
        let back = Session::open("Doc", path.clone());
        assert_eq!(back.exec.doc().current_page(), 1);
        assert_eq!(back.vp.zoom, 2.5);
        assert_eq!(back.vp.pan.x, -40.0);
        assert!(back.centered, "a restored view is not re-centered");
        assert_eq!(back.state(), SaveState::Saved);
        let _ = fs::remove_dir_all(path.parent().unwrap().parent().unwrap());
    }
}
