//! Top bar and side panels of the editor: layers of the document and properties of the
//! selection (values are shown, editing them comes with the number-field widget).

use dv_core::NodeKind;
use dv_ui::draw::Cx;
use dv_ui::icons::Icon;
use dv_ui::theme::colors::*;
use dv_ui::theme::metrics::*;
use kurbo::{Point, Rect};

use super::canvas::num;
use super::flip::Action;
use super::interact::Interaction;
use super::layer_drag::Zone;
use super::session::SaveState;
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

/// The lock button left of the eye.
pub fn lock_rect(row: Rect) -> Rect {
    Rect::new(row.x1 - 62.0, row.y0, row.x1 - 32.0, row.y1)
}

/// The expand/collapse caret of a row with children.
pub fn caret_rect(row: Rect, depth: usize) -> Rect {
    let x = row.x0 + 6.0 + depth as f64 * 14.0;
    Rect::new(x, row.y0, x + 18.0, row.y1)
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
        NodeKind::Group => Icon::Folder,
    }
}

/// Height of a section header (its divider and title) in the right panel.
pub(super) const SEC_HEAD: f64 = 30.0;
pub(super) const ROW: f64 = FIELD_H + GAP;
/// Tops of the sections of the right panel, from the top of the panel.
const POS_TOP: f64 = 78.0;
const LAYOUT_TOP: f64 = POS_TOP + SEC_HEAD + 2.0 * ROW + 4.0;
const APPEAR_TOP: f64 = LAYOUT_TOP + SEC_HEAD + ROW + 4.0;
pub(super) const FILL_TOP: f64 = APPEAR_TOP + SEC_HEAD + ROW + 4.0;

/// A cell of the two-column grid of a section: column `col`, row `row`.
fn cell(right: Rect, top: f64, row: usize, col: usize) -> Rect {
    let (x0, x1) = (right.x0 + 1.0, right.x1);
    let fw = (x1 - x0 - 24.0 - GAP) / 2.0;
    let fx = x0 + 12.0 + col as f64 * (fw + GAP);
    let fy = right.y0 + top + SEC_HEAD + row as f64 * ROW;
    Rect::new(fx, fy, fx + fw, fy + FIELD_H)
}

/// Rectangle of the `i`-th property field of the right panel: X, Y and the angle in Position,
/// W and H in Layout, the radius in Appearance.
pub fn field_rect(right: Rect, i: usize) -> Rect {
    match i {
        0 | 1 => cell(right, POS_TOP, 0, i),
        2 => cell(right, POS_TOP, 1, 0),
        3 | 4 => cell(right, LAYOUT_TOP, 0, i - 3),
        _ => cell(right, APPEAR_TOP, 0, 0),
    }
}

/// The `k`-th of the buttons that turn and mirror the selection (next to the angle field).
pub fn action_rect(right: Rect, k: usize) -> Rect {
    let cell = cell(right, POS_TOP, 1, 1);
    let n = Action::ALL.len() as f64;
    let w = (cell.width() - 2.0 * (n - 1.0)) / n;
    let x = cell.x0 + k as f64 * (w + 2.0);
    Rect::new(x, cell.y0, x + w, cell.y1)
}

fn action_icon(a: Action) -> Icon {
    match a {
        Action::TurnLeft => Icon::RotateCcw,
        Action::TurnRight => Icon::RotateCw,
        Action::FlipHorizontal => Icon::FlipH,
        Action::FlipVertical => Icon::FlipV,
    }
}

pub(super) fn section_title(
    cx: &mut Cx,
    x0: f64,
    x1: f64,
    y: f64,
    label: &str,
    divider_line: bool,
) {
    if divider_line {
        divider(cx, x0, x1, y);
    }
    cx.text(label, FONT_SM, x0 + 12.0, y + 22.0, hex(text_secondary()));
}

/// The numeric value shown in the `i`-th field (`None` when the selection has mixed values).
pub fn field_value(ed: &Editor, i: usize) -> Option<f64> {
    selection_info(ed)?.fields.get(i)?.2
}

/// Where the value text starts inside a property field (px from its left edge).
pub const VALUE_X: f64 = 28.0;

/// The background and label of a property field.
fn field_box(cx: &mut Cx, r: Rect, label: &str) {
    cx.fill(r, R_FIELD, hex(bg_raised()));
    cx.text(
        label,
        FONT_SM,
        r.x0 + 9.0,
        r.y0 + 18.0,
        hex(text_disabled()),
    );
}

fn field(cx: &mut Cx, r: Rect, label: &str, value: &str) {
    field_box(cx, r, label);
    cx.text(
        value,
        FONT_UI,
        r.x0 + VALUE_X,
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
    let (state, zoom) = {
        let s = ed.session.borrow();
        (s.state(), format!("{:.0}%", s.vp.zoom * 100.0))
    };
    cx.text(&zoom, FONT_UI, share.x0 - 52.0, 25.0, hex(text_secondary()));
    let (label, color) = match state {
        SaveState::Saved => (tr(Msg::Saved), hex(text_disabled())),
        SaveState::Unsaved => (tr(Msg::Unsaved), hex(accent())),
        SaveState::NoFile => ("", hex(text_disabled())),
    };
    let w = cx.text_width(label, FONT_SM);
    cx.text(label, FONT_SM, share.x0 - 70.0 - w, 24.0, color);
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
        drop_indicator(cx, r, ed);
    });
}

