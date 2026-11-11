//! 插曲：撞角落（占位，待实现）
use super::*;

pub struct Corner {
    t: f32,
}

impl Corner {
    pub fn new(_app: &mut App) -> Self {
        Corner { t: 0.0 }
    }
}

impl Interlude for Corner {
    fn name(&self) -> &'static str {
        "撞角落"
    }
    fn update(&mut self, _app: &mut App, dt: f32) -> bool {
        self.t += dt;
        self.t > 1.0
    }
    fn draw(&mut self, app: &mut App) {
        app.draw_logs(9.0, 0, 0.0);
    }
}
