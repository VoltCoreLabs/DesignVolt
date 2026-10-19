//! Settings screen (a full page, not a modal): profile name, theme, language.

use std::cell::RefCell;
use std::rc::Rc;

use vello::Scene;
use vello::kurbo::{Affine, Point, Rect};

use super::backdrop;
use crate::anim::{ease_out_cubic, progress};
use crate::draw::Cx;
use crate::i18n::{Lang, Msg, set_lang, tr};
use crate::icons::Icon;
use crate::input::{Key, Mods, Screen, SharedAssets, Transition, Viewport};
use crate::store::{NAME_MAX_CHARS, Store};
use crate::theme::colors::*;
use crate::theme::metrics::*;
use crate::theme::motion::{PANEL_INTRO, STAGGER};
use crate::widgets::button::Button;
use crate::widgets::segmented::{Segment, Segmented, segment_at};
use crate::widgets::text_input::TextInput;

const COLUMN_W: f64 = 580.0;
const MARGIN: f64 = 32.0;
const TOP: f64 = 40.0;
const CARD_PAD: f64 = 24.0;
const FLASH_SECS: f32 = 1.6;

pub struct Settings {
    assets: SharedAssets,
    store: Rc<RefCell<Store>>,
    vp: Viewport,
    time: f32,
    mouse: Point,
    name: TextInput,
    save: Button,
    theme: Segmented,
    language: Segmented,
    /// Seconds left to show the "Saved" confirmation.
    flash: f32,
    next: Option<Transition>,
}

struct Layout {
    title: (f64, f64),
    cards: [Rect; 3],
    name_input: Rect,
    save: Rect,
    theme: Rect,
    language: Rect,
}

impl Settings {
    pub fn new(assets: SharedAssets, store: Rc<RefCell<Store>>) -> Self {
        let (dark, lang, current) = {
            let s = store.borrow();
            (s.dark, s.lang, s.name.clone().unwrap_or_default())
        };
        let mut name = TextInput::new(NAME_MAX_CHARS, Msg::NamePlaceholder, true);
        name.insert(&current);
        Self {
            assets,
            store,
            vp: Viewport::new(),
            time: 0.0,
            mouse: Point::new(-1.0, -1.0),
            name,
            save: Button::new(),
            theme: Segmented::new(usize::from(dark), 2),
            language: Segmented::new(usize::from(lang == Lang::En), 2),
            flash: 0.0,
            next: None,
        }
    }

    fn layout(&self) -> Layout {
        let w = COLUMN_W.min(self.vp.w - 2.0 * MARGIN);
        let x0 = (self.vp.w - w) / 2.0;
        let card = |y: f64, h: f64| Rect::new(x0, y, x0 + w, y + h);
        let profile = card(TOP + 64.0, 132.0);
        let look = card(profile.y1 + 16.0, 88.0);
        let lang = card(look.y1 + 16.0, 88.0);
        let inner_x1 = profile.x1 - CARD_PAD;
        let save = Rect::new(
            inner_x1 - 130.0,
            profile.y0 + 68.0,
            inner_x1,
            profile.y0 + 68.0 + 44.0,
        );
        let name_input = Rect::new(profile.x0 + CARD_PAD, save.y0, save.x0 - 12.0, save.y1);
        let seg = |c: Rect| {
            Rect::new(
                c.x1 - CARD_PAD - 280.0,
                c.center().y - 20.0,
                c.x1 - CARD_PAD,
                c.center().y + 20.0,
            )
        };
        Layout {
            title: (x0, TOP + 28.0),
            cards: [profile, look, lang],
            name_input,
            save,
            theme: seg(look),
            language: seg(lang),
        }
    }

    fn at(&self, r: Rect) -> bool {
        r.contains(self.mouse)
    }

    fn name_changed(&self) -> bool {
        let s = self.store.borrow();
        !self.name.is_blank() && s.name.as_deref() != Some(self.name.text().trim())
    }

    fn commit_name(&mut self) {
        if !self.name_changed() {
            return;
        }
        let mut s = self.store.borrow_mut();
        s.set_name(self.name.text().trim());
        if let Err(e) = s.save() {
            eprintln!("could not save profile: {e}");
        }
        drop(s);
        self.save.press();
        self.flash = FLASH_SECS;
    }

    fn set_theme(&mut self, dark: bool) {
        set_dark(dark);
        self.theme.select(usize::from(dark));
        self.persist(|s| s.dark = dark);
    }

    fn set_language(&mut self, lang: Lang) {
        set_lang(lang);
        self.language.select(usize::from(lang == Lang::En));
        self.persist(|s| s.lang = lang);
    }

    fn persist(&self, change: impl FnOnce(&mut Store)) {
        let mut s = self.store.borrow_mut();
        change(&mut s);
        if let Err(e) = s.save() {
            eprintln!("could not save settings: {e}");
        }
    }

    fn intro(&self, i: f32) -> f32 {
        ease_out_cubic(progress(self.time, i * STAGGER, PANEL_INTRO))
    }
}

fn card_title(cx: &mut Cx, card: Rect, msg: Msg) {
    cx.text(
        tr(msg),
        FONT_MD,
        card.x0 + CARD_PAD,
        card.y0 + 34.0,
        hex(text_primary()),
    );
}

