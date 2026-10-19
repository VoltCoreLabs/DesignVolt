//! Context menu of a project/folder card: actions, geometry, painting.

use vello::kurbo::{Point, Rect};

use super::projects::{MenuState, Projects};
use crate::draw::Cx;
use crate::i18n::{Msg, tr};
use crate::icons::Icon;
use crate::library::{Item, Location};
use crate::theme::colors::*;
use crate::theme::metrics::*;

const WIDTH: f64 = 232.0;
const ROW: f64 = 34.0;
const PAD: f64 = 6.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    Open,
    Rename,
    Favorite,
    Delete,
    Restore,
    DeleteForever,
}

impl Projects {
    pub fn menu_actions(&self, item: Item) -> Vec<Action> {
        if self.loc == Location::Trash {
            return vec![Action::Restore, Action::DeleteForever];
        }
        match item {
            Item::Project(_) => vec![
                Action::Open,
                Action::Rename,
                Action::Favorite,
                Action::Delete,
            ],
            Item::Folder(_) => vec![Action::Open, Action::Rename, Action::Delete],
        }
    }

    /// Menu rectangle, kept inside the window.
    pub fn menu_rect(&self) -> Option<Rect> {
        let m = self.menu.as_ref()?;
        let h = PAD * 2.0 + self.menu_actions(m.item).len() as f64 * ROW;
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
        let n = self.menu_actions(m.item).len();
        (0..n).find(|i| self.menu_row(rect, *i).contains(self.mouse))
    }

    pub fn menu_contains(&self, p: Point) -> bool {
        self.menu_rect().is_some_and(|r| r.contains(p))
    }

    /// Runs the `i`-th action; "delete forever" needs a second click.
    pub fn run_menu_action(&mut self, i: usize) {
        let Some(MenuState { item, .. }) = self.menu.as_ref() else {
            return;
        };
        let item = *item;
        let Some(action) = self.menu_actions(item).get(i).copied() else {
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
        match action {
            Action::Open => self.open_item(item),
            Action::Rename => self.start_rename(item),
            Action::Favorite => {
                if let Item::Project(id) = item {
                    self.toggle_favorite(id);
                }
            }
            Action::Delete => self.trash(item),
            Action::Restore => self.restore(item),
            Action::DeleteForever => self.purge(item),
        }
    }

    fn action_look(&self, action: Action, item: Item, confirm: bool) -> (Icon, &'static str, bool) {
        let favorite = matches!(item, Item::Project(id)
            if self.store.borrow().lib.project(id).is_some_and(|p| p.favorite));
        match action {
            Action::Open => (Icon::OpenOut, tr(Msg::MenuOpen), false),
            Action::Rename => (Icon::Pencil, tr(Msg::MenuRename), false),
            Action::Favorite if favorite => (Icon::StarFill, tr(Msg::MenuUnfavorite), false),
            Action::Favorite => (Icon::Star, tr(Msg::MenuFavorite), false),
            Action::Delete => (Icon::Trash, tr(Msg::MenuDelete), true),
            Action::Restore => (Icon::Undo, tr(Msg::MenuRestore), false),
            Action::DeleteForever if confirm => (Icon::Trash, tr(Msg::MenuConfirmDelete), true),
            Action::DeleteForever => (Icon::Trash, tr(Msg::MenuDeleteForever), true),
        }
    }

    pub(super) fn paint_menu(&self, cx: &mut Cx) {
        let (Some(rect), Some(m)) = (self.menu_rect(), self.menu.as_ref()) else {
            return;
        };
        cx.glass(rect, R_CARD);
        let hovered = self.menu_hit();
        for (i, action) in self.menu_actions(m.item).into_iter().enumerate() {
            let row = self.menu_row(rect, i);
            let (icon, label, danger_row) = self.action_look(action, m.item, m.confirm);
            if hovered == Some(i) {
                let tint = if danger_row { danger() } else { bg_hover() };
                cx.fill(
                    row,
                    R_BUTTON,
                    alpha(tint, if danger_row { 0.14 } else { 0.9 }),
                );
            }
            let color = if danger_row {
                hex(danger())
            } else {
                hex(text_primary())
            };
            cx.icon(icon, row.x0 + 10.0, row.center().y - 8.0, 16.0, color);
            cx.text(label, FONT_UI, row.x0 + 36.0, row.center().y + 4.3, color);
        }
    }
}
