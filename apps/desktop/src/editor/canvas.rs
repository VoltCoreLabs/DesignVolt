//! Canvas drawing: the document (through `dv-render`), selection overlay, frame titles,
//! drawing previews and the small status line.

use std::time::Instant;

use dv_core::{NodeId, NodeKind, Page};
use dv_geom::world_affine;
use dv_render::Viewport;
use dv_ui::draw::Cx;
use dv_ui::icons::Icon;
use dv_ui::theme::colors::*;
use dv_ui::theme::metrics::*;
use kurbo::{Affine, BezPath, Point, Rect};

use super::handles::{HANDLE_PX, Handle};
use super::interact::Interaction;
use super::state::Editor;
use crate::i18n::{Msg, objects_count, tr};
use crate::input::Cursor;

const LABEL_PX: f32 = 11.0;

/// The title above a root frame; clicking it selects the frame.
pub(super) struct FrameLabel {
    pub id: NodeId,
    pub rect: Rect,
    pub text: String,
}

pub(super) fn frame_labels(ed: &Editor) -> Vec<FrameLabel> {
    let s = ed.session.borrow();
    let page = s.exec.doc().page();
    let c = ed.layout().canvas;
    let (w, h) = (c.width(), c.height());
    page.roots()
        .iter()
        .filter_map(|id| {
            let n = page.node(*id)?;
            if n.kind != NodeKind::Frame || !n.visible {
                return None;
            }
            let b = dv_geom::world_bounds(page, *id)?;
            let tl = s.vp.to_screen(Point::new(b.x0, b.y0));
            let text_w = ed.assets.font.layout(&n.name, LABEL_PX).1 as f64;
            let rect = Rect::new(tl.x, tl.y - 20.0, tl.x + text_w + 8.0, tl.y - 2.0);
            let visible = rect.x1 >= 0.0 && rect.x0 <= w && rect.y1 >= 0.0 && rect.y0 <= h;
            visible.then(|| FrameLabel {
                id: *id,
                rect,
                text: n.name.clone(),
            })
        })
        .collect()
}

pub(super) fn frame_label_at(ed: &Editor, p: Point) -> Option<NodeId> {
    frame_labels(ed)
        .into_iter()
        .find(|l| l.rect.contains(p))
        .map(|l| l.id)
}

pub(super) fn draw(ed: &Editor, cx: &mut Cx, canvas: Rect) {
    cx.fill(canvas, 0.0, hex(bg_canvas()));
    let size = (canvas.width(), canvas.height());
    let origin = Affine::translate((canvas.x0, canvas.y0));
    let s = ed.session.borrow();
    let page = s.exec.doc().page();
    cx.clip(canvas, |cx| {
        // Infinite dot grid, like Figma: only the visible dots are generated.
        let (minor, major) = (alpha(overlay(), 0.18), alpha(overlay(), 0.34));
        dv_render::paint_dots(cx.scene, &s.vp, size, cx.xf * origin, minor, major);
        let started = Instant::now();
        let stats = dv_render::paint_page(cx.scene, page, &ed.index, &s.vp, size, cx.xf * origin);
        ed.paint_ms.set(started.elapsed().as_secs_f32() * 1000.0);
        ed.paint_stats.set(stats);

        let outer = cx.xf;
        cx.xf = outer * origin;
        paint_overlay(ed, cx, &s.vp, page, size);
        super::guides::draw(ed, cx, &s.vp, size);
        super::grad_handles::draw(ed, cx, &s.vp);
        super::image_crop_frame::draw(ed, cx);
        super::pen::draw(ed, cx);
        status_line(ed, cx, &s.vp, page.len(), size);
        if page.is_empty() && ed.interaction.is_none() {
            let hint = tr(Msg::EmptyCanvasHint);
            let w = cx.text_width(hint, FONT_MD);
            cx.text(
                hint,
                FONT_MD,
                (size.0 - w) / 2.0,
                size.1 / 2.0,
                hex(text_disabled()),
            );
        }
        cx.xf = outer;
    });
}

