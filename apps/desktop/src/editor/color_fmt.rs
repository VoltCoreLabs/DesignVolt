//! Ways to show and type a color in the picker: HEX, RGB, HSL, HSB and CSS.

use dv_core::Color;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Hex,
    Rgb,
    Hsl,
    Hsb,
    Css,
}

impl Mode {
    pub const ALL: [Mode; 5] = [Mode::Hex, Mode::Rgb, Mode::Hsl, Mode::Hsb, Mode::Css];

    pub fn next(self) -> Mode {
        let i = Mode::ALL.iter().position(|m| *m == self).unwrap_or(0);
        Mode::ALL[(i + 1) % Mode::ALL.len()]
    }

    pub fn label(self) -> &'static str {
        match self {
            Mode::Hex => "Hex",
            Mode::Rgb => "RGB",
            Mode::Hsl => "HSL",
            Mode::Hsb => "HSB",
            Mode::Css => "CSS",
        }
    }

    /// How many text fields show the color in this mode (opacity is a separate field,
    /// except in CSS where it is part of the text).
    pub fn fields(self) -> usize {
        match self {
            Mode::Hex | Mode::Css => 1,
            _ => 3,
        }
    }

    pub fn has_alpha_field(self) -> bool {
        self != Mode::Css
    }
}

fn pct(x: f64) -> String {
    format!("{:.0}", x * 100.0)
}

/// The texts of the fields for `c`.
pub fn components(mode: Mode, c: Color) -> Vec<String> {
    match mode {
        Mode::Hex => vec![c.to_hex()],
        Mode::Rgb => vec![c.r.to_string(), c.g.to_string(), c.b.to_string()],
        Mode::Hsl => {
            let (h, s, l) = c.to_hsl();
            vec![format!("{h:.0}"), pct(s), pct(l)]
        }
        Mode::Hsb => {
            let (h, s, v) = c.to_hsv();
            vec![format!("{h:.0}"), pct(s), pct(v)]
        }
        Mode::Css => vec![css(c)],
    }
}

/// `rgb(r, g, b)`, or `rgba(r, g, b, a)` when not opaque.
pub fn css(c: Color) -> String {
    if c.a == 255 {
        format!("rgb({}, {}, {})", c.r, c.g, c.b)
    } else {
        let a = (f64::from(c.a) / 255.0 * 100.0).round() / 100.0;
        format!("rgba({}, {}, {}, {a})", c.r, c.g, c.b)
    }
}

fn num(s: &str) -> Option<f64> {
    let v: f64 = s
        .trim()
        .trim_end_matches('%')
        .replace(',', ".")
        .parse()
        .ok()?;
    v.is_finite().then_some(v)
}

/// The color after field `i` of `mode` was typed over; the opacity stays, except where the
/// typed text carries its own (CSS). `None` when the text is not valid.
pub fn with_component(mode: Mode, c: Color, i: usize, text: &str) -> Option<Color> {
    match mode {
        Mode::Hex => Color::from_hex(text).map(|n| Color { a: c.a, ..n }),
        Mode::Css => parse_css(text, c.a),
        Mode::Rgb => {
            let v = num(text)?.clamp(0.0, 255.0).round() as u8;
            let mut n = c;
            match i {
                0 => n.r = v,
                1 => n.g = v,
                _ => n.b = v,
            }
            Some(n)
        }
        Mode::Hsl | Mode::Hsb => {
            let v = num(text)?;
            let (mut h, mut s, mut x) = if mode == Mode::Hsl {
                c.to_hsl()
            } else {
                c.to_hsv()
            };
            match i {
                0 => h = v.rem_euclid(360.0),
                1 => s = (v / 100.0).clamp(0.0, 1.0),
                _ => x = (v / 100.0).clamp(0.0, 1.0),
            }
            Some(if mode == Mode::Hsl {
                Color::from_hsl(h, s, x, c.a)
            } else {
                Color::from_hsv(h, s, x, c.a)
            })
        }
    }
}

