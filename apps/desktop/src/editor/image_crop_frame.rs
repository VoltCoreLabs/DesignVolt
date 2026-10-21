//! The frame shown around the image while it is cropped: its outline and four corner handles.
//! Dragging a corner scales the image around the opposite corner.

use dv_core::{ImageFill, NodeId, Paint};
use dv_ui::draw::Cx;
use dv_ui::theme::colors::*;
use kurbo::{BezPath, Point, Rect};

use super::image_crop::Target;
use super::interact::Interaction;
use super::ops_paint::edit_fill_at;
use super::state::Editor;

/// Half the side of a corner handle, and how close a press must be to grab it.
const HANDLE: f64 = 5.0;
const GRAB: f64 = 9.0;

/// A corner handle being dragged (0 top-left, 1 top-right, 2 bottom-right, 3 bottom-left).
#[derive(Clone, Copy, Debug)]
pub struct CornerDrag {
    pub key: u64,
    pub id: NodeId,
    pub corner: usize,
}

/// The zoom that makes the image exactly cover a `w` x `h` box (crop scale 1).
fn base_scale(iw: f64, ih: f64, w: f64, h: f64) -> f64 {
    (w / iw).max(h / ih)
}

/// Corners of the image in the shape's own coordinates.
pub fn frame_corners(f: &ImageFill, (iw, ih): (f64, f64), (w, h): (f64, f64)) -> [Point; 4] {
    let s = base_scale(iw, ih, w, h) * f.scale;
    let x0 = (w - iw * s) / 2.0 + f.offset.0 * w;
    let y0 = (h - ih * s) / 2.0 + f.offset.1 * h;
    [
        Point::new(x0, y0),
        Point::new(x0 + iw * s, y0),
        Point::new(x0 + iw * s, y0 + ih * s),
        Point::new(x0, y0 + ih * s),
    ]
}

/// The zoom and offset after `corner` was dragged to `p`: the opposite corner stays where it
/// is and the image keeps its proportions.
pub fn scaled_about(
    corner: usize,
    p: Point,
    f: &ImageFill,
    (iw, ih): (f64, f64),
    (w, h): (f64, f64),
) -> (f64, (f64, f64)) {
    let anchor = frame_corners(f, (iw, ih), (w, h))[(corner + 2) % 4];
    let at_image = [(0.0, 0.0), (iw, 0.0), (iw, ih), (0.0, ih)][(corner + 2) % 4];
    let base = base_scale(iw, ih, w, h);
    let s = ((p.x - anchor.x).abs() / iw)
        .max((p.y - anchor.y).abs() / ih)
        .clamp(base * 0.05, base * 50.0);
    let x0 = anchor.x - at_image.0 * s;
    let y0 = anchor.y - at_image.1 * s;
    (
        s / base,
        ((x0 - (w - iw * s) / 2.0) / w, (y0 - (h - ih * s) / 2.0) / h),
    )
}

impl Editor {
    fn crop_frame(&self, t: &Target) -> Option<[Point; 4]> {
        let s = self.session.borrow();
        let img = dv_render::image::image_size(s.exec.doc().page(), t.image.asset)?;
        Some(frame_corners(&t.image, img, t.size))
    }

    /// Starts dragging a corner handle under the pointer; false if there is none.
    pub(super) fn press_crop_corner(&mut self) -> bool {
        let Some(t) = self.crop_target() else {
            return false;
        };
        let Some(c) = self.crop_frame(&t) else {
            return false;
        };
        let to_screen = t.to_local.inverse();
        let cp = self.canvas_point();
        let Some(corner) = (0..4).find(|i| (to_screen * c[*i]).distance(cp) <= GRAB) else {
            return false;
        };
        let key = self.next_drag_key();
        self.interaction = Some(Interaction::CropCorner(CornerDrag {
            key,
            id: t.id,
            corner,
        }));
        true
    }

    pub(super) fn crop_corner_hover(&self) -> bool {
        let Some(t) = self.crop_target() else {
            return false;
        };
        let Some(c) = self.crop_frame(&t) else {
            return false;
        };
        let to_screen = t.to_local.inverse();
        let cp = self.canvas_point();
        self.over_canvas() && (0..4).any(|i| (to_screen * c[i]).distance(cp) <= GRAB)
    }

