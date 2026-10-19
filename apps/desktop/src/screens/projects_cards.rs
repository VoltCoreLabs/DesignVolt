//! Drawing of project/folder cards and the drag ghost.

use vello::kurbo::Rect;

use super::projects_input::{check_rect, name_rect, star_rect};
use crate::i18n::{ago, edited, items_count};
use crate::library::{Folder, Project};
use dv_ui::draw::Cx;
use dv_ui::icons::Icon;
use dv_ui::theme::colors::*;
use dv_ui::theme::metrics::*;

const GOLD: u32 = 0xF5B301;

/// How a card is drawn right now.
pub struct CardLook {
    pub hover: f32,
    /// Drop-target highlight while dragging over a folder.
    pub drop: bool,
    /// The name is being edited, so the card does not paint it.
    pub renaming: bool,
    pub in_trash: bool,
    /// The card is part of the selection.
    pub selected: bool,
    /// Selection mode: every card shows a checkbox.
    pub selecting: bool,
}

fn frame(cx: &mut Cx, r: Rect, look: &CardLook) {
    let lift = look.hover;
    cx.shadow(r, R_CARD, alpha(0x000000, 0.10 + 0.04 * lift), 14.0, 6.0);
    cx.fill(r, R_CARD, hex(bg_panel()));
    if look.drop || look.selected {
        cx.fill(
            r,
            R_CARD,
            alpha(accent(), if look.drop { 0.12 } else { 0.08 }),
        );
    }
    let (width, a) = if look.drop || look.selected {
        (2.0, 1.0)
    } else {
        (1.0, 0.12 + 0.45 * lift)
    };
    cx.stroke(r, R_CARD, width, alpha(accent(), a));
}

/// Checkbox in the top-left corner of a card, shown in selection mode and on hover.
fn checkbox(cx: &mut Cx, card: Rect, look: &CardLook) {
    if !look.selecting && look.hover < 0.05 {
        return;
    }
    let c = check_rect(card).center();
    let (cxm, cym) = (c.x, c.y);
    let k = if look.selecting { 1.0 } else { look.hover };
    if look.selected {
        cx.circle(cxm, cym, 11.0, hex(accent()));
        cx.icon(Icon::Check, cxm - 7.0, cym - 7.0, 14.0, hex(WHITE));
    } else {
        cx.circle(cxm, cym, 11.0, alpha(bg_panel(), 0.85 * k));
        let ring = Rect::new(cxm - 11.0, cym - 11.0, cxm + 11.0, cym + 11.0);
        cx.stroke(ring, 11.0, 1.5, alpha(overlay(), 0.35 * k));
    }
}

fn name_and_meta(cx: &mut Cx, r: Rect, name: &str, meta: &str, look: &CardLook) {
    if !look.renaming {
        let shown = cx.fit(name, FONT_MD, r.width() - 28.0);
        cx.text(
            &shown,
            FONT_MD,
            r.x0 + 14.0,
            r.y0 + 148.0,
            hex(text_primary()),
        );
    }
    cx.text(
        meta,
        FONT_SM,
        r.x0 + 14.0,
        r.y0 + 166.0,
        hex(text_secondary()),
    );
}

pub fn project_card(cx: &mut Cx, r: Rect, p: &Project, now: u64, look: &CardLook) {
    frame(cx, r, look);
    let preview = Rect::new(r.x0 + 8.0, r.y0 + 8.0, r.x1 - 8.0, r.y0 + 124.0);
    cx.fill(preview, R_BUTTON + 2.0, alpha(accent(), 0.16));
    let c = preview.center();
    let mark = Rect::new(c.x - 44.0, c.y - 28.0, c.x + 44.0, c.y + 28.0);
    cx.fill(mark, 10.0, alpha(WHITE, 0.22));

    if !look.in_trash && (p.favorite || look.hover > 0.05) {
        let s = star_rect(r);
        let k = if p.favorite { 1.0 } else { look.hover };
        cx.fill(s, 15.0, alpha(0x000000, 0.28 * k));
        let (icon, color) = if p.favorite {
            (Icon::StarFill, hex(GOLD))
        } else {
            (Icon::Star, alpha(WHITE, k))
        };
        cx.icon(icon, s.x0 + 6.0, s.y0 + 6.0, 18.0, color);
    }
    let meta = match p.deleted {
        Some(t) => ago(now, t),
        None => edited(now, p.modified),
    };
    checkbox(cx, r, look);
    name_and_meta(cx, r, &p.name, &meta, look);
}

pub fn folder_card(cx: &mut Cx, r: Rect, f: &Folder, children: usize, look: &CardLook) {
    frame(cx, r, look);
    let preview = Rect::new(r.x0 + 8.0, r.y0 + 8.0, r.x1 - 8.0, r.y0 + 124.0);
    cx.fill(
        preview,
        R_BUTTON + 2.0,
        alpha(accent(), 0.10 + 0.06 * look.hover),
    );
    let c = preview.center();
    cx.icon(
        Icon::Folder,
        c.x - 30.0,
        c.y - 30.0,
        60.0,
        alpha(accent(), 0.85),
    );
    checkbox(cx, r, look);
    name_and_meta(cx, r, &f.name, &items_count(children as u64), look);
}

/// Small card that follows the pointer while dragging.
pub fn ghost(cx: &mut Cx, at: (f64, f64), name: &str, folder: bool) {
    let r = Rect::new(
        at.0 + 12.0,
        at.1 + 12.0,
        at.0 + 12.0 + 190.0,
        at.1 + 12.0 + 44.0,
    );
    cx.shadow(r, R_BUTTON + 2.0, alpha(0x000000, 0.25), 16.0, 8.0);
    cx.fill(r, R_BUTTON + 2.0, alpha(bg_panel(), 0.96));
    cx.stroke(r, R_BUTTON + 2.0, 1.5, hex(accent()));
    let icon = if folder { Icon::Folder } else { Icon::Square };
    cx.icon(icon, r.x0 + 12.0, r.center().y - 9.0, 18.0, hex(accent()));
    let shown = cx.fit(name, FONT_UI, r.width() - 52.0);
    cx.text(
        &shown,
        FONT_UI,
        r.x0 + 38.0,
        r.center().y + 4.3,
        hex(text_primary()),
    );
}

pub fn rename_area(r: Rect) -> Rect {
    name_rect(r)
}
