//! Window, GPU surface, screen switching, title bar and the frame loop.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use vello::kurbo::Affine;
use vello::peniko::Color;
use vello::util::{RenderContext, RenderSurface};
use vello::wgpu::{self, CurrentSurfaceTexture, PresentMode};
use vello::{AaConfig, RenderParams, Renderer, RendererOptions, Scene};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key as WKey, KeyCode, NamedKey, PhysicalKey};
use winit::window::{CursorIcon, Window, WindowId};

use crate::chrome::{cursor_for, resize_direction};
use crate::draw::Cx;
use crate::editor::Editor;
use crate::icons::Icons;
use crate::input::{Assets, Key, Mods, Screen, SharedAssets, Transition};
use crate::screens::projects::Projects;
use crate::screens::welcome::Welcome;
use crate::store::{Store, default_dir};
use crate::text::Font;
use crate::titlebar::{BarState, HEIGHT as BAR_H, Part, TitleBar};

const FONT_BYTES: &[u8] = include_bytes!("../assets/fonts/Inter.ttf");
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
    Editor(String),
}

impl Place {
    fn bar_state(&self, maximized: bool) -> BarState<'_> {
        match self {
            Place::Welcome => BarState { home: false, label: None, maximized },
            Place::Projects => BarState { home: true, label: Some("All projects"), maximized },
            Place::Editor(name) => BarState { home: true, label: Some(name), maximized },
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
        let (screen, place): (Box<dyn Screen>, Place) = if store.borrow().name.is_some() {
            (Box::new(Projects::new(assets.clone(), store.clone())), Place::Projects)
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
        self.screen.set_viewport(w, h.saturating_sub(bar).max(1), self.scale());
    }

    fn switch(&mut self, t: Transition) {
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
                let name = self.store.borrow().project(id).map(|p| p.name.clone());
                self.place = Place::Editor(name.unwrap_or_default());
                self.screen = Box::new(Editor::new(self.assets.clone()));
            }
            Transition::BackToProjects => self.show_projects(),
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
        self.gfx.as_ref().map_or(1.0, |g| g.surface.config.width as f64 / g.window.scale_factor())
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
        self.scene
            .append(&self.child, Some(Affine::translate((0.0, (BAR_H * scale).round()))));
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
            base_color: Color::from_rgba8(10, 12, 20, 255),
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
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
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
        resize_direction(self.mouse.0, self.mouse.1, size.width as f64, size.height as f64, border)
    }

    fn pointer_moved(&mut self, x: f64, y: f64) {
        self.mouse = (x, y);
        let s = self.scale();
        self.bar.mouse_moved(x / s, y / s);
        self.screen.mouse_moved(x, y - self.bar_phys());
        let home = !matches!(self.place, Place::Welcome);
        let icon = if let Some(dir) = self.resize_dir() {
            cursor_for(dir)
        } else {
            match self.bar.hit(self.logical_width(), home) {
                Some(Part::Drag) => CursorIcon::Default,
                Some(_) => CursorIcon::Pointer,
                None if self.screen.pointer_hand() => CursorIcon::Pointer,
                None => CursorIcon::Default,
            }
        };
        if let Some(g) = &self.gfx {
            g.window.set_cursor(icon);
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
        let Some(part) = self.bar.hit(self.logical_width(), home) else { return false };
        match part {
            Part::Close => event_loop.exit(),
            Part::Minimize => gfx.window.set_minimized(true),
            Part::Maximize => gfx.window.set_maximized(!gfx.window.is_maximized()),
            Part::Home => {
                if !matches!(self.place, Place::Projects) {
                    self.switch(Transition::BackToProjects);
                }
            }
            Part::Drag => {
                let double = self.last_bar_click.is_some_and(|t| t.elapsed() < DOUBLE_CLICK);
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
                NamedKey::ArrowLeft => Some(Key::Left),
                NamedKey::ArrowRight => Some(Key::Right),
                NamedKey::Home => Some(Key::Home),
                NamedKey::End => Some(Key::End),
                NamedKey::Enter => Some(Key::Enter),
                NamedKey::Escape => Some(Key::Escape),
                _ => None,
            };
            if let Some(k) = key {
                self.screen.key(k, self.mods);
                return;
            }
        }
        if self.mods.ctrl {
            // Shortcuts use the physical key so they work on any keyboard layout (e.g. Russian).
            if let PhysicalKey::Code(KeyCode::KeyA) = event.physical_key {
                self.screen.key(Key::SelectAll, self.mods);
            }
            return;
        }
        if let Some(text) = &event.text {
            self.screen.text(text);
        }
    }
}

impl ApplicationHandler for App {
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
        self.gfx = Some(Gfx { window: window.clone(), surface, renderer });
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
                    self.ctx.resize_surface(&mut gfx.surface, size.width, size.height);
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
                self.mods = Mods { ctrl: s.control_key() || s.super_key(), shift: s.shift_key() };
                return;
            }
            WindowEvent::MouseInput { state: ElementState::Released, button: MouseButton::Left, .. } => {
                self.screen.mouse_up();
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => {
                if !self.chrome_click(event_loop) {
                    self.screen.click();
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y as f64 * WHEEL_LINE,
                    MouseScrollDelta::PixelDelta(p) => p.y / self.scale(),
                };
                self.screen.scroll(dy);
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
        if let Some(gfx) = &self.gfx {
            gfx.window.request_redraw();
        }
    }
}
