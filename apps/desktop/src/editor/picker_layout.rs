//! Geometry of the color picker popup and what a point in it hits.

use dv_core::GradientKind;
use dv_ui::theme::metrics::{FIELD_H, TOPBAR_H};
use kurbo::{Point, Rect};

use super::color_fmt::Mode;
use super::paint_ui::{Slot, anchor};

pub const W: f64 = 300.0;
const PAD: f64 = 12.0;
const SV_H: f64 = 150.0;
pub const BAR_H: f64 = 12.0;
const BAR_GAP: f64 = 10.0;
const GAP_V: f64 = 12.0;
const BTN: f64 = 28.0;
const MODE_W: f64 = 56.0;
const FIELD_GAP: f64 = 4.0;
const ALPHA_W: f64 = 52.0;
pub const SW: f64 = 20.0;
const SW_GAP: f64 = 6.0;
pub const PER_ROW: usize = 10;
const TAB_H: f64 = 26.0;
const TILING_W: f64 = 92.0;
/// Height of the gradient line of the stops bar; the handles sit under it.
pub const STOPS_BAR_H: f64 = 16.0;
const STOPS_H: f64 = STOPS_BAR_H + 16.0;
pub const HANDLE_R: f64 = 7.0;
/// Number of tabs: solid color, the gradient types and the pattern.
pub const TABS: usize = 6;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Part {
    Sv,
    Hue,
    Alpha,
    /// Dragging the handle of a gradient stop.
    Stop(usize),
}

/// A numeric parameter of a gradient.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GField {
    Pos,
    Angle,
    X,
    Y,
    Scale,
}

/// A numeric parameter of a pattern.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PatField {
    Scale,
    GapX,
    GapY,
    Angle,
}

pub const PAT_FIELDS: [PatField; 4] = [
    PatField::Scale,
    PatField::GapX,
    PatField::GapY,
    PatField::Angle,
];

/// The parameter fields shown for a gradient type.
pub fn grad_fields(kind: GradientKind) -> &'static [GField] {
    match kind {
        GradientKind::Linear => &[GField::Pos, GField::Angle, GField::Scale],
        GradientKind::Radial | GradientKind::Diamond => {
            &[GField::Pos, GField::X, GField::Y, GField::Scale]
        }
        GradientKind::Angular => &[GField::Pos, GField::Angle, GField::X, GField::Y],
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PHit {
    Part(Part),
    Eyedrop,
    Mode,
    Field(usize),
    AlphaField,
    Recent(usize),
    Tab(usize),
    /// A press on the gradient line (not on a handle), at this position.
    StopsBar(f64),
    GField(GField),
    RemoveStop,
    /// The box that shows the pattern source: click to choose another one.
    PickSource,
    Tiling,
    PatField(PatField),
}

/// What the popup contains: decides its height and where things are.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Shape {
    /// The type tabs (fill only).
    pub tabs: bool,
    /// A gradient is being edited: stops bar and parameter row.
    pub grad: Option<GradientKind>,
    /// A pattern is being edited: its controls replace the color editor.
    pub pattern: bool,
    /// Rows of recent colors.
    pub rows: usize,
}

struct Offs {
    tabs: f64,
    stops: f64,
    sv: f64,
    bars: f64,
    fields: f64,
    params: f64,
    recents: f64,
    pat_a: f64,
    pat_b: f64,
    end: f64,
}

fn offs(s: Shape) -> Offs {
    let mut y = PAD;
    let tabs = y;
    if s.tabs {
        y += TAB_H + GAP_V;
    }
    if s.pattern {
        let pat_a = y;
        let pat_b = pat_a + FIELD_H + GAP_V;
        return Offs {
            tabs,
            stops: 0.0,
            sv: 0.0,
            bars: 0.0,
            fields: 0.0,
            params: 0.0,
            recents: 0.0,
            pat_a,
            pat_b,
            end: pat_b + FIELD_H + PAD,
        };
    }
    let stops = y;
    if s.grad.is_some() {
        y += STOPS_H + GAP_V;
    }
    let sv = y;
    let bars = sv + SV_H + GAP_V;
    let fields = bars + BAR_H * 2.0 + BAR_GAP + GAP_V;
    let mut end = fields + FIELD_H;
    let params = end + GAP_V;
    if s.grad.is_some() {
        end = params + FIELD_H;
    }
    let recents = end + GAP_V;
    if s.rows > 0 {
        end = recents + s.rows as f64 * (SW + SW_GAP) - SW_GAP;
    }
    Offs {
        tabs,
        stops,
        sv,
        bars,
        fields,
        params,
        recents,
        pat_a: 0.0,
        pat_b: 0.0,
        end: end + PAD,
    }
}

