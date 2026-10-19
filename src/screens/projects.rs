//! Projects screen: state, layout and actions.
//! Input handling lives in `projects_input.rs`, painting in `projects_paint.rs`,
//! the context menu in `projects_menu.rs`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use vello::kurbo::{Point, Rect};

use crate::anim::Spring;
use crate::i18n::{Msg, tr};
use crate::input::{Mods, SharedAssets, Transition, Viewport};
use crate::library::{Item, Location, SortKey};
use crate::store::Store;
use crate::theme::motion::*;
use crate::widgets::button::Button;
use crate::widgets::text_input::TextInput;

pub const SIDEBAR_W: f64 = 232.0;
pub const CARD_W: f64 = 224.0;
pub const CARD_H: f64 = 180.0;
pub const CARD_GAP: f64 = 20.0;
pub const MARGIN: f64 = 32.0;
pub const GRID_TOP: f64 = 148.0;
pub const DRAG_THRESHOLD: f64 = 6.0;
pub const SORT_OPTIONS: [(SortKey, Msg); 3] = [
    (SortKey::Name, Msg::SortName),
    (SortKey::Modified, Msg::SortModified),
    (SortKey::Created, Msg::SortCreated),
];
pub const NAV: [(Location, Msg); 4] = [
    (Location::All(None), Msg::NavAll),
    (Location::Recent, Msg::NavRecent),
    (Location::Favorites, Msg::NavFavorites),
    (Location::Trash, Msg::NavTrash),
];

pub struct CardAnim {
    pub hover: Spring,
    /// Seconds since the card appeared (large for cards that were already there).
    pub age: f32,
}

pub struct Press {
    pub item: Item,
    pub at: Point,
    pub mods: Mods,
}

pub struct MenuState {
    /// What the action applies to: the whole selection or just `item`.
    pub items: Vec<Item>,
    pub pos: Point,
    /// "Delete forever" was clicked once and waits for the second click.
    pub confirm: bool,
}

pub struct Crumb {
    pub folder: Option<u64>,
    pub label: String,
    pub rect: Rect,
}

pub struct Projects {
    pub assets: SharedAssets,
    pub store: Rc<RefCell<Store>>,
    pub vp: Viewport,
    pub time: f32,
    pub mouse: Point,
    pub search: TextInput,
    pub new_project: Button,
    pub new_folder: Button,
    pub sort_key: SortKey,
    pub ascending: bool,
    pub sort_open: bool,
    pub loc: Location,
    pub scroll: Spring,
    pub cards: HashMap<Item, CardAnim>,
    pub press: Option<Press>,
    pub drag: Option<Item>,
    pub menu: Option<MenuState>,
    pub rename: Option<(Item, TextInput)>,
    /// Selected cards in selection order; `anchor` is the start of Shift-ranges.
    pub selected: Vec<Item>,
    pub anchor: Option<Item>,
    /// "Empty trash" waits for a second click.
    pub confirm_empty: bool,
    /// "Delete forever" in the selection bar waits for a second click.
    pub bar_confirm: bool,
    pub next: Option<Transition>,
}

pub struct Layout {
    pub new_project: Rect,
    pub new_folder: Rect,
    pub search: Rect,
    pub sort_button: Rect,
    pub sort_menu: Rect,
}

pub fn hover_spring() -> Spring {
    Spring::new(0.0, HOVER_STIFFNESS, HOVER_DAMPING)
}

impl Projects {
    pub fn new(assets: SharedAssets, store: Rc<RefCell<Store>>) -> Self {
        let cards = {
            let s = store.borrow();
            let projects = s.lib.projects.iter().map(|p| Item::Project(p.id));
            let folders = s.lib.folders.iter().map(|f| Item::Folder(f.id));
            projects
                .chain(folders)
                .map(|i| {
                    (
                        i,
                        CardAnim {
                            hover: hover_spring(),
                            age: 10.0,
                        },
                    )
                })
                .collect()
        };
        let (sort_key, ascending) = {
            let s = store.borrow();
            (s.sort_key, s.sort_asc)
        };
        Self {
            assets,
            store,
            vp: Viewport::new(),
            time: 0.0,
            mouse: Point::new(-1.0, -1.0),
            search: TextInput::new(40, Msg::SearchProjects, false),
            new_project: Button::new(),
            new_folder: Button::new(),
            sort_key,
            ascending,
            sort_open: false,
            loc: Location::All(None),
            scroll: Spring::new(0.0, MOVE_STIFFNESS, MOVE_DAMPING),
            cards,
            press: None,
            drag: None,
            menu: None,
            rename: None,
            selected: Vec::new(),
            anchor: None,
            confirm_empty: false,
            bar_confirm: false,
            next: None,
        }
    }

