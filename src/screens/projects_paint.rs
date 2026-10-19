//! Projects screen painting: sidebar, header, card grid, drag ghost, sort menu, empty states.

use vello::Scene;
use vello::kurbo::{Affine, Rect};

use super::backdrop;
use super::projects::*;
use super::projects_cards::{CardLook, folder_card, ghost, project_card, rename_area};
use crate::anim::{ease_out_cubic, progress};
use crate::draw::Cx;
use crate::i18n::{Msg, items_count, tr};
use crate::icons::Icon;
use crate::library::{Item, Location};
use crate::store::now_secs;
use crate::theme::colors::*;
use crate::theme::metrics::*;
use crate::theme::motion::{PANEL_INTRO, STAGGER};

impl Projects {
    fn intro(&self, i: f32) -> f32 {
        ease_out_cubic(progress(self.time, i * STAGGER, PANEL_INTRO))
    }

    pub(super) fn paint(&self, scene: &mut Scene) {
        scene.reset();
        let l = self.layout();
        let mut cx = Cx {
            scene,
            font: &self.assets.font,
            icons: &self.assets.icons,
            xf: Affine::scale(self.vp.scale),
        };
        backdrop::draw(&mut cx, self.vp);
        let full = Rect::new(0.0, 0.0, self.vp.w, self.vp.h);

        let p = self.intro(0.0);
        cx.layer(full, p, (-20.0 * (1.0 - p as f64), 0.0), |cx| {
            self.sidebar(cx)
        });
        let p = self.intro(1.0);
        cx.layer(full, p, (0.0, -12.0 * (1.0 - p as f64)), |cx| {
            self.header(cx, &l)
        });
        let p = self.intro(2.0);
        cx.layer(full, p, (0.0, 0.0), |cx| self.grid(cx));

        if self.sort_open {
            self.sort_menu(&mut cx, &l);
        }
        if let Some(item) = self.drag {
            let store = self.store.borrow();
            let name = store.lib.name_of(item).unwrap_or("");
            ghost(
                &mut cx,
                (self.mouse.x, self.mouse.y),
                name,
                matches!(item, Item::Folder(_)),
            );
        }
        self.paint_bar(&mut cx);
        self.paint_menu(&mut cx);
        self.paint_modal(&mut cx);
    }

    fn nav_active(&self, loc: Location) -> bool {
        matches!((self.loc, loc), (Location::All(_), Location::All(_))) || self.loc == loc
    }

    fn sidebar(&self, cx: &mut Cx) {
        let r = Rect::new(0.0, 0.0, SIDEBAR_W, self.vp.h);
        cx.fill(r, 0.0, alpha(bg_panel(), 0.92));
        cx.fill(
            Rect::new(r.x1 - 1.0, 0.0, r.x1, r.y1),
            0.0,
            alpha(overlay(), 0.08),
        );

        let trash = self.store.borrow().lib.trash_count();
        for (i, (loc, msg)) in NAV.iter().enumerate() {
            let row = self.nav_row(i);
            let active = self.nav_active(*loc);
            if active {
                cx.fill(row, R_BUTTON, alpha(accent(), 0.16));
            } else if self.at(row) {
                cx.fill(row, R_BUTTON, alpha(bg_hover(), 0.8));
            }
            let c = if active {
                hex(text_primary())
            } else {
                hex(text_secondary())
            };
            cx.text(tr(*msg), FONT_MD, row.x0 + 14.0, row.center().y + 4.5, c);
            if *loc == Location::Trash && trash > 0 {
                let n = trash.to_string();
                let w = cx.text_width(&n, FONT_SM);
                cx.text(
                    &n,
                    FONT_SM,
                    row.x1 - 14.0 - w,
                    row.center().y + 4.0,
                    hex(text_disabled()),
                );
            }
        }

        let name = self.store.borrow().name.clone().unwrap_or_default();
        let chip = self.profile_chip();
        let hot = self.at(chip);
        cx.fill(
            chip,
            R_CARD,
            if hot {
                hex(bg_hover())
            } else {
                hex(bg_raised())
            },
        );
        let (ax, ay) = (chip.x0 + 28.0, chip.center().y);
        let avatar = Rect::new(ax - 16.0, ay - 16.0, ax + 16.0, ay + 16.0);
        cx.fill(avatar, 16.0, hex(accent()));
        let initial: String = name
            .chars()
            .next()
            .map(|c| c.to_uppercase().collect())
            .unwrap_or_default();
        let w = cx.text_width(&initial, 14.0);
        cx.text(&initial, 14.0, ax - w / 2.0, ay + 5.0, hex(WHITE));
        let shown = cx.fit(&name, FONT_MD, chip.width() - 64.0);
        cx.text(
            &shown,
            FONT_MD,
            chip.x0 + 52.0,
            ay + 4.5,
            hex(text_primary()),
        );
    }

