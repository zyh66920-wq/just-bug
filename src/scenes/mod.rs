//! 插曲：日常之后随机插一段 15–25 秒的「经典」——经典屏保效果，或鸭鸭和 Bug 自动在玩的经典小游戏。
//!
//! 约定（每个场景都要遵守）：
//! - 只借玩法，不借名字/角色/美术/配色/音效；画面是终端字符 + 我们自己的像素鸭和飞蛾，结尾要有鸭鸭×Bug 的梗。
//! - 坐标：字符格 (x, y) 用 `app.scr`；像素层 `app.layer` 是半格像素，(x, py) 中 py = 行 × 2（上下两个像素一格）。
//! - 彩虹优先用 `rb(Band::Text, x, y)`（调色板色，终端自己流动，几乎不占带宽）；逐帧变化的真彩色只用在 `app.hi` 时。
//! - 性能：每帧只有变化的格子会写到终端。别每帧全屏换色/抖动；大片移动的东西注意数量。`app.hi == false`（省流量档）
//!   时要更克制：减少同时在动的东西、不要逐帧变色。
//! - 插曲期间 App 不再自动驱动飞蛾（app.moths 不会被 update_moths 动），鸭鸭只做 x/y 往 tx/ty 的缓动；
//!   要瞬移就同时设 x 和 tx。想画大鸭鸭用 app.draw_glow / app.draw_duck(None)，小鸭鸭用 mini_duck。
//! - update 返回 true 表示结束；结束后 App 自动回到「打盹」主线（鸭鸭回到底部、飞蛾保留最多 10 只）。
//! - 可以 push 日志、set_title、quack 冒泡，让插曲和主线的「终端里的故事」接得上。
use crate::*;

mod corner;
mod life;
mod rain;
mod runner;
mod snake;
mod sweeper;
mod warp;

pub trait Interlude {
    /// 简短中文名（标题栏、调试输出用）
    fn name(&self) -> &'static str;
    /// 每帧逻辑；返回 true 表示插曲结束
    fn update(&mut self, app: &mut App, dt: f32) -> bool;
    /// 画在字符层和像素层（像素层会在之后统一合成到字符层上面）
    fn draw(&mut self, app: &mut App);
    /// 合成像素层之后再画的字符（盖在像素上面的文字等）
    fn draw_over(&mut self, _app: &mut App) {}
}

pub const COUNT: usize = 7;

pub fn make(i: usize, app: &mut App) -> Box<dyn Interlude> {
    match i {
        0 => Box::new(rain::Rain::new(app)),
        1 => Box::new(warp::Warp::new(app)),
        2 => Box::new(life::Life::new(app)),
        3 => Box::new(corner::Corner::new(app)),
        4 => Box::new(sweeper::Sweeper::new(app)),
        5 => Box::new(snake::Snake::new(app)),
        _ => Box::new(runner::Runner::new(app)),
    }
}

// ───────────────────────── 共用小工具 ─────────────────────────

/// 小鸭鸭：10×7 像素（5 行字符高），头在右上。'#' 彩虹身子，'e' 眼睛，'B' 嘴
pub const MINI_DUCK: [&str; 7] = [
    ".....##...",
    "....#e##BB",
    "....####B.",
    "#...###...",
    "########..",
    ".#######..",
    "..#####...",
];

