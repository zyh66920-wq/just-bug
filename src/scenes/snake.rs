//! 插曲：贪吃 Bug（占位，待实现）
use super::*;

pub struct Snake {
    t: f32,
}

impl Snake {
    pub fn new(_app: &mut App) -> Self {
        Snake { t: 0.0 }
    }
}

impl Interlude for Snake {
    fn name(&self) -> &'static str {
        "贪吃 Bug"
    }
    fn update(&mut self, _app: &mut App, dt: f32) -> bool {
        self.t += dt;
        self.t > 1.0
    }
    fn draw(&mut self, app: &mut App) {
        app.draw_logs(9.0, 0, 0.0);
    }
}
