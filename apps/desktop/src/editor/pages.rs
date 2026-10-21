//! The pages list of the left panel: switching, adding, renaming and removing pages.

use dv_core::{Command, NodeId, Page};
use dv_render::Viewport;
use dv_ui::draw::Cx;
use dv_ui::icons::Icon;
use dv_ui::theme::colors::*;
use dv_ui::theme::metrics::*;
use kurbo::Rect;

use super::rename::Rename;
use super::state::Editor;
use crate::i18n::{Lang, lang};

/// Most page rows shown; the layers list moves down by the rows in use.
pub const MAX_ROWS: usize = 8;
/// Top of the first page row, from the top of the left panel.
const ROWS_TOP: f64 = 108.0;
const MAX_CHARS: usize = 64;

/// Rows of the list for `pages` pages.
pub fn rows_for(pages: usize) -> usize {
    pages.clamp(1, MAX_ROWS)
}

/// The panel rectangle the layers list is laid out in: the left panel without the extra
/// space the page rows use.
pub fn layers_area(left: Rect, pages: usize) -> Rect {
    Rect::new(
        left.x0,
        left.y0 + (rows_for(pages) - 1) as f64 * ROW_H,
        left.x1,
        left.y1,
    )
}

pub fn row_rect(left: Rect, i: usize) -> Rect {
    let y = left.y0 + ROWS_TOP + i as f64 * ROW_H;
    Rect::new(left.x0 + 8.0, y, left.x1 - 9.0, y + ROW_H)
}

/// The plus button in the section header.
pub fn add_rect(left: Rect) -> Rect {
    Rect::new(
        left.x1 - 36.0,
        left.y0 + 82.0,
        left.x1 - 10.0,
        left.y0 + 106.0,
    )
}

/// The remove button at the end of a row.
pub fn remove_rect(row: Rect) -> Rect {
    Rect::new(row.x1 - 28.0, row.y0, row.x1, row.y1)
}

/// A name for a new page: "Page N" with the first free number.
pub fn new_name(existing: &[Page]) -> String {
    let word = if lang() == Lang::Ru {
        "Страница"
    } else {
        "Page"
    };
    let mut n = existing.len() + 1;
    while existing.iter().any(|p| p.name == format!("{word} {n}")) {
        n += 1;
    }
    format!("{word} {n}")
}

impl Editor {
    pub(super) fn page_count(&self) -> usize {
        self.session.borrow().exec.doc().pages().len()
    }

    /// The left panel as seen by the layers list (below the page rows).
    pub(super) fn layers_left(&self) -> Rect {
        layers_area(self.layout().left, self.page_count())
    }

    /// Follows the document's current page: clears what belonged to the old page and swaps
    /// the view. Runs after undo and redo too, which can switch pages.
    pub(super) fn sync_page(&mut self) {
        let cur = self.session.borrow().exec.doc().current_page();
        if cur == self.shown_page {
            return;
        }
        {
            let mut s = self.session.borrow_mut();
            let old = self.shown_page;
            if s.views.len() <= old.max(cur) {
                s.views.resize(old.max(cur) + 1, None);
            }
            let vp = s.vp;
            s.views[old] = Some(vp);
            match s.views[cur] {
                Some(v) => s.vp = v,
                None => {
                    s.vp = Viewport::default();
                    s.centered = false;
                }
            }
        }
        self.shown_page = cur;
        self.selection.clear();
        self.hover = None;
        self.interaction = None;
        self.rename = None;
        self.paint_edit = None;
        self.dropdown = None;
        self.close_picker();
    }

    pub(super) fn switch_page(&mut self, i: usize) {
        self.session.borrow_mut().exec.set_page(i);
        self.sync_page();
    }

    pub(super) fn add_page(&mut self) {
        let mut s = self.session.borrow_mut();
        let at = s.exec.doc().pages().len();
        let name = new_name(s.exec.doc().pages());
        let cmd = Command::InsertPage {
            index: at,
            page: Box::new(Page::new(name)),
        };
        let _ = s.exec.execute("Add page", cmd);
        drop(s);
        self.sync_page();
    }

    fn remove_page(&mut self, i: usize) {
        let _ = self
            .session
            .borrow_mut()
            .exec
            .execute("Delete page", Command::RemovePage { index: i });
        // Page indices shifted: forget the saved views of the pages after it.
        self.session.borrow_mut().views.clear();
        self.shown_page = usize::MAX;
        self.sync_page();
    }

