//! Pattern fills: a node of the document repeated over the shape.

use dv_core::{Node, NodeId, NodeKind, Page, Pattern, Tiling};
use dv_geom::local_affine;
use vello::Scene;
use vello::kurbo::{Affine, Point, Rect, Shape, Vec2};
use vello::peniko::Fill;

/// Most tiles drawn for one shape; more would only stall the frame.
pub const MAX_TILES: usize = 2500;

/// Top-left corners of the tiles that touch `area`, for tiles of `tw` x `th`.
pub fn tile_origins(area: Rect, tw: f64, th: f64, p: &Pattern) -> Vec<Point> {
    let (sx, sy) = (tw + p.gap_x, th + p.gap_y);
    if tw <= 0.0 || th <= 0.0 || sx <= 0.5 || sy <= 0.5 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let (r0, r1) = (
        (area.y0 / sy).floor() as i64 - 1,
        (area.y1 / sy).ceil() as i64,
    );
    let (c0, c1) = (
        (area.x0 / sx).floor() as i64 - 1,
        (area.x1 / sx).ceil() as i64,
    );
    for r in r0..=r1 {
        for c in c0..=c1 {
            let mut x = c as f64 * sx;
            let mut y = r as f64 * sy;
            match p.tiling {
                Tiling::Grid => {}
                Tiling::BrickRows if r.rem_euclid(2) == 1 => x += sx / 2.0,
                Tiling::BrickColumns if c.rem_euclid(2) == 1 => y += sy / 2.0,
                _ => {}
            }
            if x + tw > area.x0 && x < area.x1 && y + th > area.y0 && y < area.y1 {
                out.push(Point::new(x, y));
                if out.len() >= MAX_TILES {
                    return out;
                }
            }
        }
    }
    out
}

/// Draws the tile node and everything inside it; the root is placed at the origin.
fn draw_tree(scene: &mut Scene, page: &Page, id: NodeId, base: Affine, root: bool) {
    let Some(node) = page.node(id) else { return };
    if !node.visible {
        return;
    }
    let mut g = node.geom;
    if root {
        g.x = 0.0;
        g.y = 0.0;
    }
    let world = base * local_affine(&g);
    crate::paint::draw_node(scene, page, node, world, false);
    for c in &node.children {
        draw_tree(scene, page, *c, world, false);
    }
}

/// The box of a tile in the space of its source node: the node box, or for a group the
/// box around everything inside it.
pub fn tile_rect(page: &Page, id: NodeId) -> Rect {
    let Some(node) = page.node(id) else {
        return Rect::ZERO;
    };
    if node.kind != NodeKind::Group {
        return Rect::new(0.0, 0.0, node.geom.w, node.geom.h);
    }
    let inv = dv_geom::world_affine(page, id).inverse();
    page.geo_tops(&[id])
        .into_iter()
        .filter_map(|c| dv_geom::world_corners(page, c))
        .flatten()
        .map(|p| inv * p)
        .fold(None, |acc: Option<Rect>, p| {
            Some(acc.map_or(Rect::from_points(p, p), |r| r.union_pt(p)))
        })
        .unwrap_or(Rect::ZERO)
}

pub(crate) fn draw(
    scene: &mut Scene,
    page: &Page,
    world: Affine,
    shape: &impl Shape,
    node: &Node,
    p: &Pattern,
) {
    if !page.contains(p.source) {
        return;
    }
    let (w, h) = (node.geom.w, node.geom.h);
    let scale = p.scale.max(0.01);
    let tile = tile_rect(page, p.source);
    let (tw, th) = (tile.width() * scale, tile.height() * scale);
    let center = Vec2::new(w / 2.0, h / 2.0);
    let grid = Affine::translate(center)
        * Affine::rotate(p.angle.to_radians())
        * Affine::translate(-center);
    let reach = (w * w + h * h).sqrt() / 2.0;
    let area = if p.angle == 0.0 {
        Rect::new(0.0, 0.0, w, h)
    } else {
        Rect::new(
            w / 2.0 - reach,
            h / 2.0 - reach,
            w / 2.0 + reach,
            h / 2.0 + reach,
        )
    };
    scene.push_clip_layer(Fill::NonZero, world, shape);
    for o in tile_origins(area, tw, th, p) {
        let at = world
            * grid
            * Affine::translate(o.to_vec2())
            * Affine::scale(scale)
            * Affine::translate(-tile.origin().to_vec2());
        draw_tree(scene, page, p.source, at, true);
    }
    scene.pop_layer();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pat(tiling: Tiling) -> Pattern {
        Pattern {
            tiling,
            ..Pattern::new(NodeId(1))
        }
    }

    #[test]
    fn grid_tiles_cover_the_area_with_gaps() {
        let area = Rect::new(0.0, 0.0, 100.0, 100.0);
        let o = tile_origins(area, 20.0, 20.0, &pat(Tiling::Grid));
        assert_eq!(o.len(), 25);
        let mut p = pat(Tiling::Grid);
        p.gap_x = 30.0;
        p.gap_y = 30.0;
        assert_eq!(tile_origins(area, 20.0, 20.0, &p).len(), 4);
    }

    #[test]
    fn brick_rows_shift_every_other_row() {
        let area = Rect::new(0.0, 0.0, 100.0, 40.0);
        let o = tile_origins(area, 20.0, 20.0, &pat(Tiling::BrickRows));
        assert!(o.iter().any(|p| p.y == 20.0 && p.x == 10.0));
        assert!(o.iter().all(|p| p.y != 0.0 || p.x % 20.0 == 0.0));
    }

    #[test]
    fn degenerate_tiles_and_huge_areas_are_bounded() {
        let area = Rect::new(0.0, 0.0, 100.0, 100.0);
        assert!(tile_origins(area, 0.0, 10.0, &pat(Tiling::Grid)).is_empty());
        let big = Rect::new(0.0, 0.0, 1e6, 1e6);
        assert!(tile_origins(big, 10.0, 10.0, &pat(Tiling::Grid)).len() <= MAX_TILES);
    }
}
