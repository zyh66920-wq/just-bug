//! 插曲：生命游戏（占位，待实现）
use super::*;

pub struct Life {
    t: f32,
}

impl Life {
    pub fn new(_app: &mut App) -> Self {
        Life { t: 0.0 }
    }
}

impl Interlude for Life {
    fn name(&self) -> &'static str {
        "生命游戏"
    }
    fn update(&mut self, _app: &mut App, dt: f32) -> bool {
        self.t += dt;
        self.t > 1.0
    }
    fn draw(&mut self, app: &mut App) {
        app.draw_logs(9.0, 0, 0.0);
    }
}
