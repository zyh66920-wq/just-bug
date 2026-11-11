//! 插曲：扑腾鸭（占位，待实现）
use super::*;

pub struct Flap {
    t: f32,
}

impl Flap {
    pub fn new(_app: &mut App) -> Self {
        Flap { t: 0.0 }
    }
}

impl Interlude for Flap {
    fn name(&self) -> &'static str {
        "扑腾鸭"
    }
    fn update(&mut self, _app: &mut App, dt: f32) -> bool {
        self.t += dt;
        self.t > 1.0
    }
    fn draw(&mut self, app: &mut App) {
        app.draw_logs(9.0, 0, 0.0);
    }
}