pub fn recent_rows(n: usize) -> usize {
    n.div_ceil(PER_ROW)
}

pub fn height(s: Shape) -> f64 {
    offs(s).end
}

/// The popup: left of the right panel, level with the swatch, kept on screen.
pub fn popup_rect(right: Rect, slot: Slot, win_h: f64, s: Shape) -> Rect {
    let h = height(s);
    let y = (anchor(right, slot).y - 8.0)
        .min(win_h - h - 8.0)
        .max(TOPBAR_H + 8.0);
    let x1 = right.x0 - 8.0;
    Rect::new(x1 - W, y, x1, y + h)
}

pub fn part_rect(p: Rect, s: Shape, part: Part) -> Rect {
    let o = offs(s);
    let (x0, x1) = (p.x0 + PAD, p.x1 - PAD);
    match part {
        Part::Sv => Rect::new(x0, p.y0 + o.sv, x1, p.y0 + o.sv + SV_H),
        Part::Hue => Rect::new(x0 + BTN + 8.0, p.y0 + o.bars, x1, p.y0 + o.bars + BAR_H),
        Part::Alpha => {
            let y = p.y0 + o.bars + BAR_H + BAR_GAP;
            Rect::new(x0 + BTN + 8.0, y, x1, y + BAR_H)
        }
        Part::Stop(_) => stops_bar(p, s),
    }
}

/// The gradient line that shows the stops.
pub fn stops_bar(p: Rect, s: Shape) -> Rect {
    let y = p.y0 + offs(s).stops;
    Rect::new(
        p.x0 + PAD + HANDLE_R,
        y,
        p.x1 - PAD - HANDLE_R,
        y + STOPS_BAR_H,
    )
}

/// Center of the handle of a stop at `pos`.
pub fn handle_center(p: Rect, s: Shape, pos: f64) -> Point {
    let bar = stops_bar(p, s);
    Point::new(bar.x0 + pos * bar.width(), bar.y1 + HANDLE_R + 1.0)
}

pub fn tab_rect(p: Rect, s: Shape, i: usize) -> Rect {
    let y = p.y0 + offs(s).tabs;
    let inner = W - 2.0 * PAD;
    let w = (inner - FIELD_GAP * (TABS as f64 - 1.0)) / TABS as f64;
    let x = p.x0 + PAD + i as f64 * (w + FIELD_GAP);
    Rect::new(x, y, x + w, y + TAB_H)
}

pub fn eyedrop_rect(p: Rect, s: Shape) -> Rect {
    let y = p.y0 + offs(s).bars + (BAR_H * 2.0 + BAR_GAP - BTN) / 2.0;
    Rect::new(p.x0 + PAD, y, p.x0 + PAD + BTN, y + BTN)
}

pub fn mode_rect(p: Rect, s: Shape) -> Rect {
    let y = p.y0 + offs(s).fields;
    Rect::new(p.x0 + PAD, y, p.x0 + PAD + MODE_W, y + FIELD_H)
}

fn field_area(p: Rect, s: Shape, mode: Mode) -> (f64, f64, f64, f64) {
    let x0 = p.x0 + PAD + MODE_W + 6.0;
    let right = p.x1 - PAD;
    let y = p.y0 + offs(s).fields;
    let end = if mode.has_alpha_field() {
        right - ALPHA_W - FIELD_GAP
    } else {
        right
    };
    (x0, end, y, right)
}

pub fn field_rect(p: Rect, s: Shape, mode: Mode, i: usize) -> Rect {
    let (x0, end, y, _) = field_area(p, s, mode);
    let n = mode.fields() as f64;
    let w = (end - x0 - FIELD_GAP * (n - 1.0)) / n;
    let x = x0 + i as f64 * (w + FIELD_GAP);
    Rect::new(x, y, x + w, y + FIELD_H)
}

pub fn alpha_field_rect(p: Rect, s: Shape, mode: Mode) -> Rect {
    let (_, _, y, right) = field_area(p, s, mode);
    Rect::new(right - ALPHA_W, y, right, y + FIELD_H)
}

/// The `i`-th parameter field of the gradient row.
pub fn gfield_rect(p: Rect, s: Shape, i: usize) -> Rect {
    let n = s.grad.map_or(1, |k| grad_fields(k).len()) as f64;
    let y = p.y0 + offs(s).params;
    let x0 = p.x0 + PAD;
    let avail = W - 2.0 * PAD - BTN - FIELD_GAP;
    let w = (avail - FIELD_GAP * (n - 1.0)) / n;
    let x = x0 + i as f64 * (w + FIELD_GAP);
    Rect::new(x, y, x + w, y + FIELD_H)
}

