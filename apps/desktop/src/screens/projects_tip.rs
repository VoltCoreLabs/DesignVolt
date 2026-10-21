//! Tooltips of the projects screen: the star on a card and the close button of the
//! selection bar.

use vello::kurbo::Rect;

use super::projects::Projects;
use super::projects_input::star_rect;
use crate::i18n::{Lang, lang};
use crate::library::{Item, Location};
use dv_ui::draw::Cx;

fn t(ru: &'static str, en: &'static str) -> &'static str {
    if lang() == Lang::Ru { ru } else { en }
}

impl Projects {
    fn tip_target(&self) -> Option<(Rect, &'static str)> {
        if self.modal.is_some() || self.sort_open || self.drag.is_some() || self.menu.is_some() {
            return None;
        }
        if let Some(bar) = self.bar_layout() {
            return bar
                .close
                .contains(self.mouse)
                .then_some((bar.close, t("Снять выделение", "Clear selection")));
        }
        if self.loc == Location::Trash {
            return None;
        }
        let Some(Item::Project(id)) = self.item_at(self.mouse) else {
            return None;
        };
        let star = star_rect(self.card_of(Item::Project(id))?);
        if !star.contains(self.mouse) {
            return None;
        }
        let favorite = self
            .store
            .borrow()
            .lib
            .projects
            .iter()
            .any(|p| p.id == id && p.favorite);
        let text = if favorite {
            t("Убрать из избранного", "Remove from favorites")
        } else {
            t("В избранное", "Add to favorites")
        };
        Some((star, text))
    }

    pub(super) fn update_tip(&mut self, dt: f32) -> bool {
        let target = self.tip_target();
        self.tip.update(dt, target)
    }

    pub(super) fn paint_tip(&self, cx: &mut Cx) {
        self.tip.draw(cx, Rect::new(0.0, 0.0, self.vp.w, self.vp.h));
    }
}
