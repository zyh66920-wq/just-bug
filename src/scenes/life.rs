//! 插曲：生命游戏——「bug 繁殖模拟器 v0.1」
//!
//! 飞蛾落地变成活细胞，按 B3/S23 演化；上方一台「孵化器」（滑翔机枪）不停朝鸭鸭发射 bug。
//! 鸭鸭是一块永远不死、也不参与计数的区域：碰到它的 bug 被染成彩虹，彩虹会遗传。
//! 结尾：鸭鸭全功率发光 → 全体彩虹 → 停止繁殖凝固 → 自然灭绝 → 只剩一只飞来蹲在鸭鸭头上：
//! 「规则要 2 个邻居，它说鸭鸭一个顶俩」。
use super::*;

/// 经典滑翔机枪（周期 30，朝右下方发射）。图样是数学，公有领域
const GUN: [&str; 9] = [
    "........................O...........",
    "......................O.O...........",
    "............OO......OO............OO",
    "...........O...O....OO............OO",
    "OO........O.....O...OO..............",
    "OO........O...O.OO....O.O...........",
    "..........O.....O.......O...........",
    "...........O...O....................",
    "............OO......................",
];
const GUN_W: i32 = 36;
const GUN_H: i32 = 9;

const RAINBOW: u8 = 0x80;

// 时间线（秒，插曲内）
const T_SIM: f32 = 1.0; // 开始演化
const T_INFECT: f32 = 6.5; // 彩虹开始「传染」
const T_WAVE: f32 = 13.6; // 鸭鸭全功率发光
const T_FREEZE: f32 = 15.2; // 停止繁殖
const T_DIE0: f32 = 16.3; // 灭绝开始
const T_DIE1: f32 = 17.6; // 灭绝完成
const T_LONE: f32 = 18.0; // 最后一只飞来
const T_QUACK: f32 = 19.5;
const T_BACK: f32 = 20.4; // 日志淡回
const T_END: f32 = 21.6;

struct Fly {
    sx: f32,
    sy: f32,
    tx: f32,
    ty: f32,
    t0: f32,
    dur: f32,
    shade: u8,
    flap: f32,
    wob: f32,
    done: bool,
    cell: (i32, i32),
}

impl Fly {
    fn pos(&self, t: f32) -> (f32, f32) {
        let u = ((t - self.t0) / self.dur).clamp(0.0, 1.0);
        let e = u * u * (3.0 - 2.0 * u);
        let side = (u * std::f32::consts::PI).sin() * self.wob;
        let (dx, dy) = (self.tx - self.sx, self.ty - self.sy);
        let l = (dx * dx + dy * dy).sqrt().max(1.0);
        (self.sx + dx * e - dy / l * side, self.sy + dy * e + dx / l * side)
    }
}

pub struct Life {
    t: f32,
    step: usize,
    k: i32,
    gw: i32,
    gh: i32,
    cur: Vec<u8>, // 0 死；否则低 7 位 = 年龄 + 1，最高位 = 彩虹
    nxt: Vec<u8>,
    mask: Vec<bool>, // 鸭鸭占的格子
    duck0: (i32, i32),
    duck_from: (f32, f32),
    gun: Option<(i32, i32)>,
    gun_st: Vec<Vec<bool>>,
    gun_g: usize,
    gun_on: bool,
    gen_n: u32,
    acc: f32,
    running: bool,
    births: bool,
    pop: usize,
    rain: usize,
    flies: Vec<Fly>,
    landed: usize,
    next_fly: f32,
    lone: Option<Fly>,
    msgs: [String; 2],
    touched: bool,
}

fn hash(i: usize) -> f32 {
    let mut x = (i as u32).wrapping_mul(0x9E37_79B9) ^ 0x5bd1_e995;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2c1b_3c6d);
    x ^= x >> 12;
    (x & 0xffff) as f32 / 65536.0
}