    fn header(&self, cx: &mut Cx, l: &Layout) {
        let x = SIDEBAR_W + MARGIN;
        let all = matches!(self.loc, Location::All(_));
        if all {
            self.breadcrumbs(cx);
        } else {
            let title = NAV
                .iter()
                .find(|(loc, _)| *loc == self.loc)
                .map_or("", |(_, m)| tr(*m));
            cx.text(title, 22.0, x, 50.0, hex(text_primary()));
        }
        let n = self.items().len() as u64;
        let sub = items_count(n);
        let w = cx.text_width(&sub, FONT_UI);
        cx.text(
            &sub,
            FONT_UI,
            self.vp.w - MARGIN - w,
            100.0,
            hex(text_secondary()),
        );

        self.search.draw(cx, l.search);
        let label = SORT_OPTIONS
            .iter()
            .find(|o| o.0 == self.sort_key)
            .map_or("", |o| tr(o.1));
        let arrow = if self.ascending {
            "\u{2191}"
        } else {
            "\u{2193}"
        };
        let sb = l.sort_button;
        let hot = self.sort_open || self.at(sb);
        cx.fill(
            sb,
            R_BUTTON,
            if hot {
                hex(bg_hover())
            } else {
                hex(bg_raised())
            },
        );
        cx.stroke(sb, R_BUTTON, 1.0, alpha(overlay(), 0.12));
        cx.text(
            &format!("{label} {arrow}"),
            FONT_UI,
            sb.x0 + 14.0,
            sb.center().y + 4.3,
            hex(text_primary()),
        );
        cx.icon(
            Icon::CaretDown,
            sb.x1 - 28.0,
            sb.center().y - 7.0,
            14.0,
            hex(text_secondary()),
        );
        if all {
            let folder = tr(Msg::NewFolder);
            self.new_folder
                .draw_secondary(cx, l.new_folder, folder, Some(Icon::FolderPlus));
            let label = tr(Msg::NewProject);
            self.new_project
                .draw(cx, l.new_project, label, Some(Icon::Plus), true);
        } else if self.loc == Location::Trash && self.store.borrow().lib.trash_count() > 0 {
            let label = tr(Msg::EmptyTrash);
            self.new_project
                .draw_secondary(cx, l.new_project, label, Some(Icon::Trash));
        }
    }

    fn breadcrumbs(&self, cx: &mut Cx) {
        let crumbs = self.crumbs();
        let last = crumbs.len().saturating_sub(1);
        let target = self.drag.and_then(|d| self.drop_target(d));
        for (i, c) in crumbs.iter().enumerate() {
            if self.drag.is_some() && target == Some(c.folder) {
                cx.fill(c.rect, R_BUTTON, alpha(accent(), 0.16));
                cx.stroke(c.rect, R_BUTTON, 1.5, hex(accent()));
            } else if self.at(c.rect) && crumbs.len() > 1 {
                cx.fill(c.rect, R_BUTTON, alpha(bg_hover(), 0.8));
            }
            let color = if i == last {
                hex(text_primary())
            } else {
                hex(text_secondary())
            };
            cx.text(&c.label, 20.0, c.rect.x0 + 6.0, 50.0, color);
            if i < last {
                cx.icon(
                    Icon::CaretRight,
                    c.rect.x1 + 2.0,
                    34.0,
                    14.0,
                    hex(text_disabled()),
                );
            }
        }
    }