fn status_line(ed: &Editor, cx: &mut Cx, vp: &Viewport, count: usize, size: (f64, f64)) {
    let text = format!(
        "{:.0}%  ·  {}  ·  {:.1} ms",
        vp.zoom * 100.0,
        objects_count(count as u64),
        ed.paint_ms.get()
    );
    cx.text(&text, LABEL_PX, 14.0, size.1 - 14.0, hex(text_secondary()));
}

/// Outline of a node (follows its rotation) in canvas screen space.
fn outline(cx: &mut Cx, page: &Page, vp: &Viewport, id: NodeId, width: f64, color: u32, a: f32) {
    let Some(n) = page.node(id) else { return };
    if n.kind == NodeKind::Group {
        if let Some(b) = dv_geom::selection_bounds(page, &[id]) {
            let (p0, p1) = (
                vp.to_screen(Point::new(b.x0, b.y0)),
                vp.to_screen(Point::new(b.x1, b.y1)),
            );
            cx.stroke(
                Rect::new(p0.x, p0.y, p1.x, p1.y),
                0.0,
                width,
                alpha(color, a),
            );
        }
        return;
    }
    let m = vp.transform() * world_affine(page, id);
    let g = &n.geom;
    let [a0, a1, a2, a3] =
        [(0.0, 0.0), (g.w, 0.0), (g.w, g.h), (0.0, g.h)].map(|(x, y)| m * Point::new(x, y));
    let mut path = BezPath::new();
    path.move_to(a0);
    path.line_to(a1);
    path.line_to(a2);
    path.line_to(a3);
    path.close_path();
    cx.stroke_path(&path, width, alpha(color, a));
}

fn paint_overlay(ed: &Editor, cx: &mut Cx, vp: &Viewport, page: &Page, size: (f64, f64)) {
    if let Some(h) = ed.hover
        && !ed.selection.contains(&h)
    {
        outline(cx, page, vp, h, 1.5, accent(), 0.8);
    }
    for l in frame_labels(ed) {
        let selected = ed.selection.contains(&l.id);
        let color = if selected {
            hex(accent())
        } else if ed.hover == Some(l.id) {
            hex(text_primary())
        } else {
            hex(text_secondary())
        };
        cx.text(&l.text, LABEL_PX, l.rect.x0 + 2.0, l.rect.y1 - 5.0, color);
    }
    super::measure_ui::draw(ed, cx, vp, page);
    for g in &ed.guides {
        draw_guide(cx, vp, g);
    }
    for id in &ed.selection {
        outline(cx, page, vp, *id, 1.5, accent(), 1.0);
    }
    if let Some(b) = ed.selection_screen_bounds() {
        let moving = matches!(ed.interaction, Some(Interaction::Move { .. }));
        if ed.selection.len() > 1 {
            cx.stroke(b, 0.0, 1.0, alpha(accent(), 0.8));
        }
        if !moving && let Some(frame) = ed.selection_frame() {
            // The handles sit on the frame, which follows a rotated shape.
            for h in Handle::ALL {
                let p = h.position_on(&frame.quad);
                let r = Rect::new(
                    p.x - HANDLE_PX / 2.0,
                    p.y - HANDLE_PX / 2.0,
                    p.x + HANDLE_PX / 2.0,
                    p.y + HANDLE_PX / 2.0,
                );
                cx.fill(r, 2.0, hex(WHITE));
                cx.stroke(r, 2.0, 1.5, hex(accent()));
            }
        }
        size_pill(cx, b, vp.zoom);
    }
    match &ed.interaction {
        Some(Interaction::Create {
            current: Some(r), ..
        }) => {
            let (p0, p1) = (
                vp.to_screen(Point::new(r.x0, r.y0)),
                vp.to_screen(Point::new(r.x1, r.y1)),
            );
            let screen = Rect::new(p0.x, p0.y, p1.x, p1.y);
            cx.fill(screen, 0.0, alpha(accent(), 0.12));
            cx.stroke(screen, 0.0, 1.5, hex(accent()));
        }
        Some(Interaction::Marquee { start, .. }) => {
            let (p0, p1) = (vp.to_screen(*start), ed.canvas_point());
            let r = Rect::from_points(p0, p1);
            cx.fill(r, 0.0, alpha(accent(), 0.08));
            cx.stroke(r, 0.0, 1.0, alpha(accent(), 0.8));
        }
        Some(Interaction::Rotate {
            delta, originals, ..
        }) => {
            // One shape shows its own angle, a group shows how far it was turned.
            let angle = match originals[..] {
                [(id, ..)] => page.node(id).map_or(*delta, |n| n.geom.rotation),
                _ => *delta,
            };
            let p = ed.canvas_point();
            angle_pill(
                cx,
                Point::new(p.x + 18.0, p.y + 18.0),
                &format!("{}°", num(angle)),
            );
        }
        _ => {}
    }
    if ed.editor_cursor() == Cursor::Rotate {
        rotate_badge(cx, ed.canvas_point());
    }
    let _ = size;
}