impl Life {
    pub fn new(app: &mut App) -> Self {
        let k = app.s();
        let gw = (app.scr.w as i32 / k).max(1);
        let gh = (app.scr.h as i32 * 2 / k).max(1);
        // 孵化器放在鸭鸭正上方偏左，滑翔机沿对角线正好撞上鸭鸭的脑袋
        // （滑翔机先在空地上飞一段，让人看清它是从孵化器里一只只造出来的）
        let gun_fits = gw >= 52 && gh >= 44;
        let (gun, duck0) = if gun_fits {
            let gy0 = (gh / 2 - 29).max(3);
            let gx0 = (gw / 2 - 40).clamp(1, gw - 50);
            (Some((gx0, gy0)), (gx0 + 31, gy0 + 23))
        } else {
            (None, (gw / 2 - 9, (gh / 2 - 7).max(0)))
        };
        let n = (gw * gh) as usize;
        let mut mask = vec![false; n];
        for (j, row) in DUCK.iter().enumerate() {
            for (i, ch) in row.chars().enumerate() {
                if ch == '.' {
                    continue;
                }
                let (x, y) = (duck0.0 + 17 - i as i32, duck0.1 + j as i32);
                if x >= 0 && y >= 0 && x < gw && y < gh {
                    mask[(y * gw + x) as usize] = true;
                }
            }
        }
        // 预先算好孵化器 30 个相位（只取它自己的方框）；之后每代覆盖回去，它就「永动」且不怕被撞坏
        let mut gun_st = Vec::new();
        {
            let m = 14;
            let (pw, ph) = (GUN_W + 2 * m, GUN_H + 2 * m);
            let mut g = vec![false; (pw * ph) as usize];
            for (j, row) in GUN.iter().enumerate() {
                for (i, ch) in row.chars().enumerate() {
                    if ch == 'O' {
                        g[((j as i32 + m) * pw + i as i32 + m) as usize] = true;
                    }
                }
            }
            for _ in 0..30 {
                let mut st = vec![false; (GUN_W * GUN_H) as usize];
                for y in 0..GUN_H {
                    for x in 0..GUN_W {
                        st[(y * GUN_W + x) as usize] = g[((y + m) * pw + x + m) as usize];
                    }
                }
                gun_st.push(st);
                let mut ng = vec![false; g.len()];
                for y in 0..ph {
                    for x in 0..pw {
                        let mut c = 0;
                        for dy in -1..=1 {
                            for dx in -1..=1 {
                                if (dx != 0 || dy != 0) && x + dx >= 0 && y + dy >= 0 && x + dx < pw && y + dy < ph && g[((y + dy) * pw + x + dx) as usize] {
                                    c += 1;
                                }
                            }
                        }
                        let a = g[(y * pw + x) as usize];
                        ng[(y * pw + x) as usize] = c == 3 || (a && c == 2);
                    }
                }
                g = ng;
            }
        }

        // 降落的飞蛾：先用屏上现有的，再从四边补
        let nfly = if app.hi { 42 } else { 28 };
        let mut flies = Vec::new();
        let mut starts: Vec<(f32, f32, u8)> = app.moths.iter().map(|m| (m.x, m.y, m.shade)).collect();
        app.moths.clear();
        let (w, h) = (app.w(), app.hp());
        while starts.len() < nfly {
            let (x, y) = match app.rng.int(4) {
                0 => (-3.0, app.rng.range(0.0, h)),
                1 => (w + 3.0, app.rng.range(0.0, h)),
                2 => (app.rng.range(0.0, w), -3.0),
                _ => (app.rng.range(0.0, w), h + 3.0),
            };
            starts.push((x, y, app.rng.int(5) as u8));
        }
        let blocked = |x: i32, y: i32| -> bool {
            let near_duck = x >= duck0.0 - 8 && x < duck0.0 + 26 && y >= duck0.1 - 8 && y < duck0.1 + 22;
            let near_gun = gun.map_or(false, |(gx, gy)| x >= gx - 7 && x < gx + GUN_W + 7 && y >= gy - 7 && y < gy + GUN_H + 7);
            near_duck || near_gun
        };
        for (i, (sx, sy, shade)) in starts.into_iter().enumerate() {
            let mut cell = (0, 0);
            for _ in 0..40 {
                let x = 3 + app.rng.int((gw - 6).max(1) as usize) as i32;
                let y = 3 + app.rng.int((gh - 6).max(1) as usize) as i32;
                cell = (x, y);
                if !blocked(x, y) {
                    break;
                }
            }
            let t0 = 0.05 + i as f32 / nfly as f32 * 1.3 + app.rng.range(0.0, 0.15);
            flies.push(Fly {
                sx,
                sy,
                tx: (cell.0 * k) as f32 + k as f32 * 0.5,
                ty: (cell.1 * k) as f32 + k as f32 * 0.5,
                t0,
                dur: app.rng.range(0.8, 1.3),
                shade,
                flap: app.rng.range(0.0, 10.0),
                wob: app.rng.range(-12.0, 12.0),
                done: false,
                cell,
            });
        }

        app.duck.visible = true;
        app.duck.asleep = false;
        app.duck.bubble = None;
        app.set_title("bug 繁殖模拟器 v0.1");
        Life {
            t: 0.0,
            step: 0,
            k,
            gw,
            gh,
            cur: vec![0; n],
            nxt: vec![0; n],
            mask,
            duck0,
            duck_from: (app.duck.x, app.duck.y),
            gun,
            gun_st,
            gun_g: 0,
            gun_on: false,
            gen_n: 0,
            acc: 0.0,
            running: false,
            births: true,
            pop: 0,
            rain: 0,
            flies,
            landed: 0,
            next_fly: T_SIM + 0.5,
            lone: None,
            msgs: [String::new(), String::new()],
            touched: false,
        }
    }

