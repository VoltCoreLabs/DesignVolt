//! Color conversions: hex text and HSV (for the color picker).

use crate::document::Color;

impl Color {
    /// `RRGGBB` or `RGB` (with or without `#`); alpha is opaque.
    pub fn from_hex(s: &str) -> Option<Color> {
        let s = s.trim().trim_start_matches('#');
        if !s.is_ascii() {
            return None;
        }
        let digit = |i: usize, n: usize| u8::from_str_radix(&s[i..i + n], 16).ok();
        match s.len() {
            6 => Some(Color::rgb(digit(0, 2)?, digit(2, 2)?, digit(4, 2)?)),
            3 => Some(Color::rgb(
                digit(0, 1)? * 17,
                digit(1, 1)? * 17,
                digit(2, 1)? * 17,
            )),
            _ => None,
        }
    }

    /// `RRGGBB`, uppercase, without alpha.
    pub fn to_hex(self) -> String {
        format!("{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }

    /// Hue in degrees (0..360), saturation and value (0..1).
    pub fn to_hsv(self) -> (f64, f64, f64) {
        let (r, g, b) = (
            f64::from(self.r) / 255.0,
            f64::from(self.g) / 255.0,
            f64::from(self.b) / 255.0,
        );
        let max = r.max(g).max(b);
        let d = max - r.min(g).min(b);
        let h = if d == 0.0 {
            0.0
        } else if max == r {
            60.0 * (((g - b) / d).rem_euclid(6.0))
        } else if max == g {
            60.0 * ((b - r) / d + 2.0)
        } else {
            60.0 * ((r - g) / d + 4.0)
        };
        (h, if max == 0.0 { 0.0 } else { d / max }, max)
    }

    /// Hue in degrees, saturation and lightness (0..1).
    pub fn to_hsl(self) -> (f64, f64, f64) {
        let (h, sv, v) = self.to_hsv();
        let l = v * (1.0 - sv / 2.0);
        let s = if l == 0.0 || l == 1.0 {
            0.0
        } else {
            (v - l) / l.min(1.0 - l)
        };
        (h, s, l)
    }

    pub fn from_hsl(h: f64, s: f64, l: f64, a: u8) -> Color {
        let (s, l) = (s.clamp(0.0, 1.0), l.clamp(0.0, 1.0));
        let v = l + s * l.min(1.0 - l);
        let sv = if v == 0.0 { 0.0 } else { 2.0 * (1.0 - l / v) };
        Color::from_hsv(h, sv, v, a)
    }

    pub fn from_hsv(h: f64, s: f64, v: f64, a: u8) -> Color {
        let (s, v) = (s.clamp(0.0, 1.0), v.clamp(0.0, 1.0));
        let h = h.rem_euclid(360.0) / 60.0;
        let c = v * s;
        let x = c * (1.0 - (h % 2.0 - 1.0).abs());
        let (r, g, b) = match h as u32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        let m = v - c;
        let byte = |f: f64| ((f + m) * 255.0).round() as u8;
        Color {
            r: byte(r),
            g: byte(g),
            b: byte(b),
            a,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_parses_short_and_long_forms_and_rejects_junk() {
        assert_eq!(Color::from_hex("#ff8000"), Some(Color::rgb(255, 128, 0)));
        assert_eq!(Color::from_hex("F80"), Some(Color::rgb(255, 136, 0)));
        assert_eq!(Color::from_hex("12345"), None);
        assert_eq!(Color::from_hex("gg0000"), None);
        assert_eq!(Color::from_hex("ёёё"), None);
        assert_eq!(Color::rgb(1, 171, 255).to_hex(), "01ABFF");
    }

    #[test]
    fn hsl_matches_known_colors_and_roundtrips() {
        let (h, s, l) = Color::rgb(255, 0, 0).to_hsl();
        assert_eq!((h, s, l), (0.0, 1.0, 0.5));
        let (_, s, l) = Color::rgb(255, 255, 255).to_hsl();
        assert_eq!((s, l), (0.0, 1.0));
        for c in [
            Color::rgb(12, 200, 99),
            Color::rgb(90, 30, 160),
            Color::rgb(7, 7, 7),
        ] {
            let (h, s, l) = c.to_hsl();
            assert_eq!(Color::from_hsl(h, s, l, 255), c);
        }
    }

    #[test]
    fn hsv_roundtrips_for_every_primary_and_gray() {
        for c in [
            Color::rgb(255, 0, 0),
            Color::rgb(0, 255, 0),
            Color::rgb(0, 0, 255),
            Color::rgb(12, 200, 99),
            Color::rgb(128, 128, 128),
            Color::rgb(0, 0, 0),
            Color::rgb(255, 255, 255),
        ] {
            let (h, s, v) = c.to_hsv();
            assert_eq!(Color::from_hsv(h, s, v, 255), c);
        }
        assert_eq!(Color::rgb(0, 255, 0).to_hsv().0, 120.0);
    }
}
