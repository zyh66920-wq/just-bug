//! 插曲：弹弓（占位，待实现）
use super::*;

pub struct Sling {
    t: f32,
}

impl Sling {
    pub fn new(_app: &mut App) -> Self {
        Sling { t: 0.0 }
    }
}

impl Interlude for Sling {
    fn name(&self) -> &'static str {
        "弹弓"
    }
    fn update(&mut self, _app: &mut App, dt: f32) -> bool {
        self.t += dt;
        self.t > 1.0
    }
    fn draw(&mut self, app: &mut App) {
        app.draw_logs(9.0, 0, 0.0);
    }
}
