//! Window, GPU surface and the frame loop.

use std::sync::Arc;
use std::time::Instant;

use vello::kurbo::Point;
use vello::peniko::Color;
use vello::util::{RenderContext, RenderSurface};
use vello::wgpu::{self, CurrentSurfaceTexture, PresentMode};
use vello::{AaConfig, RenderParams, Renderer, RendererOptions, Scene};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::window::{CursorIcon, Window, WindowId};

use crate::text::Font;
use crate::ui::Ui;

const FONTS: &[&str] = &[
    "C:/Windows/Fonts/segoeuib.ttf",
    "C:/Windows/Fonts/segoeui.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
    "/System/Library/Fonts/Helvetica.ttc",
];

struct Gfx {
    window: Arc<Window>,
    surface: RenderSurface<'static>,
    renderer: Renderer,
}

pub struct App {
    ctx: RenderContext,
    gfx: Option<Gfx>,
    scene: Scene,
    ui: Ui,
    last: Instant,
}

impl App {
    pub fn new() -> Self {
        Self {
            ctx: RenderContext::new(),
            gfx: None,
            scene: Scene::new(),
            ui: Ui::new(Font::load(FONTS)),
            last: Instant::now(),
        }
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

        let animating = self.ui.update(dt);
        self.ui.draw(&mut self.scene);

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
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gfx.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("DesignVolt")
            .with_inner_size(LogicalSize::new(960.0, 640.0));
        let window = Arc::new(event_loop.create_window(attrs).expect("window"));
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
        self.ui
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
                self.ui
                    .set_viewport(size.width, size.height, gfx.window.scale_factor());
                gfx.window.request_redraw();
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.ui.mouse = Point::new(position.x, position.y);
                gfx.window.set_cursor(if self.ui.hovered() {
                    CursorIcon::Pointer
                } else {
                    CursorIcon::Default
                });
                gfx.window.request_redraw();
            }
            WindowEvent::CursorLeft { .. } => {
                self.ui.mouse = Point::new(-1.0, -1.0);
                gfx.window.request_redraw();
            }
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                match state {
                    ElementState::Pressed => {
                        self.ui.pressed = true;
                        if self.ui.hovered() {
                            self.ui.click();
                        }
                    }
                    ElementState::Released => self.ui.pressed = false,
                }
                gfx.window.request_redraw();
            }
            WindowEvent::RedrawRequested => self.render(),
            _ => {}
        }
    }
}
