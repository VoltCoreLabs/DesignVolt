//! Drawing context: all coordinates are logical px; `xf` carries DPI scale and animation offsets.

use vello::Scene;
use vello::kurbo::{Affine, Circle, Rect, RoundedRect, Stroke};
use vello::peniko::{BlendMode, Color, Fill};

use crate::icons::{Icon, Icons, VIEWBOX};
use crate::text::Font;
use crate::theme::colors::{alpha, bg_panel, hex, overlay, palette};

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

    pub fn stroke(&mut self, r: Rect, radius: f64, width: f64, c: Color) {
        let shape = RoundedRect::from_rect(r, radius);
        self.scene
            .stroke(&Stroke::new(width), self.xf, c, None, &shape);
    }

    pub fn stroke_path(&mut self, path: &vello::kurbo::BezPath, width: f64, c: Color) {
        self.scene
            .stroke(&Stroke::new(width), self.xf, c, None, path);
    }

    pub fn fill_path(&mut self, path: &vello::kurbo::BezPath, c: Color) {
        self.scene.fill(Fill::NonZero, self.xf, c, None, path);
    }

    pub fn circle(&mut self, cx: f64, cy: f64, radius: f64, c: Color) {
        let shape = Circle::new((cx, cy), radius);
        self.scene.fill(Fill::NonZero, self.xf, c, None, &shape);
    }

    pub fn shadow(&mut self, r: Rect, radius: f64, c: Color, blur: f64, dy: f64) {
        let t = self.xf * Affine::translate((0.0, dy));
        let c = c.multiply_alpha(palette().shadow_k);
        self.scene.draw_blurred_rounded_rect(t, r, c, radius, blur);
    }

    /// Draws text with its baseline at `y`; returns the advance width.
    pub fn text(&mut self, s: &str, size: f32, x: f64, y: f64, c: Color) -> f64 {
        let (glyphs, width) = self.font.layout(s, size);
        self.scene
            .draw_glyphs(&self.font.data)
            .font_size(size)
            .normalized_coords(self.font.coords(size))
            .transform(self.xf * Affine::translate((x, y)))
            .brush(c)
            .draw(Fill::NonZero, glyphs.into_iter());
        width as f64
    }

    pub fn text_width(&self, s: &str, size: f32) -> f64 {
        self.font.layout(s, size).1 as f64
    }

    /// Cuts `s` with an ellipsis so that it fits in `max_w` logical px.
    pub fn fit(&self, s: &str, size: f32, max_w: f64) -> String {
        if self.text_width(s, size) <= max_w {
            return s.to_string();
        }
        let mut out = s.to_string();
        while out.pop().is_some() {
            let candidate = format!("{}…", out.trim_end());
            if self.text_width(&candidate, size) <= max_w {
                return candidate;
            }
        }
        String::new()
    }

    pub fn icon(&mut self, icon: Icon, x: f64, y: f64, size: f64, c: Color) {
        let Some(path) = self.icons.get(icon) else {
            return;
        };
        let t = self.xf * Affine::translate((x, y)) * Affine::scale(size / VIEWBOX);
        self.scene.fill(Fill::NonZero, t, c, None, path);
    }

    /// Glass surface (imitation until the backdrop blur pass exists, see D-006): translucent
    /// panel color, soft shadow and a thin border.
    pub fn glass(&mut self, r: Rect, radius: f64) {
        self.shadow(r, radius, alpha(0x000000, 0.35), 22.0, 10.0);
        self.fill(r, radius, alpha(bg_panel(), 0.82));
        self.stroke(r, radius, 1.0, alpha(overlay(), 0.14));
    }
}

/// Linear blend of two `0xRRGGBB` colors, `t` in 0..=1.
pub fn mix(a: u32, b: u32, t: f32) -> Color {
    let ch = |s: u32| ((a >> s) & 255) as f32 * (1.0 - t) + ((b >> s) & 255) as f32 * t;
    hex(((ch(16) as u32) << 16) | ((ch(8) as u32) << 8) | ch(0) as u32)
}
