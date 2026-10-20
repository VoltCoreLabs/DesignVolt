//! The stroke settings window: line style, dashes, position, caps, joins and the miter angle.

use dv_core::{LineCap, LineJoin, Stroke, StrokePos};
use dv_ui::draw::Cx;
use dv_ui::icons::Icon;
use dv_ui::theme::colors::*;
use dv_ui::theme::metrics::*;
use kurbo::Rect;

use super::dropdown::Target;
use super::ops_paint::edit_stroke;
use super::paint_layout::{Hit, Look, PField, Slot};
use super::paint_ui::{drop_button, editable, looks, panel, style_name};
use super::state::Editor;
use super::stroke_pop_layout::*;
use crate::i18n::{Lang, lang};

fn t(ru: &'static str, en: &'static str) -> &'static str {
    if lang() == Lang::Ru { ru } else { en }
}

fn row_label(r: Row) -> &'static str {
    match r {
        Row::Style => t("Стиль", "Style"),
        Row::Dash => t("Штрих, пробел", "Dash, gap"),
        Row::Position => t("Позиция", "Position"),
        Row::Cap => t("Концы", "Caps"),
        Row::Join => t("Углы", "Join"),
        Row::Miter => t("Угол среза", "Miter angle"),
    }
}

fn seg_label(r: Row, k: usize) -> &'static str {
    match (r, k) {
        (Row::Position, 0) => t("Внутри", "Inside"),
        (Row::Position, 1) => t("Центр", "Center"),
        (Row::Position, _) => t("Снаружи", "Outside"),
        (Row::Cap, 0) => t("Плоский", "Butt"),
        (Row::Cap, 1) => t("Круг", "Round"),
        (Row::Cap, _) => t("Квадрат", "Square"),
        (Row::Join, 0) => t("Острый", "Miter"),
        (Row::Join, 1) => t("Круг", "Round"),
        (_, _) => t("Срез", "Bevel"),
    }
}

/// Which segment of a row is the current value.
fn selected(r: Row, s: &Stroke) -> usize {
    match r {
        Row::Position => StrokePos::ALL
            .iter()
            .position(|p| *p == s.position)
            .unwrap_or(1),
        Row::Cap => LineCap::ALL.iter().position(|p| *p == s.cap).unwrap_or(0),
        _ => LineJoin::ALL.iter().position(|p| *p == s.join).unwrap_or(0),
    }
}

/// The text shown in a field of this window.
pub fn field_value(s: &Stroke, f: PField) -> Option<f64> {
    match f {
        PField::Dash => Some(s.dash),
        PField::Gap => Some(s.gap),
        PField::Miter => Some(s.miter_angle),
        _ => None,
    }
}

/// The stroke with one field of this window set, in a sane range.
pub fn with_field(s: Stroke, f: PField, v: f64) -> Stroke {
    match f {
        PField::Dash => Stroke {
            dash: v.clamp(0.1, 10_000.0),
            ..s
        },
        PField::Gap => Stroke {
            gap: v.clamp(0.0, 10_000.0),
            ..s
        },
        PField::Miter => Stroke {
            miter_angle: v.clamp(1.0, 180.0),
            ..s
        },
        _ => s,
    }
}

impl Editor {
    fn pop_state(&self) -> Option<(Rect, Stroke, Vec<Row>)> {
        if !self.stroke_pop {
            return None;
        }
        let Look::One(s) = looks(self).1 else {
            return None;
        };
        let rows = rows(s.style, s.join);
        let p = panel(self);
        let popup = popup_rect(p.right, p.style_rect().origin(), rows.len(), self.win.h);
        Some((popup, s, rows))
    }

    /// The rectangle of a field of this window (empty while it is closed).
    pub(super) fn pop_field_rect(&self, f: PField) -> Rect {
        let Some((popup, _, rows)) = self.pop_state() else {
            return Rect::ZERO;
        };
        let row = if f == PField::Miter {
            Row::Miter
        } else {
            Row::Dash
        };
        let Some(i) = rows.iter().position(|r| *r == row) else {
            return Rect::ZERO;
        };
        let c = control_rect(popup, i);
        if f == PField::Miter {
            c
        } else {
            dash_field(c, f)
        }
    }

    /// The style button inside the window, when it is open (the drop-down opens under it).
    pub(super) fn stroke_pop_style_rect(&self) -> Option<Rect> {
        let (popup, _, _) = self.pop_state()?;
        Some(control_rect(popup, 0))
    }

    pub(super) fn toggle_stroke_pop(&mut self) {
        self.stroke_pop = !self.stroke_pop;
        if self.stroke_pop {
            self.close_picker();
        }
    }

