//! Color tokens. Two palettes (light, dark) selected at runtime; widgets only call the token
//! functions below (`bg_panel()`, `accent()`, ...), never raw hex values.

use std::cell::Cell;

use vello::peniko::Color;
use vello::peniko::color::AlphaColor;

pub struct Palette {
    pub bg_app: u32,
    pub bg_canvas: u32,
    pub bg_panel: u32,
    pub bg_raised: u32,
    pub bg_hover: u32,
    pub text_primary: u32,
    pub text_secondary: u32,
    pub text_disabled: u32,
    pub accent: u32,
    pub danger: u32,
    /// Base color for borders, dividers and subtle overlays (white on dark, navy on light).
    pub overlay: u32,
    /// Scales the opacity of every shadow and glow.
    pub shadow_k: f32,
}

pub static DARK: Palette = Palette {
    bg_app: 0x0A0C14,
    bg_canvas: 0x0E1019,
    bg_panel: 0x141725,
    bg_raised: 0x1B1F31,
    bg_hover: 0x232842,
    text_primary: 0xF5F6FF,
    text_secondary: 0x969CBE,
    text_disabled: 0x5A6080,
    accent: 0x7C5CFF,
    danger: 0xF87171,
    overlay: 0xFFFFFF,
    shadow_k: 1.0,
};

pub static LIGHT: Palette = Palette {
    bg_app: 0xF3F6FC,
    bg_canvas: 0xE9EEF7,
    bg_panel: 0xFFFFFF,
    bg_raised: 0xF1F4FA,
    bg_hover: 0xE4EAF6,
    text_primary: 0x0F172A,
    text_secondary: 0x5B6478,
    text_disabled: 0x9AA3B8,
    accent: 0x2563EB,
    danger: 0xEF4444,
    overlay: 0x0F172A,
    shadow_k: 0.55,
};

thread_local! {
    static CURRENT: Cell<&'static Palette> = const { Cell::new(&LIGHT) };
}

pub fn palette() -> &'static Palette {
    CURRENT.with(Cell::get)
}

pub fn set_dark(dark: bool) {
    CURRENT.with(|c| c.set(if dark { &DARK } else { &LIGHT }));
}

pub const WHITE: u32 = 0xFFFFFF;

macro_rules! tokens {
    ($($name:ident),* $(,)?) => {
        $(pub fn $name() -> u32 { palette().$name })*
    };
}

tokens!(
    bg_app,
    bg_canvas,
    bg_panel,
    bg_raised,
    bg_hover,
    text_primary,
    text_secondary,
    text_disabled,
    accent,
    danger,
    overlay,
);

/// Opaque color from `0xRRGGBB`.
pub fn hex(rgb: u32) -> Color {
    alpha(rgb, 1.0)
}

/// Color from `0xRRGGBB` with opacity `a` in 0..=1.
pub fn alpha(rgb: u32, a: f32) -> Color {
    let (r, g, b) = ((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8);
    AlphaColor::from_rgba8(r, g, b, 255).multiply_alpha(a)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lum(rgb: u32) -> f64 {
        let ch = |s: u32| {
            let c = ((rgb >> s) & 255) as f64 / 255.0;
            if c <= 0.03928 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * ch(16) + 0.7152 * ch(8) + 0.0722 * ch(0)
    }

    fn contrast(a: u32, b: u32) -> f64 {
        let (x, y) = (lum(a), lum(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    #[test]
    fn text_meets_wcag_aa_on_panels_in_both_themes() {
        for p in [&DARK, &LIGHT] {
            assert!(
                contrast(p.text_primary, p.bg_panel) >= 7.0,
                "primary {:06X}",
                p.bg_panel
            );
            assert!(
                contrast(p.text_secondary, p.bg_panel) >= 4.5,
                "secondary {:06X}",
                p.bg_panel
            );
            assert!(
                contrast(p.text_primary, p.bg_raised) >= 4.5,
                "raised {:06X}",
                p.bg_panel
            );
        }
    }

    #[test]
    fn white_on_accent_is_readable() {
        assert!(contrast(WHITE, LIGHT.accent) >= 4.5);
        assert!(contrast(WHITE, DARK.accent) >= 4.0);
    }

    #[test]
    fn toggling_switches_the_palette() {
        set_dark(true);
        assert_eq!(bg_panel(), DARK.bg_panel);
        set_dark(false);
        assert_eq!(bg_panel(), LIGHT.bg_panel);
    }
}
