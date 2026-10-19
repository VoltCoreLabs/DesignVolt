//! Projects screen: state, input handling and layout. Painting lives in `projects_paint.rs`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use vello::Scene;
use vello::kurbo::{Point, Rect};

use crate::anim::Spring;
use crate::input::{Key, Mods, Screen, SharedAssets, Transition, Viewport};
use crate::store::{SortKey, Store, now_secs};
use crate::theme::motion::*;
use crate::widgets::button::Button;
use crate::widgets::text_input::TextInput;

pub const SIDEBAR_W: f64 = 232.0;
pub const CARD_W: f64 = 224.0;
pub const CARD_H: f64 = 180.0;
pub const CARD_GAP: f64 = 20.0;
pub const MARGIN: f64 = 32.0;
pub const GRID_TOP: f64 = 112.0;
pub const SORT_OPTIONS: [(SortKey, &str); 3] = [
    (SortKey::Name, "Name"),
    (SortKey::Modified, "Last modified"),
    (SortKey::Created, "Created"),
];

pub struct CardAnim {
    pub hover: Spring,
    /// Seconds since the card appeared (large for cards that were already there).
    pub age: f32,
}

pub struct Projects {
    pub assets: SharedAssets,
    pub store: Rc<RefCell<Store>>,
    pub vp: Viewport,
    pub time: f32,
    pub mouse: (f64, f64),
    pub search: TextInput,
    pub new_button: Button,
    pub sort_key: SortKey,
    pub ascending: bool,
    pub menu_open: bool,
    pub scroll: Spring,
    pub cards: HashMap<u64, CardAnim>,
    next: Option<Transition>,
}

pub struct Layout {
    pub new_button: Rect,
    pub sort_button: Rect,
    pub search: Rect,
    pub menu: Rect,
}

impl Projects {
    pub fn new(assets: SharedAssets, store: Rc<RefCell<Store>>) -> Self {
        let cards = store
            .borrow()
            .projects
            .iter()
            .map(|p| (p.id, CardAnim { hover: hover_spring(), age: 10.0 }))
            .collect();
        Self {
            assets,
            store,
            vp: Viewport::new(),
            time: 0.0,
            mouse: (-1.0, -1.0),
            search: TextInput::new(40, "Search projects", false),
            new_button: Button::new(),
            sort_key: SortKey::Modified,
            ascending: false,
            menu_open: false,
            scroll: Spring::new(0.0, MOVE_STIFFNESS, MOVE_DAMPING),
            cards,
            next: None,
        }
    }

    pub fn layout(&self) -> Layout {
        let end = self.vp.w - MARGIN;
        let new_button = Rect::new(end - 148.0, 26.0, end, 62.0);
        let sort_button = Rect::new(new_button.x0 - 12.0 - 190.0, 26.0, new_button.x0 - 12.0, 62.0);
        let search = Rect::new(sort_button.x0 - 12.0 - 240.0, 26.0, sort_button.x0 - 12.0, 62.0);
        let menu = Rect::new(sort_button.x0, 68.0, sort_button.x1, 68.0 + 8.0 + 3.0 * 32.0 + 8.0);
        Layout { new_button, sort_button, search, menu }
    }

    pub fn at(&self, r: Rect) -> bool {
        r.contains(Point::new(self.mouse.0, self.mouse.1))
    }

    pub fn columns(&self) -> usize {
        let room = self.vp.w - SIDEBAR_W - 2.0 * MARGIN + CARD_GAP;
        ((room / (CARD_W + CARD_GAP)).floor() as usize).max(1)
    }

    /// Card rectangle for the `i`-th visible project (before scrolling).
    pub fn card_rect(&self, i: usize) -> Rect {
        let cols = self.columns();
        let x = SIDEBAR_W + MARGIN + (i % cols) as f64 * (CARD_W + CARD_GAP);
        let y = GRID_TOP + (i / cols) as f64 * (CARD_H + CARD_GAP) - self.scroll.pos as f64;
        Rect::new(x, y, x + CARD_W, y + CARD_H)
    }

    pub fn visible_ids(&self) -> Vec<u64> {
        let s = self.store.borrow();
        s.view(self.sort_key, self.ascending, self.search.text())
            .iter()
            .map(|p| p.id)
            .collect()
    }

    fn max_scroll(&self, count: usize) -> f32 {
        let rows = count.div_ceil(self.columns()) as f64;
        let content = GRID_TOP + rows * (CARD_H + CARD_GAP);
        (content - self.vp.h).max(0.0) as f32
    }

