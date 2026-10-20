//! Window, GPU surface, screen switching, title bar and the frame loop.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use vello::kurbo::Affine;
use vello::util::{RenderContext, RenderSurface};
use vello::wgpu::{self, CurrentSurfaceTexture, PresentMode};
use vello::{AaConfig, RenderParams, Renderer, RendererOptions, Scene};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key as WKey, KeyCode, NamedKey, PhysicalKey};
use winit::window::{CursorIcon, Window, WindowId};

use crate::chrome::{cursor_for, icon_of, resize_direction};
use crate::editor::{Editor, Session};
use crate::i18n::{Msg, set_lang, tr};
use crate::input::{Assets, Key, Mods, Screen, SharedAssets, Transition};
use crate::screens::projects::Projects;
use crate::screens::settings::Settings;
use crate::screens::welcome::Welcome;
use crate::store::{Store, default_dir};
use crate::titlebar::{BarState, HEIGHT as BAR_H, Part, TitleBar};
use dv_ui::draw::Cx;
use dv_ui::icons::Icons;
use dv_ui::text::Font;
use dv_ui::theme::colors::{bg_app, hex, set_dark};

const FONT_BYTES: &[u8] = include_bytes!("../../../assets/fonts/GolosText.ttf");
/// Logical pixels scrolled per mouse-wheel line.
const WHEEL_LINE: f64 = 64.0;
/// Logical width of the invisible resize border.
const RESIZE_BORDER: f64 = 6.0;
const DOUBLE_CLICK: Duration = Duration::from_millis(400);

struct Gfx {
    window: Arc<Window>,
    surface: RenderSurface<'static>,
    renderer: Renderer,
}

/// Where the user currently is; drives what the title bar shows.
enum Place {
    Welcome,
    Projects,
    Settings,
    Editor(String),
}

impl Place {
    fn bar_state(&self, maximized: bool) -> BarState<'_> {
        match self {
            Place::Welcome => BarState {
                home: false,
                label: None,
                maximized,
            },
            Place::Projects => BarState {
                home: true,
                label: Some(tr(Msg::NavAll)),
                maximized,
            },
            Place::Settings => BarState {
                home: true,
                label: Some(tr(Msg::Settings)),
                maximized,
            },
            Place::Editor(name) => BarState {
                home: true,
                label: Some(name),
                maximized,
            },
        }
    }
}

pub struct App {
    ctx: RenderContext,
    gfx: Option<Gfx>,
    scene: Scene,
    child: Scene,
    assets: SharedAssets,
    store: Rc<RefCell<Store>>,
    screen: Box<dyn Screen>,
    place: Place,
    bar: TitleBar,
    last: Instant,
    last_bar_click: Option<Instant>,
    mods: Mods,
    /// Open documents, kept for the whole app session (not saved to disk yet).
    sessions: HashMap<u64, Rc<RefCell<Session>>>,
    /// Pointer position in physical pixels.
    mouse: (f64, f64),
}

impl App {
    pub fn new() -> Self {
        let assets = Rc::new(Assets {
            font: Font::from_bytes(FONT_BYTES).expect("embedded font"),
            icons: Icons::new(),
        });
        let store = Rc::new(RefCell::new(Store::load(&default_dir())));
        set_dark(store.borrow().dark);
        set_lang(store.borrow().lang);
        let (screen, place): (Box<dyn Screen>, Place) = if store.borrow().name.is_some() {
            (
                Box::new(Projects::new(assets.clone(), store.clone())),
                Place::Projects,
            )
        } else {
            (Box::new(Welcome::new(assets.clone())), Place::Welcome)
        };
        Self {
            ctx: RenderContext::new(),
            gfx: None,
            scene: Scene::new(),
            child: Scene::new(),
            assets,
            store,
            screen,
            place,
            bar: TitleBar::new(),
            last: Instant::now(),
            last_bar_click: None,
            mods: Mods::default(),
            sessions: HashMap::new(),
            mouse: (-1.0, -1.0),
        }
    }

