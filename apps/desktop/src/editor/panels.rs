//! Top bar and side panels of the editor: layers of the document and properties of the
//! selection (values are shown, editing them comes with the number-field widget).

use dv_core::{Color, NodeKind};
use dv_ui::draw::Cx;
use dv_ui::icons::Icon;
use dv_ui::theme::colors::*;
use dv_ui::theme::metrics::*;
use kurbo::{Point, Rect};
use vello::peniko::Color as VColor;

use super::canvas::num;
use super::state::Editor;
use crate::i18n::{Msg, objects_count, tr};

/// Distance from the top of the left panel to the first layer row.
const LAYERS_TOP: f64 = 188.0;

fn divider(cx: &mut Cx, x0: f64, x1: f64, y: f64) {
    cx.fill(Rect::new(x0, y, x1, y + 1.0), 0.0, alpha(overlay(), 0.08));
}

/// Active tab label with an accent underline.
fn tab(cx: &mut Cx, label: &str, x: f64, y: f64, active: bool) -> f64 {
    let color = if active {
        hex(text_primary())
    } else {
        hex(text_secondary())
    };
    let w = cx.text(label, FONT_UI, x, y + 18.0, color);
    if active {
        cx.fill(Rect::new(x, y + 27.0, x + w, y + 29.0), 1.0, hex(accent()));
    }
    w
}

/// Rectangle of the `i`-th layer row.
pub fn layer_row_rect(left: Rect, i: usize) -> Rect {
    let y = left.y0 + LAYERS_TOP + i as f64 * ROW_H;
    Rect::new(left.x0 + 8.0, y, left.x1 - 9.0, y + ROW_H)
}

/// The eye button at the end of a layer row.
pub fn eye_rect(row: Rect) -> Rect {
    Rect::new(row.x1 - 32.0, row.y0, row.x1, row.y1)
}

/// How many layer rows fit into the left panel.
pub fn max_layer_rows(left: Rect) -> usize {
    (((left.height() - LAYERS_TOP - 8.0) / ROW_H)
        .floor()
        .max(0.0)) as usize
}

fn kind_icon(kind: NodeKind) -> Icon {
    match kind {
        NodeKind::Frame => Icon::Frame,
        NodeKind::Rectangle => Icon::Square,
        NodeKind::Ellipse => Icon::Circle,
    }
}

fn field(cx: &mut Cx, r: Rect, label: &str, value: &str) {
    cx.fill(r, R_FIELD, hex(bg_raised()));
    cx.text(
        label,
        FONT_SM,
        r.x0 + 9.0,
        r.y0 + 18.0,
        hex(text_disabled()),
    );
    cx.text(
        value,
        FONT_UI,
        r.x0 + 28.0,
        r.y0 + 18.0,
        hex(text_primary()),
    );
}

pub fn topbar(cx: &mut Cx, r: Rect, ed: &Editor) {
    cx.fill(r, 0.0, hex(bg_panel()));
    divider(cx, r.x0, r.x1, r.y1 - 1.0);
    let mut mx = 16.0;
    for item in [Msg::MenuFile, Msg::MenuEdit, Msg::MenuView] {
        mx += cx.text(tr(item), FONT_UI, mx, 25.0, hex(text_secondary())) + 20.0;
    }

    let title = format!("{}  /  {}", ed.project, tr(Msg::Page1));
    let title = cx.fit(&title, FONT_UI, (r.width() - 2.0 * (mx + 220.0)).max(80.0));
    let w = cx.text_width(&title, FONT_UI);
    cx.text(
        &title,
        FONT_UI,
        r.center().x - w / 2.0,
        25.0,
        hex(text_secondary()),
    );

    let share_w = 29.0 + cx.text_width(tr(Msg::Share), FONT_UI) + 14.0;
    let share = Rect::new(r.x1 - 12.0 - share_w, 7.0, r.x1 - 12.0, 33.0);
    cx.fill(share, R_BUTTON, hex(accent()));
    cx.icon(Icon::Share, share.x0 + 10.0, 13.0, 14.0, hex(WHITE));
    cx.text(tr(Msg::Share), FONT_UI, share.x0 + 29.0, 24.0, hex(WHITE));
    let zoom = format!("{:.0}%", ed.session.borrow().vp.zoom * 100.0);
    cx.text(&zoom, FONT_UI, share.x0 - 52.0, 25.0, hex(text_secondary()));
}

