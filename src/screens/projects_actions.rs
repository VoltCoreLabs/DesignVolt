//! Actions of the projects screen: creating, renaming, moving, deleting, selecting.

use super::projects::*;
use super::projects_menu::Action;
use crate::i18n::{Msg, tr};
use crate::input::Transition;
use crate::library::{Item, Location, SortKey};
use crate::store::now_secs;
use crate::widgets::text_input::TextInput;

fn fresh_card() -> CardAnim {
    CardAnim {
        hover: hover_spring(),
        age: 0.0,
    }
}

impl Projects {
    fn save(&self) {
        self.store.borrow().save_logged();
    }

    /// Items a drag of `item` moves: the whole selection if `item` is part of it.
    pub fn drag_set(&self, item: Item) -> Vec<Item> {
        if self.selected.contains(&item) {
            self.store.borrow().lib.prune_nested(&self.selected)
        } else {
            vec![item]
        }
    }

    /// Where the dragged item(s) would land: a folder card or a breadcrumb segment.
    pub fn drop_target(&self, item: Item) -> Option<Option<u64>> {
        let set = self.drag_set(item);
        let store = self.store.borrow();
        let allowed = |target: Option<u64>| set.iter().any(|i| store.lib.can_move(*i, target));
        for c in self.crumbs() {
            if c.rect.contains(self.mouse) && allowed(c.folder) {
                return Some(c.folder);
            }
        }
        if let Some(Item::Folder(id)) = self.item_at(self.mouse)
            && allowed(Some(id))
        {
            return Some(Some(id));
        }
        None
    }

    pub fn drop_items(&mut self, item: Item, target: Option<u64>) {
        let set = self.drag_set(item);
        let moved = {
            let mut store = self.store.borrow_mut();
            set.iter()
                .filter(|i| store.lib.move_to(**i, target))
                .count()
        };
        if moved > 0 {
            self.save();
        }
    }

    pub fn open_item(&mut self, item: Item) {
        match item {
            Item::Project(id) => {
                self.store.borrow_mut().lib.touch(id, now_secs());
                self.save();
                self.next = Some(Transition::OpenProject(id));
            }
            Item::Folder(id) => self.go(Location::All(Some(id))),
        }
    }

    pub fn go(&mut self, loc: Location) {
        self.loc = loc;
        self.scroll.pos = 0.0;
        self.scroll.target = 0.0;
        self.sort_open = false;
        self.menu = None;
        self.modal = None;
        self.clear_selection();
    }

    /// Scrolls so that the card of `item` is fully visible.
    pub fn reveal(&mut self, item: Item) {
        let Some(i) = self.items().iter().position(|x| *x == item) else {
            return;
        };
        let row = (i / self.columns()) as f64;
        let top = GRID_TOP + row * (CARD_H + CARD_GAP);
        let (view_top, view_bottom) = (GRID_TOP, self.vp.h - 24.0);
        let pos = self.scroll.target as f64;
        if top - pos < view_top {
            self.scroll.target = (top - view_top).max(0.0) as f32;
        } else if top + CARD_H - pos > view_bottom {
            self.scroll.target = (top + CARD_H - view_bottom) as f32;
        }
    }

    pub fn create_project(&mut self) {
        let folder = self.current_folder();
        let id = self.store.borrow_mut().lib.create_project(
            now_secs(),
            tr(Msg::UntitledProject),
            folder,
        );
        self.save();
        self.cards.insert(Item::Project(id), fresh_card());
        self.search.focused = false;
        self.reveal(Item::Project(id));
    }

    pub fn create_folder(&mut self) {
        let parent = self.current_folder();
        let id =
            self.store
                .borrow_mut()
                .lib
                .create_folder(now_secs(), tr(Msg::UntitledFolder), parent);
        self.save();
        self.cards.insert(Item::Folder(id), fresh_card());
        self.search.focused = false;
        self.reveal(Item::Folder(id));
        self.start_rename(Item::Folder(id));
    }

    pub fn start_rename(&mut self, item: Item) {
        let current = self.store.borrow().lib.name_of(item).map(str::to_string);
        let Some(name) = current else { return };
        let mut input = TextInput::new(crate::library::NAME_MAX, Msg::UntitledProject, false);
        input.insert(&name);
        input.select_all();
        input.focused = true;
        self.rename = Some((item, input));
        self.menu = None;
    }

