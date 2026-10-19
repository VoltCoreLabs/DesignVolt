//! Custom window title bar: home button, current location, app name and window controls.

use vello::kurbo::{Point, Rect};

use crate::anim::Spring;
use crate::draw::Cx;
use crate::icons::Icon;
use crate::theme::colors::*;
use crate::theme::metrics::{FONT_MD, FONT_UI, R_BUTTON};
use crate::theme::motion::*;

pub const HEIGHT: f64 = 40.0;
const BTN_W: f64 = 46.0;
const HOME: f64 = 32.0;
const APP_NAME: &str = "DesignVolt";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Part {
    Home,
    Minimize,
    Maximize,
    Close,
    Theme,
    /// Empty area: dragging moves the window, double-click maximizes.
    Drag,
}

/// What the bar shows; supplied by the app each frame.
pub struct BarState<'a> {
    /// Show the home button (hidden on the welcome screen).
    pub home: bool,
    /// Current location, e.g. "All projects" or a project name.
    pub label: Option<&'a str>,
    pub maximized: bool,
}

pub struct TitleBar {
    hover: [Spring; 5],
    label_in: Spring,
    shown_label: Option<String>,
    mouse: Point,
}

struct Rects {
    home: Rect,
    min: Rect,
    max: Rect,
    close: Rect,
    theme: Rect,
}

fn rects(w: f64) -> Rects {
    let btn = |i: f64| Rect::new(w - BTN_W * (i + 1.0), 0.0, w - BTN_W * i, HEIGHT);
    Rects {
        home: Rect::new(8.0, (HEIGHT - HOME) / 2.0, 8.0 + HOME, (HEIGHT + HOME) / 2.0),
        close: btn(0.0),
        max: btn(1.0),
        min: btn(2.0),
        theme: btn(3.0),
    }
}

impl TitleBar {
    pub fn new() -> Self {
        Self {
            hover: std::array::from_fn(|_| Spring::new(0.0, HOVER_STIFFNESS, HOVER_DAMPING)),
            label_in: Spring::new(0.0, MOVE_STIFFNESS, MOVE_DAMPING),
            shown_label: None,
            mouse: Point::new(-1.0, -1.0),
        }
    }

    pub fn mouse_moved(&mut self, x: f64, y: f64) {
        self.mouse = Point::new(x, y);
    }

    pub fn mouse_left(&mut self) {
        self.mouse = Point::new(-1.0, -1.0);
    }

    /// Part under the pointer, or `None` when the pointer is below the bar.
    pub fn hit(&self, w: f64, home_visible: bool) -> Option<Part> {
        let p = self.mouse;
        if p.y < 0.0 || p.y >= HEIGHT || p.x < 0.0 || p.x >= w {
            return None;
        }
        let r = rects(w);
        Some(if r.close.contains(p) {
            Part::Close
        } else if r.max.contains(p) {
            Part::Maximize
        } else if r.theme.contains(p) {
            Part::Theme
        } else if r.min.contains(p) {
            Part::Minimize
        } else if home_visible && r.home.contains(p) {
            Part::Home
        } else {
            Part::Drag
        })
    }

    /// Returns true while an animation is running.
    pub fn update(&mut self, dt: f32, w: f64, state: &BarState) -> bool {
        let hot = self.hit(w, state.home);
        for (i, part) in [Part::Home, Part::Minimize, Part::Maximize, Part::Close, Part::Theme].iter().enumerate() {
            self.hover[i].target = if hot == Some(*part) { 1.0 } else { 0.0 };
            self.hover[i].step(dt);
        }
        if self.shown_label.as_deref() != state.label {
            self.shown_label = state.label.map(str::to_string);
            self.label_in.pos = 0.0;
        }
        self.label_in.target = 1.0;
        self.label_in.step(dt);
        self.hover.iter().any(|h| !h.settled()) || !self.label_in.settled()
    }

