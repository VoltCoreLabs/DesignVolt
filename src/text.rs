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
    /// Loads the first available font file from `paths`.
    pub fn load(paths: &[&str]) -> Option<Self> {
        let bytes = paths.iter().find_map(|p| std::fs::read(p).ok())?;
        FontRef::from_index(&bytes, 0).ok()?;
        let blob = Blob::new(Arc::new(bytes));
        Some(Self { data: FontData::new(blob, 0) })
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
            glyphs.push(Glyph { id: gid.to_u32(), x, y: 0.0 });
            x += metrics.advance_width(gid).unwrap_or(0.0) * scale;
        }
        (glyphs, x)
    }
}
