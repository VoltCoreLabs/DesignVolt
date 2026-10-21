//! Image fills: putting a dropped image file into shapes and changing how it is placed.

use std::path::Path;
use std::sync::Arc;

use dv_core::{Command, Executor, Geometry, ImageBytes, ImageFill, ImageMode, NodeId, NodeKind};
use dv_core::{Paint, Props};
use kurbo::Point;

use super::paint_layout::Look;
use super::paint_ui::fill_look;
use super::state::Editor;

/// Largest image file taken from a drop, in bytes.
const MAX_FILE: u64 = 64 * 1024 * 1024;
/// Longest side of the rectangle made for an image dropped on an empty selection.
const NEW_SHAPE_SIDE: f64 = 480.0;

/// Stores `bytes` in the page and makes the image the fill number `index` of every shape of
/// `ids` (replacing that fill, or adding one when the shape has fewer), as one undo step.
/// An image that was there keeps its placement mode.
pub fn set_image(ex: &mut Executor, ids: &[NodeId], index: usize, bytes: ImageBytes) {
    let asset = ex.alloc_id().0;
    let page = ex.doc().page();
    let mut cmds = vec![Command::AddImage { id: asset, bytes }];
    for id in page.geo_tops(ids) {
        let Some(n) = page.node(id) else { continue };
        let mut all: Vec<Paint> = n.fill.iter().chain(&n.extra_fills).cloned().collect();
        let mut image = ImageFill::new(asset);
        if let Some(Paint::Image(old)) = all.get(index) {
            image.mode = old.mode;
            image.scale = old.scale;
        }
        if index < all.len() {
            all[index] = Paint::Image(image);
        } else {
            all.push(Paint::Image(image));
        }
        let mut rest = all.into_iter();
        cmds.push(Command::SetProps {
            id,
            props: Props {
                fill: Some(rest.next()),
                extra_fills: Some(rest.collect()),
                ..Props::default()
            },
        });
    }
    let _ = ex.execute("Image", Command::Batch(cmds));
}

/// Changes the placement mode of the image fill number `index`.
pub fn set_mode(ex: &mut Executor, ids: &[NodeId], index: usize, mode: ImageMode) {
    super::ops_paint::edit_fill_at(ex, ids, None, index, |old| {
        old.map(|p| match p {
            Paint::Image(i) => Paint::Image(ImageFill { mode, ..i }),
            other => other,
        })
    });
}

/// The size of the rectangle made for an image of `w` x `h` pixels.
pub fn shape_size(w: u32, h: u32) -> (f64, f64) {
    let (w, h) = (f64::from(w), f64::from(h));
    let k = (NEW_SHAPE_SIDE / w.max(h)).min(1.0);
    ((w * k).round().max(1.0), (h * k).round().max(1.0))
}

impl Editor {
    /// An image file dropped on the window: it fills the selected shapes (the fill the open
    /// picker edits, else the first one), or becomes a new rectangle when nothing is selected.
    pub(super) fn drop_image_file(&mut self, path: &Path) {
        let ok = std::fs::metadata(path).is_ok_and(|m| m.len() <= MAX_FILE);
        let Some(bytes) = ok.then(|| std::fs::read(path).ok()).flatten() else {
            return;
        };
        let Some(img) = dv_render::image::decode(&bytes) else {
            return;
        };
        let bytes: ImageBytes = Arc::new(bytes);
        let has_target = {
            let s = self.session.borrow();
            let page = s.exec.doc().page();
            !page.geo_tops(&self.selection).is_empty()
        };
        if has_target {
            let index = self.fill_idx();
            set_image(
                &mut self.session.borrow_mut().exec,
                &self.selection,
                index,
                bytes,
            );
            self.image_wait = false;
            return;
        }
        let (w, h) = shape_size(img.width, img.height);
        let c = self.layout().canvas;
        let center: Point = self
            .session
            .borrow()
            .vp
            .to_world(Point::new(c.width() / 2.0, c.height() / 2.0));
        let geom = Geometry {
            x: (center.x - w / 2.0).round(),
            y: (center.y - h / 2.0).round(),
            w,
            h,
            rotation: 0.0,
        };
        let mut s = self.session.borrow_mut();
        let Some(id) =
            super::ops::create(&mut s.exec, NodeKind::Rectangle, "Image".into(), None, geom)
        else {
            return;
        };
        set_image(&mut s.exec, &[id], 0, bytes);
        drop(s);
        self.selection = vec![id];
    }

