//! Static content of the top bar and side panels (mockup: no real data yet).

use vello::kurbo::Rect;

use crate::i18n::{Msg, tr};
use dv_ui::draw::Cx;
use dv_ui::icons::Icon;
use dv_ui::theme::colors::*;
use dv_ui::theme::metrics::*;

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

fn row(cx: &mut Cx, r: Rect, icon: Option<Icon>, label: &str, indent: f64, selected: bool) {
    if selected {
        cx.fill(r, R_BUTTON, alpha(accent(), 0.22));
    }
    let mut x = r.x0 + 12.0 + indent * 16.0;
    let color = if selected {
        hex(text_primary())
    } else {
        hex(text_secondary())
    };
    if let Some(i) = icon {
        cx.icon(i, x, r.y0 + 7.0, 14.0, color);
        x += 22.0;
    }
    cx.text(label, FONT_UI, x, r.y0 + 18.0, hex(text_primary()));
    cx.icon(
        Icon::Eye,
        r.x1 - 28.0,
        r.y0 + 7.0,
        14.0,
        alpha(text_secondary(), 0.5),
    );
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

pub fn topbar(cx: &mut Cx, r: Rect) {
    cx.fill(r, 0.0, hex(bg_panel()));
    divider(cx, r.x0, r.x1, r.y1 - 1.0);
    let mut mx = 16.0;
    for item in [Msg::MenuFile, Msg::MenuEdit, Msg::MenuView] {
        mx += cx.text(tr(item), FONT_UI, mx, 25.0, hex(text_secondary())) + 20.0;
    }

    let title = tr(Msg::DocumentTitle);
    let w = cx.text_width(title, FONT_UI);
    cx.text(
        title,
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
    cx.text(
        "100%",
        FONT_UI,
        share.x0 - 52.0,
        25.0,
        hex(text_secondary()),
    );
}

pub fn left_panel(cx: &mut Cx, r: Rect) {
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
    for (i, name) in [Msg::Page1, Msg::Page2].iter().enumerate() {
        let ry = y0 + 108.0 + i as f64 * ROW_H;
        row(
            cx,
            Rect::new(x0 + 8.0, ry, x1 - 8.0, ry + ROW_H),
            None,
            tr(*name),
            0.0,
            i == 0,
        );
    }
    divider(cx, x0, x1, y0 + 176.0);

    let layers = [
        (Icon::Frame, tr(Msg::LayerFrame), 0.0, false),
        (Icon::Square, tr(Msg::LayerRectangle), 1.0, true),
        (Icon::Circle, tr(Msg::LayerEllipse), 1.0, false),
        (Icon::Text, tr(Msg::LayerTitle), 1.0, false),
    ];
    for (i, (icon, name, indent, sel)) in layers.iter().enumerate() {
        let ry = y0 + 188.0 + i as f64 * ROW_H;
        row(
            cx,
            Rect::new(x0 + 8.0, ry, x1 - 8.0, ry + ROW_H),
            Some(*icon),
            name,
            *indent,
            *sel,
        );
    }
}

pub fn right_panel(cx: &mut Cx, r: Rect) {
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

    cx.text(
        tr(Msg::SectionFrame),
        FONT_SM,
        x0 + 12.0,
        y0 + 62.0,
        hex(text_secondary()),
    );
    let fw = (x1 - x0 - 24.0 - GAP) / 2.0;
    let cells = [
        ("X", "120"),
        ("Y", "80"),
        ("W", "360"),
        ("H", "240"),
        ("R", "16"),
    ];
    for (i, (l, v)) in cells.iter().enumerate() {
        let fx = x0 + 12.0 + (i % 2) as f64 * (fw + GAP);
        let fy = y0 + 72.0 + (i / 2) as f64 * (FIELD_H + GAP);
        field(cx, Rect::new(fx, fy, fx + fw, fy + FIELD_H), l, v);
    }
    divider(cx, x0, x1, y0 + 176.0);

    cx.text(
        tr(Msg::SectionFill),
        FONT_SM,
        x0 + 12.0,
        y0 + 198.0,
        hex(text_secondary()),
    );
    cx.icon(
        Icon::Plus,
        x1 - 28.0,
        y0 + 186.0,
        14.0,
        hex(text_secondary()),
    );
    let sw = Rect::new(x0 + 12.0, y0 + 208.0, x0 + 32.0, y0 + 228.0);
    cx.fill(sw, R_FIELD, hex(accent()));
    cx.text(
        &format!("{:06X}", accent()),
        FONT_UI,
        x0 + 42.0,
        y0 + 222.0,
        hex(text_primary()),
    );
    cx.text(
        "100%",
        FONT_UI,
        x1 - 48.0,
        y0 + 222.0,
        hex(text_secondary()),
    );
    divider(cx, x0, x1, y0 + 244.0);

    cx.text(
        tr(Msg::SectionEffects),
        FONT_SM,
        x0 + 12.0,
        y0 + 266.0,
        hex(text_secondary()),
    );
    cx.icon(
        Icon::Plus,
        x1 - 28.0,
        y0 + 254.0,
        14.0,
        hex(text_secondary()),
    );
    let er = Rect::new(x0 + 8.0, y0 + 276.0, x1 - 8.0, y0 + 276.0 + ROW_H);
    cx.fill(er, R_BUTTON, hex(bg_raised()));
    cx.text(
        tr(Msg::DropShadow),
        FONT_UI,
        er.x0 + 12.0,
        er.y0 + 18.0,
        hex(text_primary()),
    );
    cx.icon(
        Icon::CaretDown,
        er.x1 - 26.0,
        er.y0 + 7.0,
        14.0,
        hex(text_secondary()),
    );
}
