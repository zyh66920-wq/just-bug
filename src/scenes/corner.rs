//! 插曲：撞角落——显示器待机，鸭鸭变成待机标志斜着弹墙、每撞一次墙换一种颜色；
//! 底下一排飞蛾观众盯着看它会不会正好撞进角落。差一点、差一点……最后真的撞进去了：满屏彩虹烟花，
//! 飞蛾全体扑向角落里那只最亮的鸭鸭（趋光性，没办法）。
//!
//! 轨迹做法：从「终点在角落」倒推。把墙展开成直线，x(t) = tri(vx·(t−T) + cx·Lx)，tri 是在 [0, L] 里来回反射的三角波，
//! t = T 时必然正好落在角 (cx·Lx, cy·Ly)。再按当前屏幕尺寸网格搜索速度和角，挑出「前面有一两次差一点、
//! 之前绝不会提前进角」的那一组。屏幕再小也一样成立。终点只选上面两个角（下面有观众席，撞进去看着不像角）。
use super::*;

/// 正好撞进角落的时刻（插曲内秒数）
const T_HIT: f32 = 15.6;
/// 标志出现、开始弹的时刻
const T0: f32 = 1.3;
/// 屏幕重新亮起（回到日志）的时刻
const T_ON: f32 = 19.1;
const T_END: f32 = 20.2;

/// 每撞一次墙换一种纯色（静态真彩色：不动的时候零成本）
const COLORS: [Rgb; 8] = [
    [255, 95, 95],
    [255, 165, 60],
    [245, 225, 80],
    [100, 225, 120],
    [80, 215, 235],
    [105, 145, 255],
    [190, 115, 255],
    [255, 125, 205],
];

/// 高潮大字「！！！」：粗感叹号点阵（4×14），画在像素层、按调色板彩虹上色
const BANG: [&str; 14] = [
    "####", "####", "####", "####", "####", "####", "####", ".##.", ".##.", ".##.", "....", "####", "####", "####",
];

const MISS: [&str; 3] = ["差一点！", "啊啊啊就差一格！", "又差一点！！"];
const SIGH: [&str; 3] = ["唉——", "（失望地趴回去）", "这都能不进？"];

/// 三角波：在 [0, l] 里来回反射
fn tri(u: f32, l: f32) -> f32 {
    let m = u.rem_euclid(2.0 * l);
    if m <= l { m } else { 2.0 * l - m }
}

#[derive(Clone)]
struct Plan {
    lx: f32,
    ly: f32,
    vx: f32,
    vy: f32,
    cx: f32,
    cy: f32,
    /// 撞到角之前的每次撞墙（时刻，是否撞的是左右墙）
    bounces: Vec<(f32, bool)>,
    /// 「差一点」发生的时刻
    near: Vec<f32>,
}

impl Plan {
    fn ux(&self, t: f32) -> f32 {
        self.vx * (t.min(T_HIT) - T_HIT) + self.cx * self.lx
    }
    fn pos(&self, t: f32) -> (f32, f32) {
        let uy = self.vy * (t.min(T_HIT) - T_HIT) + self.cy * self.ly;
        (tri(self.ux(t), self.lx), tri(uy, self.ly))
    }
    /// 鸭头朝向：x 正在变大就朝右
    fn right(&self, t: f32) -> bool {
        let m = self.ux(t.min(T_HIT - 0.001)).rem_euclid(2.0 * self.lx);
        m < self.lx
    }

