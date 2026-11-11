//! 插曲：珠链（占位，待实现）
use super::*;

pub struct Chain {
    t: f32,
}

impl Chain {
    pub fn new(_app: &mut App) -> Self {
        Chain { t: 0.0 }
    }
}

impl Interlude for Chain {
    fn name(&self) -> &'static str {
        "珠链"
    }
    fn update(&mut self, _app: &mut App, dt: f32) -> bool {
        self.t += dt;
        self.t > 1.0
    }
    fn draw(&mut self, app: &mut App) {
        app.draw_logs(9.0, 0, 0.0);
    }
}
