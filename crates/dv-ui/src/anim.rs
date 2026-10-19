//! Minimal animation primitives: critically-tunable spring and easing.

/// Spring that moves `pos` toward `target`, keeping velocity when retargeted.
pub struct Spring {
    pub pos: f32,
    vel: f32,
    pub target: f32,
    stiffness: f32,
    damping: f32,
}

impl Spring {
    pub fn new(value: f32, stiffness: f32, damping: f32) -> Self {
        Self {
            pos: value,
            vel: 0.0,
            target: value,
            stiffness,
            damping,
        }
    }

    pub fn step(&mut self, dt: f32) {
        // Fixed sub-steps keep the integration stable on long frames.
        let n = ((dt / 0.004).ceil() as u32).clamp(1, 16);
        let h = dt / n as f32;
        for _ in 0..n {
            let accel = -self.stiffness * (self.pos - self.target) - self.damping * self.vel;
            self.vel += accel * h;
            self.pos += self.vel * h;
        }
    }

    pub fn settled(&self) -> bool {
        (self.pos - self.target).abs() < 0.0005 && self.vel.abs() < 0.0005
    }
}

pub fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

/// Progress of an animation that starts at `delay` and lasts `duration` seconds.
pub fn progress(t: f32, delay: f32, duration: f32) -> f32 {
    ((t - delay) / duration).clamp(0.0, 1.0)
}
