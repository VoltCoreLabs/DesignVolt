//! The dot grid of the infinite canvas. The grid is infinite because only the dots inside the
//! visible area are generated; the step doubles or halves with the zoom so the dots keep
//! a similar density on screen.

use vello::Scene;
use vello::kurbo::{Affine, BezPath, Circle, Point, Shape};
use vello::peniko::{Color, Fill};

use crate::viewport::Viewport;

/// The dots are never closer than this on screen (px).
pub const MIN_SPACING_PX: f64 = 14.0;
/// Every `MAJOR_EVERY`-th dot (counted from the page origin) is drawn bigger.
pub const MAJOR_EVERY: i64 = 4;
/// Nothing is drawn when the view would need more dots than this (safety net).
const MAX_DOTS: usize = 40_000;

const MINOR_RADIUS: f64 = 0.9;
const MAJOR_RADIUS: f64 = 1.5;

/// Dots of the visible area, in canvas screen space.
#[derive(Debug, Default, PartialEq)]
pub struct GridDots {
    /// Grid step in page units.
    pub step: f64,
    pub minor: Vec<Point>,
    pub major: Vec<Point>,
}

/// The grid step (a power of two in page units, 1/64 at the finest) for a zoom: the smallest one whose on-screen
/// spacing is at least `MIN_SPACING_PX`.
pub fn grid_step(zoom: f64) -> f64 {
    let mut step = 1.0;
    while step * zoom < MIN_SPACING_PX && step < 1e12 {
        step *= 2.0;
    }
    // When zoomed in, the grid subdivides (down to 1/64 of a page unit).
    while step * 0.5 * zoom >= MIN_SPACING_PX && step > 1.0 / 64.0 {
        step *= 0.5;
    }
    step
}

/// All dots that fall into a canvas of `size` px.
pub fn grid_dots(vp: &Viewport, size: (f64, f64)) -> GridDots {
    let step = grid_step(vp.zoom);
    let view = vp.visible_world(size);
    let (i0, i1) = (
        (view.x0 / step).floor() as i64,
        (view.x1 / step).ceil() as i64,
    );
    let (j0, j1) = (
        (view.y0 / step).floor() as i64,
        (view.y1 / step).ceil() as i64,
    );
    let count = ((i1 - i0 + 1).max(0) as usize).saturating_mul((j1 - j0 + 1).max(0) as usize);
    let mut dots = GridDots {
        step,
        ..GridDots::default()
    };
    if count > MAX_DOTS {
        return dots;
    }
    for j in j0..=j1 {
        for i in i0..=i1 {
            let p = vp.to_screen(Point::new(i as f64 * step, j as f64 * step));
            if p.x < -2.0 || p.y < -2.0 || p.x > size.0 + 2.0 || p.y > size.1 + 2.0 {
                continue;
            }
            if i % MAJOR_EVERY == 0 && j % MAJOR_EVERY == 0 {
                dots.major.push(p);
            } else {
                dots.minor.push(p);
            }
        }
    }
    dots
}

fn dots_path(points: &[Point], radius: f64) -> BezPath {
    let mut path = BezPath::new();
    for p in points {
        path.extend(Circle::new(*p, radius).path_elements(0.5));
    }
    path
}