    fn scale(&self) -> f64 {
        self.gfx.as_ref().map_or(1.0, |g| g.window.scale_factor())
    }

    fn bar_phys(&self) -> f64 {
        (BAR_H * self.scale()).round()
    }

    /// Gives the active screen the area below the title bar.
    fn sync_viewport(&mut self, w: u32, h: u32) {
        let bar = self.bar_phys() as u32;
        self.screen
            .set_viewport(w, h.saturating_sub(bar).max(1), self.scale());
    }

    /// Writes every document that has unsaved changes and marks its project as edited.
    fn flush_sessions(&mut self) {
        let mut touched = false;
        for (id, session) in &self.sessions {
            if session.borrow_mut().save() {
                self.store
                    .borrow_mut()
                    .lib
                    .touch(*id, crate::store::now_secs());
                touched = true;
            }
        }
        if touched {
            self.store.borrow().save_logged();
        }
    }

    fn switch(&mut self, t: Transition) {
        self.flush_sessions();
        match t {
            Transition::SetName(name) => {
                let mut s = self.store.borrow_mut();
                s.set_name(&name);
                if let Err(e) = s.save() {
                    eprintln!("could not save profile: {e}");
                }
                drop(s);
                self.show_projects();
            }
            Transition::OpenProject(id) => {
                let name = self
                    .store
                    .borrow()
                    .lib
                    .project(id)
                    .map(|p| p.name.clone())
                    .unwrap_or_default();
                let session = self
                    .sessions
                    .entry(id)
                    .or_insert_with(|| {
                        let path = self.store.borrow().document_path(id);
                        Rc::new(RefCell::new(Session::open(&name, path)))
                    })
                    .clone();
                self.place = Place::Editor(name.clone());
                self.screen = Box::new(Editor::new(self.assets.clone(), session, name));
            }
            Transition::BackToProjects => self.show_projects(),
            Transition::OpenSettings => {
                self.place = Place::Settings;
                self.screen = Box::new(Settings::new(self.assets.clone(), self.store.clone()));
            }
        }
        if let Some(gfx) = &self.gfx {
            let size = gfx.window.inner_size();
            self.sync_viewport(size.width, size.height);
            if let Some(g) = &self.gfx {
                g.window.request_redraw();
            }
        }
        self.last = Instant::now();
    }

    fn show_projects(&mut self) {
        self.place = Place::Projects;
        self.screen = Box::new(Projects::new(self.assets.clone(), self.store.clone()));
    }

    fn logical_width(&self) -> f64 {
        self.gfx.as_ref().map_or(1.0, |g| {
            g.surface.config.width as f64 / g.window.scale_factor()
        })
    }

    fn render(&mut self) {
        let Some(gfx) = &mut self.gfx else { return };
        let (w, h) = (gfx.surface.config.width, gfx.surface.config.height);
        if w == 0 || h == 0 {
            return;
        }
        let now = Instant::now();
        let dt = now.duration_since(self.last).as_secs_f32().min(0.05);
        self.last = now;

        let scale = gfx.window.scale_factor();
        let lw = w as f64 / scale;
        let state = self.place.bar_state(gfx.window.is_maximized());
        let mut animating = self.bar.update(dt, lw, &state);
        animating |= self.screen.update(dt);

        self.screen.draw(&mut self.child);
        self.scene.reset();
        self.scene.append(
            &self.child,
            Some(Affine::translate((0.0, (BAR_H * scale).round()))),
        );
        let mut cx = Cx {
            scene: &mut self.scene,
            font: &self.assets.font,
            icons: &self.assets.icons,
            xf: Affine::scale(scale),
        };
        self.bar.draw(&mut cx, lw, &state);

        let handle = &self.ctx.devices[gfx.surface.dev_id];
        let frame = match gfx.surface.surface.get_current_texture() {
            CurrentSurfaceTexture::Success(t) | CurrentSurfaceTexture::Suboptimal(t) => t,
            CurrentSurfaceTexture::Outdated | CurrentSurfaceTexture::Lost => {
                self.ctx.resize_surface(&mut gfx.surface, w, h);
                gfx.window.request_redraw();
                return;
            }
            _ => {
                gfx.window.request_redraw();
                return;
            }
        };
        let params = RenderParams {
            base_color: hex(bg_app()),
            width: w,
            height: h,
            antialiasing_method: AaConfig::Area,
        };
        if let Err(e) = gfx.renderer.render_to_texture(
            &handle.device,
            &handle.queue,
            &self.scene,
            &gfx.surface.target_view,
            &params,
        ) {
            eprintln!("render error: {e}");
            return;
        }
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut enc = handle
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        gfx.surface
            .blitter
            .copy(&handle.device, &mut enc, &gfx.surface.target_view, &view);
        handle.queue.submit([enc.finish()]);
        handle.queue.present(frame);

        if animating {
            gfx.window.request_redraw();
        }
    }