/// Reads `#hex`, `rgb(...)`, `rgba(...)`, `hsl(...)` and `hsla(...)`; `alpha` is used when the
/// text gives none.
pub fn parse_css(text: &str, alpha: u8) -> Option<Color> {
    let t = text.trim().to_lowercase();
    if let Some(c) = Color::from_hex(&t) {
        return Some(Color { a: alpha, ..c });
    }
    let (name, rest) = t.split_once('(')?;
    let args = rest.strip_suffix(')')?;
    let parts: Vec<&str> = args
        .split(|c: char| c == ',' || c == '/' || c.is_whitespace())
        .filter(|p| !p.is_empty())
        .collect();
    if !(3..=4).contains(&parts.len()) {
        return None;
    }
    let a = match parts.get(3) {
        Some(p) => {
            let v = num(p)?;
            let f = if p.ends_with('%') { v / 100.0 } else { v };
            (f.clamp(0.0, 1.0) * 255.0).round() as u8
        }
        None => alpha,
    };
    let (x, y, z) = (num(parts[0])?, num(parts[1])?, num(parts[2])?);
    match name.trim() {
        "rgb" | "rgba" => {
            let b = |v: f64| v.clamp(0.0, 255.0).round() as u8;
            Some(Color {
                r: b(x),
                g: b(y),
                b: b(z),
                a,
            })
        }
        "hsl" | "hsla" => Some(Color::from_hsl(
            x.rem_euclid(360.0),
            y / 100.0,
            z / 100.0,
            a,
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORANGE: Color = Color::rgb(255, 128, 0);

    #[test]
    fn components_show_each_model() {
        assert_eq!(components(Mode::Hex, ORANGE), ["FF8000"]);
        assert_eq!(components(Mode::Rgb, ORANGE), ["255", "128", "0"]);
        assert_eq!(components(Mode::Hsb, ORANGE), ["30", "100", "100"]);
        assert_eq!(components(Mode::Hsl, ORANGE), ["30", "100", "50"]);
        assert_eq!(components(Mode::Css, ORANGE), ["rgb(255, 128, 0)"]);
        let half = Color { a: 128, ..ORANGE };
        assert_eq!(components(Mode::Css, half), ["rgba(255, 128, 0, 0.5)"]);
    }

    #[test]
    fn typing_one_field_changes_only_that_part_and_keeps_opacity() {
        let c = Color { a: 100, ..ORANGE };
        let n = with_component(Mode::Rgb, c, 1, "300").unwrap();
        assert_eq!((n.r, n.g, n.b, n.a), (255, 255, 0, 100));
        let n = with_component(Mode::Hsb, c, 2, "50").unwrap();
        assert_eq!((n.r, n.g, n.a), (128, 64, 100));
        assert_eq!(with_component(Mode::Hex, c, 0, "#00f").unwrap().b, 255);
        assert!(with_component(Mode::Rgb, c, 0, "x").is_none());
        assert!(with_component(Mode::Hex, c, 0, "12").is_none());
    }

    #[test]
    fn css_text_is_understood() {
        assert_eq!(
            parse_css("rgb(10, 20, 30)", 255),
            Some(Color::rgb(10, 20, 30))
        );
        assert_eq!(parse_css("rgba(10 20 30 / 50%)", 255).unwrap().a, 128);
        assert_eq!(
            parse_css("hsl(0, 100%, 50%)", 255),
            Some(Color::rgb(255, 0, 0))
        );
        assert_eq!(
            parse_css("#0f0", 7),
            Some(Color {
                a: 7,
                ..Color::rgb(0, 255, 0)
            })
        );
        assert!(parse_css("rgb(1, 2)", 255).is_none());
        assert!(parse_css("blue", 255).is_none());
    }

    #[test]
    fn mode_cycles_through_all() {
        let mut m = Mode::Hex;
        for _ in 0..Mode::ALL.len() {
            m = m.next();
        }
        assert_eq!(m, Mode::Hex);
    }
}
