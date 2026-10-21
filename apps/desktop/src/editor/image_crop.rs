//! Cropping an image by hand: while the picker edits an image fill in crop mode, dragging the
//! shape on the canvas moves the image and the wheel zooms it.

use dv_core::{ImageFill, ImageMode, NodeId, Paint};
use kurbo::{Affine, Point};

use super::interact::Interaction;
use super::ops_paint::edit_fill_at;
use super::state::Editor;

/// What a crop drag started from.
#[derive(Clone, Copy, Debug)]
pub struct CropDrag {
    pub key: u64,
    pub id: NodeId,
    /// Pointer position in the shape's own coordinates when the drag began.
    pub start: Point,
    pub offset: (f64, f64),
}

/// Offset after the pointer moved from `start` to `now` (shape coordinates, box `w` x `h`).
pub fn dragged_offset(d: &CropDrag, now: Point, w: f64, h: f64) -> (f64, f64) {
    (
        (d.offset.0 + (now.x - d.start.x) / w).clamp(-50.0, 50.0),
        (d.offset.1 + (now.y - d.start.y) / h).clamp(-50.0, 50.0),
    )
}

/// The zoom after a wheel movement of `dy` px (wheel up zooms in).
pub fn zoomed(scale: f64, dy: f64) -> f64 {
    (scale * (dy * 0.002).exp()).clamp(0.05, 50.0)
}

struct Target {
    id: NodeId,
    to_local: Affine,
    size: (f64, f64),
    image: ImageFill,
}

impl Editor {
    /// The single selected shape whose image is being cropped, with the way from the canvas
    /// to the shape's own coordinates.
    fn crop_target(&self) -> Option<Target> {
        self.picker.as_ref()?;
        let image = self.current_image().filter(|i| i.mode == ImageMode::Crop)?;
        let s = self.session.borrow();
        let page = s.exec.doc().page();
        let [id] = page.geo_tops(&self.selection)[..] else {
            return None;
        };
        let n = page.node(id)?;
        let to_screen = s.vp.transform() * dv_geom::world_affine(page, id);
        Some(Target {
            id,
            to_local: to_screen.inverse(),
            size: (n.geom.w, n.geom.h),
            image,
        })
    }

    fn crop_hit(&self) -> Option<Target> {
        let t = self.crop_target()?;
        let p = t.to_local * self.canvas_point();
        let inside = p.x >= 0.0 && p.y >= 0.0 && p.x <= t.size.0 && p.y <= t.size.1;
        inside.then_some(t)
    }

    pub(super) fn crop_hover(&self) -> bool {
        self.over_canvas() && self.crop_hit().is_some()
    }

    /// Starts moving the image when the press is on the shape being cropped.
    pub(super) fn press_crop(&mut self) -> bool {
        if !self.over_canvas() {
            return false;
        }
        let Some(t) = self.crop_hit() else {
            return false;
        };
        let key = self.next_drag_key();
        self.interaction = Some(Interaction::Crop(CropDrag {
            key,
            id: t.id,
            start: t.to_local * self.canvas_point(),
            offset: t.image.offset,
        }));
        true
    }

    pub(super) fn drag_crop(&mut self, d: CropDrag) {
        let Some(t) = self.crop_target() else { return };
        let now = t.to_local * self.canvas_point();
        let offset = dragged_offset(&d, now, t.size.0, t.size.1);
        let ex = &mut self.session.borrow_mut().exec;
        edit_fill_at(ex, &[d.id], Some(d.key), self.fill_idx(), |old| {
            old.map(|p| match p {
                Paint::Image(i) => Paint::Image(ImageFill { offset, ..i }),
                other => other,
            })
        });
    }

    /// Zooms the image of the shape under the pointer; false when the wheel is not for it.
    pub(super) fn wheel_crop(&mut self, dy: f64) -> bool {
        let Some(t) = self.crop_hit() else {
            return false;
        };
        self.set_image_scale(zoomed(t.image.scale, dy));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dragging_moves_the_image_by_a_fraction_of_the_shape() {
        let d = CropDrag {
            key: 1,
            id: NodeId(1),
            start: Point::new(10.0, 10.0),
            offset: (0.1, 0.0),
        };
        let (x, y) = dragged_offset(&d, Point::new(60.0, 0.0), 100.0, 50.0);
        assert!((x - 0.6).abs() < 1e-9 && (y + 0.2).abs() < 1e-9);
    }

    #[test]
    fn the_wheel_zooms_in_and_out_within_limits() {
        assert!(zoomed(1.0, 100.0) > 1.0);
        assert!(zoomed(1.0, -100.0) < 1.0);
        assert_eq!(zoomed(1.0, 1e6), 50.0);
        assert_eq!(zoomed(1.0, -1e6), 0.05);
    }
}
