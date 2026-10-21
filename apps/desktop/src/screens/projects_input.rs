//! `Screen` implementation of the projects screen: updates and input.

use vello::Scene;
use vello::kurbo::{Point, Rect};

use super::projects::*;
use crate::input::{Key, Mods, Screen, Transition};
use crate::library::{Item, Location};
use dv_ui::theme::motion::{PANEL_INTRO, STAGGER};

/// Star button in the top-right corner of a project card.
pub fn star_rect(card: Rect) -> Rect {
    Rect::new(
        card.x1 - 44.0,
        card.y0 + 14.0,
        card.x1 - 14.0,
        card.y0 + 44.0,
    )
}

/// Selection checkbox in the top-left corner of a card (a bit larger than the drawn circle).pub fn check_rect(card: Rect) -> Rect {    Rect::new(card.x0 + 14.0, card.y0 + 13.0, card.x0 + 46.0, card.y0 + 45.0)}
/// Selection checkbox in the top-left corner of a card (a bit larger than the drawn circle).
pub fn check_rect(card: Rect) -> Rect {
    Rect::new(
        card.x0 + 14.0,
        card.y0 + 13.0,
        card.x0 + 46.0,
        card.y0 + 45.0,
    )
}

/// Area of a card where its name is edited.
pub fn name_rect(card: Rect) -> Rect {
    Rect::new(
        card.x0 + 8.0,
        card.y0 + 128.0,
        card.x1 - 8.0,
        card.y0 + 164.0,
    )
}

impl Projects {
    fn hovered_item(&self) -> Option<Item> {
        let blocked = self.modal.is_some()
            || self.sort_open
            || self.drag.is_some()
            || self.rename.is_some()
            || self.menu_contains(self.mouse)
            || self.over_bar();
        if blocked {
            None
        } else {
            self.item_at(self.mouse)
        }
    }

    fn over_bar(&self) -> bool {
        self.bar_layout().is_some_and(|b| self.at(b.rect))
    }

    fn rename_rect(&self) -> Option<Rect> {
        let (item, _) = self.rename.as_ref()?;
        self.card_of(*item).map(name_rect)
    }

    /// The button in the header's top-right corner is "New project" or, in the trash, "Empty trash".
    fn main_button_active(&self) -> bool {
        match self.loc {
            Location::All(_) => true,
            Location::Trash => self.store.borrow().lib.trash_count() > 0,
            _ => false,
        }
    }

    fn header_hand(&self, l: &Layout) -> bool {
        let all = matches!(self.loc, Location::All(_));
        self.at(l.search)
            || self.at(l.sort_button)
            || (all && self.at(l.new_folder))
            || (self.main_button_active() && self.at(l.new_project))
            || (all && self.crumbs().iter().any(|c| self.at(c.rect)))
    }

    /// Handles a click on the selection bar; returns true when the bar consumed it.
    fn bar_click(&mut self) -> bool {
        let Some(bar) = self.bar_layout() else {
            return false;
        };
        if !self.at(bar.rect) {
            return false;
        }
        if self.at(bar.close) {
            self.clear_selection();
            return true;
        }
        let hit = bar
            .buttons
            .iter()
            .find(|(_, r)| self.at(*r))
            .map(|(a, _)| *a);
        if let Some(action) = hit {
            if action == super::projects_menu::Action::DeleteForever && !self.bar_confirm {
                self.bar_confirm = true;
            } else {
                let items = self.selected.clone();
                self.apply(action, &items);
            }
        }
        true
    }
}

impl Screen for Projects {
    fn set_viewport(&mut self, w: u32, h: u32, scale: f64) {
        self.vp.set(w, h, scale);
    }