    fn hovered_card(&self) -> Option<u64> {
        if self.menu_open || self.mouse.1 < GRID_TOP - CARD_GAP {
            return None;
        }
        let ids = self.visible_ids();
        ids.iter()
            .enumerate()
            .find(|(i, _)| self.at(self.card_rect(*i)))
            .map(|(_, id)| *id)
    }

    fn create_project(&mut self) {
        let id = self.store.borrow_mut().create_project(now_secs());
        if let Err(e) = self.store.borrow().save() {
            eprintln!("could not save projects: {e}");
        }
        self.cards.insert(id, CardAnim { hover: hover_spring(), age: 0.0 });
        self.search.focused = false;
        self.sort_key = SortKey::Created;
        self.ascending = false;
        self.scroll.target = 0.0;
    }

    fn pick_sort(&mut self, key: SortKey) {
        if key == self.sort_key {
            self.ascending = !self.ascending;
        } else {
            self.sort_key = key;
            self.ascending = key == SortKey::Name;
        }
        self.menu_open = false;
        self.scroll.target = 0.0;
    }
}

fn hover_spring() -> Spring {
    Spring::new(0.0, HOVER_STIFFNESS, HOVER_DAMPING)
}

impl Screen for Projects {
    fn set_viewport(&mut self, w: u32, h: u32, scale: f64) {
        self.vp.set(w, h, scale);
    }

    fn update(&mut self, dt: f32) -> bool {
        self.time += dt;
        let l = self.layout();
        let hovered = self.hovered_card();
        let mut busy = self.search.update(dt);
        busy |= self.new_button.update(dt, self.at(l.new_button));
        for (id, c) in self.cards.iter_mut() {
            c.age += dt;
            c.hover.target = if hovered == Some(*id) { 1.0 } else { 0.0 };
            c.hover.step(dt);
            busy |= !c.hover.settled() || c.age < PANEL_INTRO + 0.1;
        }
        let max = self.max_scroll(self.visible_ids().len());
        self.scroll.target = self.scroll.target.clamp(0.0, max);
        self.scroll.step(dt);
        busy | !self.scroll.settled() | (self.time < PANEL_INTRO + 4.0 * STAGGER)
    }

    fn draw(&self, scene: &mut Scene) {
        self.paint(scene);
    }

    fn mouse_moved(&mut self, x: f64, y: f64) {
        self.mouse = (x / self.vp.scale, y / self.vp.scale);
        let left = self.layout().search.x0;
        self.search.drag_to(&self.assets.font, self.mouse.0 - left);
    }

    fn mouse_up(&mut self) {
        self.search.dragging = false;
    }

    fn mouse_left(&mut self) {
        self.mouse = (-1.0, -1.0);
    }

    fn click(&mut self, mods: Mods) {
        let l = self.layout();
        if self.menu_open {
            let row = (self.mouse.1 - l.menu.y0 - 8.0) / 32.0;
            if self.at(l.menu) && (0.0..3.0).contains(&row) {
                self.pick_sort(SORT_OPTIONS[row as usize].0);
            } else {
                self.menu_open = false;
            }
            return;
        }
        if self.at(l.new_button) {
            self.new_button.press();
            self.create_project();
        } else if self.at(l.sort_button) {
            self.menu_open = true;
            self.search.focused = false;
        } else if self.at(l.search) {
            self.search.press_at(&self.assets.font, self.mouse.0 - l.search.x0, mods.shift);
        } else if let Some(id) = self.hovered_card() {
            self.next = Some(Transition::OpenProject(id));
        } else {
            self.search.focused = false;
        }
    }

    fn scroll(&mut self, dy: f64) {
        self.scroll.target -= dy as f32;
    }

    fn key(&mut self, key: Key, mods: Mods) {
        if key == Key::Escape {
            self.menu_open = false;
            self.search.focused = false;
        } else if self.search.focused {
            self.search.key(key, mods);
            self.scroll.target = 0.0;
        }
    }

    fn text(&mut self, s: &str) {
        if self.search.focused {
            self.search.insert(s);
            self.scroll.target = 0.0;
        }
    }

    fn pointer_hand(&self) -> bool {
        let l = self.layout();
        self.at(l.new_button)
            || self.at(l.sort_button)
            || self.at(l.search)
            || (self.menu_open && self.at(l.menu))
            || self.hovered_card().is_some()
    }

    fn take_transition(&mut self) -> Option<Transition> {
        self.next.take()
    }
}