/// Paints the dot grid. `base` maps canvas coordinates to the scene.
pub fn paint_dots(
    scene: &mut Scene,
    vp: &Viewport,
    size: (f64, f64),
    base: Affine,
    minor: Color,
    major: Color,
) {
    let dots = grid_dots(vp, size);
    if !dots.minor.is_empty() {
        scene.fill(
            Fill::NonZero,
            base,
            minor,
            None,
            &dots_path(&dots.minor, MINOR_RADIUS),
        );
    }
    if !dots.major.is_empty() {
        scene.fill(
            Fill::NonZero,
            base,
            major,
            None,
            &dots_path(&dots.major, MAJOR_RADIUS),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vello::kurbo::Vec2;

    fn vp(zoom: f64, pan: (f64, f64)) -> Viewport {
        Viewport {
            zoom,
            pan: Vec2::new(pan.0, pan.1),
        }
    }

    #[test]
    fn the_step_keeps_dots_at_least_the_minimum_apart_but_not_much_further() {
        for zoom in [0.02, 0.05, 0.1, 0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 64.0, 256.0] {
            let screen = grid_step(zoom) * zoom;
            assert!(screen >= MIN_SPACING_PX, "zoom {zoom}: {screen}");
            assert!(screen < 2.0 * MIN_SPACING_PX, "zoom {zoom}: {screen}");
        }
        assert_eq!(grid_step(1.0), 16.0);
        assert_eq!(grid_step(0.25), 64.0);
        assert_eq!(grid_step(4.0), 4.0);
        assert_eq!(grid_step(64.0), 0.25, "zooming in subdivides the grid");
    }

    #[test]
    fn the_dot_at_the_page_origin_sits_at_the_pan() {
        let dots = grid_dots(&vp(1.0, (100.0, 60.0)), (400.0, 300.0));
        assert!(
            dots.major.contains(&Point::new(100.0, 60.0)),
            "origin is a major dot"
        );
    }

    #[test]
    fn every_dot_is_inside_the_view_and_evenly_spaced() {
        let size = (500.0, 300.0);
        let dots = grid_dots(&vp(1.0, (37.0, -12.0)), size);
        assert!(!dots.minor.is_empty());
        for p in dots.minor.iter().chain(&dots.major) {
            assert!(
                p.x >= -2.0 && p.x <= size.0 + 2.0 && p.y >= -2.0 && p.y <= size.1 + 2.0,
                "{p:?}"
            );
        }
        let mut xs: Vec<f64> = dots
            .minor
            .iter()
            .filter(|p| (p.y - dots.minor[0].y).abs() < 1e-9)
            .map(|p| p.x)
            .collect();
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        for pair in xs.windows(2) {
            assert!(pair[1] - pair[0] >= MIN_SPACING_PX - 1e-9);
        }
    }

    #[test]
    fn major_dots_are_every_fourth_step() {
        let dots = grid_dots(&vp(1.0, (0.0, 0.0)), (400.0, 400.0));
        let step_px = dots.step;
        for p in &dots.major {
            let (i, j) = (
                (p.x / step_px).round() as i64,
                (p.y / step_px).round() as i64,
            );
            assert!(i % MAJOR_EVERY == 0 && j % MAJOR_EVERY == 0);
        }
        // 400 px at 16 px per step: 26 columns, so 7 major columns (0, 4, ..., 24) in x and y.
        assert_eq!(dots.major.len(), 7 * 7);
    }

    #[test]
    fn the_grid_is_infinite_far_from_the_origin() {
        let far = vp(1.0, (-5.0e6, 3.0e6));
        let dots = grid_dots(&far, (800.0, 600.0));
        assert!(
            dots.minor.len() + dots.major.len() > 1000,
            "dots exist even 5 million units away"
        );
        let again = grid_dots(&vp(1.0, (-5.0e6 + 16.0 * 7.0, 3.0e6)), (800.0, 600.0));
        assert_eq!(
            dots.minor.len() + dots.major.len(),
            again.minor.len() + again.major.len()
        );
    }

    #[test]
    fn panning_by_one_step_moves_the_pattern_by_exactly_one_step() {
        let a = grid_dots(&vp(1.0, (0.0, 0.0)), (300.0, 200.0));
        let b = grid_dots(&vp(1.0, (16.0, 0.0)), (300.0, 200.0));
        for p in &a.minor {
            let moved = Point::new(p.x + 16.0, p.y);
            if moved.x <= 302.0 {
                assert!(
                    b.minor
                        .iter()
                        .chain(&b.major)
                        .any(|q| (q.x - moved.x).abs() < 1e-9 && (q.y - moved.y).abs() < 1e-9)
                );
            }
        }
    }

    #[test]
    fn painting_adds_drawing_commands_and_empty_views_add_none() {
        let mut scene = Scene::new();
        let c = Color::from_rgba8(0, 0, 0, 255);
        paint_dots(
            &mut scene,
            &vp(1.0, (0.0, 0.0)),
            (200.0, 100.0),
            Affine::IDENTITY,
            c,
            c,
        );
        assert!(scene.encoding().n_paths > 0);
        let mut empty = Scene::new();
        paint_dots(
            &mut empty,
            &vp(1.0, (0.0, 0.0)),
            (0.0, 0.0),
            Affine::IDENTITY,
            c,
            c,
        );
        assert!(empty.encoding().n_paths <= 2);
    }
}