    fn update(&mut self, dt: f32) -> bool {
        self.time += dt;
        let l = self.layout();
        let hovered = self.hovered_item();
        let all = matches!(self.loc, Location::All(_));
        let mut busy = self.search.update(dt) | self.modal_update(dt);
        busy |= self.update_tip(dt);
        busy |= self.new_project.update(
            dt,
            self.modal.is_none() && self.main_button_active() && self.at(l.new_project),
        );
        busy |= self
            .new_folder
            .update(dt, self.modal.is_none() && all && self.at(l.new_folder));
        if let Some((_, input)) = self.rename.as_mut() {
            busy |= input.update(dt);
        }
        let items = self.items();
        for item in &items {
            self.cards.entry(*item).or_insert_with(|| CardAnim {
                hover: hover_spring(),
                age: 0.0,
            });
        }
        for (item, c) in self.cards.iter_mut() {
            c.age += dt;
            c.hover.target = if hovered == Some(*item) { 1.0 } else { 0.0 };
            c.hover.step(dt);
            busy |= !c.hover.settled() || c.age < PANEL_INTRO + 0.1;
        }
        self.prune_selection();
        let max = self.max_scroll(items.len());
        self.scroll.target = self.scroll.target.clamp(0.0, max);
        self.scroll.step(dt);
        busy | !self.scroll.settled()
            | (self.time < PANEL_INTRO + 4.0 * STAGGER)
            | self.drag.is_some()
    }

    fn draw(&self, scene: &mut Scene) {
        self.paint(scene);
    }

    fn mouse_moved(&mut self, x: f64, y: f64) {
        self.mouse = Point::new(x / self.vp.scale, y / self.vp.scale);
        let font = &self.assets.font;
        let left = self.layout().search.x0;
        self.search.drag_to(font, self.mouse.x - left);
        if let (Some(rect), Some((_, input))) = (self.rename_rect(), self.rename.as_mut()) {
            input.drag_to(font, self.mouse.x - rect.x0);
        }
        if let Some(p) = &self.press {
            let moved = (self.mouse.x - p.at.x).hypot(self.mouse.y - p.at.y);
            if moved > DRAG_THRESHOLD && matches!(self.loc, Location::All(_)) {
                self.drag = self.press.take().map(|p| p.item);
            }
        }
    }

    fn mouse_left(&mut self) {
        self.mouse = Point::new(-1.0, -1.0);
    }

    fn mouse_up(&mut self) {
        self.search.dragging = false;
        if let Some((_, input)) = self.rename.as_mut() {
            input.dragging = false;
        }
        if let Some(item) = self.drag.take() {
            if let Some(target) = self.drop_target(item) {
                self.drop_items(item, target);
            }
            return;
        }
        let Some(p) = self.press.take() else { return };
        if self.item_at(self.mouse) != Some(p.item) {
            return;
        }
        if p.mods.shift {
            self.range_select(p.item);
        } else if p.mods.ctrl || !self.selected.is_empty() {
            self.toggle_select(p.item);
        } else if self.loc != Location::Trash {
            self.open_item(p.item);
        }
    }

    fn click(&mut self, mods: Mods) {
        if self.modal_click() {
            return;
        }
        let l = self.layout();
        if self.menu.is_some() {
            match self.menu_hit() {
                Some(i) => self.run_menu_action(i),
                None => self.menu = None,
            }
            return;
        }
        if let Some(rect) = self.rename_rect() {
            if rect.contains(self.mouse) {
                let font = &self.assets.font;
                if let Some((_, input)) = self.rename.as_mut() {
                    input.press_at(font, self.mouse.x - rect.x0, mods.shift);
                }
                return;
            }
            self.commit_rename();
        }
        if self.bar_click() {
            return;
        }
        if self.sort_open {
            let row = (self.mouse.y - l.sort_menu.y0 - 8.0) / 32.0;
            if l.sort_menu.contains(self.mouse) && (0.0..3.0).contains(&row) {
                self.pick_sort(SORT_OPTIONS[row as usize].0);
            } else {
                self.sort_open = false;
            }
            return;
        }
        for (i, (loc, _)) in NAV.iter().enumerate() {
            if self.at(self.nav_row(i)) {
                self.go(*loc);
                return;
            }
        }
        if self.at(self.profile_chip()) {
            self.next = Some(Transition::OpenSettings);
            return;
        }
        let all = matches!(self.loc, Location::All(_));
        if all {
            if let Some(c) = self.crumbs().into_iter().find(|c| self.at(c.rect)) {
                self.go(Location::All(c.folder));
                return;
            }
            if self.at(l.new_folder) {
                self.new_folder.press();
                self.create_folder();
                return;
            }
        }
        if self.main_button_active() && self.at(l.new_project) {
            self.new_project.press();
            match self.loc {
                Location::Trash => self.open_empty_trash_modal(),
                _ => self.create_project(),
            }
            return;
        }
        if self.at(l.sort_button) {
            self.sort_open = true;
            self.search.focused = false;
        } else if self.at(l.search) {
            let font = &self.assets.font;
            self.search
                .press_at(font, self.mouse.x - l.search.x0, mods.shift);
        } else if let Some(item) = self.hovered_item() {
            self.search.focused = false;
            let on_check = self
                .card_of(item)
                .map(check_rect)
                .is_some_and(|r| r.contains(self.mouse));
            if on_check {
                if mods.shift {
                    self.range_select(item);
                } else {
                    self.toggle_select(item);
                }
                return;
            }
            let star = self.card_of(item).map(star_rect);
            match (item, star) {
                (Item::Project(id), Some(s))
                    if self.loc != Location::Trash && s.contains(self.mouse) =>
                {
                    self.toggle_favorite(id);
                }
                _ => {
                    self.press = Some(Press {
                        item,
                        at: self.mouse,
                        mods,
                    })
                }
            }
        } else {
            self.search.focused = false;
            if !mods.ctrl && !mods.shift {
                self.clear_selection();
            }
        }
    }