/// 在像素层画小鸭鸭。(cx, cy) 是中心像素坐标；k 是放大倍数（1 或 2）。自带深色描边。
pub fn mini_duck(app: &mut App, cx: f32, cy: f32, right: bool, k: i32) {
    let (gw, gh) = (10 * k + 2, 7 * k + 2);
    let ox = cx.round() as i32 - 5 * k - 1;
    let oy = cy.round() as i32 - (7 * k) / 2 - 1;
    let hi = app.hi;
    let tq = (app.t * 14.0).floor() / 10.0;
    let mut buf: Vec<Option<Col>> = vec![None; (gw * gh) as usize];
    for (j, row) in MINI_DUCK.iter().enumerate() {
        for (i, ch) in row.chars().enumerate() {
            if ch == '.' {
                continue;
            }
            let ii = if right { i as i32 } else { 9 - i as i32 };
            for sy in 0..k {
                for sx in 0..k {
                    let (px, py) = (ii * k + sx + 1, j as i32 * k + sy + 1);
                    let c: Col = match ch {
                        'e' => [12, 10, 20].into(),
                        'B' => [255, 160, 40].into(),
                        _ if hi => mix(lolcat((ox + px) as f32 * 0.8, (oy + py) as f32 * 0.4, tq), WHITE, 0.25).into(),
                        _ => rb(Band::Duck, px as f32 * 1.6 / k as f32, py as f32 * 0.8 / k as f32),
                    };
                    buf[(py * gw + px) as usize] = Some(c);
                }
            }
        }
    }
    for y in 0..gh {
        for x in 0..gw {
            let c = buf[(y * gw + x) as usize].or_else(|| {
                let near = [(-1, 0), (1, 0), (0, -1), (0, 1)].iter().any(|(dx, dy)| {
                    let (nx, ny) = (x + dx, y + dy);
                    nx >= 0 && ny >= 0 && nx < gw && ny < gh && buf[(ny * gw + nx) as usize].is_some()
                });
                near.then(|| Col::T([14, 12, 24]))
            });
            if let Some(c) = c {
                app.layer.set(ox + x, oy + y, c);
            }
        }
    }
}

/// 在像素层画一只飞蛾（5×3 像素）。shade 0..=4：0/1 白、2 浅灰、3 深灰、4 黑翅白身；flap 取整的奇偶决定翅膀帧。
/// rainbow 0..1：被染成彩虹的程度。
pub fn moth(app: &mut App, x: f32, y: f32, shade: u8, flap: f32, rainbow: f32) {
    let mut wing = match shade {
        0 | 1 => [238, 238, 238],
        2 => [175, 175, 175],
        3 => [120, 120, 120],
        _ => [22, 22, 22],
    };
    let mut body = if shade == 4 { [235, 235, 235] } else { scale(wing, 0.4) };
    if rainbow > 0.0 {
        let c = lolcat(x, y / 2.0, app.pal_t());
        wing = mix(wing, c, rainbow);
        body = mix(body, scale(c, 0.5), rainbow);
    }
    let fr = &MOTH[((flap as i32) & 1) as usize];
    let (ox, oy) = (x.round() as i32 - 2, y.round() as i32 - 1);
    for (j, row) in fr.iter().enumerate() {
        for (k, ch) in row.chars().enumerate() {
            let c = match ch {
                '#' => wing,
                'o' => body,
                _ => continue,
            };
            app.layer.set(ox + k as i32, oy + j as i32, c);
        }
    }
}

/// 字符层画一个带标题的窗口框（单线框），框内填 bg。返回内区 (x0, y0, x1, y1)，含边界内侧。
pub fn window(app: &mut App, x0: i32, y0: i32, x1: i32, y1: i32, title: &str, fg: Rgb, bg: Rgb) -> (i32, i32, i32, i32) {
    for y in y0..=y1 {
        for x in x0..=x1 {
            let ch = match (x == x0, x == x1, y == y0, y == y1) {
                (true, _, true, _) => '┌',
                (_, true, true, _) => '┐',
                (true, _, _, true) => '└',
                (_, true, _, true) => '┘',
                (_, _, true, _) | (_, _, _, true) => '─',
                (true, _, _, _) | (_, true, _, _) => '│',
                _ => ' ',
            };
            app.scr.put(x, y, ch, fg, Some(bg));
        }
    }
    if !title.is_empty() {
        let t = format!(" {} ", title);
        app.scr.text(x0 + 2, y0, &t, fg, Some(bg));
    }
    (x0 + 1, y0 + 1, x1 - 1, y1 - 1)
}

/// 清一块字符区域为纯色背景
#[allow(dead_code)]
pub fn fill(app: &mut App, x0: i32, y0: i32, x1: i32, y1: i32, bg: Rgb) {
    for y in y0..=y1 {
        for x in x0..=x1 {
            app.scr.put(x, y, ' ', bg, Some(bg));
        }
    }
}
