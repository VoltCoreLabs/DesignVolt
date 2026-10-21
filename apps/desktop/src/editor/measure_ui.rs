//! Alt held: distances between the selection and the shape under the pointer.

use dv_core::Page;
use dv_render::Viewport;
use dv_ui::draw::Cx;
use dv_ui::theme::colors::*;
use kurbo::{BezPath, Point, Rect};

use super::canvas::num;
use super::state::Editor;

const FONT: f32 = 10.0;

pub(super) fn draw(ed: &Editor, cx: &mut Cx, vp: &Viewport, page: &Page) {
    if !ed.mods.alt || ed.selection.is_empty() {
        return;
    }
    let Some(target) = ed.hover else { return };
    if ed.selection.contains(&target) {
        return;
    }
    let (Some(from), Some(to)) = (
        dv_geom::selection_bounds(page, &ed.selection),
        dv_geom::selection_bounds(page, &[target]),
    ) else {
        return;
    };
    for m in dv_geom::measure(from, to) {
        let (a, b) = (vp.to_screen(m.a), vp.to_screen(m.b));
        let mut path = BezPath::new();
        path.move_to(a);
        path.line_to(b);
        cx.stroke_path(&path, 1.0, hex(danger()));
        let horizontal = (a.y - b.y).abs() < 1e-6;
        for p in [a, b] {
            let cap = if horizontal {
                Rect::new(p.x - 0.5, p.y - 4.0, p.x + 0.5, p.y + 4.0)
            } else {
                Rect::new(p.x - 4.0, p.y - 0.5, p.x + 4.0, p.y + 0.5)
            };
            cx.fill(cap, 0.0, hex(danger()));
        }
        let label = num(m.len);
        let w = cx.text_width(&label, FONT);
        let mid = Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
        let pill = Rect::new(
            mid.x - w / 2.0 - 5.0,
            mid.y - 8.0,
            mid.x + w / 2.0 + 5.0,
            mid.y + 8.0,
        );
        cx.fill(pill, 4.0, hex(danger()));
        cx.text(&label, FONT, pill.x0 + 5.0, pill.y1 - 4.5, hex(WHITE));
    }
}