impl Screen for Settings {
    fn set_viewport(&mut self, w: u32, h: u32, scale: f64) {
        self.vp.set(w, h, scale);
    }

    fn update(&mut self, dt: f32) -> bool {
        self.time += dt;
        self.flash = (self.flash - dt).max(0.0);
        let l = self.layout();
        let theme_hover = segment_at(l.theme, 2, self.mouse);
        let lang_hover = segment_at(l.language, 2, self.mouse);
        let mut busy = self.name.update(dt);
        busy |= self.save.update(dt, self.at(l.save) && self.name_changed());
        busy |= self.theme.update(dt, theme_hover);
        busy |= self.language.update(dt, lang_hover);
        busy || self.flash > 0.0 || self.time < PANEL_INTRO + 5.0 * STAGGER
    }

    fn draw(&self, scene: &mut Scene) {
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
        cx.layer(full, p, (0.0, -12.0 * (1.0 - p as f64)), |cx| {
            cx.text(
                tr(Msg::Settings),
                22.0,
                l.title.0,
                l.title.1,
                hex(text_primary()),
            );
        });

        for (i, card) in l.cards.iter().enumerate() {
            let p = self.intro(1.0 + i as f32);
            cx.layer(
                card.inflate(30.0, 30.0),
                p,
                (0.0, 16.0 * (1.0 - p as f64)),
                |cx| {
                    cx.shadow(*card, R_CARD, alpha(0x000000, 0.12), 16.0, 6.0);
                    cx.fill(*card, R_CARD, hex(bg_panel()));
                    cx.stroke(*card, R_CARD, 1.0, alpha(overlay(), 0.08));
                    match i {
                        0 => self.profile_card(cx, *card, &l),
                        1 => {
                            card_title(cx, *card, Msg::SettingsTheme);
                            self.theme.draw(
                                cx,
                                l.theme,
                                &[
                                    Segment {
                                        icon: Some(Icon::Sun),
                                        label: tr(Msg::ThemeLight),
                                    },
                                    Segment {
                                        icon: Some(Icon::Moon),
                                        label: tr(Msg::ThemeDark),
                                    },
                                ],
                            );
                        }
                        _ => {
                            card_title(cx, *card, Msg::SettingsLanguage);
                            self.language.draw(
                                cx,
                                l.language,
                                &[
                                    Segment {
                                        icon: None,
                                        label: "Русский",
                                    },
                                    Segment {
                                        icon: None,
                                        label: "English",
                                    },
                                ],
                            );
                        }
                    }
                },
            );
        }
    }

    fn mouse_moved(&mut self, x: f64, y: f64) {
        self.mouse = Point::new(x / self.vp.scale, y / self.vp.scale);
        let left = self.layout().name_input.x0;
        self.name.drag_to(&self.assets.font, self.mouse.x - left);
    }

    fn mouse_left(&mut self) {
        self.mouse = Point::new(-1.0, -1.0);
    }

    fn mouse_up(&mut self) {
        self.name.dragging = false;
    }

    fn click(&mut self, mods: Mods) {
        let l = self.layout();
        if let Some(i) = segment_at(l.theme, 2, self.mouse) {
            self.set_theme(i == 1);
        } else if let Some(i) = segment_at(l.language, 2, self.mouse) {
            self.set_language(if i == 0 { Lang::Ru } else { Lang::En });
        } else if self.at(l.save) {
            self.commit_name();
        } else if self.at(l.name_input) {
            self.name.press_at(
                &self.assets.font,
                self.mouse.x - l.name_input.x0,
                mods.shift,
            );
        } else {
            self.name.focused = false;
        }
    }

    fn key(&mut self, key: Key, mods: Mods) {
        match key {
            Key::Escape => self.next = Some(Transition::BackToProjects),
            Key::Enter if self.name.focused => self.commit_name(),
            k if self.name.focused => self.name.key(k, mods),
            _ => {}
        }
    }

    fn text(&mut self, s: &str) {
        if self.name.focused {
            self.name.insert(s);
        }
    }

    fn pointer_hand(&self) -> bool {
        let l = self.layout();
        segment_at(l.theme, 2, self.mouse).is_some()
            || segment_at(l.language, 2, self.mouse).is_some()
            || (self.at(l.save) && self.name_changed())
            || self.at(l.name_input)
    }

    fn take_transition(&mut self) -> Option<Transition> {
        self.next.take()
    }
}

impl Settings {
    fn profile_card(&self, cx: &mut Cx, card: Rect, l: &Layout) {
        card_title(cx, card, Msg::SettingsProfile);
        cx.text(
            tr(Msg::SettingsYourName),
            FONT_SM,
            l.name_input.x0,
            l.name_input.y0 - 8.0,
            hex(text_secondary()),
        );
        self.name.draw(cx, l.name_input);
        self.save
            .draw(cx, l.save, tr(Msg::Save), None, self.name_changed());
        if self.flash > 0.0 {
            let a = (self.flash / 0.4).min(1.0);
            let w = cx.text_width(tr(Msg::Saved), FONT_SM);
            cx.text(
                tr(Msg::Saved),
                FONT_SM,
                card.x1 - CARD_PAD - w,
                card.y0 + 34.0,
                alpha(accent(), a),
            );
        }
    }
}
