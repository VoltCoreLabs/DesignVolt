//! Zoom and pan of the infinite canvas. Screen coordinates are relative to the canvas
//! top-left corner; `pan` is where the page origin lands on the screen.

use vello::kurbo::{Affine, Point, Rect, Vec2};

pub const MIN_ZOOM: f64 = 0.02;
pub const MAX_ZOOM: f64 = 256.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pub zoom: f64,
    pub pan: Vec2,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: Vec2::ZERO,
        }
    }
}

impl Viewport {
    /// Page space → canvas screen space.
    pub fn transform(&self) -> Affine {
        Affine::translate(self.pan) * Affine::scale(self.zoom)
    }

    pub fn to_screen(&self, world: Point) -> Point {
        self.transform() * world
    }

    pub fn to_world(&self, screen: Point) -> Point {
        self.transform().inverse() * screen
    }

    /// Zooms by `factor` keeping the page point under `anchor` (screen) fixed.
    pub fn zoom_at(&mut self, anchor: Point, factor: f64) {
        let world = self.to_world(anchor);
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        self.pan = anchor.to_vec2() - world.to_vec2() * self.zoom;
    }

    /// Sets an absolute zoom around `anchor`.
    pub fn set_zoom_at(&mut self, anchor: Point, zoom: f64) {
        self.zoom_at(anchor, zoom / self.zoom);
    }

    pub fn pan_by(&mut self, delta: Vec2) {
        self.pan += delta;
    }

    /// The part of the page that is visible on a canvas of `size` px.
    pub fn visible_world(&self, size: (f64, f64)) -> Rect {
        let a = self.to_world(Point::ZERO);
        let b = self.to_world(Point::new(size.0, size.1));
        Rect::new(a.x, a.y, b.x, b.y)
    }

    /// Fits `content` into a canvas of `size` px with `margin` px around it (never above 100%).
    pub fn fit(&mut self, content: Rect, size: (f64, f64), margin: f64) {
        let (w, h) = (content.width().max(1.0), content.height().max(1.0));
        let zoom = ((size.0 - 2.0 * margin) / w)
            .min((size.1 - 2.0 * margin) / h)
            .clamp(MIN_ZOOM, 1.0);
        self.zoom = zoom;
        let c = content.center();
        self.pan = Vec2::new(size.0 / 2.0 - c.x * zoom, size.1 / 2.0 - c.y * zoom);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: Point, b: Point) -> bool {
        (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9
    }

    #[test]
    fn screen_and_world_are_inverse() {
        let vp = Viewport {
            zoom: 2.5,
            pan: Vec2::new(30.0, -40.0),
        };
        let p = Point::new(12.0, 34.0);
        assert!(near(vp.to_world(vp.to_screen(p)), p));
        assert!(near(vp.to_screen(Point::ZERO), Point::new(30.0, -40.0)));
    }

    #[test]
    fn zoom_keeps_the_anchor_fixed() {
        let mut vp = Viewport {
            zoom: 1.0,
            pan: Vec2::new(100.0, 50.0),
        };
        let anchor = Point::new(400.0, 300.0);
        let before = vp.to_world(anchor);
        vp.zoom_at(anchor, 3.0);
        assert!((vp.zoom - 3.0).abs() < 1e-12);
        assert!(near(vp.to_world(anchor), before));
        vp.zoom_at(anchor, 1.0 / 3.0);
        assert!(near(vp.to_world(anchor), before));
    }

    #[test]
    fn zoom_is_clamped() {
        let mut vp = Viewport::default();
        vp.zoom_at(Point::ZERO, 1e9);
        assert_eq!(vp.zoom, MAX_ZOOM);
        vp.zoom_at(Point::ZERO, 1e-9);
        assert_eq!(vp.zoom, MIN_ZOOM);
    }

    #[test]
    fn set_zoom_at_reaches_the_target() {
        let mut vp = Viewport::default();
        vp.zoom_at(Point::new(10.0, 10.0), 4.0);
        vp.set_zoom_at(Point::new(500.0, 200.0), 1.0);
        assert!((vp.zoom - 1.0).abs() < 1e-12);
    }

    #[test]
    fn visible_world_follows_pan_and_zoom() {
        let vp = Viewport {
            zoom: 2.0,
            pan: Vec2::new(-100.0, -50.0),
        };
        let r = vp.visible_world((800.0, 600.0));
        assert_eq!((r.x0, r.y0, r.x1, r.y1), (50.0, 25.0, 450.0, 325.0));
    }

    #[test]
    fn fit_centers_content_and_never_zooms_in_past_100_percent() {
        let mut vp = Viewport::default();
        vp.fit(Rect::new(0.0, 0.0, 4000.0, 2000.0), (1000.0, 800.0), 50.0);
        assert!((vp.zoom - 0.225).abs() < 1e-9, "zoom {}", vp.zoom);
        let center = vp.to_screen(Point::new(2000.0, 1000.0));
        assert!(near(center, Point::new(500.0, 400.0)));
        vp.fit(Rect::new(0.0, 0.0, 10.0, 10.0), (1000.0, 800.0), 50.0);
        assert_eq!(vp.zoom, 1.0);
    }
}