pub fn remove_stop_rect(p: Rect, s: Shape) -> Rect {
    let y = p.y0 + offs(s).params;
    Rect::new(p.x1 - PAD - BTN, y, p.x1 - PAD, y + FIELD_H)
}

/// The source box and the tiling button of the pattern editor.
pub fn pat_source_rect(p: Rect, s: Shape) -> Rect {
    let y = p.y0 + offs(s).pat_a;
    Rect::new(
        p.x0 + PAD,
        y,
        p.x1 - PAD - TILING_W - FIELD_GAP,
        y + FIELD_H,
    )
}

pub fn pat_tiling_rect(p: Rect, s: Shape) -> Rect {
    let y = p.y0 + offs(s).pat_a;
    Rect::new(p.x1 - PAD - TILING_W, y, p.x1 - PAD, y + FIELD_H)
}

pub fn pat_field_rect(p: Rect, s: Shape, i: usize) -> Rect {
    let n = PAT_FIELDS.len() as f64;
    let y = p.y0 + offs(s).pat_b;
    let w = (W - 2.0 * PAD - FIELD_GAP * (n - 1.0)) / n;
    let x = p.x0 + PAD + i as f64 * (w + FIELD_GAP);
    Rect::new(x, y, x + w, y + FIELD_H)
}

pub fn recent_rect(p: Rect, s: Shape, i: usize) -> Rect {
    let (row, col) = (i / PER_ROW, i % PER_ROW);
    let x = p.x0 + PAD + col as f64 * (SW + SW_GAP);
    let y = p.y0 + offs(s).recents + row as f64 * (SW + SW_GAP);
    Rect::new(x, y, x + SW, y + SW)
}