    fn resize_dir(&self) -> Option<winit::window::ResizeDirection> {
        let gfx = self.gfx.as_ref()?;
        if gfx.window.is_maximized() {
            return None;
        }
        let size = gfx.window.inner_size();
        let border = RESIZE_BORDER * gfx.window.scale_factor();
        resize_direction(
            self.mouse.0,
            self.mouse.1,
            size.width as f64,
            size.height as f64,
            border,
        )
    }

    fn pointer_moved(&mut self, x: f64, y: f64) {
        self.mouse = (x, y);
        let s = self.scale();
        self.bar.mouse_moved(x / s, y / s);
        self.screen.mouse_moved(x, y - self.bar_phys());
        self.update_cursor();
    }

    /// Chooses the cursor from the window chrome (resize edges, title bar) or the screen.
    fn update_cursor(&self) {
        let home = !matches!(self.place, Place::Welcome);
        let icon = if let Some(dir) = self.resize_dir() {
            cursor_for(dir)
        } else {
            match self.bar.hit(self.logical_width(), home) {
                Some(Part::Drag) => CursorIcon::Default,
                Some(_) => CursorIcon::Pointer,
                None => icon_of(self.screen.cursor()),
            }
        };
        let hide = self.resize_dir().is_none()
            && self.bar.hit(self.logical_width(), home).is_none()
            && self.screen.cursor() == crate::input::Cursor::Rotate;
        if let Some(g) = &self.gfx {
            g.window.set_cursor(icon);
            g.window.set_cursor_visible(!hide);
        }
    }

    /// Returns true when the click was consumed by the window chrome.
    fn chrome_click(&mut self, event_loop: &ActiveEventLoop) -> bool {
        let Some(gfx) = &self.gfx else { return false };
        if let Some(dir) = self.resize_dir() {
            let _ = gfx.window.drag_resize_window(dir);
            return true;
        }
        let home = !matches!(self.place, Place::Welcome);
        let Some(part) = self.bar.hit(self.logical_width(), home) else {
            return false;
        };
        match part {
            Part::Close => event_loop.exit(),
            Part::Settings => {
                if !matches!(self.place, Place::Settings) {
                    self.switch(Transition::OpenSettings);
                }
            }
            Part::Minimize => gfx.window.set_minimized(true),
            Part::Maximize => gfx.window.set_maximized(!gfx.window.is_maximized()),
            Part::Home => {
                if !matches!(self.place, Place::Projects) {
                    self.switch(Transition::BackToProjects);
                }
            }
            Part::Drag => {
                let double = self
                    .last_bar_click
                    .is_some_and(|t| t.elapsed() < DOUBLE_CLICK);
                if double {
                    gfx.window.set_maximized(!gfx.window.is_maximized());
                    self.last_bar_click = None;
                } else {
                    self.last_bar_click = Some(Instant::now());
                    let _ = gfx.window.drag_window();
                }
            }
        }
        true
    }