    pub(super) fn drag_crop_corner(&mut self, d: CornerDrag) {
        let Some(t) = self.crop_target() else { return };
        let Some(img) = ({
            let s = self.session.borrow();
            dv_render::image::image_size(s.exec.doc().page(), t.image.asset)
        }) else {
            return;
        };
        let p = t.to_local * self.canvas_point();
        let (scale, offset) = scaled_about(d.corner, p, &t.image, img, t.size);
        let ex = &mut self.session.borrow_mut().exec;
        edit_fill_at(ex, &[d.id], Some(d.key), self.fill_idx(), |old| {
            old.map(|p| match p {
                Paint::Image(i) => Paint::Image(ImageFill { scale, offset, ..i }),
                other => other,
            })
        });
    }
}

pub(super) fn draw(ed: &Editor, cx: &mut Cx) {
    let Some(t) = ed.crop_target() else { return };
    let Some(c) = ed.crop_frame(&t) else { return };
    let to_screen = t.to_local.inverse();
    let pts: Vec<Point> = c.iter().map(|p| to_screen * *p).collect();
    let mut path = BezPath::new();
    path.move_to(pts[0]);
    for p in &pts[1..] {
        path.line_to(*p);
    }
    path.close_path();
    cx.stroke_path(&path, 3.0, alpha(WHITE, 0.9));
    cx.stroke_path(&path, 1.0, hex(accent()));
    for p in pts {
        let r = Rect::new(p.x - HANDLE, p.y - HANDLE, p.x + HANDLE, p.y + HANDLE);
        cx.fill(r.inflate(1.0, 1.0), 2.0, hex(accent()));
        cx.fill(r, 1.5, hex(WHITE));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const IMG: (f64, f64) = (200.0, 100.0);
    const BOX: (f64, f64) = (100.0, 100.0);

    #[test]
    fn the_frame_of_an_uncropped_fill_covers_the_box() {
        let c = frame_corners(&ImageFill::new(1), IMG, BOX);
        // The wide image is scaled to the box height and centered: 200 x 100 at x = -50.
        assert_eq!(c[0], Point::new(-50.0, 0.0));
        assert_eq!(c[2], Point::new(150.0, 100.0));
    }

    #[test]
    fn dragging_a_corner_scales_around_the_opposite_one() {
        let mut f = ImageFill::new(1);
        f.mode = dv_core::ImageMode::Crop;
        let before = frame_corners(&f, IMG, BOX);
        // Drag the bottom-right corner 50 px further along the width.
        let (scale, offset) = scaled_about(2, Point::new(200.0, 100.0), &f, IMG, BOX);
        let after = frame_corners(&ImageFill { scale, offset, ..f }, IMG, BOX);
        assert_eq!(after[0], before[0], "top-left stays");
        assert!((after[2].x - 200.0).abs() < 1e-6);
        assert!(scale > 1.0);
        // The same drag from the top-left corner keeps the bottom-right one.
        let (scale, offset) = scaled_about(0, Point::new(-100.0, -50.0), &f, IMG, BOX);
        let after = frame_corners(&ImageFill { scale, offset, ..f }, IMG, BOX);
        assert!((after[2].x - before[2].x).abs() < 1e-6 && (after[2].y - before[2].y).abs() < 1e-6);
        // The proportions are kept: the larger of the two drags decides the size.
        assert!((after[0].x + 150.0).abs() < 1e-6 && (after[0].y + 50.0).abs() < 1e-6);
    }

    #[test]
    fn the_zoom_stays_in_a_sane_range() {
        let f = ImageFill::new(1);
        let (tiny, _) = scaled_about(2, Point::new(-50.0, 0.0), &f, IMG, BOX);
        assert!(tiny >= 0.05 - 1e-9);
        let (huge, _) = scaled_about(2, Point::new(1e9, 1e9), &f, IMG, BOX);
        assert!(huge <= 50.0 + 1e-9);
    }
}
