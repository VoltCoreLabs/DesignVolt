//! The canvas view (zoom and pan of every page, and the page being edited) saved next to the
//! document, so a project opens where it was left.
//!
//! Text format: first line `page<TAB><index>`, then one `view<TAB><index><TAB><zoom><TAB><x><TAB><y>`
//! line per page that has a saved view.

use dv_render::Viewport;
use kurbo::Vec2;

pub struct SavedView {
    pub page: usize,
    pub views: Vec<Option<Viewport>>,
}

/// `views[page]` is replaced by `current` for the page being edited.
pub fn to_text(page: usize, views: &[Option<Viewport>], current: Viewport) -> String {
    let mut out = format!("page\t{page}\n");
    let n = views.len().max(page + 1);
    for i in 0..n {
        let v = if i == page {
            Some(current)
        } else {
            views.get(i).copied().flatten()
        };
        if let Some(v) = v {
            out.push_str(&format!(
                "view\t{i}\t{}\t{}\t{}\n",
                v.zoom, v.pan.x, v.pan.y
            ));
        }
    }
    out
}

/// Reads a saved view; garbage lines are skipped, so a damaged file only loses the view.
pub fn from_text(text: &str) -> SavedView {
    let mut saved = SavedView {
        page: 0,
        views: Vec::new(),
    };
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        match f.as_slice() {
            ["page", p] => saved.page = p.parse().unwrap_or(0),
            ["view", i, z, x, y] => {
                let (Ok(i), Ok(zoom), Ok(x), Ok(y)) = (
                    i.parse::<usize>(),
                    z.parse::<f64>(),
                    x.parse::<f64>(),
                    y.parse::<f64>(),
                ) else {
                    continue;
                };
                if i > 10_000
                    || !zoom.is_finite()
                    || zoom <= 0.0
                    || !x.is_finite()
                    || !y.is_finite()
                {
                    continue;
                }
                if saved.views.len() <= i {
                    saved.views.resize(i + 1, None);
                }
                saved.views[i] = Some(Viewport {
                    zoom,
                    pan: Vec2::new(x, y),
                });
            }
            _ => {}
        }
    }
    saved
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vp(zoom: f64, x: f64, y: f64) -> Viewport {
        Viewport {
            zoom,
            pan: Vec2::new(x, y),
        }
    }

    #[test]
    fn views_round_trip_and_the_current_one_wins() {
        let views = vec![Some(vp(1.0, 5.0, 6.0)), None, Some(vp(2.0, 0.0, 1.0))];
        let text = to_text(2, &views, vp(0.25, -30.5, 12.0));
        let back = from_text(&text);
        assert_eq!(back.page, 2);
        assert_eq!(back.views[0], Some(vp(1.0, 5.0, 6.0)));
        assert_eq!(back.views[1], None);
        assert_eq!(back.views[2], Some(vp(0.25, -30.5, 12.0)));
    }

    #[test]
    fn damaged_lines_are_skipped() {
        let back =
            from_text("page\tx\nview\t0\tnan\t1\t1\nview\t1\t0\t1\t1\nview\t2\t3\t4\t5\njunk");
        assert_eq!(back.page, 0);
        assert_eq!(back.views[2], Some(vp(3.0, 4.0, 5.0)));
        assert_eq!(back.views[0], None);
        assert_eq!(back.views[1], None);
    }
}
