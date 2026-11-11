//! 插曲：星空跃迁（占位，待实现）
use super::*;

pub struct Warp {
    t: f32,
}

impl Warp {
    pub fn new(_app: &mut App) -> Self {
        Warp { t: 0.0 }
    }
}

impl Interlude for Warp {
    fn name(&self) -> &'static str {
        "星空跃迁"
    }
    fn update(&mut self, _app: &mut App, dt: f32) -> bool {
        self.t += dt;
        self.t > 1.0
    }
    fn draw(&mut self, app: &mut App) {
        app.draw_logs(9.0, 0, 0.0);
    }
}