    /// 按场地大小搜一条轨迹：T_HIT 时正好进角，之前有 1–2 次差一点、绝不提前进角
    fn search(lx: f32, ly: f32, rng: &mut Rng) -> Plan {
        // 「差一点」的距离窗口：左右差 1.5..nmx 列，上下差 1..nmy 行
        let nmx = (lx * 0.12).clamp(2.2, 6.0);
        let nmy = (ly * 0.25).clamp(1.3, 2.6);
        let mut best: Option<(f32, Plan)> = None;
        for corner in 0..2 {
            let (cx, cy) = ((corner & 1) as f32, (corner >> 1) as f32);
            for ix in 0..48 {
                let tx = 2.6 + ix as f32 * 0.075;
                for iy in 0..48 {
                    let ty = 1.9 + iy as f32 * 0.065;
                    let mut p = Plan { lx, ly, vx: lx / tx, vy: ly / ty, cx, cy, bounces: Vec::new(), near: Vec::new() };
                    let mut ok = true;
                    let mut dsum = 0.0;
                    for (is_x, per) in [(true, tx), (false, ty)] {
                        let mut j = 1;
                        loop {
                            let tb = T_HIT - j as f32 * per;
                            if tb < T0 {
                                break;
                            }
                            let (x, y) = p.pos(tb);
                            let (d, lo, hi) = if is_x { (y.min(ly - y), 1.0, nmy) } else { (x.min(lx - x), 1.5, nmx) };
                            if d < lo {
                                ok = false;
                                break;
                            }
                            if d <= hi {
                                p.near.push(tb);
                                dsum += if is_x { d * 2.0 } else { d };
                            }
                            p.bounces.push((tb, is_x));
                            j += 1;
                        }
                        if !ok {
                            break;
                        }
                    }
                    if !ok {
                        continue;
                    }
                    p.bounces.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
                    p.near.sort_by(|a, b| a.partial_cmp(b).unwrap());
                    // 一次擦角会先撞一面墙、紧接着撞另一面：合并成一次，以后一下为准
                    let mut merged: Vec<f32> = Vec::new();
                    for &t in &p.near {
                        match merged.last_mut() {
                            Some(l) if t - *l < 1.0 => *l = t,
                            _ => merged.push(t),
                        }
                    }
                    p.near = merged;
                    // 太早的差一点（观众还没坐好）和太晚的（抢了结局的戏）都不要
                    let early = p.near.iter().filter(|&&t| t < 3.4).count();
                    if p.near.iter().any(|&t| t > T_HIT - 2.4) {
                        continue;
                    }
                    p.near.retain(|&t| t >= 3.4);
                    let n = p.near.len() as f32;
                    let mut s = -(n - 2.0).abs() * 10.0 - early as f32 * 4.0;
                    s -= (5.0 - p.bounces.len() as f32).max(0.0) * 3.0;
                    s -= (p.vx / (p.vy * 2.0)).ln().abs() * 3.0;
                    s -= dsum * 0.4;
                    for w in p.near.windows(2) {
                        if w[1] - w[0] < 2.6 {
                            s -= 12.0;
                        }
                    }
                    if let Some(&a) = p.near.first() {
                        s -= (a - 7.5).abs() * 0.4;
                    }
                    if let Some(&b) = p.near.get(1) {
                        s -= (b - 11.5).abs() * 0.4;
                    }
                    s += rng.f() * 2.0;
                    if best.as_ref().map_or(true, |b| s > b.0) {
                        best = Some((s, p));
                    }
                }
            }
        }
        if let Some((_, p)) = best {
            return p;
        }
        // 场地小到搜不出来：照样保证结尾进角，只是没有差一点
        let mut p = Plan { lx, ly, vx: lx / 3.3, vy: ly / 2.4, cx: 0.0, cy: 0.0, bounces: Vec::new(), near: Vec::new() };
        for (is_x, per) in [(true, 3.3), (false, 2.4)] {
            let mut j = 1;
            while T_HIT - j as f32 * per >= T0 {
                p.bounces.push((T_HIT - j as f32 * per, is_x));
                j += 1;
            }
        }
        p.bounces.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        p
    }
}

struct Aud {
    seat: f32,
    hop: f32,
    timer: f32,
    flutter: f32,
    ph: f32,
    sp: f32,
}

struct Spark {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    age: f32,
    life: f32,
    hue: u8,
    rocket: bool,
    /// 火箭的减速度（像素/秒²）
    g: f32,
    /// 火箭炸开的大小倍数
    size: f32,
}

pub struct Corner {
    t: f32,
    size: (usize, usize),
    k: i32,
    bw: i32,
    duck_rows: i32,
    area_h: i32,
    plan: Plan,
    bounce_i: usize,
    color: usize,
    aud: Vec<Aud>,
    sparks: Vec<Spark>,
    /// 待发射的火箭：(时刻, 横向位置比例, 炸点高度比例, 大小)
    launches: Vec<(f32, f32, f32, f32)>,
    hit: bool,
    /// 大字底部所在的字符行（没画大字时为 None）
    big_bottom: Option<i32>,
    logged: bool,
    handed: bool,
}