/// The turn cursor: a curved arrow on a round white badge, drawn in place of the system cursor.
fn rotate_badge(cx: &mut Cx, p: Point) {
    const R: f64 = 13.0;
    let r = Rect::new(p.x - R, p.y - R, p.x + R, p.y + R);
    cx.fill(r, R, alpha(WHITE, 0.96));
    cx.stroke(r, R, 1.0, alpha(overlay(), 0.35));
    cx.icon(Icon::RotateCw, p.x - 8.0, p.y - 8.0, 16.0, hex(0x1B1F31));
}

/// "W × H" label under the selection box.
fn size_pill(cx: &mut Cx, b: Rect, zoom: f64) {
    let label = format!("{} × {}", num(b.width() / zoom), num(b.height() / zoom));
    let w = cx.text_width(&label, LABEL_PX);
    let pill = Rect::new(
        b.center().x - w / 2.0 - 8.0,
        b.y1 + 8.0,
        b.center().x + w / 2.0 + 8.0,
        b.y1 + 28.0,
    );
    cx.fill(pill, 6.0, hex(accent()));
    cx.text(&label, LABEL_PX, pill.x0 + 8.0, pill.y1 - 6.0, hex(WHITE));
}

/// A smart-guide line (page space) drawn in canvas screen space.
fn draw_guide(cx: &mut Cx, vp: &Viewport, g: &dv_geom::Guide) {
    let (a, b) = match g.axis {
        dv_geom::Axis::Vertical => (Point::new(g.pos, g.from), Point::new(g.pos, g.to)),
        dv_geom::Axis::Horizontal => (Point::new(g.from, g.pos), Point::new(g.to, g.pos)),
    };
    let (a, b) = (vp.to_screen(a), vp.to_screen(b));
    let mut path = BezPath::new();
    path.move_to(a);
    path.line_to(b);
    cx.stroke_path(&path, 1.0, hex(danger()));
    for end in [a, b] {
        let r = Rect::new(end.x - 3.0, end.y - 3.0, end.x + 3.0, end.y + 3.0);
        cx.stroke(r, 0.0, 1.0, hex(danger()));
    }
}

/// A small accent label (angle, size) at `at`.
fn angle_pill(cx: &mut Cx, at: Point, label: &str) {
    let w = cx.text_width(label, LABEL_PX);
    let pill = Rect::new(at.x, at.y, at.x + w + 16.0, at.y + 20.0);
    cx.fill(pill, 6.0, hex(accent()));
    cx.text(label, LABEL_PX, pill.x0 + 8.0, pill.y1 - 6.0, hex(WHITE));
}

/// A number without a pointless fraction: 12, 12.5.
pub(super) fn num(v: f64) -> String {
    if (v - v.round()).abs() < 0.05 {
        format!("{:.0}", v.round() + 0.0)
    } else {
        format!("{v:.1}")
    }
}

#[cfg(test)]
mod tests {
    use super::num;

    #[test]
    fn numbers_drop_insignificant_fractions() {
        assert_eq!(num(12.0), "12");
        assert_eq!(num(12.02), "12");
        assert_eq!(num(12.5), "12.5");
        assert_eq!(num(-0.01), "0");
        assert_eq!(num(99.96), "100");
    }
}
