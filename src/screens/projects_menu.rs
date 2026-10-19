//! Context menu of project/folder cards: actions, geometry, painting.

use vello::kurbo::{Point, Rect};

use super::projects::Projects;
use crate::draw::Cx;
use crate::i18n::{Msg, tr};
use crate::icons::Icon;
use crate::library::{Item, Location};
use crate::theme::colors::*;
use crate::theme::metrics::*;

const WIDTH: f64 = 240.0;
const ROW: f64 = 34.0;
const PAD: f64 = 6.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    Open,
    Rename,
    Favorite,
    Duplicate,
    Delete,
    Restore,
    DeleteForever,
}

impl Projects {
    /// Actions available for `items` (one card or a selection).
    pub fn actions_for(&self, items: &[Item]) -> Vec<Action> {
        if self.loc == Location::Trash {
            return vec![Action::Restore, Action::DeleteForever];
        }
        match items {
            [Item::Project(_)] => vec![
                Action::Open,
                Action::Rename,
                Action::Favorite,
                Action::Duplicate,
                Action::Delete,
            ],
            [Item::Folder(_)] => vec![
                Action::Open,
                Action::Rename,
                Action::Duplicate,
                Action::Delete,
            ],
            _ => {
                let mut v = Vec::new();
                if items.iter().any(|i| matches!(i, Item::Project(_))) {
                    v.push(Action::Favorite);
                }
                v.extend([Action::Duplicate, Action::Delete]);
                v
            }
        }
    }

    /// Icon, label and "destructive" flag of an action for `items`.
    pub fn action_look(
        &self,
        action: Action,
        items: &[Item],
        confirm: bool,
    ) -> (Icon, &'static str, bool) {
        let store = self.store.borrow();
        let projects: Vec<bool> = items
            .iter()
            .filter_map(|i| match i {
                Item::Project(id) => store.lib.project(*id).map(|p| p.favorite),
                Item::Folder(_) => None,
            })
            .collect();
        let all_favorite = !projects.is_empty() && projects.iter().all(|f| *f);
        match action {
            Action::Open => (Icon::OpenOut, tr(Msg::MenuOpen), false),
            Action::Rename => (Icon::Pencil, tr(Msg::MenuRename), false),
            Action::Favorite if all_favorite => (Icon::StarFill, tr(Msg::MenuUnfavorite), false),
            Action::Favorite => (Icon::Star, tr(Msg::MenuFavorite), false),
            Action::Duplicate => (Icon::Duplicate, tr(Msg::MenuDuplicate), false),
            Action::Delete => (Icon::Trash, tr(Msg::MenuDelete), true),
            Action::Restore => (Icon::Undo, tr(Msg::MenuRestore), false),
            Action::DeleteForever if confirm => (Icon::Trash, tr(Msg::MenuConfirmDelete), true),
            Action::DeleteForever => (Icon::Trash, tr(Msg::MenuDeleteForever), true),
        }
    }

    /// Menu rectangle, kept inside the window.
    pub fn menu_rect(&self) -> Option<Rect> {
        let m = self.menu.as_ref()?;
        let h = PAD * 2.0 + self.actions_for(&m.items).len() as f64 * ROW;
        let x = m.pos.x.min(self.vp.w - WIDTH - 8.0).max(8.0);
        let y = m.pos.y.min(self.vp.h - h - 8.0).max(8.0);
        Some(Rect::new(x, y, x + WIDTH, y + h))
    }

    fn menu_row(&self, rect: Rect, i: usize) -> Rect {
        let y = rect.y0 + PAD + i as f64 * ROW;
        Rect::new(rect.x0 + PAD, y, rect.x1 - PAD, y + ROW)
    }

    /// Index of the menu row under the pointer.
    pub fn menu_hit(&self) -> Option<usize> {
        let (rect, m) = (self.menu_rect()?, self.menu.as_ref()?);
        let n = self.actions_for(&m.items).len();
        (0..n).find(|i| self.menu_row(rect, *i).contains(self.mouse))
    }

    pub fn menu_contains(&self, p: Point) -> bool {
        self.menu_rect().is_some_and(|r| r.contains(p))
    }

    /// Runs the `i`-th action; "delete forever" needs a second click.
    pub fn run_menu_action(&mut self, i: usize) {
        let Some(m) = self.menu.as_ref() else { return };
        let items = m.items.clone();
        let Some(action) = self.actions_for(&items).get(i).copied() else {
            return;
        };
        if action == Action::DeleteForever
            && let Some(m) = self.menu.as_mut()
            && !m.confirm
        {
            m.confirm = true;
            return;
        }
        self.menu = None;
        self.apply(action, &items);
    }

    pub(super) fn paint_menu(&self, cx: &mut Cx) {
        let (Some(rect), Some(m)) = (self.menu_rect(), self.menu.as_ref()) else {
            return;
        };
        cx.glass(rect, R_CARD);
        let hovered = self.menu_hit();
        for (i, action) in self.actions_for(&m.items).into_iter().enumerate() {
            let row = self.menu_row(rect, i);
            let (icon, label, destructive) = self.action_look(action, &m.items, m.confirm);
            if hovered == Some(i) {
                let tint = if destructive { danger() } else { bg_hover() };
                cx.fill(
                    row,
                    R_BUTTON,
                    alpha(tint, if destructive { 0.14 } else { 0.9 }),
                );
            }
            let color = if destructive {
                hex(danger())
            } else {
                hex(text_primary())
            };
            cx.icon(icon, row.x0 + 10.0, row.center().y - 8.0, 16.0, color);
            cx.text(label, FONT_UI, row.x0 + 36.0, row.center().y + 4.3, color);
        }
    }
}