impl Corner {
    pub fn new(app: &mut App) -> Self {
        app.duck.visible = false;
        app.duck.bubble = None;
        app.duck.zz.clear();
        app.info("display::pm", "30 分钟无操作，显示器进入待机（鸭鸭接管屏幕）");
        app.set_title("鸭鸭.exe — 待机中");
        // 观众：现有的飞蛾全留下，不够再从屏幕边上叫几只
        let n = (app.scr.w / 10).clamp(4, 22).max(app.moths.len());
        while app.moths.len() < n {
            app.spawn_moth_edge();
        }
        app.moths.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());
        let mut aud = Vec::new();
        for _ in 0..n {
            let r = &mut app.rng;
            aud.push(Aud {
                seat: 0.0,
                hop: r.range(6.0, 16.0),
                timer: r.range(1.5, 6.0),
                flutter: 0.0,
                ph: r.range(0.0, TAU),
                sp: r.range(0.6, 1.3),
            });
        }
        let color = app.rng.int(COLORS.len());
        let mut c = Corner {
            t: 0.0,
            size: (0, 0),
            k: 1,
            bw: 12,
            duck_rows: 5,
            area_h: 10,
            plan: Plan { lx: 1.0, ly: 1.0, vx: 1.0, vy: 1.0, cx: 0.0, cy: 0.0, bounces: Vec::new(), near: Vec::new() },
            bounce_i: 0,
            color,
            aud,
            sparks: Vec::new(),
            launches: Vec::new(),
            hit: false,
            big_bottom: None,
            logged: false,
            handed: false,
        };
        c.layout(app);
        c
    }

    /// 按当前屏幕尺寸定标志大小、场地和轨迹（尺寸变了就重算，进角时刻不变）
    fn layout(&mut self, app: &mut App) {
        let (w, h) = (app.scr.w, app.scr.h);
        self.size = (w, h);
        self.k = if w >= 200 && h >= 55 {
            3
        } else if w >= 100 && h >= 30 {
            2
        } else {
            1
        };
        // 标志外框就是鸭身像素本身（不留描边），撞进角时身子正好贴住两条边
        self.bw = 10 * self.k;
        self.duck_rows = (7 * self.k + 1) / 2;
        let aud_rows = if h >= 24 { 3 } else { 2 };
        self.area_h = h as i32 - aud_rows;
        let lx = (w as i32 - self.bw).max(2) as f32;
        let ly = (self.area_h - self.duck_rows - 1).max(1) as f32;
        self.plan = Plan::search(lx, ly, &mut app.rng);
        self.bounce_i = self.plan.bounces.iter().take_while(|b| b.0 <= self.t).count();
        let n = self.aud.len();
        for (i, a) in self.aud.iter_mut().enumerate() {
            a.seat = (w as f32) * (i as f32 + 0.5) / n as f32;
        }
    }

    /// 标志左上角（字符格）
    fn box_at(&self, t: f32) -> (i32, i32) {
        let (x, y) = self.plan.pos(t.max(T0));
        (x.round() as i32, y.round() as i32)
    }
    /// 标志中心（像素坐标）
    fn box_center(&self) -> (f32, f32) {
        let (bx, by) = self.box_at(self.t);
        (bx as f32 + self.bw as f32 / 2.0, (by * 2 + self.duck_rows + 1) as f32)
    }

    /// 观众兴奋度：每次「快进角了」前 1 秒慢慢抬头，撞墙那一下一起蹦起来，然后泄气
    fn excite(&self, t: f32) -> f32 {
        let mut e: f32 = 0.0;
        for &te in self.plan.near.iter().chain(std::iter::once(&T_HIT)) {
            let v = if t < te - 1.0 || t > te + 1.8 {
                0.0
            } else if t < te {
                smooth(te - 1.0, te, t) * 0.35
            } else {
                let a = t - te;
                if a < 0.15 { 0.35 + 0.65 * a / 0.15 } else { 1.0 - smooth(0.15, 1.8, a) }
            };
            e = e.max(v);
        }
        e
    }

    /// 同屏粒子上限：按屏幕面积给，省流量档减半
    fn cap(app: &App) -> usize {
        let c = (app.scr.w * app.scr.h / 10).clamp(150, 1600);
        if app.hi { c } else { c / 2 }
    }

    /// 炸开一朵：像素坐标，按角度排成一圈彩虹（调色板色，颜色流动不花带宽）
    fn burst(&mut self, x: f32, y: f32, n: usize, a0: f32, a1: f32, spd: f32, rng: &mut Rng, cap: usize) {
        let hue = rng.int(PN as usize) as u8;
        for _ in 0..n {
            if self.sparks.len() >= cap {
                break;
            }
            let a = rng.range(a0, a1);
            // 七成在外圈成环，三成填在里面
            let s = if rng.f() < 0.7 { rng.range(0.75, 1.0) } else { rng.range(0.2, 0.75) } * spd;
            self.sparks.push(Spark {
                x,
                y,
                vx: a.cos() * s,
                vy: a.sin() * s,
                age: 0.0,
                life: rng.range(1.1, 1.9),
                hue: (hue as f32 + ((a - a0) / (a1 - a0)).fract() * PN as f32 + rng.range(0.0, 2.0)) as u8 % PN,
                rocket: false,
                g: 0.0,
                size: 0.0,
            });
        }
    }

    fn update_sparks(&mut self, app: &mut App, dt: f32) {
        let (w, hp) = (app.w(), app.hp());
        let g = hp * 0.35;
        let cap = Self::cap(app);
        let per = if app.hi { 110.0 } else { 55.0 };
        let r0 = w.min(hp) * 0.2;
        // 按波次发射
        let t = self.t;
        let mut i = 0;
        while i < self.launches.len() {
            let (at, xf, af, size) = self.launches[i];
            if t >= at {
                self.launches.swap_remove(i);
                let y0 = hp - 1.0;
                let dh = y0 - hp * af;
                let rise = 0.55;
                let x = w * xf;
                let vx = app.rng.range(-6.0, 6.0);
                self.sparks.push(Spark { x, y: y0, vx, vy: -2.0 * dh / rise, age: 0.0, life: 9.0, hue: 0, rocket: true, g: 2.0 * dh / (rise * rise), size });
            } else {
                i += 1;
            }
        }
        let mut pops: Vec<(f32, f32, f32)> = Vec::new();
        for p in self.sparks.iter_mut() {
            p.age += dt;
            if p.rocket {
                p.vy += p.g * dt;
                p.y += p.vy * dt;
                p.x += p.vx * dt;
                if p.vy > -p.g * 0.06 {
                    pops.push((p.x, p.y, p.size));
                    p.age = p.life + 1.0;
                }
            } else {
                let drag = (1.0 - 1.5 * dt).max(0.0);
                p.vx *= drag;
                p.vy = p.vy * drag + g * dt;
                p.x += p.vx * dt;
                p.y += p.vy * dt;
            }
        }
        self.sparks.retain(|p| p.age <= p.life && p.y < hp + 2.0);
        for (x, y, size) in pops {
            self.burst(x, y, (per * size) as usize, 0.0, TAU, r0 * size * 2.4, &mut app.rng, cap);
        }
    }

    fn draw_logo(&self, app: &mut App) {
        let t = self.t;
        let (bx, by) = self.box_at(t);
        let k = self.k;
        let right = self.plan.right(t);
        let after = t - T_HIT;
        // 撞进角的一瞬间闪白，之后露出鸭鸭本色（彩虹）
        // 全屏白闪那一下，鸭鸭是一个深色剪影
        let body: Option<Rgb> = if self.hit && after < 0.15 {
            Some([25, 22, 35])
        } else if self.hit {
            None
        } else {
            Some(COLORS[self.color])
        };
        let (gw, gh) = (10 * k + 2, 7 * k + 2);
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
                        let c: Col = match (ch, body) {
                            ('e', _) => [8, 8, 14].into(),
                            ('B', Some(c)) => mix(c, WHITE, 0.55).into(),
                            ('B', None) => [255, 160, 40].into(),
                            (_, Some(c)) => c.into(),
                            (_, None) => rb(Band::Duck, px as f32 * 1.6 / k as f32, py as f32 * 0.8 / k as f32),
                        };
                        buf[(py * gw + px) as usize] = Some(c);
                    }
                }
            }
        }
        let (ox, oy) = (bx - 1, by * 2 - 1);
        for y in 0..gh {
            for x in 0..gw {
                if let Some(c) = buf[(y * gw + x) as usize] {
                    app.layer.set(ox + x, oy + y, c);
                }
            }
        }
        // 底下的名牌：一条圆头色带，抠出「嘎」字
        let (pw, text) = match k {
            3 => (20, "嘎   嘎   嘎"),
            2 => (14, "嘎  嘎"),
            _ => (8, "嘎嘎"),
        };
        let px0 = bx + (self.bw - pw) / 2;
        let py = by + self.duck_rows;
        let tw = str_w(text) as i32;
        let tx0 = px0 + (pw - tw) / 2;
        for x in px0 - 1..px0 + pw + 1 {
            let c: Col = match body {
                Some(c) => c.into(),
                None => rb(Band::Text, x as f32 * 3.0, 0.0),
            };
            if x == px0 - 1 {
                app.scr.put_c(x, py, '▐', c, None);
            } else if x == px0 + pw {
                app.scr.put_c(x, py, '▌', c, None);
            } else {
                app.scr.put_c(x, py, ' ', c, Some(c));
            }
        }
        let mut x = tx0;
        for ch in text.chars() {
            let bg: Col = match body {
                Some(c) => c.into(),
                None => rb(Band::Text, x as f32 * 3.0, 0.0),
            };
            if ch != ' ' {
                app.scr.put_c(x, py, ch, Col::T([10, 10, 16]), Some(bg));
            }
            x += char_w(ch) as i32;
        }
    }

    /// 烟花画在像素层：半格一颗的粗颗粒，带一颗暗色拖尾，后半生转暗
    fn draw_sparks(&self, app: &mut App) {
        for p in &self.sparks {
            let (x, y) = (p.x.round() as i32, p.y.round() as i32);
            if p.rocket {
                app.layer.set(x, y, [255, 240, 210]);
                for k in 1..=4 {
                    let f = k as f32 * 0.014;
                    let c = scale([255, 190, 110], 1.0 - k as f32 * 0.2);
                    app.layer.set((p.x - p.vx * f).round() as i32, (p.y - p.vy * f).round() as i32, c);
                }
                continue;
            }
            let f = p.age / p.life;
            if f < 0.75 {
                let (tx, ty) = ((p.x - p.vx * 0.06).round() as i32, (p.y - p.vy * 0.06).round() as i32);
                if (tx, ty) != (x, y) {
                    app.layer.set(tx, ty, Col::P(Band::Dim as u8 + p.hue));
                }
            }
            let band = if f < 0.6 { Band::Text } else { Band::Dim };
            app.layer.set(x, y, Col::P(band as u8 + p.hue));
        }
    }

    /// 高潮大字「！！！」：从上面掉下来定住，彩虹点阵 + 深色描边。返回大字底部所在的字符行
    fn draw_big(&self, app: &mut App, a: f32) -> Option<i32> {
        let (w, hp) = (app.scr.w as i32, app.hp());
        if app.scr.h < 16 {
            return None;
        }
        let units = 4 + 3 + 4 + 3 + 4; // ！ 空 ！ 空 ！
        let s = ((w as f32 * 0.42 / units as f32).min(hp * 0.36 / 14.0) as i32).clamp(1, 3);
        let gw = units * s;
        let gh = 14 * s;
        let x0 = (w - gw) / 2;
        let drop = (1.0 - smooth(0.0, 0.22, a)) * hp * 0.35;
        let y0 = (hp * 0.16 - drop).round() as i32;
        let mut buf = vec![false; (gw * gh) as usize];
        let put = |g: &[&str], ox: i32, buf: &mut Vec<bool>| {
            for (j, row) in g.iter().enumerate() {
                for (i, ch) in row.chars().enumerate() {
                    if ch == '#' {
                        for sy in 0..s {
                            for sx in 0..s {
                                let (px, py) = (ox + i as i32 * s + sx, j as i32 * s + sy);
                                buf[(py * gw + px) as usize] = true;
                            }
                        }
                    }
                }
            }
        };
        put(&BANG, 0, &mut buf);
        put(&BANG, 7 * s, &mut buf);
        put(&BANG, 14 * s, &mut buf);
        let on = |x: i32, y: i32| x >= 0 && y >= 0 && x < gw && y < gh && buf[(y * gw + x) as usize];
        for y in -1..=gh {
            for x in -1..=gw {
                if on(x, y) {
                    let c = rb(Band::Duck, (x0 + x) as f32 * 0.9, (y0 + y) as f32 * 0.45);
                    app.layer.set(x0 + x, y0 + y, c);
                } else if on(x - 1, y) || on(x + 1, y) || on(x, y - 1) || on(x, y + 1) {
                    app.layer.set(x0 + x, y0 + y, [10, 8, 18]);
                }
            }
        }
        Some((y0 + gh + 1) / 2)
    }

    /// 撞击瞬间：从角落射出一圈彩虹放射线
    fn draw_rays(&self, app: &mut App, a: f32) {
        let k = smooth(0.08, 0.5, a);
        if k <= 0.0 || a > 0.6 {
            return;
        }
        let (w, hp) = (app.w(), app.hp());
        let cx = if self.plan.cx > 0.5 { w - 1.0 } else { 0.0 };
        let dir = if self.plan.cx > 0.5 { -1.0 } else { 1.0 };
        let len = (w * w + hp * hp).sqrt() * k;
        for r in 0..11 {
            let ang = 0.08 + r as f32 * 0.14;
            let (dx, dy) = (ang.cos() * dir, ang.sin());
            let c = rb(Band::Text, r as f32 * 9.0, 0.0);
            let mut d = len * 0.45;
            while d < len {
                app.layer.set((cx + dx * d).round() as i32, (dy * d).round() as i32, c);
                d += 2.0;
            }
        }
    }

    fn draw_audience(&self, app: &mut App) {
        // 围着鸭鸭的飞蛾被照成彩虹；屏幕亮回日志时褪回本色，和主线的画法接上
        let rain = if self.hit { smooth(T_HIT + 1.6, T_HIT + 3.0, self.t) * 0.6 * (1.0 - smooth(T_ON, T_END - 0.2, self.t)) } else { 0.0 };
        let ms: Vec<(f32, f32, u8, f32)> = app.moths.iter().map(|m| (m.x, m.y, m.shade, m.flap)).collect();
        for (x, y, shade, flap) in ms {
            // 黑翅膀的飞蛾在黑屏上看不见，提成深灰
            let mut wing = match shade {
                0 | 1 => [238, 238, 238],
                2 => [175, 175, 175],
                3 => [120, 120, 120],
                _ => [72, 72, 80],
            };
            let mut bodyc = if shade == 4 { [235, 235, 235] } else { scale(wing, 0.4) };
            if rain > 0.0 {
                let c = lolcat(x, y / 2.0, app.pal_t());
                wing = mix(wing, c, rain);
                bodyc = mix(bodyc, scale(c, 0.5), rain);
            }
            let fr = &MOTH[((flap as i32) & 1) as usize];
            let (ox, oy) = (x.round() as i32 - 2, y.round() as i32 - 1);
            for (j, row) in fr.iter().enumerate() {
                for (k, ch) in row.chars().enumerate() {
                    let c = match ch {
                        '#' => wing,
                        'o' => bodyc,
                        _ => continue,
                    };
                    app.layer.set(ox + k as i32, oy + j as i32, c);
                }
            }
        }
    }

    fn update_audience(&mut self, app: &mut App, dt: f32) {
        let t = self.t;
        let (w, hp) = (app.w(), app.hp());
        let seat_y = hp - 2.0;
        let e = self.excite(t);
        let (ccx, ccy) = self.box_center();
        let base = (hp * 0.5 - ccy).atan2(w * 0.5 - ccx);
        let n = self.aud.len().min(app.moths.len());
        let hi = app.hi;
        for i in 0..n {
            let a = &mut self.aud[i];
            let m = &mut app.moths[i];
            let (tx, ty, rate);
            if self.hit && t > T_HIT + 1.6 {
                // 趋光：全体扑向角落里最亮的那只鸭鸭
                let f = i as f32 / (n.max(2) - 1) as f32;
                let ang = base + (f - 0.5) * 2.6 + (t * 2.0 + a.ph).sin() * 0.25;
                let r = self.bw as f32 * 0.55 + 4.0 + (i % 3) as f32 * 4.0 + (t * 3.0 + a.ph).sin() * 1.5;
                tx = ccx + ang.cos() * r;
                ty = ccy + ang.sin() * r;
                rate = 2.4;
                m.flap += dt * 22.0;
            } else if self.hit {
                // 狂欢：满屏乱飞
                tx = w * 0.5 + (t * a.sp * 1.7 + a.ph).sin() * w * 0.42;
                ty = hp * 0.45 + (t * a.sp * 2.2 + a.ph * 2.0).cos() * hp * 0.36;
                rate = 3.0;
                m.flap += dt * 24.0;
            } else {
                a.timer -= dt;
                if a.timer <= 0.0 {
                    a.flutter = 0.45;
                    a.timer = if hi { app.rng.range(3.0, 9.0) } else { app.rng.range(6.0, 14.0) };
                }
                let mut hop = e * a.hop;
                if a.flutter > 0.0 {
                    a.flutter -= dt;
                    hop += 1.5;
                    m.flap += dt * 16.0;
                } else if e > 0.05 {
                    m.flap += dt * (8.0 + 18.0 * e);
                } else {
                    m.flap = 1.0; // 趴着：翅膀收起
                }
                tx = a.seat + (t * 9.0 + a.ph).sin() * e * 2.0;
                ty = seat_y - hop;
                rate = if t < 2.4 { 2.6 } else { 10.0 };
            }
            let kk = (rate * dt).min(1.0);
            m.x += (tx - m.x) * kk;
            m.y += (ty - m.y) * kk;
            m.vx = 0.0;
            m.vy = 0.0;
        }
    }

    /// 屏幕开关机的「电视关机」效果：只留中间 open 比例的行
    fn crt(&self, app: &mut App, open: f32, line: f32, dot: f32) {
        let (w, h) = (app.scr.w as i32, app.scr.h as i32);
        if open > 0.01 {
            app.draw_logs(9.0, 0, 0.0);
            let hh = h as f32 / 2.0 * open;
            for y in 0..h {
                if (y as f32 + 0.5 - h as f32 / 2.0).abs() > hh {
                    fill(app, 0, y, w - 1, y, BLACK);
                }
            }
        }
        let py = h; // 中线像素行
        if line > 0.0 {
            let half = (w as f32 * line / 2.0).max(1.0);
            let cx = w as f32 / 2.0;
            for x in (cx - half) as i32..(cx + half) as i32 {
                app.layer.set(x, py, WHITE);
            }
        }
        if dot > 0.0 {
            let c = scale(WHITE, dot);
            let cx = w / 2;
            app.layer.set(cx - 1, py, c);
            app.layer.set(cx, py, c);
        }
    }
}

