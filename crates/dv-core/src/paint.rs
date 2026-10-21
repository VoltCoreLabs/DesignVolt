//! What a shape is filled with: a color or a gradient.

use crate::document::{Color, NodeId};
use crate::image::ImageFill;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GradientKind {
    Linear,
    Radial,
    Angular,
    Diamond,
}

impl GradientKind {
    pub const ALL: [GradientKind; 4] = [
        GradientKind::Linear,
        GradientKind::Radial,
        GradientKind::Angular,
        GradientKind::Diamond,
    ];

    pub fn code(self) -> &'static str {
        match self {
            GradientKind::Linear => "linear",
            GradientKind::Radial => "radial",
            GradientKind::Angular => "angular",
            GradientKind::Diamond => "diamond",
        }
    }

    pub fn from_code(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.code() == s)
    }
}

/// A color at a position (0..1) along the gradient.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Stop {
    pub pos: f64,
    pub color: Color,
}

/// A gradient over the shape's box. `angle` turns linear gradients (0 = left to right) and
/// the start of angular ones; `cx`/`cy` is the center as a fraction of the box; `scale`
/// stretches the gradient (1 = it just covers the box).
#[derive(Clone, PartialEq, Debug)]
pub struct Gradient {
    pub kind: GradientKind,
    pub stops: Vec<Stop>,
    pub angle: f64,
    pub cx: f64,
    pub cy: f64,
    pub scale: f64,
}

/// Fewest stops a gradient keeps.
pub const MIN_STOPS: usize = 2;

impl Gradient {
    /// From `color` to the same color fully transparent (what Figma starts with).
    pub fn from_color(kind: GradientKind, color: Color) -> Self {
        Self {
            kind,
            stops: vec![
                Stop { pos: 0.0, color },
                Stop {
                    pos: 1.0,
                    color: Color { a: 0, ..color },
                },
            ],
            angle: 0.0,
            cx: 0.5,
            cy: 0.5,
            scale: 1.0,
        }
    }

    /// The color at `t` (0..1), blending the two neighbouring stops.
    pub fn color_at(&self, t: f64) -> Color {
        let t = t.clamp(0.0, 1.0);
        let mut prev = self.stops[0];
        for s in &self.stops {
            if t <= s.pos {
                let span = s.pos - prev.pos;
                if span <= f64::EPSILON {
                    return s.color;
                }
                return mix(prev.color, s.color, (t - prev.pos) / span);
            }
            prev = *s;
        }
        prev.color
    }

    /// Puts the stops in position order and keeps positions inside 0..1.
    pub fn normalize(&mut self) {
        for s in &mut self.stops {
            s.pos = s.pos.clamp(0.0, 1.0);
        }
        self.stops.sort_by(|a, b| a.pos.total_cmp(&b.pos));
    }

    /// Adds a stop at `pos` with the color the gradient has there; returns its index.
    pub fn add_stop(&mut self, pos: f64) -> usize {
        let color = self.color_at(pos);
        self.stops.push(Stop {
            pos: pos.clamp(0.0, 1.0),
            color,
        });
        self.normalize();
        self.stops
            .iter()
            .position(|s| s.pos == pos.clamp(0.0, 1.0) && s.color == color)
            .unwrap_or(0)
    }

    /// Removes a stop unless only the minimum is left.
    pub fn remove_stop(&mut self, i: usize) -> bool {
        if self.stops.len() <= MIN_STOPS || i >= self.stops.len() {
            return false;
        }
        self.stops.remove(i);
        true
    }
}

fn mix(a: Color, b: Color, t: f64) -> Color {
    let m = |x: u8, y: u8| (f64::from(x) + (f64::from(y) - f64::from(x)) * t).round() as u8;
    Color {
        r: m(a.r, b.r),
        g: m(a.g, b.g),
        b: m(a.b, b.b),
        a: m(a.a, b.a),
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tiling {
    Grid,
    /// Every other row is shifted by half a tile.
    BrickRows,
    /// Every other column is shifted by half a tile.
    BrickColumns,
}

impl Tiling {
    pub const ALL: [Tiling; 3] = [Tiling::Grid, Tiling::BrickRows, Tiling::BrickColumns];

    pub fn code(self) -> &'static str {
        match self {
            Tiling::Grid => "grid",
            Tiling::BrickRows => "rows",
            Tiling::BrickColumns => "cols",
        }
    }

    pub fn from_code(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.code() == s)
    }

    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|t| *t == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }
}

/// A shape of the document repeated over the fill area, like a pattern fill in Figma.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Pattern {
    /// The node (with everything inside it) that is the tile.
    pub source: NodeId,
    pub tiling: Tiling,
    /// Tile size relative to the source (1 = as it is).
    pub scale: f64,
    /// Space between tiles.
    pub gap_x: f64,
    pub gap_y: f64,
    /// Turns the whole grid, in degrees.
    pub angle: f64,
}