    fn grid(&self, cx: &mut Cx) {
        let items = self.items();
        let area = Rect::new(SIDEBAR_W, GRID_TOP - 16.0, self.vp.w, self.vp.h);
        if items.is_empty() {
            return self.empty_state(cx, area);
        }
        let store = self.store.borrow();
        let now = now_secs();
        let target = self.drag.and_then(|d| self.drop_target(d));
        cx.clip(area, |cx| {
            for (i, item) in items.iter().enumerate() {
                let Some(anim) = self.cards.get(item) else {
                    continue;
                };
                let r = self.card_rect(i);
                if r.y1 < area.y0 || r.y0 > area.y1 {
                    continue;
                }
                let hover = anim.hover.pos.clamp(0.0, 1.0);
                let born = ease_out_cubic(progress(anim.age, 0.0, PANEL_INTRO));
                let lifted = self.drag == Some(*item);
                let look = CardLook {
                    hover,
                    drop: matches!((item, target), (Item::Folder(id), Some(Some(t))) if *id == t),
                    renaming: self.rename.as_ref().is_some_and(|(it, _)| it == item),
                    in_trash: self.loc == Location::Trash,
                    selected: self.selected.contains(item),
                    selecting: !self.selected.is_empty(),
                };
                let alpha_k = born
                    * if lifted {
                        0.35
                    } else if look.in_trash {
                        0.8
                    } else {
                        1.0
                    };
                let dy = 8.0 * (1.0 - born as f64);
                cx.layer(r.inflate(24.0, 24.0), alpha_k, (0.0, dy), |cx| match item {
                    Item::Project(id) => {
                        if let Some(p) = store.lib.project(*id) {
                            project_card(cx, r, p, now, &look);
                        }
                    }
                    Item::Folder(id) => {
                        if let Some(f) = store.lib.folder(*id) {
                            folder_card(cx, r, f, store.lib.child_count(*id), &look);
                        }
                    }
                });
                if let Some((it, input)) = &self.rename
                    && it == item
                {
                    input.draw(cx, rename_area(r));
                }
            }
        });
    }

    fn empty_state(&self, cx: &mut Cx, area: Rect) {
        let searching = !self.search.is_blank();
        let (icon, title, sub) = match (self.loc, searching) {
            (_, true) => (Icon::Search, Msg::NoMatchTitle, Some(Msg::NoMatchSub)),
            (Location::All(None), _) => (Icon::Plus, Msg::EmptyTitle, Some(Msg::EmptySub)),
            (Location::All(Some(_)), _) => (Icon::Folder, Msg::FolderEmpty, None),
            (Location::Recent, _) => (Icon::Plus, Msg::RecentEmptyTitle, Some(Msg::RecentEmptySub)),
            (Location::Favorites, _) => (Icon::Star, Msg::FavEmptyTitle, Some(Msg::FavEmptySub)),
            (Location::Trash, _) => (Icon::Trash, Msg::TrashEmptyTitle, Some(Msg::TrashEmptySub)),
        };
        let c = area.center();
        let badge = Rect::new(c.x - 32.0, c.y - 96.0, c.x + 32.0, c.y - 32.0);
        cx.shadow(badge, 20.0, alpha(accent(), 0.45), 18.0, 8.0);
        cx.fill(badge, 20.0, hex(accent()));
        cx.icon(icon, c.x - 14.0, c.y - 78.0, 28.0, hex(WHITE));
        let mut lines = vec![(tr(title), 18.0, 0.0, text_primary())];
        if let Some(s) = sub {
            lines.push((tr(s), FONT_MD, 26.0, text_secondary()));
        }
        for (s, size, dy, col) in lines {
            let w = cx.text_width(s, size);
            cx.text(s, size, c.x - w / 2.0, c.y + 4.0 + dy, hex(col));
        }
    }

    fn sort_menu(&self, cx: &mut Cx, l: &Layout) {
        cx.glass(l.sort_menu, R_CARD);
        for (i, (key, msg)) in SORT_OPTIONS.iter().enumerate() {
            let m = l.sort_menu;
            let row = Rect::new(
                m.x0 + 6.0,
                m.y0 + 8.0 + i as f64 * 32.0,
                m.x1 - 6.0,
                m.y0 + 8.0 + (i + 1) as f64 * 32.0,
            );
            let selected = *key == self.sort_key;
            if self.at(row) {
                cx.fill(row, R_BUTTON, alpha(bg_hover(), 0.9));
            }
            let c = if selected {
                hex(text_primary())
            } else {
                hex(text_secondary())
            };
            cx.text(tr(*msg), FONT_UI, row.x0 + 12.0, row.center().y + 4.3, c);
            if selected {
                let arrow = if self.ascending {
                    "\u{2191}"
                } else {
                    "\u{2193}"
                };
                cx.text(
                    arrow,
                    FONT_UI,
                    row.x1 - 24.0,
                    row.center().y + 4.3,
                    hex(accent()),
                );
            }
        }
    }
}
