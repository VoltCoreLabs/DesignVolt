//! Dark theme color tokens (see docs/design-system.md).

use vello::peniko::Color;
use vello::peniko::color::AlphaColor;

pub const BG_APP: u32 = 0x0A0C14;
pub const BG_CANVAS: u32 = 0x0E1019;
pub const BG_PANEL: u32 = 0x141725;
pub const BG_RAISED: u32 = 0x1B1F31;
pub const BG_HOVER: u32 = 0x232842;
pub const TEXT_PRIMARY: u32 = 0xF5F6FF;
pub const TEXT_SECONDARY: u32 = 0x969CBE;
pub const TEXT_DISABLED: u32 = 0x5A6080;
pub const ACCENT_FROM: u32 = 0x6366F1;
pub const ACCENT_TO: u32 = 0xA855F7;
pub const ACCENT: u32 = 0x7C5CFF;
pub const WHITE: u32 = 0xFFFFFF;

/// Opaque color from `0xRRGGBB`.
pub fn hex(rgb: u32) -> Color {
    alpha(rgb, 1.0)
}

/// Color from `0xRRGGBB` with opacity `a` in 0..=1.
pub fn alpha(rgb: u32, a: f32) -> Color {
    let (r, g, b) = ((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8);
    AlphaColor::from_rgba8(r, g, b, 255).multiply_alpha(a)
}
