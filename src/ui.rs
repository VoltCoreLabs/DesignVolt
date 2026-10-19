//! The demo screen: staggered intro animation and an animated button.

use vello::Scene;
use vello::kurbo::{Affine, Circle, Point, Rect, RoundedRect, Vec2};
use vello::peniko::color::AlphaColor;
use vello::peniko::{Color, Fill, Gradient};

use crate::anim::{Spring, ease_out_cubic, progress};
use crate::text::Font;

const BTN_W: f64 = 240.0;
const BTN_H: f64 = 60.0;
const RIPPLE_SECS: f32 = 0.7;

pub struct Ui {
    pub font: Option<Font>,
    pub mouse: Point,
    pub pressed: bool,
    time: f32,
    hover: Spring,
    press: Spring,
    ripple: Option<(Point, f32)>,
    scale: f64,
    size: (f64, f64),
}

fn rgb(r: u8, g: u8, b: u8, a: f32) -> Color {
    AlphaColor::from_rgba8(r, g, b, 255).multiply_alpha(a)
}

impl Ui {
    pub fn new(font: Option<Font>) -> Self {
        Self {
            font,
            mouse: Point::new(-1.0, -1.0),
            pressed: false,
            time: 0.0,
            hover: Spring::new(0.0, 260.0, 22.0),
            press: Spring::new(0.0, 520.0, 28.0),
            ripple: None,
            scale: 1.0,
            size: (1.0, 1.0),
        }
    }

    pub fn set_viewport(&mut self, w: u32, h: u32, scale: f64) {
        self.size = (w as f64, h as f64);
        self.scale = scale;
    }

    fn button_rect(&self) -> Rect {
        let (w, h) = self.size;
        let (bw, bh) = (BTN_W * self.scale, BTN_H * self.scale);
        let y = h * 0.5 + 40.0 * self.scale;
        Rect::new((w - bw) / 2.0, y, (w + bw) / 2.0, y + bh)
    }

    pub fn hovered(&self) -> bool {
        self.button_rect().contains(self.mouse)
    }

    pub fn click(&mut self) {
        self.ripple = Some((self.mouse, 0.0));
    }

    /// Advances animations; returns true while another frame is needed.
    pub fn update(&mut self, dt: f32) -> bool {
        self.time += dt;
        self.hover.target = if self.hovered() { 1.0 } else { 0.0 };
        self.press.target = if self.pressed && self.hovered() { 1.0 } else { 0.0 };
        self.hover.step(dt);
        self.press.step(dt);
        if let Some((_, t)) = &mut self.ripple {
            *t += dt;
            if *t > RIPPLE_SECS {
                self.ripple = None;
            }
        }
        self.time < 1.6 || !self.hover.settled() || !self.press.settled() || self.ripple.is_some()
    }

    pub fn draw(&self, scene: &mut Scene) {
        scene.reset();
        let (w, h) = self.size;
        let s = self.scale;
        let full = Rect::new(0.0, 0.0, w, h);
        let bg = Gradient::new_linear((0.0, 0.0), (w, h)).with_stops([
            rgb(10, 12, 20, 1.0),
            rgb(22, 18, 40, 1.0),
        ]);
        scene.fill(Fill::NonZero, Affine::IDENTITY, &bg, None, &full);

        let cx = w / 2.0;
        let cy = h / 2.0;
        self.draw_text(scene, "DesignVolt", 56.0, cx, cy - 70.0 * s, 0.10, rgb(245, 246, 255, 1.0));
        self.draw_text(
            scene,
            "Design at the speed of light",
            20.0,
            cx,
            cy - 20.0 * s,
            0.25,
            rgb(150, 156, 190, 1.0),
        );
        self.draw_button(scene);
    }

    /// Intro animation: fade in while sliding up, per element delay.
    fn intro(&self, delay: f32) -> (f32, f64) {
        let p = ease_out_cubic(progress(self.time, delay, 0.7));
        (p, (1.0 - p as f64) * 28.0 * self.scale)
    }

    fn draw_text(&self, scene: &mut Scene, text: &str, size: f32, cx: f64, y: f64, delay: f32, color: Color) {
        let Some(font) = &self.font else { return };
        let (alpha, dy) = self.intro(delay);
        let px = size * self.scale as f32;
        let (glyphs, width) = font.layout(text, px);
        let t = Affine::translate((cx - width as f64 / 2.0, y + dy));
        scene
            .draw_glyphs(&font.data)
            .font_size(px)
            .transform(t)
            .brush(color)
            .brush_alpha(alpha)
            .draw(Fill::NonZero, glyphs.into_iter());
    }

    fn draw_button(&self, scene: &mut Scene) {
        let (alpha, dy) = self.intro(0.45);
        if alpha <= 0.0 {
            return;
        }
        let hov = self.hover.pos as f64;
        let prs = self.press.pos as f64;
        let rect = self.button_rect();
        let scale = (1.0 + 0.05 * hov - 0.06 * prs) * (0.92 + 0.08 * alpha as f64);
        let c = rect.center();
        let xf = Affine::translate(Vec2::new(c.x, c.y + dy))
            * Affine::scale(scale)
            * Affine::translate(Vec2::new(-c.x, -c.y));
        let r = self.scale * 16.0;
        let shape = RoundedRect::from_rect(rect, r);

        // Glow grows on hover.
        let glow = rgb(124, 92, 255, (0.35 + 0.35 * hov as f32) * alpha);
        scene.draw_blurred_rounded_rect(
            xf * Affine::translate((0.0, 10.0 * self.scale)),
            rect,
            glow,
            r,
            (18.0 + 14.0 * hov) * self.scale,
        );

        let shift = 0.15 * hov;
        let grad = Gradient::new_linear(
            (rect.x0, rect.y0),
            (rect.x1 + rect.width() * shift, rect.y1),
        )
        .with_stops([rgb(99, 102, 241, alpha), rgb(168, 85, 247, alpha)]);
        scene.fill(Fill::NonZero, xf, &grad, None, &shape);

        // Top highlight.
        let hi = Gradient::new_linear((0.0, rect.y0), (0.0, rect.y1)).with_stops([
            rgb(255, 255, 255, 0.22 * alpha),
            rgb(255, 255, 255, 0.0),
        ]);
        scene.fill(Fill::NonZero, xf, &hi, None, &shape);

        scene.push_clip_layer(Fill::NonZero, xf, &shape);
        if let Some((p, t)) = self.ripple {
            let k = ease_out_cubic(t / RIPPLE_SECS) as f64;
            let max = rect.width();
            let ring = Circle::new(p, max * k);
            let a = (1.0 - t / RIPPLE_SECS) * 0.35 * alpha;
            scene.fill(Fill::NonZero, xf, rgb(255, 255, 255, a), None, &ring);
        }
        scene.pop_layer();

        if let Some(font) = &self.font {
            let px = (20.0 * self.scale) as f32;
            let (glyphs, width) = font.layout("Get started", px);
            let tx = c.x - width as f64 / 2.0;
            let ty = c.y + px as f64 * 0.35;
            let t = xf * Affine::translate((tx, ty));
            scene
                .draw_glyphs(&font.data)
                .font_size(px)
                .transform(t)
                .brush(rgb(255, 255, 255, 1.0))
                .brush_alpha(alpha)
                .draw(Fill::NonZero, glyphs.into_iter());
        }
    }
}