    fn handle_key(&mut self, event: &winit::event::KeyEvent) {
        if let WKey::Named(named) = &event.logical_key {
            let key = match named {
                NamedKey::Backspace => Some(Key::Backspace),
                NamedKey::Delete => Some(Key::Delete),
                NamedKey::Insert => Some(Key::Insert),
                NamedKey::ArrowLeft => Some(Key::Left),
                NamedKey::ArrowRight => Some(Key::Right),
                NamedKey::ArrowUp => Some(Key::Up),
                NamedKey::ArrowDown => Some(Key::Down),
                NamedKey::Home => Some(Key::Home),
                NamedKey::End => Some(Key::End),
                NamedKey::Enter => Some(Key::Enter),
                NamedKey::Escape => Some(Key::Escape),
                NamedKey::Tab => Some(Key::Tab),
                _ => None,
            };
            if let Some(k) = key {
                self.screen.key(k, self.mods);
                return;
            }
            if *named == NamedKey::Space {
                // Space is both a key (pan in the editor) and a typed character.
                self.screen.key(Key::Space, self.mods);
            }
        }
        // Shortcuts use the physical key so they work on any keyboard layout (e.g. Russian).
        let letter = physical_char(event.physical_key);
        if self.mods.ctrl {
            let key = match event.physical_key {
                PhysicalKey::Code(KeyCode::KeyA) => Some(Key::SelectAll),
                PhysicalKey::Code(KeyCode::KeyC) => Some(Key::Copy),
                PhysicalKey::Code(KeyCode::KeyX) => Some(Key::Cut),
                PhysicalKey::Code(KeyCode::KeyV) => Some(Key::Paste),
                PhysicalKey::Code(KeyCode::KeyY) => Some(Key::Redo),
                PhysicalKey::Code(KeyCode::KeyZ) if self.mods.shift => Some(Key::Redo),
                PhysicalKey::Code(KeyCode::KeyZ) => Some(Key::Undo),
                _ => letter.map(Key::Char),
            };
            if let Some(k) = key {
                self.screen.key(k, self.mods);
            }
            return;
        }
        if let Some(c) = letter {
            self.screen.key(Key::Char(c), self.mods);
        }
        if let Some(text) = &event.text {
            self.screen.text(text);
        }
    }
}