/// Line or outline showing where dragged layers would land.
fn drop_indicator(cx: &mut Cx, left: Rect, ed: &Editor) {
    if !matches!(
        ed.interaction,
        Some(Interaction::LayerDrag { moved: true, .. })
    ) {
        return;
    }
    let Some((i, zone)) = ed.layer_drop() else {
        return;
    };
    let row = layer_row_rect(left, i);
    match zone {
        Zone::Into => cx.fill(row, R_BUTTON, alpha(accent(), 0.3)),
        Zone::Above => cx.fill(
            Rect::new(row.x0, row.y0 - 1.0, row.x1, row.y0 + 1.0),
            0.0,
            hex(accent()),
        ),
        Zone::Below => cx.fill(
            Rect::new(row.x0, row.y1 - 1.0, row.x1, row.y1 + 1.0),
            0.0,
            hex(accent()),
        ),
    }
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
    let x = rect.x0 + 26.0 + row.depth as f64 * 14.0;
    if row.has_children {
        let caret = if row.collapsed {
            Icon::CaretRight
        } else {
            Icon::CaretDown
        };
        let c = caret_rect(rect, row.depth);
        cx.icon(
            caret,
            c.x0 + 1.0,
            rect.y0 + 8.0,
            12.0,
            alpha(text_secondary(), 0.7),
        );
    }
    cx.icon(kind_icon(row.kind), x, rect.y0 + 7.0, 14.0, strong);
    if let Some(r) = ed.rename.as_ref().filter(|r| r.id == row.id) {
        let boxr = Rect::new(x + 18.0, rect.y0 + 3.0, rect.x1 - 6.0, rect.y1 - 3.0);
        cx.fill(boxr, R_BUTTON, hex(bg_raised()));
        cx.stroke(boxr, R_BUTTON, 1.5, hex(accent()));
        r.input.draw_inline(cx, boxr, x + 22.0, rect.y0 + 18.0);
        return;
    }
    let name = cx.fit(&row.name, FONT_UI, (rect.x1 - 70.0 - (x + 22.0)).max(20.0));
    cx.text(&name, FONT_UI, x + 22.0, rect.y0 + 18.0, strong);
    if row.locked || rect.contains(ed.mouse) {
        let a = if row.locked { 0.85 } else { 0.25 };
        let lock = lock_rect(rect);
        cx.icon(
            Icon::Lock,
            lock.x0 + 8.0,
            rect.y0 + 7.0,
            14.0,
            alpha(text_secondary(), a),
        );
    }
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

    let titles = [
        (POS_TOP, Msg::SectionPosition),
        (LAYOUT_TOP, Msg::SectionLayout),
        (APPEAR_TOP, Msg::SectionAppearance),
    ];
    for (top, msg) in titles {
        section_title(cx, x0, x1, y0 + top, tr(msg), top > POS_TOP);
    }
    for (k, action) in Action::ALL.into_iter().enumerate() {
        let rect = action_rect(r, k);
        let hot = rect.contains(ed.mouse);
        let (bg, fg) = if hot {
            (hex(bg_hover()), hex(text_primary()))
        } else {
            (hex(bg_raised()), hex(text_secondary()))
        };
        cx.fill(rect, R_FIELD, bg);
        let c = rect.center();
        cx.icon(action_icon(action), c.x - 7.0, c.y - 7.0, 14.0, fg);
    }

    for (i, (label, value, _)) in info.fields.iter().enumerate() {
        let rect = field_rect(r, i);
        match &ed.prop_edit {
            Some(edit) if edit.field.index() == i => {
                // Same field, same size and font: only the outline shows that it is being typed into.
                field_box(cx, rect, label);
                edit.input
                    .draw_inline(cx, rect, rect.x0 + VALUE_X, rect.y0 + 18.0);
                cx.stroke(rect, R_FIELD, 1.5, hex(accent()));
            }
            _ => field(cx, rect, label, value),
        }
    }

    super::paint_ui::sections(cx, r, ed);
}

/// What the right panel shows about the current selection.
struct Info {
    icon: Icon,
    title: String,
    /// Label and value pairs in two columns: X, Y, W, H, rotation, radius.
    fields: Vec<(&'static str, String, Option<f64>)>,
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
        [n] if n.kind != NodeKind::Group => Some(Info {
            icon: kind_icon(n.kind),
            title: n.name.clone(),
            fields: vec![
                ("X", num(n.geom.x), Some(n.geom.x)),
                ("Y", num(n.geom.y), Some(n.geom.y)),
                ("°", num(n.geom.rotation), Some(n.geom.rotation)),
                ("W", num(n.geom.w), Some(n.geom.w)),
                ("H", num(n.geom.h), Some(n.geom.h)),
                ("R", num(n.radius), Some(n.radius)),
            ],
        }),
        many => {
            let b = dv_geom::selection_bounds(page, &ed.selection)?;
            let mixed = tr(Msg::Mixed).to_string();
            let _ = Point::ZERO;
            // A group (alone or among others) is measured by what is inside it.
            let (icon, title) = match many {
                [n] => (kind_icon(n.kind), n.name.clone()),
                _ => (Icon::Square, objects_count(many.len() as u64)),
            };
            Some(Info {
                icon,
                title,
                fields: vec![
                    ("X", num(b.x0), Some(b.x0)),
                    ("Y", num(b.y0), Some(b.y0)),
                    ("°", mixed.clone(), None),
                    ("W", num(b.width()), Some(b.width())),
                    ("H", num(b.height()), Some(b.height())),
                    ("R", mixed, None),
                ],
            })
        }
    }
}