    fn start_page_rename(&mut self, i: usize, text_x: f64) {
        let name = self.session.borrow().exec.doc().pages()[i].name.clone();
        let mut input =
            dv_ui::widgets::text_input::TextInput::new(MAX_CHARS, false).with_font_size(FONT_UI);
        input.insert(&name);
        input.select_all();
        input.focused = true;
        self.rename = Some(Rename {
            id: NodeId(0),
            page: Some(i),
            input,
            text_x,
        });
    }

    /// A press inside the left panel; true when it hit the pages section.
    pub(super) fn press_pages(&mut self) -> bool {
        let left = self.layout().left;
        if add_rect(left).contains(self.mouse) {
            self.add_page();
            return true;
        }
        let n = self.page_count();
        for i in 0..rows_for(n).min(n) {
            let r = row_rect(left, i);
            if !r.contains(self.mouse) {
                continue;
            }
            if n > 1 && remove_rect(r).contains(self.mouse) {
                self.remove_page(i);
            } else if self.is_page_double_click(i) {
                self.switch_page(i);
                self.start_page_rename(i, r.x0 + 12.0);
            } else {
                self.switch_page(i);
            }
            return true;
        }
        false
    }

    fn is_page_double_click(&mut self, i: usize) -> bool {
        // Pages share the double-click memory of layers through a sentinel id per page.
        self.is_row_double_click(NodeId(u64::MAX - i as u64))
    }
}

pub fn draw(cx: &mut Cx, left: Rect, ed: &Editor) {
    let s = ed.session.borrow();
    let doc = s.exec.doc();
    let pages = doc.pages();
    let cur = doc.current_page();
    let add = add_rect(left);
    let a = if add.contains(ed.mouse) { 1.0 } else { 0.7 };
    cx.icon(
        Icon::Plus,
        add.x0 + 6.0,
        add.y0 + 5.0,
        14.0,
        alpha(text_secondary(), a),
    );
    for (i, page) in pages.iter().enumerate().take(MAX_ROWS) {
        let r = row_rect(left, i);
        if i == cur {
            cx.fill(r, R_BUTTON, alpha(accent(), 0.16));
        } else if r.contains(ed.mouse) {
            cx.fill(r, R_BUTTON, alpha(overlay(), 0.06));
        }
        let color = if i == cur {
            hex(text_primary())
        } else {
            hex(text_secondary())
        };
        if let Some(rn) = ed.rename.as_ref().filter(|rn| rn.page == Some(i)) {
            let boxr = Rect::new(r.x0 + 6.0, r.y0 + 3.0, r.x1 - 6.0, r.y1 - 3.0);
            cx.fill(boxr, R_BUTTON, hex(bg_raised()));
            cx.stroke(boxr, R_BUTTON, 1.5, hex(accent()));
            rn.input.draw_inline(cx, boxr, r.x0 + 12.0, r.y0 + 18.0);
            continue;
        }
        let room = if pages.len() > 1 { 44.0 } else { 24.0 };
        let name = cx.fit(&page.name, FONT_UI, r.width() - room);
        cx.text(&name, FONT_UI, r.x0 + 12.0, r.y0 + 18.0, color);
        if pages.len() > 1 && r.contains(ed.mouse) {
            let x = remove_rect(r);
            cx.icon(
                Icon::Close,
                x.x0 + 8.0,
                r.y0 + 7.0,
                12.0,
                alpha(text_secondary(), 0.7),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layers_move_down_with_the_page_rows() {
        let left = Rect::new(0.0, 48.0, 240.0, 700.0);
        assert_eq!(layers_area(left, 1).y0, left.y0);
        assert_eq!(layers_area(left, 3).y0, left.y0 + 2.0 * ROW_H);
        assert_eq!(
            layers_area(left, 99).y0,
            left.y0 + (MAX_ROWS - 1) as f64 * ROW_H
        );
    }

    #[test]
    fn new_pages_get_the_first_free_number() {
        let pages = vec![Page::new("Page 1"), Page::new("Page 3")];
        let name = new_name(&pages);
        assert!(name.ends_with(" 4") || name.ends_with(" 3") || name.ends_with(" 2"));
        assert!(!pages.iter().any(|p| p.name == name));
    }
}