    /// The image tab: shows the image controls and waits for a file to be dropped.
    pub(super) fn begin_image(&mut self) {
        self.pick_source = false;
        if self.current_image().is_none() {
            self.image_wait = true;
        }
    }

    /// Opens the system file dialog on its own thread; the chosen file is taken by
    /// `poll_upload`.
    pub(super) fn begin_upload(&mut self) {
        if self.upload.is_some() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let path = rfd::FileDialog::new()
                .add_filter("Images", &["png", "jpg", "jpeg"])
                .pick_file();
            let _ = tx.send(path);
        });
        self.upload = Some(rx);
    }

    /// Applies the file chosen in the dialog; true while the dialog is still open.
    pub(super) fn poll_upload(&mut self) -> bool {
        let Some(rx) = &self.upload else {
            return false;
        };
        match rx.try_recv() {
            Ok(path) => {
                self.upload = None;
                if let Some(path) = path {
                    self.drop_image_file(&path);
                }
                false
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => true,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.upload = None;
                false
            }
        }
    }

    pub(super) fn set_image_scale(&mut self, scale: f64) {
        let index = self.fill_idx();
        super::ops_paint::edit_fill_at(
            &mut self.session.borrow_mut().exec,
            &self.selection,
            None,
            index,
            |old| {
                old.map(|p| match p {
                    Paint::Image(i) => Paint::Image(ImageFill { scale, ..i }),
                    other => other,
                })
            },
        );
    }

    /// Placement mode of the image fill the picker edits.
    pub(super) fn current_image(&self) -> Option<ImageFill> {
        match fill_look(self, self.fill_idx()) {
            Look::One(Paint::Image(i)) => Some(i),
            _ => None,
        }
    }

    pub(super) fn set_image_mode(&mut self, mode: ImageMode) {
        let index = self.fill_idx();
        set_mode(
            &mut self.session.borrow_mut().exec,
            &self.selection,
            index,
            mode,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dv_core::Document;

    fn shape(ex: &mut Executor) -> NodeId {
        let g = Geometry {
            x: 0.0,
            y: 0.0,
            w: 10.0,
            h: 10.0,
            rotation: 0.0,
        };
        super::super::ops::create(ex, NodeKind::Rectangle, "r".into(), None, g).unwrap()
    }

    #[test]
    fn an_image_replaces_the_fill_keeps_its_mode_and_undoes_in_one_step() {
        let mut ex = Executor::new(Document::default());
        let a = shape(&mut ex);
        let bytes: ImageBytes = Arc::new(vec![1, 2, 3]);
        set_image(&mut ex, &[a], 0, bytes.clone());
        let n = ex.doc().page().node(a).unwrap();
        let Some(Paint::Image(f)) = n.fill.clone() else {
            panic!("not an image fill");
        };
        assert_eq!(ex.doc().page().image(f.asset), Some(&bytes));
        set_mode(&mut ex, &[a], 0, ImageMode::Tile);
        set_image(&mut ex, &[a], 0, Arc::new(vec![9]));
        let Some(Paint::Image(g)) = ex.doc().page().node(a).unwrap().fill.clone() else {
            panic!("not an image fill");
        };
        assert_eq!(g.mode, ImageMode::Tile);
        assert_ne!(g.asset, f.asset);
        ex.undo();
        let Some(Paint::Image(h)) = ex.doc().page().node(a).unwrap().fill.clone() else {
            panic!("not an image fill");
        };
        assert_eq!(h.asset, f.asset);
        assert!(
            ex.doc().page().image(g.asset).is_none(),
            "the image went with the step"
        );
    }

    #[test]
    fn a_second_index_adds_a_fill_on_top() {
        let mut ex = Executor::new(Document::default());
        let a = shape(&mut ex);
        set_image(&mut ex, &[a], 1, Arc::new(vec![1]));
        let n = ex.doc().page().node(a).unwrap();
        assert!(matches!(n.fill, Some(Paint::Solid(_))));
        assert!(matches!(n.extra_fills[..], [Paint::Image(_)]));
    }

    #[test]
    fn new_shapes_are_capped_but_never_enlarged() {
        assert_eq!(shape_size(4000, 2000), (480.0, 240.0));
        assert_eq!(shape_size(100, 50), (100.0, 50.0));
        assert_eq!(shape_size(1, 5000), (1.0, 480.0));
    }
}