    fn ev(&mut self, k: usize, at: f32) -> bool {
        if self.step == k && self.t >= at {
            self.step += 1;
            true
        } else {
            false
        }
    }

    /// 屏幕底部的小控制台 + 写进日志（日志在插曲结束后接着显示，故事接得上）
    fn say(&mut self, app: &mut App, s: String) {
        if self.t < T_BACK - 0.6 {
            app.push(log_line("INFO", "life::sim", &s, Kind::Info));
        }
        self.msgs[0] = std::mem::take(&mut self.msgs[1]);
        self.msgs[1] = s;
    }

    fn duck_px(&self) -> (f32, f32) {
        ((self.duck0.0 + 9) as f32 * self.k as f32, (self.duck0.1 + 7) as f32 * self.k as f32)
    }

    fn idx(&self, x: i32, y: i32) -> Option<usize> {
        (x >= 0 && y >= 0 && x < self.gw && y < self.gh).then(|| (y * self.gw + x) as usize)
    }

    /// 落地：在落点撒一小团 bug
    fn seed(&mut self, app: &mut App, cx: i32, cy: i32, r: i32) {
        for dy in -r..=r {
            for dx in -r..=r {
                if (dx * dx + dy * dy <= r * r && app.rng.f() < 0.38) || (dx == 0 && dy == 0) {
                    let (x, y) = (cx + dx, cy + dy);
                    if self.in_gun(x, y) {
                        continue;
                    }
                    if let Some(i) = self.idx(x, y) {
                        if !self.mask[i] {
                            self.cur[i] = 1;
                        }
                    }
                }
            }
        }
    }

    fn in_gun(&self, x: i32, y: i32) -> bool {
        self.gun_on && self.gun.map_or(false, |(gx, gy)| x >= gx && x < gx + GUN_W && y >= gy && y < gy + GUN_H)
    }