    pub fn draw(&self, cx: &mut Cx, w: f64, state: &BarState) {
        let bar = Rect::new(0.0, 0.0, w, HEIGHT);
        cx.fill(bar, 0.0, hex(bg_panel()));
        cx.fill(Rect::new(0.0, HEIGHT - 1.0, w, HEIGHT), 0.0, alpha(overlay(), 0.08));
        let r = rects(w);
        let h = |i: usize| self.hover[i].pos.clamp(0.0, 1.0);

        // Window controls.
        let max_icon = if state.maximized { Icon::Restore } else { Icon::Square };
        for (i, (rect, icon)) in [(r.min, Icon::Minimize), (r.max, max_icon)].iter().enumerate() {
            cx.fill(*rect, 0.0, alpha(bg_hover(), h(i + 1)));
            let c = rect.center();
            let col = mix_color(text_secondary(), text_primary(), h(i + 1));
            cx.icon(*icon, c.x - 7.0, c.y - 7.0, 14.0, col);
        }
        // Theme toggle: shows where a click leads (moon in light theme, sun in dark).
        cx.fill(r.theme, 0.0, alpha(bg_hover(), h(4)));
        let tc = r.theme.center();
        let theme_icon = if is_dark() { Icon::Sun } else { Icon::Moon };
        cx.icon(theme_icon, tc.x - 8.0, tc.y - 8.0, 16.0, mix_color(text_secondary(), text_primary(), h(4)));
        cx.fill(r.close, 0.0, alpha(danger(), h(3)));
        let c = r.close.center();
        cx.icon(Icon::Close, c.x - 7.0, c.y - 7.0, 14.0, mix_color(text_secondary(), WHITE, h(3)));

        // Center: logo mark + app name.
        let name_w = cx.text_width(APP_NAME, FONT_MD);
        let x = w / 2.0 - (name_w + 24.0) / 2.0;
        cx.fill_grad(Rect::new(x, 12.0, x + 16.0, 28.0), 5.0, hex(accent_from()), hex(accent_to()));
        cx.text(APP_NAME, FONT_MD, x + 24.0, 25.0, hex(text_primary()));

        // Left: home button and location.
        if !state.home {
            return;
        }
        cx.fill(r.home, R_BUTTON, alpha(bg_hover(), h(0)));
        let hc = r.home.center();
        cx.icon(Icon::Home, hc.x - 9.0, hc.y - 9.0, 18.0, mix_color(text_secondary(), text_primary(), h(0)));

        let Some(label) = state.label else { return };
        let t = self.label_in.pos.clamp(0.0, 1.0);
        let max_w = (w / 2.0 - name_w / 2.0 - 24.0 - (r.home.x1 + 28.0)).max(40.0);
        let text = fit(cx, label, FONT_UI, max_w - 24.0);
        let tw = cx.text_width(&text, FONT_UI);
        let chip = Rect::new(r.home.x1 + 8.0, 7.0, r.home.x1 + 8.0 + tw + 24.0, HEIGHT - 7.0);
        cx.layer(chip.inflate(4.0, 4.0), t, (-8.0 * (1.0 - t as f64), 0.0), |cx| {
            cx.fill(chip, R_BUTTON, alpha(accent(), 0.16));
            cx.stroke(chip, R_BUTTON, 1.0, alpha(accent(), 0.35));
            cx.text(&text, FONT_UI, chip.x0 + 12.0, chip.center().y + 4.3, hex(text_primary()));
        });
    }
}

/// Cuts `s` with an ellipsis so that it fits in `max_w`.
fn fit(cx: &Cx, s: &str, size: f32, max_w: f64) -> String {
    if cx.text_width(s, size) <= max_w {
        return s.to_string();
    }
    let mut out: String = s.to_string();
    while out.pop().is_some() {
        let candidate = format!("{}…", out.trim_end());
        if cx.text_width(&candidate, size) <= max_w {
            return candidate;
        }
    }
    String::new()
}

fn mix_color(a: u32, b: u32, t: f32) -> vello::peniko::Color {
    crate::draw::mix(a, b, t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar_at(x: f64, y: f64) -> TitleBar {
        let mut b = TitleBar::new();
        b.mouse_moved(x, y);
        b
    }

    #[test]
    fn hit_testing_finds_window_controls_on_the_right() {
        let w = 1000.0;
        assert_eq!(bar_at(w - 10.0, 20.0).hit(w, true), Some(Part::Close));
        assert_eq!(bar_at(w - BTN_W - 10.0, 20.0).hit(w, true), Some(Part::Maximize));
        assert_eq!(bar_at(w - 2.0 * BTN_W - 10.0, 20.0).hit(w, true), Some(Part::Minimize));
    }

    #[test]
    fn home_is_hit_only_when_visible() {
        let w = 1000.0;
        assert_eq!(bar_at(20.0, 20.0).hit(w, true), Some(Part::Home));
        assert_eq!(bar_at(20.0, 20.0).hit(w, false), Some(Part::Drag));
    }

    #[test]
    fn empty_area_drags_and_outside_is_none() {
        let w = 1000.0;
        assert_eq!(bar_at(500.0, 20.0).hit(w, true), Some(Part::Drag));
        assert_eq!(bar_at(500.0, HEIGHT + 1.0).hit(w, true), None);
        assert_eq!(bar_at(-1.0, -1.0).hit(w, true), None);
    }
}