/// What the point hits inside the popup, if anything. `stops` are the positions of the
/// gradient stops (empty when no gradient is edited).
pub fn hit(p: Rect, s: Shape, at: Point, mode: Mode, recent: usize, stops: &[f64]) -> Option<PHit> {
    if s.pattern {
        if s.tabs
            && let Some(i) = (0..TABS).find(|i| tab_rect(p, s, *i).contains(at))
        {
            return Some(PHit::Tab(i));
        }
        if pat_source_rect(p, s).contains(at) {
            return Some(PHit::PickSource);
        }
        if pat_tiling_rect(p, s).contains(at) {
            return Some(PHit::Tiling);
        }
        return (0..PAT_FIELDS.len())
            .find(|i| pat_field_rect(p, s, *i).contains(at))
            .map(|i| PHit::PatField(PAT_FIELDS[i]));
    }
    if s.grad.is_some() {
        for (i, pos) in stops.iter().enumerate() {
            let c = handle_center(p, s, *pos);
            if (at - c).hypot() <= HANDLE_R + 2.0 {
                return Some(PHit::Part(Part::Stop(i)));
            }
        }
        let bar = stops_bar(p, s);
        let zone = Rect::new(
            bar.x0 - HANDLE_R,
            bar.y0,
            bar.x1 + HANDLE_R,
            bar.y1 + HANDLE_R * 2.0 + 2.0,
        );
        if zone.contains(at) {
            let t = ((at.x - bar.x0) / bar.width()).clamp(0.0, 1.0);
            return Some(PHit::StopsBar(t));
        }
        if remove_stop_rect(p, s).contains(at) {
            return Some(PHit::RemoveStop);
        }
        if let Some(k) = s.grad {
            let fields = grad_fields(k);
            if let Some(i) = (0..fields.len()).find(|i| gfield_rect(p, s, *i).contains(at)) {
                return Some(PHit::GField(fields[i]));
            }
        }
    }
    if s.tabs
        && let Some(i) = (0..TABS).find(|i| tab_rect(p, s, *i).contains(at))
    {
        return Some(PHit::Tab(i));
    }
    for part in [Part::Sv, Part::Hue, Part::Alpha] {
        if part_rect(p, s, part).inflate(0.0, 4.0).contains(at) {
            return Some(PHit::Part(part));
        }
    }
    if eyedrop_rect(p, s).contains(at) {
        return Some(PHit::Eyedrop);
    }
    if mode_rect(p, s).contains(at) {
        return Some(PHit::Mode);
    }
    if let Some(i) = (0..mode.fields()).find(|i| field_rect(p, s, mode, *i).contains(at)) {
        return Some(PHit::Field(i));
    }
    if mode.has_alpha_field() && alpha_field_rect(p, s, mode).contains(at) {
        return Some(PHit::AlphaField);
    }
    (0..recent)
        .find(|i| recent_rect(p, s, *i).contains(at))
        .map(PHit::Recent)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOLID: Shape = Shape {
        tabs: true,
        grad: None,
        pattern: false,
        rows: 2,
    };
    const GRAD: Shape = Shape {
        tabs: true,
        grad: Some(GradientKind::Radial),
        pattern: false,
        rows: 2,
    };

    fn popup(s: Shape) -> Rect {
        Rect::new(0.0, 0.0, W, height(s))
    }

    #[test]
    fn gradient_shape_is_taller_and_everything_fits_inside() {
        assert!(height(GRAD) > height(SOLID));
        for s in [SOLID, GRAD] {
            let p = popup(s);
            assert!(recent_rect(p, s, 19).y1 < p.y1);
            for mode in Mode::ALL {
                let mut last = mode_rect(p, s).x1;
                for i in 0..mode.fields() {
                    let r = field_rect(p, s, mode, i);
                    assert!(r.x0 > last && r.width() > 24.0, "{mode:?} {i}");
                    last = r.x1;
                }
                if mode.has_alpha_field() {
                    assert!(alpha_field_rect(p, s, mode).x0 > last);
                }
            }
        }
    }

    #[test]
    fn pattern_editor_is_short_and_its_rows_fit() {
        let s = Shape {
            tabs: true,
            grad: None,
            pattern: true,
            rows: 3,
        };
        let p = popup(s);
        assert!(height(s) < height(SOLID));
        let mut last = p.x0;
        for i in 0..PAT_FIELDS.len() {
            let r = pat_field_rect(p, s, i);
            assert!(r.x0 >= last && r.width() > 40.0 && r.x1 <= p.x1);
            last = r.x1;
        }
        assert!(pat_tiling_rect(p, s).x0 > pat_source_rect(p, s).x1);
        assert_eq!(
            hit(p, s, pat_tiling_rect(p, s).center(), Mode::Hex, 0, &[]),
            Some(PHit::Tiling)
        );
        assert_eq!(
            hit(p, s, pat_field_rect(p, s, 2).center(), Mode::Hex, 0, &[]),
            Some(PHit::PatField(PatField::GapY))
        );
    }

    #[test]
    fn gradient_rows_do_not_overlap() {
        let p = popup(GRAD);
        let n = grad_fields(GradientKind::Radial).len();
        let mut last = p.x0;
        for i in 0..n {
            let r = gfield_rect(p, GRAD, i);
            assert!(r.x0 >= last && r.width() > 40.0);
            last = r.x1;
        }
        assert!(remove_stop_rect(p, GRAD).x0 > last);
        assert!(stops_bar(p, GRAD).y1 < part_rect(p, GRAD, Part::Sv).y0);
    }

    #[test]
    fn points_map_to_controls() {
        let s = GRAD;
        let p = popup(s);
        let at = |r: Rect| r.center();
        let stops = [0.0, 1.0];
        let h = |pt| hit(p, s, pt, Mode::Rgb, 3, &stops);
        assert_eq!(h(at(part_rect(p, s, Part::Sv))), Some(PHit::Part(Part::Sv)));
        assert_eq!(h(at(eyedrop_rect(p, s))), Some(PHit::Eyedrop));
        assert_eq!(h(at(mode_rect(p, s))), Some(PHit::Mode));
        assert_eq!(h(at(field_rect(p, s, Mode::Rgb, 2))), Some(PHit::Field(2)));
        assert_eq!(h(at(recent_rect(p, s, 1))), Some(PHit::Recent(1)));
        assert_eq!(h(at(recent_rect(p, s, 5))), None);
        assert_eq!(h(at(tab_rect(p, s, 3))), Some(PHit::Tab(3)));
        assert_eq!(h(handle_center(p, s, 1.0)), Some(PHit::Part(Part::Stop(1))));
        assert_eq!(h(at(remove_stop_rect(p, s))), Some(PHit::RemoveStop));
        assert_eq!(h(at(gfield_rect(p, s, 0))), Some(PHit::GField(GField::Pos)));
        match h(Point::new(p.center().x, stops_bar(p, s).center().y)) {
            Some(PHit::StopsBar(t)) => assert!((t - 0.5).abs() < 0.05),
            other => panic!("{other:?}"),
        }
    }
}
