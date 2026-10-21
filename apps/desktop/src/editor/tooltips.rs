//! Tooltips of the editor: which icon-only control is under the pointer and what it says.

use dv_ui::draw::Cx;
use kurbo::Rect;

use super::flip::Action;
use super::pages;
use super::paint_layout::{Hit, Slot};
use super::paint_ui::panel;
use super::panels;
use super::state::Editor;
use crate::i18n::{Lang, lang};

fn t(ru: &'static str, en: &'static str) -> &'static str {
    if lang() == Lang::Ru { ru } else { en }
}

/// Tool names with their key, in toolbar order.
fn tool_tip(i: usize) -> &'static str {
    match i {
        0 => t("Выбор (V)", "Move (V)"),
        1 => t("Фрейм (F)", "Frame (F)"),
        2 => t("Прямоугольник (R)", "Rectangle (R)"),
        3 => t("Эллипс (O)", "Ellipse (O)"),
        4 => t("Перо (P)", "Pen (P)"),
        5 => t("Текст (T)", "Text (T)"),
        _ => t("Рука (H)", "Hand (H)"),
    }
}

fn action_tip(a: Action) -> &'static str {
    match a {
        Action::TurnLeft => t("Повернуть на 90° влево", "Rotate 90° left"),
        Action::TurnRight => t("Повернуть на 90° вправо", "Rotate 90° right"),
        Action::FlipHorizontal => t("Отразить по горизонтали", "Flip horizontal"),
        Action::FlipVertical => t("Отразить по вертикали", "Flip vertical"),
    }
}

fn slot_name(slot: Slot) -> &'static str {
    match slot {
        Slot::Fill(_) => t("заливку", "fill"),
        Slot::Stroke => t("обводку", "stroke"),
        Slot::Effect(_) => t("эффект", "effect"),
    }
}

impl Editor {
    /// The icon-only control under the pointer with its tooltip text.
    fn tip_target(&self) -> Option<(Rect, String)> {
        if self.interaction.is_some() || self.dropdown.is_some() || self.mouse.x < 0.0 {
            return None;
        }
        let l = self.layout();
        if let Some(i) = self.toolbar.hit(&l, self.mouse.x, self.mouse.y) {
            return Some((l.tools[i], tool_tip(i).to_string()));
        }
        if let Some(tip) = self.picker_tip() {
            return Some(tip);
        }
        if self.stroke_pop || self.picker.is_some() {
            return None;
        }
        if let Some(a) = self.action_hit() {
            let k = Action::ALL.iter().position(|x| *x == a)?;
            return Some((panels::action_rect(l.right, k), action_tip(a).to_string()));
        }
        if let Some(hit) = self.paint_hit() {
            return self.paint_tip(hit);
        }
        if l.left.contains(self.mouse) {
            return self.left_tip();
        }
        None
    }

    fn paint_tip(&self, hit: Hit) -> Option<(Rect, String)> {
        let p = panel(self);
        let (slot, add) = match hit {
            Hit::Add(s) => (s, true),
            Hit::Remove(s) => (s, false),
            Hit::AddEffect => (Slot::Effect(0), true),
            Hit::StrokeSettings => {
                return Some((
                    p.settings_rect(),
                    t("Настройки обводки", "Stroke settings").to_string(),
                ));
            }
            _ => return None,
        };
        let rect = [p.add_rect(slot), p.remove_rect(slot)]
            .into_iter()
            .find(|r| r.contains(self.mouse))
            .unwrap_or_else(|| Rect::from_center_size(self.mouse, (16.0, 16.0)));
        let text = if add {
            format!("{} {}", t("Добавить", "Add"), slot_name(slot))
        } else {
            format!("{} {}", t("Удалить", "Remove"), slot_name(slot))
        };
        Some((rect, text))
    }

    fn left_tip(&self) -> Option<(Rect, String)> {
        let left = self.layout().left;
        if pages::add_rect(left).contains(self.mouse) {
            return Some((
                pages::add_rect(left),
                t("Добавить страницу", "Add page").to_string(),
            ));
        }
        let n = self.page_count();
        for i in 0..pages::rows_for(n).min(n) {
            let r = pages::row_rect(left, i);
            if n > 1 && pages::remove_rect(r).contains(self.mouse) {
                return Some((
                    pages::remove_rect(r),
                    t("Удалить страницу", "Delete page").to_string(),
                ));
            }
        }
        let lr = self.layers_left();
        let rows = self.layer_rows(panels::max_layer_rows(lr));
        for (i, row) in rows.iter().enumerate() {
            let r = panels::layer_row_rect(lr, i);
            if !r.contains(self.mouse) {
                continue;
            }
            if panels::eye_rect(r).contains(self.mouse) {
                let text = if row.visible {
                    t("Скрыть", "Hide")
                } else {
                    t("Показать", "Show")
                };
                return Some((panels::eye_rect(r), text.to_string()));
            }
            if panels::lock_rect(r).contains(self.mouse) {
                let text = if row.locked {
                    t("Разблокировать", "Unlock")
                } else {
                    t("Заблокировать", "Lock")
                };
                return Some((panels::lock_rect(r), text.to_string()));
            }
        }
        None
    }

    /// Advances the tooltip timing; true while it animates.
    pub(super) fn update_tooltip(&mut self, dt: f32) -> bool {
        let target = self.tip_target();
        self.tooltip
            .update(dt, target.as_ref().map(|(r, s)| (*r, s.as_str())))
    }

    pub(super) fn draw_tooltip(&self, cx: &mut Cx) {
        let bounds = Rect::new(0.0, 0.0, self.win.w, self.win.h);
        self.tooltip.draw(cx, bounds);
    }

    pub(super) fn dismiss_tooltip(&mut self) {
        if self.tooltip.over(self.mouse) {
            self.tooltip.dismiss();
        }
    }
}

fn tab_tip(i: usize) -> &'static str {
    match i {
        0 => t("Сплошной цвет", "Solid color"),
        1 => t("Линейный градиент", "Linear gradient"),
        2 => t("Радиальный градиент", "Radial gradient"),
        3 => t("Угловой градиент", "Angular gradient"),
        4 => t("Ромбовидный градиент", "Diamond gradient"),
        5 => t("Узор", "Pattern"),
        _ => t("Изображение", "Image"),
    }
}

impl Editor {
    /// Tooltips inside the open color picker: the fill type tabs and the icon buttons.
    fn picker_tip(&self) -> Option<(Rect, String)> {
        use super::picker_layout::{TABS, eyedrop_rect, remove_stop_rect, tab_rect};
        let slot = self.picker.as_ref()?.slot;
        let popup = self.popup(slot);
        if !popup.contains(self.mouse) {
            return None;
        }
        let shape = self.picker_shape(slot);
        if shape.tabs {
            for i in 0..TABS {
                let r = tab_rect(popup, shape, i);
                if r.contains(self.mouse) {
                    return Some((r, tab_tip(i).to_string()));
                }
            }
        }
        if shape.pattern || shape.image {
            return None;
        }
        let eye = eyedrop_rect(popup, shape);
        if eye.contains(self.mouse) {
            return Some((eye, t("Пипетка", "Eyedropper").to_string()));
        }
        let rm = remove_stop_rect(popup, shape);
        if shape.grad.is_some() && rm.contains(self.mouse) {
            return Some((rm, t("Удалить точку", "Remove stop").to_string()));
        }
        None
    }
}
