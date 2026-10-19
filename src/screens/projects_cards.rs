//! Drawing of project/folder cards and the drag ghost.

use vello::kurbo::Rect;

use super::projects_input::{name_rect, star_rect};
use crate::draw::Cx;
use crate::i18n::{ago, edited, items_count};
use crate::icons::Icon;
use crate::library::{Folder, Project};
use crate::theme::colors::*;
use crate::theme::metrics::*;

/// Preview gradients, picked by project id.
const PALETTE: [(u32, u32); 6] = [
    (0x2563EB, 0x4F8CFF),
    (0x06B6D4, 0x2563EB),
    (0xF472B6, 0x8B5CF6),
    (0x10B981, 0x06B6D4),
    (0xF59E0B, 0xF472B6),
    (0x818CF8, 0x10B981),
];
const GOLD: u32 = 0xF5B301;

/// How a card is drawn right now.
pub struct CardLook {
    pub hover: f32,
    /// Drop-target highlight while dragging over a folder.
    pub drop: bool,
    /// The name is being edited, so the card does not paint it.
    pub renaming: bool,
    pub in_trash: bool,
}

fn frame(cx: &mut Cx, r: Rect, look: &CardLook) {
    let lift = look.hover;
    cx.shadow(
        r,
        R_CARD,
        alpha(0x000000, 0.14 + 0.12 * lift),
        14.0 + 8.0 * lift as f64,
        6.0,
    );
    cx.fill(r, R_CARD, hex(bg_panel()));
    if look.drop {
        cx.fill(r, R_CARD, alpha(accent(), 0.12));
    }
    let (width, a) = if look.drop {
        (2.5, 1.0)
    } else {
        (1.0 + lift as f64, 0.12 + 0.7 * lift)
    };
    cx.stroke(r, R_CARD, width, alpha(accent(), a));
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
    let (a, b) = PALETTE[(p.id as usize) % PALETTE.len()];
    frame(cx, r, look);
    let preview = Rect::new(r.x0 + 8.0, r.y0 + 8.0, r.x1 - 8.0, r.y0 + 124.0);
    cx.fill_grad(preview, R_BUTTON + 2.0, alpha(a, 0.9), alpha(b, 0.9));
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
