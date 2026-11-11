//! 插曲：掉落方块（占位，待实现）
use super::*;

pub struct Blocks {
    t: f32,
}

impl Blocks {
    pub fn new(_app: &mut App) -> Self {
        Blocks { t: 0.0 }
    }
}

impl Interlude for Blocks {
    fn name(&self) -> &'static str {
        "掉落方块"
    }
    fn update(&mut self, _app: &mut App, dt: f32) -> bool {
        self.t += dt;
        self.t > 1.0
    }
    fn draw(&mut self, app: &mut App) {
        app.draw_logs(9.0, 0, 0.0);
    }
}