impl ApplicationHandler for App {
    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.flush_sessions();
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gfx.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("DesignVolt")
            .with_decorations(false)
            .with_inner_size(LogicalSize::new(1180.0, 760.0))
            .with_min_inner_size(LogicalSize::new(900.0, 600.0));
        let window = Arc::new(event_loop.create_window(attrs).expect("window"));
        window.set_ime_allowed(true);
        let size = window.inner_size();
        let surface = pollster::block_on(self.ctx.create_surface(
            window.clone(),
            size.width.max(1),
            size.height.max(1),
            PresentMode::AutoVsync,
        ))
        .expect("surface");
        let device = &self.ctx.devices[surface.dev_id].device;
        let renderer = Renderer::new(
            device,
            RendererOptions {
                antialiasing_support: vello::AaSupport::area_only(),
                ..RendererOptions::default()
            },
        )
        .expect("renderer");
        self.gfx = Some(Gfx {
            window: window.clone(),
            surface,
            renderer,
        });
        self.sync_viewport(size.width, size.height);
        self.last = Instant::now();
        window.request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if self.gfx.is_none() {
            return;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) if size.width > 0 && size.height > 0 => {
                if let Some(gfx) = &mut self.gfx {
                    self.ctx
                        .resize_surface(&mut gfx.surface, size.width, size.height);
                }
                self.sync_viewport(size.width, size.height);
            }
            WindowEvent::CursorMoved { position, .. } => self.pointer_moved(position.x, position.y),
            WindowEvent::CursorLeft { .. } => {
                self.mouse = (-1.0, -1.0);
                self.bar.mouse_left();
                self.screen.mouse_left();
            }
            WindowEvent::ModifiersChanged(m) => {
                let s = m.state();
                self.mods = Mods {
                    ctrl: s.control_key() || s.super_key(),
                    shift: s.shift_key(),
                };
                return;
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                self.screen.mouse_up();
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                if !self.chrome_click(event_loop) {
                    self.screen.click(self.mods);
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } => {
                if self.bar.hit(self.logical_width(), true).is_none() {
                    self.screen.right_click(self.mods);
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => {
                        (x as f64 * WHEEL_LINE, y as f64 * WHEEL_LINE)
                    }
                    MouseScrollDelta::PixelDelta(p) => (p.x / self.scale(), p.y / self.scale()),
                };
                self.screen.wheel(dx, dy, self.mods);
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Middle,
                ..
            } => {
                self.screen.middle_button(state == ElementState::Pressed);
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Released => {
                if event.physical_key == PhysicalKey::Code(KeyCode::Space) {
                    self.screen.key_up(Key::Space);
                }
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                self.handle_key(&event);
            }
            WindowEvent::Ime(Ime::Commit(text)) => self.screen.text(&text),
            WindowEvent::RedrawRequested => {
                self.render();
                if let Some(t) = self.screen.take_transition() {
                    self.switch(t);
                }
                return;
            }
            _ => return,
        }
        if let Some(t) = self.screen.take_transition() {
            self.switch(t);
        }
        self.update_cursor();
        if let Some(gfx) = &self.gfx {
            gfx.window.request_redraw();
        }
    }
}

/// Letter, digit, `=` or `-` of a physical key (independent of the keyboard layout).
fn physical_char(key: PhysicalKey) -> Option<char> {
    const KEYS: [(KeyCode, char); 40] = [
        (KeyCode::KeyA, 'a'),
        (KeyCode::KeyB, 'b'),
        (KeyCode::KeyC, 'c'),
        (KeyCode::KeyD, 'd'),
        (KeyCode::KeyE, 'e'),
        (KeyCode::KeyF, 'f'),
        (KeyCode::KeyG, 'g'),
        (KeyCode::KeyH, 'h'),
        (KeyCode::KeyI, 'i'),
        (KeyCode::KeyJ, 'j'),
        (KeyCode::KeyK, 'k'),
        (KeyCode::KeyL, 'l'),
        (KeyCode::KeyM, 'm'),
        (KeyCode::KeyN, 'n'),
        (KeyCode::KeyO, 'o'),
        (KeyCode::KeyP, 'p'),
        (KeyCode::KeyQ, 'q'),
        (KeyCode::KeyR, 'r'),
        (KeyCode::KeyS, 's'),
        (KeyCode::KeyT, 't'),
        (KeyCode::KeyU, 'u'),
        (KeyCode::KeyV, 'v'),
        (KeyCode::KeyW, 'w'),
        (KeyCode::KeyX, 'x'),
        (KeyCode::KeyY, 'y'),
        (KeyCode::KeyZ, 'z'),
        (KeyCode::Digit0, '0'),
        (KeyCode::Digit1, '1'),
        (KeyCode::Digit2, '2'),
        (KeyCode::Digit3, '3'),
        (KeyCode::Digit4, '4'),
        (KeyCode::Digit5, '5'),
        (KeyCode::Digit6, '6'),
        (KeyCode::Digit7, '7'),
        (KeyCode::Digit8, '8'),
        (KeyCode::Digit9, '9'),
        (KeyCode::Equal, '='),
        (KeyCode::Minus, '-'),
        (KeyCode::BracketLeft, '['),
        (KeyCode::BracketRight, ']'),
    ];
    let PhysicalKey::Code(code) = key else {
        return None;
    };
    KEYS.iter().find(|(k, _)| *k == code).map(|(_, c)| *c)
}