pub fn left_panel(cx: &mut Cx, r: Rect, ed: &Editor) {
    cx.fill(r, 0.0, hex(bg_panel()));
    cx.fill(
        Rect::new(r.x1 - 1.0, r.y0, r.x1, r.y1),
        0.0,
        alpha(overlay(), 0.08),
    );
    let (x0, x1, y0) = (r.x0, r.x1 - 1.0, r.y0);

    let w = tab(cx, tr(Msg::TabLayers), x0 + 12.0, y0 + 8.0, true);
    tab(cx, tr(Msg::TabAssets), x0 + 24.0 + w, y0 + 8.0, false);
    divider(cx, x0, x1, y0 + 38.0);

    let search = Rect::new(x0 + 12.0, y0 + 48.0, x1 - 12.0, y0 + 48.0 + FIELD_H);
    cx.fill(search, R_FIELD, hex(bg_raised()));
    cx.icon(
        Icon::Search,
        search.x0 + 8.0,
        search.y0 + 7.0,
        14.0,
        hex(text_secondary()),
    );
    cx.text(
        tr(Msg::SearchLayers),
        FONT_UI,
        search.x0 + 30.0,
        search.y0 + 18.0,
        hex(text_disabled()),
    );

    cx.text(
        tr(Msg::Pages),
        FONT_SM,
        x0 + 12.0,
        y0 + 98.0,
        hex(text_secondary()),
    );
    cx.icon(
        Icon::Plus,
        x1 - 28.0,
        y0 + 86.0,
        14.0,
        hex(text_secondary()),
    );
    let page_row = Rect::new(x0 + 8.0, y0 + 108.0, x1 - 8.0, y0 + 108.0 + ROW_H);
    cx.fill(page_row, R_BUTTON, alpha(accent(), 0.16));
    cx.text(
        tr(Msg::Page1),
        FONT_UI,
        page_row.x0 + 12.0,
        page_row.y0 + 18.0,
        hex(text_primary()),
    );
    divider(cx, x0, x1, y0 + 176.0);

    cx.clip(Rect::new(x0, y0 + 177.0, x1, r.y1), |cx| {
        for (i, row) in ed.layer_rows(max_layer_rows(r)).iter().enumerate() {
            layer_row(cx, ed, layer_row_rect(r, i), row);
        }
    });
}

fn layer_row(cx: &mut Cx, ed: &Editor, rect: Rect, row: &super::state::LayerRow) {
    let selected = ed.selection.contains(&row.id);
    if selected {
        cx.fill(rect, R_BUTTON, alpha(accent(), 0.22));
    } else if ed.hover == Some(row.id) || rect.contains(ed.mouse) {
        cx.fill(rect, R_BUTTON, alpha(bg_hover(), 0.7));
    }
    let strong = if row.visible {
        hex(text_primary())
    } else {
        hex(text_disabled())
    };
    let x = rect.x0 + 12.0 + row.depth as f64 * 14.0;
    cx.icon(kind_icon(row.kind), x, rect.y0 + 7.0, 14.0, strong);
    let name = cx.fit(&row.name, FONT_UI, (rect.x1 - 40.0 - (x + 22.0)).max(20.0));
    cx.text(&name, FONT_UI, x + 22.0, rect.y0 + 18.0, strong);
    let eye = eye_rect(rect);
    let a = if row.visible { 0.55 } else { 0.25 };
    cx.icon(
        Icon::Eye,
        eye.x0 + 9.0,
        rect.y0 + 7.0,
        14.0,
        alpha(text_secondary(), a),
    );
}

