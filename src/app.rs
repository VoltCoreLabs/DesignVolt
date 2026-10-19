//! Window, GPU surface, screen switching and the frame loop.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use vello::peniko::Color;
use vello::util::{RenderContext, RenderSurface};
use vello::wgpu::{self, CurrentSurfaceTexture, PresentMode};
use vello::{AaConfig, RenderParams, Renderer, RendererOptions, Scene};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key as WKey, NamedKey};
use winit::window::{CursorIcon, Window, WindowId};

use crate::editor::Editor;
use crate::icons::Icons;
use crate::input::{Assets, Key, Screen, SharedAssets, Transition};
use crate::screens::projects::Projects;
use crate::screens::welcome::Welcome;
use crate::store::{Store, default_dir};
use crate::text::Font;

const FONT_BYTES: &[u8] = include_bytes!("../assets/fonts/Inter.ttf");
/// Logical pixels scrolled per mouse-wheel line.
const WHEEL_LINE: f64 = 64.0;

struct Gfx {
    window: Arc<Window>,
    surface: RenderSurface<'static>,
    renderer: Renderer,
}

pub struct App {
    ctx: RenderContext,
    gfx: Option<Gfx>,
    scene: Scene,
    assets: SharedAssets,
    store: Rc<RefCell<Store>>,
    screen: Box<dyn Screen>,
    last: Instant,
}

impl App {
    pub fn new() -> Self {
        let assets = Rc::new(Assets {
            font: Font::from_bytes(FONT_BYTES).expect("embedded font"),
            icons: Icons::new(),
        });
        let store = Rc::new(RefCell::new(Store::load(&default_dir())));
        let screen: Box<dyn Screen> = if store.borrow().name.is_some() {
            Box::new(Projects::new(assets.clone(), store.clone()))
        } else {
            Box::new(Welcome::new(assets.clone()))
        };
        Self {
            ctx: RenderContext::new(),
            gfx: None,
            scene: Scene::new(),
            assets,
            store,
            screen,
            last: Instant::now(),
        }
    }

    fn switch(&mut self, t: Transition) {
        self.screen = match t {
            Transition::SetName(name) => {
                let mut s = self.store.borrow_mut();
                s.set_name(&name);
                if let Err(e) = s.save() {
                    eprintln!("could not save profile: {e}");
                }
                drop(s);
                Box::new(Projects::new(self.assets.clone(), self.store.clone()))
            }
            Transition::OpenProject(_) => Box::new(Editor::new(self.assets.clone())),
            Transition::BackToProjects => Box::new(Projects::new(self.assets.clone(), self.store.clone())),
        };
        if let Some(gfx) = &self.gfx {
            let size = gfx.window.inner_size();
            self.screen.set_viewport(size.width, size.height, gfx.window.scale_factor());
            gfx.window.request_redraw();
        }
        self.last = Instant::now();
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

        let animating = self.screen.update(dt);
        self.screen.draw(&mut self.scene);

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
                self.screen.key(k);
                return;
            }
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
        self.screen
            .set_viewport(size.width, size.height, window.scale_factor());
        self.last = Instant::now();
        window.request_redraw();
        self.gfx = Some(Gfx { window, surface, renderer });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(gfx) = &mut self.gfx else { return };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) if size.width > 0 && size.height > 0 => {
                self.ctx.resize_surface(&mut gfx.surface, size.width, size.height);
                self.screen
                    .set_viewport(size.width, size.height, gfx.window.scale_factor());
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.screen.mouse_moved(position.x, position.y);
                gfx.window.set_cursor(if self.screen.pointer_hand() {
                    CursorIcon::Pointer
                } else {
                    CursorIcon::Default
                });
            }
            WindowEvent::CursorLeft { .. } => self.screen.mouse_left(),
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => {
                self.screen.click();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y as f64 * WHEEL_LINE,
                    MouseScrollDelta::PixelDelta(p) => p.y / gfx.window.scale_factor(),
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
