//! Editor screen state: viewport, pointer, intro animation and frame drawing.

use vello::Scene;
use vello::kurbo::{Affine, Rect};

use super::layout::Layout;
use super::toolbar::Toolbar;
use super::{canvas, panels};
use crate::anim::{ease_out_cubic, progress};
use crate::draw::Cx;
use crate::input::{Key, Mods, Screen, SharedAssets, Transition, Viewport};
use crate::theme::colors::{bg_app, hex};
use crate::theme::motion::{PANEL_INTRO, STAGGER};

/// Time after which the intro animation is over.
const INTRO_END: f32 = PANEL_INTRO + 5.0 * STAGGER + 0.1;

pub struct Editor {
    assets: SharedAssets,
    toolbar: Toolbar,
    time: f32,
    vp: Viewport,
    mouse: (f64, f64),
    next: Option<Transition>,
}

impl Editor {
    pub fn new(assets: SharedAssets) -> Self {
        Self {
            assets,
            toolbar: Toolbar::new(),
            time: 0.0,
            vp: Viewport::new(),
            mouse: (-1.0, -1.0),
            next: None,
        }
    }

    fn layout(&self) -> Layout {
        Layout::new(self.vp.w, self.vp.h)
    }

    /// Intro progress (0..=1) for an element with the given stagger index.
    fn intro(&self, index: f32) -> f32 {
        ease_out_cubic(progress(self.time, index * STAGGER, PANEL_INTRO))
    }
}

impl Screen for Editor {
    fn set_viewport(&mut self, w: u32, h: u32, scale: f64) {
        self.vp.set(w, h, scale);
    }

    fn update(&mut self, dt: f32) -> bool {
        self.time += dt;
        let hovered = self.toolbar.hit(&self.layout(), self.mouse.0, self.mouse.1);
        let busy = self.toolbar.update(dt, hovered);
        busy || self.time < INTRO_END
    }

    fn mouse_moved(&mut self, x: f64, y: f64) {
        self.mouse = (x / self.vp.scale, y / self.vp.scale);
    }

    fn mouse_left(&mut self) {
        self.mouse = (-1.0, -1.0);
    }

    fn click(&mut self, _mods: Mods) {
        if let Some(i) = self.toolbar.hit(&self.layout(), self.mouse.0, self.mouse.1) {
            self.toolbar.select(i);
        }
    }

    fn key(&mut self, key: Key, _mods: Mods) {
        if key == Key::Escape {
            self.next = Some(Transition::BackToProjects);
        }
    }

    fn pointer_hand(&self) -> bool {
        self.toolbar
            .hit(&self.layout(), self.mouse.0, self.mouse.1)
            .is_some()
    }

    fn take_transition(&mut self) -> Option<Transition> {
        self.next.take()
    }

    fn draw(&self, scene: &mut Scene) {
        scene.reset();
        let layout = self.layout();
        let mut cx = Cx {
            scene,
            font: &self.assets.font,
            icons: &self.assets.icons,
            xf: Affine::scale(self.vp.scale),
        };
        let full = Rect::new(0.0, 0.0, self.vp.w, self.vp.h);
        cx.fill(full, 0.0, hex(bg_app()));

        let p = self.intro(0.0);
        cx.layer(layout.canvas, p, (0.0, 0.0), |cx| {
            canvas::draw(cx, layout.canvas)
        });

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
