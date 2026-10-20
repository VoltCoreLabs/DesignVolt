//! Pattern part of the color picker: choosing the source object and the tile settings.

use dv_core::{NodeId, Paint, Pattern};

use super::ops_paint::edit_fill;
use super::paint_ui::{Look, looks};
use super::picker_layout::PatField;
use super::state::Editor;

/// The value of a pattern parameter as shown: scale in percent, gaps in px, angle in degrees.
pub fn field_value(p: &Pattern, f: PatField) -> f64 {
    match f {
        PatField::Scale => p.scale * 100.0,
        PatField::GapX => p.gap_x,
        PatField::GapY => p.gap_y,
        PatField::Angle => p.angle,
    }
}

/// Sets a parameter from its shown value, clamped to what makes sense.
pub fn set_field(p: &mut Pattern, f: PatField, v: f64) {
    match f {
        PatField::Scale => p.scale = (v / 100.0).clamp(0.01, 50.0),
        PatField::GapX => p.gap_x = v.clamp(-10_000.0, 10_000.0),
        PatField::GapY => p.gap_y = v.clamp(-10_000.0, 10_000.0),
        PatField::Angle => p.angle = v.rem_euclid(360.0),
    }
}

impl Editor {
    pub(super) fn current_pattern(&self) -> Option<Pattern> {
        match looks(self).0 {
            Look::One(Paint::Pattern(p)) => Some(p),
            _ => None,
        }
    }

    /// Applies `f` to the pattern of every selected shape that has one.
    pub(super) fn edit_pattern(&mut self, f: impl Fn(&mut Pattern)) {
        let ex = &mut self.session.borrow_mut().exec;
        edit_fill(ex, &self.selection, None, |old| {
            old.map(|p| match p {
                Paint::Pattern(mut q) => {
                    f(&mut q);
                    Paint::Pattern(q)
                }
                other => other,
            })
        });
    }

    /// The pattern tab: waits for a click on the object to repeat.
    pub(super) fn begin_pattern(&mut self) {
        if self.current_pattern().is_none() {
            self.pick_source = true;
        }
    }

    pub(super) fn cycle_tiling(&mut self) {
        self.edit_pattern(|p| p.tiling = p.tiling.next());
    }

    /// The pattern source click: the object under the pointer becomes the tile, unless it
    /// is (or contains) one of the shapes being filled.
    pub(super) fn sample_source(&mut self) {
        self.pick_source = false;
        let world = self.world_mouse();
        let source: Option<NodeId> = {
            let s = self.session.borrow();
            let page = s.exec.doc().page();
            let targets = page.geo_tops(&self.selection);
            self.index
                .pick(page, world)
                .map(|id| page.outer_group(id))
                .filter(|src| targets.iter().all(|t| !page.is_inside(*t, *src)))
        };
        let Some(source) = source else { return };
        let ex = &mut self.session.borrow_mut().exec;
        edit_fill(ex, &self.selection, None, |old| {
            Some(Paint::Pattern(match old {
                Some(Paint::Pattern(p)) => Pattern { source, ..p },
                _ => Pattern::new(source),
            }))
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dv_core::Tiling;

    #[test]
    fn fields_roundtrip_and_clamp() {
        let mut p = Pattern::new(NodeId(1));
        set_field(&mut p, PatField::Scale, 50.0);
        assert_eq!(p.scale, 0.5);
        set_field(&mut p, PatField::Scale, 0.0);
        assert_eq!(p.scale, 0.01);
        set_field(&mut p, PatField::Angle, -90.0);
        assert_eq!(field_value(&p, PatField::Angle), 270.0);
        set_field(&mut p, PatField::GapX, 12.5);
        assert_eq!(field_value(&p, PatField::GapX), 12.5);
        assert_eq!(p.tiling, Tiling::Grid);
    }
}
