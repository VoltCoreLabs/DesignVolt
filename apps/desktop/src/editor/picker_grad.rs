//! Gradient part of the color picker: type tabs, stops and their parameters.

use dv_core::{Color, Gradient, GradientKind, Paint};

use super::ops_paint::edit_fill_at;
use super::paint_ui::{Look, fill_look};
use super::picker_layout::{GField, Part, TABS};
use super::state::Editor;

/// The gradient type of tab `i` (tab 0 is the solid color).
pub fn tab_kind(i: usize) -> Option<GradientKind> {
    i.checked_sub(1)
        .and_then(|k| GradientKind::ALL.get(k))
        .copied()
}

pub fn tab_of(paint: Option<&Paint>) -> usize {
    match paint {
        Some(Paint::Gradient(g)) => {
            1 + GradientKind::ALL
                .iter()
                .position(|k| *k == g.kind)
                .unwrap_or(0)
        }
        Some(Paint::Pattern(_)) => 5,
        _ => 0,
    }
}

/// The value of a parameter as shown: positions and scale in percent, angle in degrees.
pub fn field_value(g: &Gradient, stop: usize, f: GField) -> f64 {
    match f {
        GField::Pos => g.stops.get(stop).map_or(0.0, |s| s.pos * 100.0),
        GField::Angle => g.angle,
        GField::X => g.cx * 100.0,
        GField::Y => g.cy * 100.0,
        GField::Scale => g.scale * 100.0,
    }
}

/// Sets a parameter from its shown value (clamped to what makes sense).
pub fn set_field(g: &mut Gradient, stop: usize, f: GField, v: f64) {
    match f {
        GField::Pos => {
            if let Some(s) = g.stops.get_mut(stop) {
                s.pos = (v / 100.0).clamp(0.0, 1.0);
            }
            g.normalize();
        }
        GField::Angle => g.angle = v.rem_euclid(360.0),
        GField::X => g.cx = (v / 100.0).clamp(-2.0, 3.0),
        GField::Y => g.cy = (v / 100.0).clamp(-2.0, 3.0),
        GField::Scale => g.scale = (v / 100.0).clamp(0.01, 20.0),
    }
}

const _: () = assert!(TABS == 6);

impl Editor {
    /// The gradient of the selection when all selected shapes share it.
    pub(super) fn current_gradient(&self) -> Option<Gradient> {
        match fill_look(self, self.fill_idx()) {
            Look::One(Paint::Gradient(g)) => Some(g),
            _ => None,
        }
    }

    /// The common fill of the selection, for marking the active tab.
    pub(super) fn fill_for_tabs(&self) -> Option<Paint> {
        match fill_look(self, self.fill_idx()) {
            Look::One(p) => Some(p),
            _ => None,
        }
    }

    pub(super) fn grad_positions(&self) -> Vec<f64> {
        self.current_gradient()
            .map(|g| g.stops.iter().map(|s| s.pos).collect())
            .unwrap_or_default()
    }

    /// Applies `f` to the gradient of every selected shape that has one.
    pub(super) fn edit_gradient(&mut self, key: Option<u64>, f: impl Fn(&mut Gradient)) {
        let ex = &mut self.session.borrow_mut().exec;
        edit_fill_at(ex, &self.selection, key, self.fill_idx(), |old| {
            old.map(|p| match p {
                Paint::Gradient(mut g) => {
                    f(&mut g);
                    Paint::Gradient(g)
                }
                other => other,
            })
        });
    }

    /// Makes stop `i` the edited one and shows its color in the picker.
    pub(super) fn select_stop(&mut self, i: usize) {
        let color = self
            .current_gradient()
            .and_then(|g| g.stops.get(i).map(|s| s.color));
        if let Some(p) = self.picker.as_mut() {
            p.stop = i;
            if let Some(c) = color {
                p.set(c);
            }
        }
    }