    fn generation(&mut self) {
        let (gw, gh) = (self.gw, self.gh);
        let late = self.t >= T_INFECT;
        let (birth_thr, conv_thr) = if late { (1, 2) } else { (2, 3) };
        let (mut pop, mut rain) = (0, 0);
        let mut touched = false;
        for y in 0..gh {
            for x in 0..gw {
                let i = (y * gw + x) as usize;
                if self.mask[i] {
                    self.nxt[i] = 0;
                    continue;
                }
                let (mut n, mut nr, mut touch) = (0u8, 0u8, false);
                for dy in -1..=1 {
                    let yy = y + dy;
                    if yy < 0 || yy >= gh {
                        continue;
                    }
                    for dx in -1..=1 {
                        let xx = x + dx;
                        if (dx == 0 && dy == 0) || xx < 0 || xx >= gw {
                            continue;
                        }
                        let j = (yy * gw + xx) as usize;
                        let v = self.cur[j];
                        if v != 0 {
                            n += 1;
                            if v & RAINBOW != 0 {
                                nr += 1;
                            }
                        } else if self.mask[j] {
                            // 鸭鸭 = 永远不死的彩虹细胞：算作活邻居
                            n += 1;
                            nr += 1;
                            touch = true;
                        }
                    }
                }
                let v = self.cur[i];
                let out = if v != 0 && (n == 2 || n == 3) {
                    let age = ((v & 0x7f) + 1).min(30);
                    let rb_ = v & RAINBOW != 0 || touch || nr >= conv_thr;
                    age | if rb_ { RAINBOW } else { 0 }
                } else if v == 0 && n == 3 && self.births {
                    1 | if touch || nr >= birth_thr { RAINBOW } else { 0 }
                } else {
                    0
                };
                if out != 0 {
                    pop += 1;
                    if out & RAINBOW != 0 {
                        rain += 1;
                    }
                    if touch {
                        touched = true;
                    }
                }
                self.nxt[i] = out;
            }
        }
        std::mem::swap(&mut self.cur, &mut self.nxt);
        if self.gun_on {
            self.gun_g += 1;
            let st = &self.gun_st[self.gun_g % 30];
            let (gx, gy) = self.gun.unwrap();
            for y in 0..GUN_H {
                for x in 0..GUN_W {
                    let Some(i) = self.idx(gx + x, gy + y) else { continue };
                    let was = self.nxt[i];
                    let now = self.cur[i];
                    if now != 0 {
                        pop -= 1;
                        if now & RAINBOW != 0 {
                            rain -= 1;
                        }
                    }
                    self.cur[i] = if st[(y * GUN_W + x) as usize] {
                        pop += 1;
                        if was != 0 { ((was & 0x7f) + 1).min(30) } else { 1 }
                    } else {
                        0
                    };
                }
            }
        }
        self.pop = pop;
        self.rain = rain;
        self.gen_n += 1;
        if touched {
            self.touched = true;
        }
    }

    fn recount(&mut self) {
        self.pop = self.cur.iter().filter(|&&v| v != 0).count();
        self.rain = self.cur.iter().filter(|&&v| v & RAINBOW != 0).count();
    }
}