    pub fn commit_rename(&mut self) {
        let Some((item, input)) = self.rename.take() else {
            return;
        };
        let changed = self.store.borrow_mut().lib.rename(item, input.text());
        if changed {
            self.save();
        }
    }

    pub fn toggle_favorite(&mut self, id: u64) {
        self.apply(Action::Favorite, &[Item::Project(id)]);
    }

    // ---- selection ----

    pub fn clear_selection(&mut self) {
        self.selected.clear();
        self.anchor = None;
        self.bar_confirm = false;
    }

    pub fn toggle_select(&mut self, item: Item) {
        match self.selected.iter().position(|i| *i == item) {
            Some(p) => {
                self.selected.remove(p);
            }
            None => self.selected.push(item),
        }
        self.anchor = Some(item);
        self.bar_confirm = false;
    }

    /// Selects everything between the anchor and `to` (inclusive) in the current order.
    pub fn range_select(&mut self, to: Item) {
        let items = self.items();
        let anchor = self.anchor.filter(|a| items.contains(a)).unwrap_or(to);
        let (Some(a), Some(b)) = (
            items.iter().position(|i| *i == anchor),
            items.iter().position(|i| *i == to),
        ) else {
            return;
        };
        self.selected = items[a.min(b)..=a.max(b)].to_vec();
        self.anchor = Some(anchor);
        self.bar_confirm = false;
    }

    pub fn select_all(&mut self) {
        self.selected = self.items();
        self.anchor = self.selected.first().copied();
        self.bar_confirm = false;
    }

    /// Drops selected items that are no longer on screen (deleted, moved, filtered out).
    pub fn prune_selection(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        let items = self.items();
        self.selected.retain(|i| items.contains(i));
        if self.selected.is_empty() {
            self.clear_selection();
        }
    }

    // ---- operations on item sets ----

    pub fn apply(&mut self, action: Action, items: &[Item]) {
        let now = now_secs();
        let suffix = tr(Msg::CopySuffix);
        {
            let mut store = self.store.borrow_mut();
            let nested = store.lib.prune_nested(items);
            match action {
                Action::Open | Action::Rename => {}
                Action::Favorite => {
                    let ids: Vec<u64> = items
                        .iter()
                        .filter_map(|i| match i {
                            Item::Project(id) => Some(*id),
                            Item::Folder(_) => None,
                        })
                        .collect();
                    let all_fav = ids
                        .iter()
                        .all(|id| store.lib.project(*id).is_some_and(|p| p.favorite));
                    for id in ids {
                        store.lib.set_favorite(id, !all_fav);
                    }
                }
                Action::Duplicate => {
                    for item in &nested {
                        match item {
                            Item::Project(id) => {
                                store.lib.duplicate_project(*id, now, suffix);
                            }
                            Item::Folder(id) => {
                                store.lib.duplicate_folder(*id, now, suffix);
                            }
                        }
                    }
                }
                Action::Delete => nested.iter().for_each(|i| store.lib.trash(*i, now)),
                Action::Restore => items.iter().for_each(|i| store.lib.restore(*i)),
                Action::DeleteForever => nested.iter().for_each(|i| store.lib.purge(*i)),
            }
        }
        match (action, items.first()) {
            (Action::Open, Some(i)) => self.open_item(*i),
            (Action::Rename, Some(i)) => self.start_rename(*i),
            _ => self.save(),
        }
        if matches!(
            action,
            Action::Delete | Action::Restore | Action::DeleteForever
        ) {
            self.clear_selection();
        }
        let store = self.store.borrow();
        self.cards.retain(|i, _| store.lib.exists(*i));
    }

    pub fn empty_trash(&mut self) {
        self.store.borrow_mut().lib.empty_trash();
        self.modal = None;
        self.clear_selection();
        self.save();
        let store = self.store.borrow();
        self.cards.retain(|i, _| store.lib.exists(*i));
    }

    pub fn pick_sort(&mut self, key: SortKey) {
        if key == self.sort_key {
            self.ascending = !self.ascending;
        } else {
            self.sort_key = key;
            self.ascending = key == SortKey::Name;
        }
        {
            let mut store = self.store.borrow_mut();
            store.sort_key = self.sort_key;
            store.sort_asc = self.ascending;
        }
        self.save();
        self.sort_open = false;
        self.scroll.target = 0.0;
    }
}
