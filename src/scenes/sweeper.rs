//! 插曲：扫 Bug（占位，待实现）
use super::*;

pub struct Sweeper {
    t: f32,
}

impl Sweeper {
    pub fn new(_app: &mut App) -> Self {
        Sweeper { t: 0.0 }
    }
}

impl Interlude for Sweeper {
    fn name(&self) -> &'static str {
        "扫 Bug"
    }
    fn update(&mut self, _app: &mut App, dt: f32) -> bool {
        self.t += dt;
        self.t > 1.0
    }
    fn draw(&mut self, app: &mut App) {
        app.draw_logs(9.0, 0, 0.0);
    }
}
