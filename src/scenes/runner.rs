//! 插曲：鸭鸭跑酷（占位，待实现）
use super::*;

pub struct Runner {
    t: f32,
}

impl Runner {
    pub fn new(_app: &mut App) -> Self {
        Runner { t: 0.0 }
    }
}

impl Interlude for Runner {
    fn name(&self) -> &'static str {
        "鸭鸭跑酷"
    }
    fn update(&mut self, _app: &mut App, dt: f32) -> bool {
        self.t += dt;
        self.t > 1.0
    }
    fn draw(&mut self, app: &mut App) {
        app.draw_logs(9.0, 0, 0.0);
    }
}