impl Pattern {
    pub fn new(source: NodeId) -> Self {
        Self {
            source,
            tiling: Tiling::Grid,
            scale: 1.0,
            gap_x: 0.0,
            gap_y: 0.0,
            angle: 0.0,
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum Paint {
    Solid(Color),
    Gradient(Gradient),
    Pattern(Pattern),
    Image(ImageFill),
}

impl From<Color> for Paint {
    fn from(c: Color) -> Self {
        Paint::Solid(c)
    }
}

impl Paint {
    /// The color that stands for the paint in swatches and for the eyedropper.
    pub fn main_color(&self) -> Color {
        match self {
            Paint::Solid(c) => *c,
            Paint::Gradient(g) => g.stops[0].color,
            Paint::Pattern(_) | Paint::Image(_) => Color::rgb(160, 160, 168),
        }
    }

    pub fn solid(&self) -> Option<Color> {
        match self {
            Paint::Solid(c) => Some(*c),
            Paint::Gradient(_) | Paint::Pattern(_) | Paint::Image(_) => None,
        }
    }

    /// One line of text without tabs, for the document file.
    pub fn to_code(&self) -> String {
        match self {
            Paint::Solid(c) => color_code(*c),
            Paint::Image(i) => format!("i;{}", i.to_code()),
            Paint::Pattern(p) => format!(
                "p;{};{};{};{};{};{}",
                p.source.0,
                p.tiling.code(),
                p.scale,
                p.gap_x,
                p.gap_y,
                p.angle
            ),
            Paint::Gradient(g) => {
                let stops: Vec<String> = g
                    .stops
                    .iter()
                    .map(|s| format!("{}={}", s.pos, color_code(s.color)))
                    .collect();
                format!(
                    "g;{};{};{};{};{};{}",
                    g.kind.code(),
                    g.angle,
                    g.cx,
                    g.cy,
                    g.scale,
                    stops.join("|")
                )
            }
        }
    }

    pub fn from_code(s: &str) -> Option<Paint> {
        if let Some(rest) = s.strip_prefix("i;") {
            return ImageFill::from_code(rest).map(Paint::Image);
        }
        if let Some(rest) = s.strip_prefix("p;") {
            return parse_pattern(rest).map(Paint::Pattern);
        }
        let Some(rest) = s.strip_prefix("g;") else {
            return color_from(s).map(Paint::Solid);
        };
        let mut f = rest.split(';');
        let kind = GradientKind::from_code(f.next()?)?;
        let mut num = || f.next()?.parse::<f64>().ok().filter(|v| v.is_finite());
        let (angle, cx, cy, scale) = (num()?, num()?, num()?, num()?);
        let stops: Option<Vec<Stop>> = f
            .next()?
            .split('|')
            .map(|p| {
                let (pos, c) = p.split_once('=')?;
                Some(Stop {
                    pos: pos.parse().ok().filter(|v: &f64| v.is_finite())?,
                    color: color_from(c)?,
                })
            })
            .collect();
        let mut g = Gradient {
            kind,
            stops: stops?,
            angle,
            cx,
            cy,
            scale,
        };
        if g.stops.len() < MIN_STOPS {
            return None;
        }
        g.normalize();
        Some(Paint::Gradient(g))
    }
}

fn parse_pattern(s: &str) -> Option<Pattern> {
    let mut f = s.split(';');
    let source = NodeId(f.next()?.parse().ok()?);
    let tiling = Tiling::from_code(f.next()?)?;
    let mut num = || f.next()?.parse::<f64>().ok().filter(|v| v.is_finite());
    let (scale, gap_x, gap_y, angle) = (num()?, num()?, num()?, num()?);
    Some(Pattern {
        source,
        tiling,
        scale: scale.max(0.01),
        gap_x,
        gap_y,
        angle,
    })
}

pub fn color_code(c: Color) -> String {
    format!("{:02x}{:02x}{:02x}{:02x}", c.r, c.g, c.b, c.a)
}

pub fn color_from(s: &str) -> Option<Color> {
    if s.len() != 8 || !s.is_ascii() {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).ok();
    Some(Color {
        r: byte(0)?,
        g: byte(2)?,
        b: byte(4)?,
        a: byte(6)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two() -> Gradient {
        Gradient::from_color(GradientKind::Linear, Color::rgb(255, 0, 0))
    }

    #[test]
    fn color_at_blends_between_stops_and_clamps() {
        let g = two();
        assert_eq!(g.color_at(0.0), Color::rgb(255, 0, 0));
        assert_eq!(g.color_at(1.0).a, 0);
        assert_eq!(g.color_at(0.5).a, 128);
        assert_eq!(g.color_at(-3.0), g.color_at(0.0));
        assert_eq!(g.color_at(9.0), g.color_at(1.0));
    }

    #[test]
    fn stops_can_be_added_in_order_and_removed_down_to_two() {
        let mut g = two();
        let i = g.add_stop(0.25);
        assert_eq!(i, 1);
        assert_eq!(g.stops.len(), 3);
        assert!(g.stops.windows(2).all(|w| w[0].pos <= w[1].pos));
        assert!(g.remove_stop(i));
        assert!(!g.remove_stop(0));
        assert_eq!(g.stops.len(), MIN_STOPS);
    }

    #[test]
    fn text_roundtrip_for_solid_and_gradient() {
        let solid = Paint::Solid(Color {
            a: 7,
            ..Color::rgb(1, 2, 3)
        });
        assert_eq!(Paint::from_code(&solid.to_code()), Some(solid));
        let mut g = two();
        g.kind = GradientKind::Angular;
        g.angle = 45.5;
        g.cx = 0.25;
        g.add_stop(0.4);
        let p = Paint::Gradient(g);
        assert_eq!(Paint::from_code(&p.to_code()), Some(p));
        assert!(Paint::from_code("g;linear;0;0.5;0.5;1;0=ff0000ff").is_none());
        assert!(Paint::from_code("g;nope;0;0.5;0.5;1;0=ff0000ff|1=00000000").is_none());
        assert!(Paint::from_code("zzzz").is_none());
        let pat = Paint::Pattern(Pattern {
            tiling: Tiling::BrickRows,
            scale: 0.5,
            gap_x: 4.0,
            gap_y: -2.0,
            angle: 30.0,
            ..Pattern::new(NodeId(12))
        });
        assert_eq!(Paint::from_code(&pat.to_code()), Some(pat));
        assert!(Paint::from_code("p;x;grid;1;0;0;0").is_none());
        assert!(Paint::from_code("p;3;weird;1;0;0;0").is_none());
    }
}
