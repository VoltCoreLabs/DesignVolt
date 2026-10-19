//! Editor screen state: viewport, pointer, intro animation and frame drawing.

use vello::Scene;
use vello::kurbo::{Affine, Rect};

use super::layout::Layout;
use super::toolbar::Toolbar;
use super::{canvas, panels};
use crate::anim::{ease_out_cubic, progress};
use crate::draw::Cx;
use crate::icons::Icons;
use crate::text::Font;
use crate::theme::colors::{BG_APP, hex};
use crate::theme::motion::{PANEL_INTRO, STAGGER};

/// Time after which the intro animation is over.
const INTRO_END: f32 = PANEL_INTRO + 5.0 * STAGGER + 0.1;

pub struct Editor {
    font: Font,
    icons: Icons,
    toolbar: Toolbar,
    time: f32,
    scale: f64,
    size: (f64, f64),
    mouse: (f64, f64),
}

impl Editor {
    pub fn new(font: Font) -> Self {
        Self {
            font,
            icons: Icons::new(),
            toolbar: Toolbar::new(),
            time: 0.0,
            scale: 1.0,
            size: (960.0, 640.0),
            mouse: (-1.0, -1.0),
        }
    }

    /// `w`/`h` are physical pixels.
    pub fn set_viewport(&mut self, w: u32, h: u32, scale: f64) {
        self.scale = scale;
        self.size = (w as f64 / scale, h as f64 / scale);
    }

    /// `x`/`y` are physical pixels.
    pub fn mouse_moved(&mut self, x: f64, y: f64) {
        self.mouse = (x / self.scale, y / self.scale);
    }

    pub fn mouse_left(&mut self) {
        self.mouse = (-1.0, -1.0);
    }

    fn layout(&self) -> Layout {
        Layout::new(self.size.0, self.size.1)
    }

    pub fn hovering_button(&self) -> bool {
        self.toolbar.hit(&self.layout(), self.mouse.0, self.mouse.1).is_some()
    }

    pub fn click(&mut self) {
        if let Some(i) = self.toolbar.hit(&self.layout(), self.mouse.0, self.mouse.1) {
            self.toolbar.select(i);
        }
    }

    /// Advances animations; returns true while another frame is needed.
    pub fn update(&mut self, dt: f32) -> bool {
        self.time += dt;
        let hovered = self.toolbar.hit(&self.layout(), self.mouse.0, self.mouse.1);
        let busy = self.toolbar.update(dt, hovered);
        busy || self.time < INTRO_END
    }

    /// Intro progress (0..=1) for an element with the given stagger index.
    fn intro(&self, index: f32) -> f32 {
        ease_out_cubic(progress(self.time, index * STAGGER, PANEL_INTRO))
    }

    pub fn draw(&self, scene: &mut Scene) {
        scene.reset();
        let layout = self.layout();
        let mut cx = Cx {
            scene,
            font: &self.font,
            icons: &self.icons,
            xf: Affine::scale(self.scale),
        };
        let full = Rect::new(0.0, 0.0, self.size.0, self.size.1);
        cx.fill(full, 0.0, hex(BG_APP));

        let p = self.intro(0.0);
        cx.layer(layout.canvas, p, (0.0, 0.0), |cx| canvas::draw(cx, layout.canvas));

        let p = self.intro(0.0);
        cx.layer(layout.topbar, p, (0.0, -16.0 * (1.0 - p as f64)), |cx| {
            panels::topbar(cx, layout.topbar)
        });
        let p = self.intro(1.0);
        cx.layer(layout.left, p, (-24.0 * (1.0 - p as f64), 0.0), |cx| {
            panels::left_panel(cx, layout.left)
        });
        let p = self.intro(2.0);
        cx.layer(layout.right, p, (24.0 * (1.0 - p as f64), 0.0), |cx| {
            panels::right_panel(cx, layout.right)
        });
        let p = self.intro(4.0);
        let clip = layout.toolbar.inflate(40.0, 40.0);
        cx.layer(clip, p, (0.0, 24.0 * (1.0 - p as f64)), |cx| {
            self.toolbar.draw(cx, &layout)
        });
    }
}