    pub fn layout(&self) -> Layout {
        let end = self.vp.w - MARGIN;
        let new_project = Rect::new(end - 168.0, 24.0, end, 60.0);
        let new_folder = Rect::new(
            new_project.x0 - 12.0 - 156.0,
            24.0,
            new_project.x0 - 12.0,
            60.0,
        );
        let x = SIDEBAR_W + MARGIN;
        let search = Rect::new(x, 76.0, x + 260.0, 112.0);
        let sort_button = Rect::new(search.x1 + 12.0, 76.0, search.x1 + 12.0 + 190.0, 112.0);
        let sort_menu = Rect::new(
            sort_button.x0,
            118.0,
            sort_button.x1,
            118.0 + 8.0 + 3.0 * 32.0 + 8.0,
        );
        Layout {
            new_project,
            new_folder,
            search,
            sort_button,
            sort_menu,
        }
    }

    pub fn nav_row(&self, i: usize) -> Rect {
        Rect::new(
            12.0,
            24.0 + i as f64 * 36.0,
            SIDEBAR_W - 12.0,
            24.0 + i as f64 * 36.0 + 32.0,
        )
    }

    /// Profile chip at the bottom of the sidebar; opens settings.
    pub fn profile_chip(&self) -> Rect {
        Rect::new(12.0, self.vp.h - 68.0, SIDEBAR_W - 12.0, self.vp.h - 12.0)
    }

    pub fn at(&self, r: Rect) -> bool {
        r.contains(self.mouse)
    }

    /// Folder shown now (`None` = root or a non-folder location).
    pub fn current_folder(&self) -> Option<u64> {
        match self.loc {
            Location::All(f) => f,
            _ => None,
        }
    }

    /// Breadcrumb segments (only in "All projects"): root, then each nested folder.
    pub fn crumbs(&self) -> Vec<Crumb> {
        let Location::All(folder) = self.loc else {
            return Vec::new();
        };
        let store = self.store.borrow();
        let mut parts: Vec<(Option<u64>, String)> = vec![(None, tr(Msg::NavAll).to_string())];
        for f in store.lib.breadcrumb(folder) {
            parts.push((Some(f.id), short(&f.name)));
        }
        let mut x = SIDEBAR_W + MARGIN;
        let mut out = Vec::new();
        for (i, (id, label)) in parts.into_iter().enumerate() {
            if i > 0 {
                x += 22.0;
            }
            let w = self.assets.font.layout(&label, 20.0).1 as f64;
            out.push(Crumb {
                folder: id,
                rect: Rect::new(x - 6.0, 22.0, x + w + 6.0, 62.0),
                label,
            });
            x += w;
        }
        out
    }

    pub fn items(&self) -> Vec<Item> {
        let s = self.store.borrow();
        s.lib
            .view(self.loc, self.sort_key, self.ascending, self.search.text())
            .iter()
            .map(|e| e.item())
            .collect()
    }

    /// Card rectangle for the `i`-th visible item (with scrolling applied).
    pub fn card_rect(&self, i: usize) -> Rect {
        let cols = self.columns();
        let x = SIDEBAR_W + MARGIN + (i % cols) as f64 * (CARD_W + CARD_GAP);
        let y = GRID_TOP + (i / cols) as f64 * (CARD_H + CARD_GAP) - self.scroll.pos as f64;
        Rect::new(x, y, x + CARD_W, y + CARD_H)
    }

    pub fn columns(&self) -> usize {
        let room = self.vp.w - SIDEBAR_W - 2.0 * MARGIN + CARD_GAP;
        ((room / (CARD_W + CARD_GAP)).floor() as usize).max(1)
    }

    pub fn max_scroll(&self, count: usize) -> f32 {
        let rows = count.div_ceil(self.columns()) as f64;
        ((GRID_TOP + rows * (CARD_H + CARD_GAP)) - self.vp.h).max(0.0) as f32
    }

    pub fn item_at(&self, p: Point) -> Option<Item> {
        if p.y < GRID_TOP - CARD_GAP / 2.0 || p.x < SIDEBAR_W {
            return None;
        }
        self.items()
            .into_iter()
            .enumerate()
            .find(|(i, _)| self.card_rect(*i).contains(p))
            .map(|(_, item)| item)
    }

    pub fn card_of(&self, item: Item) -> Option<Rect> {
        self.items()
            .iter()
            .position(|i| *i == item)
            .map(|i| self.card_rect(i))
    }
}

/// Cuts long names for the breadcrumb.
fn short(name: &str) -> String {
    const MAX: usize = 18;
    if name.chars().count() <= MAX {
        return name.to_string();
    }
    let cut: String = name.chars().take(MAX - 1).collect();
    format!("{cut}…")
}