impl Interlude for Life {
    fn name(&self) -> &'static str {
        "生命游戏"
    }

    fn update(&mut self, app: &mut App, dt: f32) -> bool {
        self.t += dt;
        let t = self.t;
        let k = self.k as f32;

        // 鸭鸭滑到培养皿里属于它的位置（自己控制，落点精确对齐格子）
        let (dx, dy) = self.duck_px();
        let u = smooth(0.0, 1.2, t);
        app.duck.x = self.duck_from.0 + (dx - self.duck_from.0) * u;
        app.duck.y = self.duck_from.1 + (dy - self.duck_from.1) * u;
        app.duck.tx = app.duck.x;
        app.duck.ty = app.duck.y;

        // 飞蛾降落
        for i in 0..self.flies.len() {
            let f = &mut self.flies[i];
            f.flap += dt * 12.0;
            if !f.done && t >= f.t0 + f.dur {
                f.done = true;
                let c = f.cell;
                let r = if f.t0 < T_SIM { 3 + app.rng.int(3) as i32 } else { 2 + app.rng.int(2) as i32 };
                self.seed(app, c.0, c.1, r);
                self.landed += 1;
            }
        }
        // 中段：飞蛾继续被鸭鸭的光吸引，落在它附近（落点 = 新的一团 bug）
        if self.running && t < T_WAVE - 1.2 && t >= self.next_fly {
            self.next_fly = t + if app.hi { 0.3 } else { 0.5 };
            let a = app.rng.range(0.0, TAU);
            let d = app.rng.range(13.0, 22.0);
            let (cx, cy) = (self.duck0.0 as f32 + 9.0 + a.cos() * d * 1.3, self.duck0.1 as f32 + 7.0 + a.sin() * d);
            let (gx, gy) = (cx as i32, cy as i32);
            let in_gun = self.gun.map_or(false, |(x0, y0)| gx >= x0 - 6 && gx < x0 + GUN_W + 6 && gy >= y0 - 6 && gy < y0 + GUN_H + 6);
            if !in_gun && gx >= 3 && gy >= 3 && gx < self.gw - 3 && gy < self.gh - 3 {
                let (w, h) = (app.w(), app.hp());
                let (sx, sy) = match app.rng.int(4) {
                    0 => (-3.0, app.rng.range(0.0, h)),
                    1 => (w + 3.0, app.rng.range(0.0, h)),
                    2 => (app.rng.range(0.0, w), -3.0),
                    _ => (app.rng.range(0.0, w), h + 3.0),
                };
                let kf = self.k as f32;
                self.flies.push(Fly {
                    sx,
                    sy,
                    tx: gx as f32 * kf + kf * 0.5,
                    ty: gy as f32 * kf + kf * 0.5,
                    t0: t,
                    dur: app.rng.range(1.0, 1.5),
                    shade: app.rng.int(5) as u8,
                    flap: 0.0,
                    wob: app.rng.range(-14.0, 14.0),
                    done: false,
                    cell: (gx, gy),
                });
            }
        }
        if let Some(f) = &mut self.lone {
            f.flap += dt * if t > f.t0 + f.dur { 2.5 } else { 12.0 };
        }

        if self.ev(0, 0.95) {
            let n = self.flies.len();
            self.say(app, format!("培养皿就绪，投放 bug ×{}", n));
        }
        if self.ev(1, T_SIM) {
            self.running = true;
            if self.gun.is_some() {
                self.gun_on = true;
                self.say(app, "孵化器上线：每 30 代自动生产一只 bug".into());
            } else {
                self.step += 0;
            }
        }
        if self.step == 2 && self.touched && self.gen_n >= 4 {
            self.step = 3;
            self.say(app, "规则补丁：鸭鸭 = 永远不死的细胞，挨着它的 bug 会变彩虹".into());
        }
        if (self.step == 2 || self.step == 3) && t >= T_INFECT {
            self.step = 4;
            let g = self.gen_n;
            self.say(app, format!("第 {} 代：彩虹会遗传，并且会传染", g));
        }
        if self.ev(4, T_WAVE) {
            let g = self.gen_n;
            self.say(app, format!("第 {} 代：鸭鸭全功率发光，bug 全部染成彩虹", g));
            app.quack("嘎！", 1.0);
            app.set_title("bug 繁殖模拟器 v0.1 — 彩虹版");
        }
        if self.ev(5, T_FREEZE) {
            self.births = false;
            self.gun_on = false;
            let g = self.gen_n;
            self.say(app, format!("第 {} 代：bug 停止繁殖（全都在盯着鸭鸭看）", g));
        }
        if self.ev(6, T_DIE1 + 0.1) {
            self.cur.fill(0);
            self.recount();
            let g = self.gen_n;
            self.say(app, format!("第 {} 代：bug 全部自然灭绝（鸭鸭毫发无损）", g));
            app.set_title("bug 繁殖模拟器 v0.1 — 存活 0");
        }
        if self.ev(7, T_LONE) {
            let (w, h) = (app.w(), app.hp());
            let (sx, sy) = (w + 3.0, h * 0.18);
            // 落在鸭鸭背上（头和尾巴之间那道凹处）
            let tx = (self.duck0.0 + 12) as f32 * k;
            let ty = (self.duck0.1 + 9) as f32 * k - 1.5 * k;
            self.lone = Some(Fly { sx, sy, tx, ty, t0: T_LONE, dur: 1.3, shade: 1, flap: 0.0, wob: -18.0, done: false, cell: (0, 0) });
        }
        if self.ev(8, T_LONE + 1.35) {
            self.pop = 1;
            let g = self.gen_n;
            self.say(app, format!("第 {} 代：还剩 1 只。规则要 2 个邻居——它说鸭鸭一个顶俩", g));
            app.set_title("bug 繁殖模拟器 v0.1 — 存活 1（够了）");
        }
        if self.ev(9, T_QUACK) {
            app.quack("嘎", 1.3);
        }

        // 演化：满效果约 9 代/秒，省流量档减半
        if self.running {
            let rate = if app.hi { 9.0 } else { 4.5 };
            self.acc += dt * rate;
            let mut n = 0;
            while self.acc >= 1.0 && n < 3 {
                self.acc -= 1.0;
                n += 1;
                if t < T_DIE1 + 0.1 {
                    self.generation();
                } else {
                    self.gen_n += 1;
                }
            }
            if n > 0 && t >= T_DIE0 && t < T_DIE1 + 0.1 {
                // 灭绝：一批一批消失，剩下的按 S23 连锁死亡
                let p = (t - T_DIE0) / (T_DIE1 - T_DIE0);
                for i in 0..self.cur.len() {
                    if self.cur[i] != 0 && hash(i) < p {
                        self.cur[i] = 0;
                    }
                }
                self.recount();
            }
        }
        // 鸭鸭发光：从鸭鸭往外扩一圈彩虹
        if t >= T_WAVE && t < T_FREEZE + 0.5 {
            let (cx, cy) = (self.duck0.0 as f32 + 9.0, self.duck0.1 as f32 + 7.0);
            let far = cx.max(self.gw as f32 - cx).hypot(cy.max(self.gh as f32 - cy));
            let r = smooth(0.0, 1.5, t - T_WAVE) * (far + 1.0);
            let r2 = r * r;
            for y in 0..self.gh {
                let ey = y as f32 - cy;
                for x in 0..self.gw {
                    let i = (y * self.gw + x) as usize;
                    if self.cur[i] != 0 {
                        let ex = x as f32 - cx;
                        if ex * ex + ey * ey < r2 {
                            self.cur[i] |= RAINBOW;
                        }
                    }
                }
            }
            self.rain = self.cur.iter().filter(|&&v| v & RAINBOW != 0).count();
        }

        if t >= T_END {
            // 收尾：把最后那只留在主线里，日志画面接着演
            if let Some(f) = &self.lone {
                let (x, y) = f.pos(t);
                app.spawn_moth(x, y, 0.0, 0.0);
                if let Some(m) = app.moths.last_mut() {
                    m.shade = 4;
                }
            }
            return true;
        }
        false
    }

    fn draw(&mut self, app: &mut App) {
        let t = self.t;
        let hi = app.hi;
        // 日志：开场分档变暗退场，结尾分档淡回（逐帧真彩色渐变太贵，只分 2 档）
        let dim = if t < 1.0 {
            if hi {
                if t < 0.3 { Some(0.0) } else if t < 0.65 { Some(0.45) } else { Some(0.8) }
            } else if t < 0.5 {
                Some(0.0)
            } else {
                Some(1.0)
            }
        } else if t >= T_BACK {
            let u = t - T_BACK;
            if hi {
                if u < 0.35 { Some(0.8) } else if u < 0.7 { Some(0.45) } else { Some(0.0) }
            } else if u < 0.5 {
                Some(1.0)
            } else {
                Some(0.0)
            }
        } else {
            None
        };
        if let Some(d) = dim {
            app.draw_logs(9.0, 0, d);
        }

        // 细胞
        let show_cells = t < T_DIE1 + 0.2;
        if show_cells {
            let k = self.k;
            let dying = t >= T_DIE0;
            let p = (t - T_DIE0) / (T_DIE1 - T_DIE0);
            for y in 0..self.gh {
                for x in 0..self.gw {
                    let i = (y * self.gw + x) as usize;
                    let v = self.cur[i];
                    if v == 0 {
                        continue;
                    }
                    let age = (v & 0x7f).saturating_sub(1);
                    let (px, py) = (x * k, y * k);
                    let fading = dying && hash(i) < p + 0.12;
                    let c: Col = if v & RAINBOW != 0 {
                        let band = if fading { Band::Dim } else if age == 0 { Band::Duck } else { Band::Text };
                        rb(band, px as f32, py as f32 * 0.5)
                    } else {
                        let g = if fading { 70 } else { [238, 200, 165, 138][age.min(3) as usize] };
                        [g, g, g].into()
                    };
                    for sy in 0..k {
                        for sx in 0..k {
                            app.layer.set(px + sx, py + sy, c);
                        }
                    }
                }
            }
        }

        // 鸭鸭（永远朝左，看着孵化器）
        app.duck.right = false;
        app.draw_duck(None);

        // 降落中的飞蛾
        for i in 0..self.flies.len() {
            let f = &self.flies[i];
            if f.done || t < f.t0 {
                continue;
            }
            let (x, y) = f.pos(t);
            let (shade, flap) = (f.shade, f.flap);
            moth(app, x, y, shade, flap, 0.0);
        }
        if let Some(f) = &self.lone {
            let (x, y) = f.pos(t);
            let (shade, flap) = (f.shade, f.flap);
            if self.k >= 2 {
                big_moth(app, x, y, shade, flap, self.k);
            } else {
                moth(app, x, y, shade, flap, 0.0);
            }
            // 它头顶冒一颗像素小心
            if t >= T_QUACK + 0.5 && t < T_BACK + 0.6 {
                const HEART: [&str; 4] = [".#.#.", "#####", ".###.", "..#.."];
                let k = self.k;
                let (ox, oy) = (x.round() as i32 - 5 * k / 2 + k, (y - 3.0 * k as f32).round() as i32 - 5 * k);
                for (j, row) in HEART.iter().enumerate() {
                    for (i, ch) in row.chars().enumerate() {
                        if ch == '#' {
                            for sy in 0..k {
                                for sx in 0..k {
                                    app.layer.set(ox + i as i32 * k + sx, oy + j as i32 * k + sy, [255, 95, 145]);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn draw_over(&mut self, app: &mut App) {
        let t = self.t;
        if t < 0.95 || t >= T_BACK {
            return;
        }
        let (w, h) = (app.scr.w as i32, app.scr.h as i32);
        let bg: Rgb = BLACK;
        // 右上角计数
        if self.running {
            let pct = if self.pop > 0 { self.rain * 100 / self.pop } else { 0 };
            let s = if t >= T_DIE1 {
                format!(" 第 {} 代 · 存活 bug ×{} ", self.gen_n, self.pop)
            } else if w >= 70 {
                format!(" 第 {} 代 · 存活 bug ×{} · 彩虹 {}% ", self.gen_n, self.pop, pct)
            } else {
                format!(" 第{}代 ×{} ", self.gen_n, self.pop)
            };
            let x = w - str_w(&s) as i32 - 1;
            app.scr.text(x.max(0), 0, &s, [175, 175, 175], Some(bg));
        }
        // 孵化器标签
        if let (Some((gx, gy)), true) = (self.gun, self.gun_on || (t >= T_SIM && t < T_FREEZE)) {
            let row = (gy * self.k) / 2 - 2;
            if row >= 1 {
                app.scr.text(gx * self.k, row, "bug 孵化器（周期 30）", [110, 110, 110], Some(bg));
            }
        }
        // 底部小控制台：最近两条
        let fade: [Rgb; 2] = [[95, 95, 95], [215, 215, 215]];
        for (j, m) in self.msgs.iter().enumerate() {
            if m.is_empty() || h < 6 || (j == 0 && h < 16) {
                continue;
            }
            let y = h - 3 + j as i32;
            let s = format!(" > {} ", m);
            app.scr.text(1, y, &s, fade[j], Some(bg));
        }
    }
}

/// 放大版飞蛾（结尾那只，和细胞同样的像素尺寸，带深色描边）
fn big_moth(app: &mut App, x: f32, y: f32, shade: u8, flap: f32, k: i32) {
    let wing: Rgb = match shade {
        0 | 1 => [238, 238, 238],
        2 => [175, 175, 175],
        3 => [120, 120, 120],
        _ => [22, 22, 22],
    };
    let body = if shade == 4 { [235, 235, 235] } else { scale(wing, 0.4) };
    let fr = &MOTH[((flap as i32) & 1) as usize];
    let (ox, oy) = (x.round() as i32 - 5 * k / 2, y.round() as i32 - 3 * k / 2);
    let on = |i: i32, j: i32| -> Option<char> {
        if i < 0 || j < 0 || i >= 5 || j >= 3 {
            return None;
        }
        let ch = fr[j as usize].as_bytes()[i as usize] as char;
        (ch != '.').then_some(ch)
    };
    for py in -1..3 * k + 1 {
        for px in -1..5 * k + 1 {
            let (i, j) = (px.div_euclid(k), py.div_euclid(k));
            let c: Option<Rgb> = match on(i, j) {
                Some('o') => Some(body),
                Some(_) => Some(wing),
                None => {
                    let near = [(-1, 0), (1, 0), (0, -1), (0, 1)].iter().any(|(dx, dy)| on((px + dx).div_euclid(k), (py + dy).div_euclid(k)).is_some());
                    near.then_some([14, 12, 24])
                }
            };
            if let Some(c) = c {
                app.layer.set(ox + px, oy + py, c);
            }
        }
    }
}
