//! `Screen` implementation of the projects screen: updates and input.

use vello::Scene;
use vello::kurbo::{Point, Rect};

use super::projects::*;
use crate::input::{Key, Mods, Screen, Transition};
use crate::library::{Item, Location};
use crate::theme::motion::{PANEL_INTRO, STAGGER};

/// Star button in the top-right corner of a project card.
pub fn star_rect(card: Rect) -> Rect {
    Rect::new(
        card.x1 - 44.0,
        card.y0 + 14.0,
        card.x1 - 14.0,
        card.y0 + 44.0,
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
        let blocked = self.sort_open
            || self.drag.is_some()
            || self.rename.is_some()
            || self.menu_contains(self.mouse);
        if blocked {
            None
        } else {
            self.item_at(self.mouse)
        }
    }

    fn rename_rect(&self) -> Option<Rect> {
        let (item, _) = self.rename.as_ref()?;
        self.card_of(*item).map(name_rect)
    }

    fn header_hand(&self, l: &Layout) -> bool {
        let all = matches!(self.loc, Location::All(_));
        self.at(l.search)
            || self.at(l.sort_button)
            || (all && (self.at(l.new_project) || self.at(l.new_folder)))
            || (all && self.crumbs().iter().any(|c| self.at(c.rect)))
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
        let mut busy = self.search.update(dt);
        busy |= self.new_project.update(dt, all && self.at(l.new_project));
        busy |= self.new_folder.update(dt, all && self.at(l.new_folder));
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
                self.drop_item(item, target);
            }
            return;
        }
        if let Some(p) = self.press.take()
            && self.item_at(self.mouse) == Some(p.item)
            && self.loc != Location::Trash
        {
            self.open_item(p.item);
        }
    }

    fn click(&mut self, mods: Mods) {
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
            if self.at(l.new_project) {
                self.new_project.press();
                self.create_project();
                return;
            }
            if self.at(l.new_folder) {
                self.new_folder.press();
                self.create_folder();
                return;
            }
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
                    })
                }
            }
        } else {
            self.search.focused = false;
        }
    }

    fn right_click(&mut self, _mods: Mods) {
        if self.rename.is_some() {
            self.commit_rename();
        }
        self.sort_open = false;
        self.menu = match self.item_at(self.mouse) {
            Some(item) if !self.menu_contains(self.mouse) => Some(MenuState {
                item,
                pos: self.mouse,
                confirm: false,
            }),
            _ => None,
        };
    }

    fn scroll(&mut self, dy: f64) {
        self.scroll.target -= dy as f32;
    }

    fn key(&mut self, key: Key, mods: Mods) {
        if let Some((_, input)) = self.rename.as_mut() {
            match key {
                Key::Enter => self.commit_rename(),
                Key::Escape => self.rename = None,
                k => input.key(k, mods),
            }
            return;
        }
        if key == Key::Escape {
            self.menu = None;
            self.drag = None;
            self.press = None;
            self.sort_open = false;
            self.search.focused = false;
        } else if self.search.focused {
            self.search.key(key, mods);
            self.scroll.target = 0.0;
        }
    }

    fn text(&mut self, s: &str) {
        if let Some((_, input)) = self.rename.as_mut() {
            input.insert(s);
        } else if self.search.focused {
            self.search.insert(s);
            self.scroll.target = 0.0;
        }
    }

    fn pointer_hand(&self) -> bool {
        let l = self.layout();
        if self.menu.is_some() {
            return self.menu_hit().is_some();
        }
        self.drag.is_some()
            || self.header_hand(&l)
            || self.at(self.profile_chip())
            || (0..NAV.len()).any(|i| self.at(self.nav_row(i)))
            || self
                .hovered_item()
                .is_some_and(|_| self.loc != Location::Trash)
    }

    fn take_transition(&mut self) -> Option<Transition> {
        self.next.take()
    }
}
