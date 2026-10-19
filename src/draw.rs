//! Drawing context: all coordinates are logical px; `xf` carries DPI scale and animation offsets.

use vello::Scene;
use vello::kurbo::{Affine, Circle, Rect, RoundedRect, Stroke};
use vello::peniko::{BlendMode, Color, Fill, Gradient};

use crate::icons::{Icon, Icons, VIEWBOX};
use crate::text::Font;
use crate::theme::colors::{BG_PANEL, WHITE, alpha, hex};

pub struct Cx<'a> {
    pub scene: &'a mut Scene,
    pub font: &'a Font,
    pub icons: &'a Icons,
    pub xf: Affine,
}

impl Cx<'_> {
    /// Runs `f` inside a group with opacity `a`, shifted by `offset`, clipped to `clip`.
    pub fn layer(&mut self, clip: Rect, a: f32, offset: (f64, f64), f: impl FnOnce(&mut Self)) {
        let old = self.xf;
        self.xf = old * Affine::translate(offset);
        self.scene
            .push_layer(Fill::NonZero, BlendMode::default(), a, self.xf, &clip);
        f(self);
        self.scene.pop_layer();
        self.xf = old;
    }

    pub fn clip(&mut self, clip: Rect, f: impl FnOnce(&mut Self)) {
        self.scene.push_clip_layer(Fill::NonZero, self.xf, &clip);
        f(self);
        self.scene.pop_layer();
    }

    pub fn fill(&mut self, r: Rect, radius: f64, c: Color) {
        let shape = RoundedRect::from_rect(r, radius);
        self.scene.fill(Fill::NonZero, self.xf, c, None, &shape);
    }

    pub fn fill_grad(&mut self, r: Rect, radius: f64, from: Color, to: Color) {
        let g = Gradient::new_linear((r.x0, r.y0), (r.x1, r.y1)).with_stops([from, to]);
        let shape = RoundedRect::from_rect(r, radius);
        self.scene.fill(Fill::NonZero, self.xf, &g, None, &shape);
    }

    pub fn stroke(&mut self, r: Rect, radius: f64, width: f64, c: Color) {
        let shape = RoundedRect::from_rect(r, radius);
        self.scene
            .stroke(&Stroke::new(width), self.xf, c, None, &shape);
    }

    pub fn circle(&mut self, cx: f64, cy: f64, radius: f64, c: Color) {
        let shape = Circle::new((cx, cy), radius);
        self.scene.fill(Fill::NonZero, self.xf, c, None, &shape);
    }

    pub fn shadow(&mut self, r: Rect, radius: f64, c: Color, blur: f64, dy: f64) {
        let t = self.xf * Affine::translate((0.0, dy));
        self.scene.draw_blurred_rounded_rect(t, r, c, radius, blur);
    }

    /// Draws text with its baseline at `y`; returns the advance width.
    pub fn text(&mut self, s: &str, size: f32, x: f64, y: f64, c: Color) -> f64 {
        let (glyphs, width) = self.font.layout(s, size);
        self.scene
            .draw_glyphs(&self.font.data)
            .font_size(size)
            .transform(self.xf * Affine::translate((x, y)))
            .brush(c)
            .draw(Fill::NonZero, glyphs.into_iter());
        width as f64
    }

    pub fn text_width(&self, s: &str, size: f32) -> f64 {
        self.font.layout(s, size).1 as f64
    }

    /// Text centered in `r` (single line).
    pub fn text_centered(&mut self, s: &str, size: f32, r: Rect, c: Color) {
        let w = self.text_width(s, size);
        let x = r.x0 + (r.width() - w) / 2.0;
        let y = r.center().y + size as f64 * 0.36;
        self.text(s, size, x, y, c);
    }

    pub fn icon(&mut self, icon: Icon, x: f64, y: f64, size: f64, c: Color) {
        let Some(path) = self.icons.get(icon) else { return };
        let t = self.xf * Affine::translate((x, y)) * Affine::scale(size / VIEWBOX);
        self.scene.fill(Fill::NonZero, t, c, None, path);
    }

    /// Glass surface (imitation until the backdrop blur pass exists, see D-006).
    pub fn glass(&mut self, r: Rect, radius: f64) {
        self.shadow(r, radius, alpha(0x000000, 0.5), 22.0, 10.0);
        self.fill(r, radius, alpha(BG_PANEL, 0.78));
        let shape = RoundedRect::from_rect(r, radius);
        let top = Gradient::new_linear((0.0, r.y0), (0.0, r.y1))
            .with_stops([alpha(WHITE, 0.10), alpha(WHITE, 0.0)]);
        self.scene.fill(Fill::NonZero, self.xf, &top, None, &shape);
        let rim = Gradient::new_linear((0.0, r.y0), (0.0, r.y1))
            .with_stops([alpha(WHITE, 0.28), alpha(WHITE, 0.06)]);
        self.scene
            .stroke(&Stroke::new(1.0), self.xf, &rim, None, &shape);
    }
}

/// Linear blend of two `0xRRGGBB` colors, `t` in 0..=1.
pub fn mix(a: u32, b: u32, t: f32) -> Color {
    let ch = |s: u32| ((a >> s) & 255) as f32 * (1.0 - t) + ((b >> s) & 255) as f32 * t;
    hex(((ch(16) as u32) << 16) | ((ch(8) as u32) << 8) | ch(0) as u32)
}
