//! Single-line text shaping on top of skrifa (no kerning/ligatures yet).
//!
//! The font is a variable font with a `wght` axis. Text of at least `STRONG_FROM` px is drawn
//! semibold (headings), smaller text regular; `layout` and `coords` apply the same rule, so the
//! measured widths always match what is drawn.

use std::sync::Arc;

use skrifa::instance::{Location, Size};
use skrifa::{FontRef, MetadataProvider};
use vello::peniko::{Blob, FontData};
use vello::{Glyph, NormalizedCoord};

pub const REGULAR: f32 = 400.0;
pub const STRONG: f32 = 600.0;
/// Text at least this big (px) is drawn in the strong weight.
pub const STRONG_FROM: f32 = 17.0;

pub struct Font {
    pub data: FontData,
    regular: Location,
    strong: Location,
    regular_coords: Vec<NormalizedCoord>,
    strong_coords: Vec<NormalizedCoord>,
}

fn coords_of(location: &Location) -> Vec<NormalizedCoord> {
    location.coords().iter().map(|c| c.to_bits()).collect()
}

impl Font {
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let font = FontRef::from_index(bytes, 0).ok()?;
        let at = |w: f32| font.axes().location([("wght", w)]);
        let (regular, strong) = (at(REGULAR), at(STRONG));
        let (regular_coords, strong_coords) = (coords_of(&regular), coords_of(&strong));
        let blob = Blob::new(Arc::new(bytes.to_vec()));
        Some(Self {
            data: FontData::new(blob, 0),
            regular,
            strong,
            regular_coords,
            strong_coords,
        })
    }

    fn location(&self, size: f32) -> &Location {
        if size >= STRONG_FROM {
            &self.strong
        } else {
            &self.regular
        }
    }

    /// Variable-font coordinates for drawing text of `size` px.
    pub fn coords(&self, size: f32) -> &[NormalizedCoord] {
        if size >= STRONG_FROM {
            &self.strong_coords
        } else {
            &self.regular_coords
        }
    }

    /// Returns positioned glyphs (baseline at y = 0) and the total advance width.
    pub fn layout(&self, text: &str, size: f32) -> (Vec<Glyph>, f32) {
        let Ok(font) = FontRef::from_index(self.data.data.data(), 0) else {
            return (Vec::new(), 0.0);
        };
        let location = self.location(size);
        let charmap = font.charmap();
        let metrics = font.glyph_metrics(Size::unscaled(), location);
        let upem = font.metrics(Size::unscaled(), location).units_per_em as f32;
        let scale = size / upem;
        let mut x = 0.0;
        let mut glyphs = Vec::new();
        for ch in text.chars() {
            let gid = charmap.map(ch).unwrap_or_default();
            glyphs.push(Glyph {
                id: gid.to_u32(),
                x,
                y: 0.0,
            });
            x += metrics.advance_width(gid).unwrap_or(0.0) * scale;
        }
        (glyphs, x)
    }

    /// Greedy word wrap into lines no wider than `max_w` (a single long word stays whole).
    pub fn wrap(&self, text: &str, size: f32, max_w: f64) -> Vec<String> {
        let mut lines: Vec<String> = Vec::new();
        let mut line = String::new();
        for word in text.split_whitespace() {
            let candidate = if line.is_empty() {
                word.to_string()
            } else {
                format!("{line} {word}")
            };
            if line.is_empty() || self.layout(&candidate, size).1 as f64 <= max_w {
                line = candidate;
            } else {
                lines.push(std::mem::take(&mut line));
                line = word.to_string();
            }
        }
        if !line.is_empty() {
            lines.push(line);
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn font() -> Font {
        Font::from_bytes(include_bytes!("../../../assets/fonts/GolosText.ttf")).expect("font")
    }

    #[test]
    fn wrap_keeps_lines_within_the_width() {
        let f = font();
        let text =
            "Все проекты и папки в корзине будут удалены навсегда. Это действие нельзя отменить.";
        let lines = f.wrap(text, 13.0, 200.0);
        assert!(lines.len() > 1);
        for l in &lines {
            assert!(f.layout(l, 13.0).1 as f64 <= 200.0, "too wide: {l}");
        }
        assert_eq!(lines.join(" "), text);
    }

    #[test]
    fn golos_covers_cyrillic_and_the_strong_weight_is_wider() {
        let f = font();
        let (glyphs, _) = f.layout("Привет, мир! ёЁ", 13.0);
        assert!(glyphs.iter().all(|g| g.id != 0), "missing Cyrillic glyph");
        let per_px = |size: f32| f.layout("Все проекты", size).1 / size;
        assert!(
            per_px(20.0) > per_px(16.0),
            "semibold text should be wider per px"
        );
        assert_ne!(f.coords(20.0), f.coords(13.0));
    }
    #[test]
    fn wrap_handles_empty_and_long_words() {
        let f = font();
        assert!(f.wrap("   ", 13.0, 100.0).is_empty());
        let long = "Supercalifragilisticexpialidocious";
        assert_eq!(f.wrap(long, 13.0, 20.0), vec![long.to_string()]);
    }
}