    /// Index of the stop that matches `pos` and `color` after the stops were re-sorted.
    fn find_stop(&self, pos: f64, color: Color) -> usize {
        self.current_gradient()
            .and_then(|g| {
                g.stops
                    .iter()
                    .position(|s| s.pos == pos && s.color == color)
            })
            .unwrap_or(0)
    }

    pub(super) fn start_stop_drag(&mut self, i: usize) {
        self.select_stop(i);
        let key = self.next_drag_key();
        if let Some(p) = self.picker.as_mut() {
            p.part = Some(Part::Stop(i));
            p.key = key;
        }
        self.interaction = Some(super::interact::Interaction::PickColor);
    }

    /// Moves the dragged stop to `t` (0..1); the stops stay sorted, so its index may change.
    pub(super) fn drag_stop(&mut self, i: usize, t: f64) {
        let key = self.picker.as_ref().map(|p| p.key);
        let Some(color) = self
            .current_gradient()
            .and_then(|g| g.stops.get(i).map(|s| s.color))
        else {
            return;
        };
        self.edit_gradient(key, |g| set_field(g, i, GField::Pos, t * 100.0));
        let now = self.find_stop(t, color);
        if let Some(p) = self.picker.as_mut() {
            p.part = Some(Part::Stop(now));
            p.stop = now;
        }
    }

    pub(super) fn add_stop_at(&mut self, t: f64) {
        self.edit_gradient(None, |g| {
            g.add_stop(t);
        });
        let color = self.current_gradient().map(|g| g.color_at(t));
        if let Some(c) = color {
            let i = self.find_stop(t, c);
            self.start_stop_drag(i);
        }
    }

    pub(super) fn remove_selected_stop(&mut self) {
        let i = self.picker.as_ref().map_or(0, |p| p.stop);
        self.edit_gradient(None, |g| {
            g.remove_stop(i);
        });
        let last = self.grad_positions().len().saturating_sub(1);
        self.select_stop(i.min(last));
    }

    /// Switches the fill between a solid color and the gradient types.
    pub(super) fn pick_tab(&mut self, i: usize) {
        if i == 5 {
            self.begin_pattern();
            return;
        }
        let kind = tab_kind(i);
        let color = self.picker.as_ref().map(|p| p.color());
        {
            let ex = &mut self.session.borrow_mut().exec;
            edit_fill_at(ex, &self.selection, None, self.fill_idx(), |old| {
                let base = old.as_ref().map(Paint::main_color).or(color)?;
                Some(match (kind, old) {
                    (None, _) => Paint::Solid(base),
                    (Some(k), Some(Paint::Gradient(mut g))) => {
                        g.kind = k;
                        Paint::Gradient(g)
                    }
                    (Some(k), _) => Paint::Gradient(Gradient::from_color(k, base)),
                })
            });
        }
        if let Some(p) = self.picker.as_mut() {
            p.stop = 0;
        }
        self.select_stop(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g() -> Gradient {
        let mut g = Gradient::from_color(GradientKind::Linear, Color::rgb(1, 2, 3));
        g.add_stop(0.5);
        g
    }

    #[test]
    fn tabs_map_to_types() {
        assert_eq!(tab_kind(0), None);
        assert_eq!(tab_kind(1), Some(GradientKind::Linear));
        assert_eq!(tab_kind(4), Some(GradientKind::Diamond));
        assert_eq!(tab_of(None), 0);
        assert_eq!(tab_of(Some(&Paint::Gradient(g()))), 1);
    }

    #[test]
    fn fields_are_read_and_clamped() {
        let mut g = g();
        set_field(&mut g, 1, GField::Pos, 150.0);
        assert_eq!(g.stops.last().unwrap().pos, 1.0);
        set_field(&mut g, 0, GField::Angle, -90.0);
        assert_eq!(field_value(&g, 0, GField::Angle), 270.0);
        set_field(&mut g, 0, GField::Scale, 0.0);
        assert_eq!(g.scale, 0.01);
        set_field(&mut g, 0, GField::X, 25.0);
        assert_eq!(field_value(&g, 0, GField::X), 25.0);
    }
}
