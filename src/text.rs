//! Single-line text shaping on top of skrifa (no kerning/ligatures yet).

use std::sync::Arc;

use skrifa::instance::{LocationRef, Size};
use skrifa::{FontRef, MetadataProvider};
use vello::Glyph;
use vello::peniko::{Blob, FontData};

pub struct Font {
    pub data: FontData,
}

impl Font {
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        FontRef::from_index(bytes, 0).ok()?;
        let blob = Blob::new(Arc::new(bytes.to_vec()));
        Some(Self {
            data: FontData::new(blob, 0),
        })
    }

    /// Returns positioned glyphs (baseline at y = 0) and the total advance width.
    pub fn layout(&self, text: &str, size: f32) -> (Vec<Glyph>, f32) {
        let Ok(font) = FontRef::from_index(self.data.data.data(), 0) else {
            return (Vec::new(), 0.0);
        };
        let charmap = font.charmap();
        let metrics = font.glyph_metrics(Size::unscaled(), LocationRef::default());
        let upem = font
            .metrics(Size::unscaled(), LocationRef::default())
            .units_per_em as f32;
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
        Font::from_bytes(include_bytes!("../assets/fonts/Inter.ttf")).expect("font")
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
    fn wrap_handles_empty_and_long_words() {
        let f = font();
        assert!(f.wrap("   ", 13.0, 100.0).is_empty());
        let long = "Supercalifragilisticexpialidocious";
        assert_eq!(f.wrap(long, 13.0, 20.0), vec![long.to_string()]);
    }
}