/// 白底气泡（3 行），尾巴朝下指向 (cx, by+3)
fn bubble(app: &mut App, cx: i32, by: i32, text: &str) {
    let w = app.scr.w as i32;
    let tw = str_w(text) as i32 + 4;
    let bx = (cx - tw / 2).clamp(0, (w - tw).max(0));
    let bgc = [250, 250, 245];
    for row in 0..3 {
        for x in 0..tw {
            app.scr.put(bx + x, by + row, ' ', BLACK, Some(bgc));
        }
    }
    app.scr.text(bx + 2, by + 1, text, [20, 20, 30], Some(bgc));
    app.scr.put(cx.clamp(bx + 1, bx + tw - 2), by + 3, '▀', bgc, None);
}

impl Interlude for Corner {
    fn name(&self) -> &'static str {
        "撞角落"
    }

    fn update(&mut self, app: &mut App, dt: f32) -> bool {
        self.t += dt;
        let t = self.t;
        if self.size != (app.scr.w, app.scr.h) {
            self.layout(app);
        }
        // 撞墙换色
        if t < T_HIT {
            while self.bounce_i < self.plan.bounces.len() && self.plan.bounces[self.bounce_i].0 <= t {
                self.bounce_i += 1;
                self.color = (self.color + 1 + app.rng.int(COLORS.len() - 2)) % COLORS.len();
                let title = format!("鸭鸭.exe — 待机中 · 反弹 ×{}", self.bounce_i);
                app.set_title(&title);
            }
        } else if !self.hit {
            self.hit = true;
            self.bounce_i += 1;
            app.set_title("鸭鸭.exe — 正中角落！！！");
            // 角落里先喷一大朵，朝屏幕里面
            let (w, hp) = (app.w(), app.hp());
            let px = if self.plan.cx > 0.5 { w - 1.0 } else { 0.0 };
            let a = (hp * 0.5).atan2(w * 0.5 - px);
            let n = if app.hi { 120 } else { 60 };
            let cap = Self::cap(app);
            self.burst(px, 0.0, n, a - 0.8, a + 0.8, w.min(hp) * 0.7, &mut app.rng, cap);
            // 三波烟花从屏幕下方升空，一波比一波大，最后正中间一朵最大
            let r = &mut app.rng;
            for (at, n, size) in [(0.05, 3, 1.0), (0.8, 4, 1.15), (1.55, 4, 1.3)] {
                for i in 0..n {
                    let xf = (i as f32 + r.range(0.25, 0.75)) / n as f32 * 0.84 + 0.08;
                    self.launches.push((T_HIT + at + i as f32 * 0.1 + r.range(0.0, 0.06), xf, r.range(0.22, 0.45), size));
                }
            }
            self.launches.push((T_HIT + 1.75, 0.5, 0.3, 1.9));
        }
        self.update_sparks(app, dt);
        self.update_audience(app, dt);
        if !self.logged && t >= T_ON - 0.05 {
            self.logged = true;
            app.info("display::idle", "鸭鸭撞到角落了（这是历史性的一刻）");
            let s = format!("共反弹 {} 次，观众 {} 只，无人伤亡", self.bounce_i, self.aud.len());
            app.info("display::idle", &s);
            app.push(log_line("WARN", "bug::radar", "全体观众已扑向角落的光源（趋光性，没办法）", Kind::Info));
            app.info("display::pm", "检测到鸭鸭，显示器已唤醒");
            app.set_title("C:\\Windows\\system32\\cmd.exe");
        }
        if !self.handed && t >= T_ON + 0.2 {
            // 大鸭鸭从角落里出来，慢慢游回日常的位置
            self.handed = true;
            let (cx, cy) = self.box_center();
            let (hw, hh) = app.duck_half();
            let (sw, sh, lane) = (app.w(), app.hp(), app.lane_y());
            let d = &mut app.duck;
            d.x = cx.clamp(hw, (sw - hw).max(hw));
            d.y = cy.clamp(hh, (sh - hh).max(hh));
            d.right = self.plan.cx < 0.5;
            d.asleep = false;
            d.glow = 0.0;
            d.glow_to = 1.0;
            d.visible = true;
            d.tx = sw / 2.0;
            d.ty = lane;
        }
        t >= T_END
    }

    fn draw(&mut self, app: &mut App) {
        let t = self.t;
        if t < 1.15 {
            let open = 1.0 - smooth(0.0, 0.5, t);
            let line = if t >= 0.4 && t < 0.88 { 1.0 - smooth(0.5, 0.85, t) } else { 0.0 };
            let dot = if t >= 0.85 { 1.0 - smooth(0.9, 1.15, t) } else { 0.0 };
            self.crt(app, open, line, dot);
            self.draw_audience(app);
            return;
        }
        if t >= T_ON {
            let p = t - T_ON;
            let dot = if p < 0.1 { 1.0 } else { 0.0 };
            let line = if p >= 0.05 && p < 0.3 { smooth(0.05, 0.25, p) } else { 0.0 };
            let open = smooth(0.2, 0.55, p);
            self.crt(app, open, line, dot);
            if app.duck.visible {
                app.draw_glow(app.duck.glow);
                app.draw_duck(None);
            }
            self.draw_audience(app);
            return;
        }
        let a = t - T_HIT;
        if self.hit && a < 0.15 {
            // 撞击瞬间全屏白闪（白 → 灰两档），之后放射线
            let c = if a < 0.07 || !app.hi { [255, 255, 250] } else { [120, 120, 125] };
            let (w, h) = (app.scr.w as i32, app.scr.h as i32);
            fill(app, 0, 0, w - 1, h - 1, c);
        }
        // 角上的计数：高潮时收起来，别压在鸭鸭身上
        if t >= T0 && !self.hit {
            let s = format!("反弹 ×{}", self.bounce_i);
            let x = app.scr.w as i32 - str_w(&s) as i32 - 2;
            app.scr.text(x, 0, &s, [105, 105, 115], None);
        }
        if self.hit {
            self.draw_rays(app, a);
        }
        self.draw_sparks(app);
        self.big_bottom = if self.hit && a > 0.12 && a < 2.5 { self.draw_big(app, a - 0.12) } else { None };
        if t >= T0 {
            self.draw_logo(app);
        }
        self.draw_audience(app);
    }

    fn draw_over(&mut self, app: &mut App) {
        let t = self.t;
        if !(T0..T_ON).contains(&t) {
            return;
        }
        let (w, h) = (app.scr.w as i32, app.scr.h as i32);
        let by = self.area_h - 4;
        // 气泡放在离标志远的那一边，别挡住主角
        let (bx, _) = self.box_at(t);
        let side = if bx + self.bw / 2 > w / 2 { w / 4 } else { w * 3 / 4 };
        for (i, &te) in self.plan.near.iter().enumerate() {
            if t >= te - 0.9 && t < te {
                bubble(app, side, by, "要进了要进了……");
            } else if t >= te && t < te + 1.3 {
                bubble(app, side, by, MISS[i.min(MISS.len() - 1)]);
            } else if t >= te + 1.3 && t < te + 2.2 {
                bubble(app, side, by, SIGH[i.min(SIGH.len() - 1)]);
            }
        }
        if t >= T_HIT - 0.9 && t < T_HIT {
            bubble(app, side, by, "要进了要进了……");
        }
        if !self.hit {
            return;
        }
        let a = t - T_HIT;
        if a > 0.12 && a < 2.5 {
            // 横幅：三行高的彩虹底色块，深色反白字（底色是调色板色，流动不花带宽）
            let text = "正 中 角 落 ！";
            let tw = str_w(text) as i32;
            let pad = if w >= 150 { 10 } else if w >= 60 { 6 } else { 3 };
            let y0 = match self.big_bottom {
                Some(b) => (b + 3).min(h - 4),
                None => (h / 2 - 2).max(1),
            };
            let x0 = (w - tw) / 2;
            for y in y0 - 1..=y0 + 1 {
                for x in x0 - pad..x0 + tw + pad {
                    let c = rb(Band::Text, x as f32 * 1.6, y as f32 * 0.6);
                    app.scr.put_c(x, y, ' ', c, Some(c));
                }
            }
            let mut x = x0;
            for ch in text.chars() {
                if ch != ' ' {
                    let bg = rb(Band::Text, x as f32 * 1.6, y0 as f32 * 0.6);
                    app.scr.put_c(x, y0, ch, Col::T([12, 10, 20]), Some(bg));
                }
                x += char_w(ch) as i32;
            }
            if a > 0.5 {
                let sub = "（这是历史性的一刻）";
                app.scr.text((w - str_w(sub) as i32) / 2, y0 + 3, sub, [190, 190, 200], Some(BLACK));
            }
        }
        if a > 1.9 && a < 3.3 {
            // 鸭鸭被一圈飞蛾围住
            let (cx, cy) = self.box_center();
            let up = if self.plan.cy > 0.5 { (cy / 2.0) as i32 - self.duck_rows - 4 } else { (cy / 2.0) as i32 + self.duck_rows - 1 };
            let x = if self.plan.cx > 0.5 { cx as i32 - self.bw } else { cx as i32 + self.bw };
            bubble(app, x, up.clamp(0, (h - 4).max(0)), "嘎？？");
        }
        if a > 2.55 {
            let s = "（观众一致认为：角落里的鸭鸭最亮）";
            app.scr.text((w - str_w(s) as i32) / 2, (h / 2 - 3).max(1), s, [150, 150, 160], Some(BLACK));
        }
    }
}