    /// A press while the window is open. Returns true when it was consumed.
    pub(super) fn press_stroke_pop(&mut self) -> bool {
        let Some((popup, s, rows)) = self.pop_state() else {
            self.stroke_pop = false;
            return false;
        };
        if !popup.contains(self.mouse) {
            // The gear that opened it only closes it.
            let on_gear = self.paint_hit() == Some(Hit::StrokeSettings);
            self.stroke_pop = false;
            return on_gear;
        }
        match hit(popup, &rows, self.mouse) {
            Some(PopHit::Close) => self.stroke_pop = false,
            Some(PopHit::StyleButton) => self.open_dropdown_for(Target::StrokeStyle),
            Some(PopHit::Segment(row, k)) => self.set_segment(row, k),
            Some(PopHit::Field(f)) => {
                self.commit_paint_edit();
                self.start_paint_edit(Slot::Stroke, f);
            }
            None => {}
        }
        let _ = s;
        true
    }

    fn set_segment(&mut self, row: Row, k: usize) {
        let ex = &mut self.session.borrow_mut().exec;
        edit_stroke(ex, &self.selection, None, |s| {
            s.map(|s| match row {
                Row::Position => Stroke {
                    position: StrokePos::ALL[k],
                    ..s
                },
                Row::Cap => Stroke {
                    cap: LineCap::ALL[k],
                    ..s
                },
                _ => Stroke {
                    join: LineJoin::ALL[k],
                    ..s
                },
            })
        });
    }

    pub(super) fn draw_stroke_pop(&self, cx: &mut Cx) {
        let Some((popup, s, rows)) = self.pop_state() else {
            return;
        };
        cx.shadow(popup, R_CARD, alpha(0x000000, 0.25), 16.0, 8.0);
        cx.fill(popup, R_CARD, hex(bg_raised()));
        cx.stroke(popup, R_CARD, 1.0, alpha(overlay(), 0.12));
        let x = label_x(popup);
        cx.text(
            t("Настройки обводки", "Stroke settings"),
            FONT_MD,
            x,
            popup.y0 + 27.0,
            hex(text_primary()),
        );
        let close = close_rect(popup);
        let c = close.center();
        cx.icon(
            Icon::Close,
            c.x - 7.0,
            c.y - 7.0,
            14.0,
            hex(text_secondary()),
        );
        for (i, row) in rows.iter().enumerate() {
            let ctl = control_rect(popup, i);
            cx.text(
                row_label(*row),
                FONT_UI,
                x,
                ctl.y0 + 18.0,
                hex(text_secondary()),
            );
            match row {
                Row::Style => drop_button(cx, ctl, style_name(s.style), ctl.contains(self.mouse)),
                Row::Dash => {
                    for f in [PField::Dash, PField::Gap] {
                        let v = field_value(&s, f).unwrap_or(0.0);
                        editable(
                            cx,
                            self,
                            (Slot::Stroke, f),
                            dash_field(ctl, f),
                            &super::canvas::num(v),
                            "",
                        );
                    }
                }
                Row::Miter => {
                    let v = super::canvas::num(s.miter_angle);
                    editable(cx, self, (Slot::Stroke, PField::Miter), ctl, &v, "°");
                }
                _ => {
                    let on = selected(*row, &s);
                    for k in 0..3 {
                        seg(cx, seg_rect(ctl, k, 3), seg_label(*row, k), k == on);
                    }
                }
            }
        }
    }
}

fn seg(cx: &mut Cx, r: Rect, label: &str, on: bool) {
    let bg = if on {
        alpha(accent(), 0.28)
    } else {
        hex(bg_app())
    };
    cx.fill(r, R_FIELD, bg);
    let fg = if on {
        hex(text_primary())
    } else {
        hex(text_secondary())
    };
    let shown = cx.fit(label, FONT_SM, r.width() - 6.0);
    let w = cx.text_width(&shown, FONT_SM);
    cx.text(&shown, FONT_SM, r.center().x - w / 2.0, r.y0 + 18.0, fg);
}

#[cfg(test)]
mod tests {
    use super::*;
    use dv_core::Color;

    #[test]
    fn fields_are_clamped_to_sane_ranges() {
        let s = Stroke::new(Color::rgb(0, 0, 0), 2.0);
        assert_eq!(with_field(s, PField::Dash, -3.0).dash, 0.1);
        assert_eq!(with_field(s, PField::Gap, 5.5).gap, 5.5);
        assert_eq!(with_field(s, PField::Miter, 400.0).miter_angle, 180.0);
        assert_eq!(with_field(s, PField::Miter, 0.0).miter_angle, 1.0);
        assert_eq!(field_value(&s, PField::Hex), None);
    }

    #[test]
    fn the_current_segment_follows_the_stroke() {
        let mut s = Stroke::new(Color::rgb(0, 0, 0), 2.0);
        assert_eq!(selected(Row::Position, &s), 1);
        s.position = StrokePos::Outside;
        s.cap = LineCap::Square;
        s.join = LineJoin::Bevel;
        assert_eq!(selected(Row::Position, &s), 2);
        assert_eq!(selected(Row::Cap, &s), 2);
        assert_eq!(selected(Row::Join, &s), 2);
    }
}