pub fn right_panel(cx: &mut Cx, r: Rect, ed: &Editor) {
    cx.fill(r, 0.0, hex(bg_panel()));
    cx.fill(
        Rect::new(r.x0, r.y0, r.x0 + 1.0, r.y1),
        0.0,
        alpha(overlay(), 0.08),
    );
    let (x0, x1, y0) = (r.x0 + 1.0, r.x1, r.y0);

    let w1 = tab(cx, tr(Msg::TabDesign), x0 + 12.0, y0 + 8.0, true);
    let w2 = tab(cx, tr(Msg::TabPrototype), x0 + 24.0 + w1, y0 + 8.0, false);
    tab(
        cx,
        tr(Msg::TabInspect),
        x0 + 36.0 + w1 + w2,
        y0 + 8.0,
        false,
    );
    divider(cx, x0, x1, y0 + 38.0);

    let info = selection_info(ed);
    let Some(info) = info else {
        let hint = tr(Msg::NothingSelected);
        let w = cx.text_width(hint, FONT_UI);
        cx.text(
            hint,
            FONT_UI,
            (x0 + x1 - w) / 2.0,
            y0 + 90.0,
            hex(text_disabled()),
        );
        return;
    };

    cx.icon(info.icon, x0 + 12.0, y0 + 52.0, 14.0, hex(text_secondary()));
    let title = cx.fit(&info.title, FONT_MD, x1 - x0 - 24.0 - 24.0);
    cx.text(&title, FONT_MD, x0 + 34.0, y0 + 64.0, hex(text_primary()));

    let fw = (x1 - x0 - 24.0 - GAP) / 2.0;
    for (i, (label, value)) in info.fields.iter().enumerate() {
        let fx = x0 + 12.0 + (i % 2) as f64 * (fw + GAP);
        let fy = y0 + 80.0 + (i / 2) as f64 * (FIELD_H + GAP);
        field(cx, Rect::new(fx, fy, fx + fw, fy + FIELD_H), label, value);
    }

    let fills_y = y0 + 80.0 + 3.0 * (FIELD_H + GAP) + 8.0;
    divider(cx, x0, x1, fills_y);
    cx.text(
        tr(Msg::SectionFill),
        FONT_SM,
        x0 + 12.0,
        fills_y + 24.0,
        hex(text_secondary()),
    );
    match info.fill {
        Fill::None => {}
        Fill::Mixed => {
            cx.text(
                tr(Msg::Mixed),
                FONT_UI,
                x0 + 12.0,
                fills_y + 52.0,
                hex(text_secondary()),
            );
        }
        Fill::Solid(c) => {
            let sw = Rect::new(x0 + 12.0, fills_y + 36.0, x0 + 32.0, fills_y + 56.0);
            cx.fill(sw, R_FIELD, VColor::from_rgba8(c.r, c.g, c.b, c.a));
            cx.stroke(sw, R_FIELD, 1.0, alpha(overlay(), 0.2));
            let hexs = format!("{:02X}{:02X}{:02X}", c.r, c.g, c.b);
            cx.text(
                &hexs,
                FONT_UI,
                x0 + 42.0,
                fills_y + 50.0,
                hex(text_primary()),
            );
            let pct = format!("{:.0}%", f64::from(c.a) / 255.0 * 100.0);
            let w = cx.text_width(&pct, FONT_UI);
            cx.text(
                &pct,
                FONT_UI,
                x1 - 12.0 - w,
                fills_y + 50.0,
                hex(text_secondary()),
            );
        }
    }
}

enum Fill {
    None,
    Mixed,
    Solid(Color),
}

/// What the right panel shows about the current selection.
struct Info {
    icon: Icon,
    title: String,
    /// Label and value pairs in two columns: X, Y, W, H, rotation, radius.
    fields: Vec<(&'static str, String)>,
    fill: Fill,
}

fn selection_info(ed: &Editor) -> Option<Info> {
    let s = ed.session.borrow();
    let page = s.exec.doc().page();
    let nodes: Vec<_> = ed
        .selection
        .iter()
        .filter_map(|id| page.node(*id))
        .collect();
    match nodes.as_slice() {
        [] => None,
        [n] => Some(Info {
            icon: kind_icon(n.kind),
            title: n.name.clone(),
            fields: vec![
                ("X", num(n.geom.x)),
                ("Y", num(n.geom.y)),
                ("W", num(n.geom.w)),
                ("H", num(n.geom.h)),
                ("°", num(n.geom.rotation)),
                ("R", num(n.radius)),
            ],
            fill: n.fill.map_or(Fill::None, Fill::Solid),
        }),
        many => {
            let b = dv_geom::selection_bounds(page, &ed.selection)?;
            let mixed = tr(Msg::Mixed).to_string();
            let first = many[0].fill;
            let same_fill = many.iter().all(|n| n.fill == first);
            let fill = match (same_fill, first) {
                (true, Some(c)) => Fill::Solid(c),
                (true, None) => Fill::None,
                (false, _) => Fill::Mixed,
            };
            let _ = Point::ZERO;
            Some(Info {
                icon: Icon::Square,
                title: objects_count(many.len() as u64),
                fields: vec![
                    ("X", num(b.x0)),
                    ("Y", num(b.y0)),
                    ("W", num(b.width())),
                    ("H", num(b.height())),
                    ("°", mixed.clone()),
                    ("R", mixed),
                ],
                fill,
            })
        }
    }
}
