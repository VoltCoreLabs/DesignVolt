//! Projects screen painting: sidebar, header, card grid, sort menu and empty state.

use vello::Scene;
use vello::kurbo::{Affine, Rect};

use super::backdrop;
use super::projects::*;
use crate::anim::{ease_out_cubic, progress};
use crate::draw::Cx;
use crate::icons::Icon;
use crate::store::{Project, ago, now_secs};
use crate::theme::colors::*;
use crate::theme::metrics::*;
use crate::theme::motion::{PANEL_INTRO, STAGGER};

/// Preview gradients, picked by project id.
const PALETTE: [(u32, u32); 6] = [
    (0x6366F1, 0xA855F7),
    (0x22D3EE, 0x6366F1),
    (0xF472B6, 0xA855F7),
    (0x34D399, 0x22D3EE),
    (0xFBBF24, 0xF472B6),
    (0x818CF8, 0x34D399),
];

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

        if self.menu_open {
            self.sort_menu(&mut cx, &l);
        }
    }

    fn sidebar(&self, cx: &mut Cx) {
        let r = Rect::new(0.0, 0.0, SIDEBAR_W, self.vp.h);
        cx.fill(r, 0.0, alpha(bg_panel(), 0.92));
        cx.fill(
            Rect::new(r.x1 - 1.0, 0.0, r.x1, r.y1),
            0.0,
            alpha(overlay(), 0.08),
        );

        for (i, name) in ["All projects", "Recent", "Favorites", "Trash"]
            .iter()
            .enumerate()
        {
            let row = Rect::new(
                12.0,
                24.0 + i as f64 * 36.0,
                SIDEBAR_W - 12.0,
                24.0 + i as f64 * 36.0 + 32.0,
            );
            let active = i == 0;
            if active {
                cx.fill(row, R_BUTTON, alpha(accent(), 0.22));
            }
            let c = if active {
                hex(text_primary())
            } else {
                hex(text_disabled())
            };
            cx.text(name, FONT_MD, row.x0 + 14.0, row.center().y + 4.5, c);
        }

        let name = self.store.borrow().name.clone().unwrap_or_default();
        let chip = Rect::new(12.0, self.vp.h - 68.0, SIDEBAR_W - 12.0, self.vp.h - 12.0);
        cx.fill(chip, R_CARD, hex(bg_raised()));
        let (ax, ay) = (chip.x0 + 28.0, chip.center().y);
        cx.fill_grad(
            Rect::new(ax - 16.0, ay - 16.0, ax + 16.0, ay + 16.0),
            16.0,
            hex(accent_from()),
            hex(accent_to()),
        );
        let initial: String = name
            .chars()
            .next()
            .map(|c| c.to_uppercase().collect())
            .unwrap_or_default();
        let w = cx.text_width(&initial, 14.0);
        cx.text(&initial, 14.0, ax - w / 2.0, ay + 5.0, hex(WHITE));
        cx.text(
            &name,
            FONT_MD,
            chip.x0 + 52.0,
            ay + 4.5,
            hex(text_primary()),
        );
    }

    fn header(&self, cx: &mut Cx, l: &Layout) {
        let x = SIDEBAR_W + MARGIN;
        cx.text("All projects", 22.0, x, 54.0, hex(text_primary()));
        let n = self.store.borrow().projects.len();
        let sub = match n {
            1 => "1 project".to_string(),
            n => format!("{n} projects"),
        };
        cx.text(&sub, FONT_UI, x, 76.0, hex(text_secondary()));

        self.search.draw(cx, l.search);
        let label = SORT_OPTIONS
            .iter()
            .find(|o| o.0 == self.sort_key)
            .map_or("", |o| o.1);
        let arrow = if self.ascending {
            "\u{2191}"
        } else {
            "\u{2193}"
        };
        let sb = l.sort_button;
        let hot = self.menu_open || self.at(sb);
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
        self.new_button
            .draw(cx, l.new_button, "New project", Some(Icon::Plus), true);
    }

    fn grid(&self, cx: &mut Cx) {
        let ids = self.visible_ids();
        let area = Rect::new(SIDEBAR_W, GRID_TOP - 16.0, self.vp.w, self.vp.h);
        if ids.is_empty() {
            return self.empty_state(cx, area);
        }
        let store = self.store.borrow();
        let now = now_secs();
        cx.clip(area, |cx| {
            for (i, id) in ids.iter().enumerate() {
                let (Some(p), Some(anim)) = (store.project(*id), self.cards.get(id)) else {
                    continue;
                };
                let r = self.card_rect(i);
                if r.y1 < area.y0 || r.y0 > area.y1 {
                    continue;
                }
                let born = ease_out_cubic(progress(anim.age, 0.0, PANEL_INTRO)) as f64;
                let lift = -5.0 * anim.hover.pos.clamp(0.0, 1.0) as f64 + 18.0 * (1.0 - born);
                cx.layer(r.inflate(24.0, 24.0), born as f32, (0.0, lift), |cx| {
                    card(cx, r, p, anim.hover.pos.clamp(0.0, 1.0), now)
                });
            }
        });
    }

    fn empty_state(&self, cx: &mut Cx, area: Rect) {
        let c = area.center();
        let filtered = !self.search.is_blank();
        let (title, sub) = if filtered {
            ("No projects match your search", "Try a different name.")
        } else {
            (
                "Create your first project",
                "Your projects will show up here.",
            )
        };
        let badge = Rect::new(c.x - 32.0, c.y - 96.0, c.x + 32.0, c.y - 32.0);
        cx.shadow(badge, 20.0, alpha(accent(), 0.45), 18.0, 8.0);
        cx.fill_grad(badge, 20.0, hex(accent_from()), hex(accent_to()));
        cx.icon(
            if filtered { Icon::Search } else { Icon::Plus },
            c.x - 14.0,
            c.y - 78.0,
            28.0,
            hex(WHITE),
        );
        for (s, size, dy, col) in [
            (title, 18.0, 0.0, text_primary()),
            (sub, FONT_MD, 26.0, text_secondary()),
        ] {
            let w = cx.text_width(s, size);
            cx.text(s, size, c.x - w / 2.0, c.y + 4.0 + dy, hex(col));
        }
    }

    fn sort_menu(&self, cx: &mut Cx, l: &Layout) {
        cx.glass(l.menu, R_CARD);
        for (i, (key, label)) in SORT_OPTIONS.iter().enumerate() {
            let row = Rect::new(
                l.menu.x0 + 6.0,
                l.menu.y0 + 8.0 + i as f64 * 32.0,
                l.menu.x1 - 6.0,
                l.menu.y0 + 8.0 + (i + 1) as f64 * 32.0,
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
            cx.text(label, FONT_UI, row.x0 + 12.0, row.center().y + 4.3, c);
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

fn card(cx: &mut Cx, r: Rect, p: &Project, hover: f32, now: u64) {
    let (a, b) = PALETTE[(p.id as usize) % PALETTE.len()];
    cx.shadow(
        r,
        R_CARD,
        alpha(0x000000, 0.25 + 0.2 * hover),
        14.0 + 8.0 * hover as f64,
        6.0,
    );
    cx.fill(r, R_CARD, hex(bg_panel()));
    let preview = Rect::new(r.x0 + 8.0, r.y0 + 8.0, r.x1 - 8.0, r.y0 + 124.0);
    cx.fill_grad(preview, R_BUTTON + 2.0, alpha(a, 0.85), alpha(b, 0.85));
    let mark = Rect::new(
        preview.center().x - 44.0,
        preview.center().y - 28.0,
        preview.center().x + 44.0,
        preview.center().y + 28.0,
    );
    cx.fill(mark, 10.0, alpha(WHITE, 0.22));
    cx.stroke(
        r,
        R_CARD,
        1.0 + hover as f64,
        alpha(accent(), 0.12 + 0.7 * hover),
    );
    cx.text(
        &p.name,
        FONT_MD,
        r.x0 + 14.0,
        r.y0 + 148.0,
        hex(text_primary()),
    );
    cx.text(
        &format!("Edited {}", ago(now, p.modified)),
        FONT_SM,
        r.x0 + 14.0,
        r.y0 + 166.0,
        hex(text_secondary()),
    );
}