    fn right_click(&mut self, _mods: Mods) {
        if self.modal.is_some() {
            return;
        }
        if self.rename.is_some() {
            self.commit_rename();
        }
        self.sort_open = false;
        self.menu = match self.item_at(self.mouse) {
            Some(item) if !self.menu_contains(self.mouse) => {
                let items = if self.selected.contains(&item) {
                    self.selected.clone()
                } else {
                    vec![item]
                };
                Some(MenuState {
                    items,
                    pos: self.mouse,
                    confirm: false,
                })
            }
            _ => None,
        };
    }

    fn scroll(&mut self, dy: f64) {
        self.scroll.target -= dy as f32;
    }

    fn key(&mut self, key: Key, mods: Mods) {
        if self.modal.is_some() {
            if key == Key::Escape {
                self.modal = None;
            }
            return;
        }
        if self.rename.is_some() {
            match key {
                Key::Enter => self.commit_rename(),
                Key::Escape => self.rename = None,
                k => {
                    if let Some((_, input)) = self.rename.as_mut() {
                        input.key(k, mods);
                    }
                }
            }
            return;
        }
        if self.search.focused && key != Key::Escape {
            self.search.key(key, mods);
            self.scroll.target = 0.0;
            return;
        }
        match key {
            Key::Escape => {
                if self.menu.is_some() || self.sort_open || self.drag.is_some() {
                    self.menu = None;
                    self.sort_open = false;
                    self.drag = None;
                    self.press = None;
                } else if !self.selected.is_empty() {
                    self.clear_selection();
                }
                self.search.focused = false;
            }
            Key::SelectAll => self.select_all(),
            Key::Delete if !self.selected.is_empty() && self.loc != Location::Trash => {
                let items = self.selected.clone();
                self.apply(super::projects_menu::Action::Delete, &items);
            }
            _ => {}
        }
    }

    fn text(&mut self, s: &str) {
        if self.modal.is_some() {
            return;
        }
        if let Some((_, input)) = self.rename.as_mut() {
            input.insert(s);
        } else if self.search.focused {
            self.search.insert(s);
            self.scroll.target = 0.0;
        }
    }

    fn pointer_hand(&self) -> bool {
        if self.modal.is_some() {
            return self.modal_hand();
        }
        let l = self.layout();
        if self.menu.is_some() {
            return self.menu_hit().is_some();
        }
        if let Some(bar) = self.bar_layout()
            && self.at(bar.rect)
        {
            return self.at(bar.close) || bar.buttons.iter().any(|(_, r)| self.at(*r));
        }
        self.drag.is_some()
            || self.header_hand(&l)
            || self.at(self.profile_chip())
            || (0..NAV.len()).any(|i| self.at(self.nav_row(i)))
            || self.hovered_item().is_some()
    }

    fn take_transition(&mut self) -> Option<Transition> {
        self.next.take()
    }
}
