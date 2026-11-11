//! 插曲：字符雨（占位，待实现）
use super::*;

pub struct Rain {
    t: f32,
}

impl Rain {
    pub fn new(_app: &mut App) -> Self {
        Rain { t: 0.0 }
    }
}

impl Interlude for Rain {
    fn name(&self) -> &'static str {
        "字符雨"
    }
    fn update(&mut self, _app: &mut App, dt: f32) -> bool {
        self.t += dt;
        self.t > 1.0
    }
    fn draw(&mut self, app: &mut App) {
        app.draw_logs(9.0, 0, 0.0);
    }
}
