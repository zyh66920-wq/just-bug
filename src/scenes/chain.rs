//! 插曲：珠链——只借「一串珠子沿轨道往终点滚、中心射手射珠子凑三个同色消除」的玩法。
//! 日志从外往里卷成一条螺旋轨道（轨道就是暗下来的日志字符），一串缩成团的彩色飞蛾球从外圈滚进来；
//! 大鸭鸭蹲在螺旋中心，吐彩虹泡自动凑三消，消掉的珠子变回小飞蛾飞走，后段倒吸回来接上，接上又凑成三个就是连击。
//! 结尾：泡泡打光了，链条滚到嘴边——鸭鸭「嘎」一声把整串连同日志轨道吸溜进嘴里，打个嗝，
//! 飞蛾全飞出来绕着它转：回收失败，refcount > 0。日志从中心铺回来。
//!
//! 性能：珠子颜色固定（真彩色但不随时间变），只有位置变化的格子会重绘；轨道用调色板暗彩虹（终端自己流动）。
use super::*;

const T_ROLL0: f32 = 0.3;
const T_ROLL1: f32 = 1.9;
const T_DUCK: f32 = 1.0;
const T_IN0: f32 = 1.7;
const T_IN1: f32 = 3.1;
const T_SHOOT0: f32 = 3.3;
const T_SHOOT1: f32 = 14.2;
const T_FEED: f32 = 11.5;
const T_NEAR: f32 = 14.4;
const T_DUD: f32 = 14.9;
const T_SLURP: f32 = 16.3;
const T_BURP: f32 = 18.0;
const T_GC: f32 = 18.4;
const T_LOVE: f32 = 19.0;
const T_OUT0: f32 = 20.1;
const T_END: f32 = 20.7;

/// 五种飞蛾球的颜色（自己调的糖果色）
const COLORS: [Rgb; 5] = [[255, 118, 150], [96, 226, 170], [104, 178, 255], [255, 214, 92], [186, 140, 255]];
const HUD_BG: Rgb = [28, 26, 40];

struct Bead {
    id: u32,
    c: u8,
    s: f32,
    g: f32, // 刚插进来时 0 → 1 长大
}

struct Shot {
    x: f32,
    y: f32,
    c: u8,
    target: u32,
}

struct Fly {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    shade: u8,
    born: f32,
    ph: f32,
    orbit: Option<f32>, // 绕鸭鸭转时的角度
}

struct Pop {
    x: f32,
    y: f32,
    t0: f32,
    text: String,
}

pub struct Chain {
    t: f32,
    step: usize,
    cx: f32,
    cy: f32,
    small: bool,
    mo: (f32, f32), // 嘴相对中心的偏移（朝右时）
    dh: (f32, f32), // 鸭鸭半宽高
    ex: f32,
    trk: Vec<(f32, f32)>,
    len: f32,
    cells: Vec<(i32, i32, f32, char)>,
    r: i32,
    d: f32,
    beads: Vec<Bead>,
    queue: Vec<u8>,
    next_id: u32,
    shot: Option<Shot>,
    cd: f32,
    hold: u8,
    flies: Vec<Fly>,
    pops: Vec<Pop>,
    joins: Vec<(u32, u32, u32)>,
    best: u32,
    logged_combo: bool,
    eaten: u32,
    jiggle: f32,
    say: Option<(String, f32)>,
}

impl Chain {
    pub fn new(app: &mut App) -> Self {
        let w = app.w();
        let hp = app.hp();
        let small = app.scr.h < 20;
        let ks = app.s() as f32;
        let (r, d) = if ks >= 2.0 { (3, 7.0) } else { (2, 5.0) };
        let rf = r as f32;
        let cx = (w / 2.0).floor();
        let cy = (hp / 2.0).floor();
        let (dh, mo) = if small { ((5.0, 3.5), (4.5, -2.0)) } else { ((9.0 * ks, 7.0 * ks), (7.5 * ks, -2.5 * ks)) };
        let top = if small { 4.0 } else { 2.0 };
        let ry_max = (cy - top - rf).max(4.0);
        let rx_max = (cx - 2.0 - rf).max(4.0);
        let ex = (rx_max / ry_max).clamp(1.0, 2.2);
        let mut r_in = (dh.1 + rf + 2.0).min(ry_max * 0.8);
        r_in = r_in.max(((dh.0 + rf + 2.0) / ex).min(ry_max * 0.8));
        let pitch = d + if ks >= 2.0 { 5.0 } else { 3.0 };
        let turns = ((ry_max - r_in) / pitch).clamp(0.75, 2.0);
        let th_out = turns * TAU;
        let b = (ry_max - r_in).max(0.0) / th_out;

        // 螺旋（外 → 内），最后接一小段直线进嘴
        let n = 4000;
        let mut raw: Vec<(f32, f32)> = (0..=n)
            .map(|i| {
                let th = th_out * (1.0 - i as f32 / n as f32);
                let rr = r_in + b * th;
                (cx + rr * th.cos() * ex, cy + rr * th.sin())
            })
            .collect();
        raw.push((cx + mo.0 + rf, cy + mo.1));
        let mut trk = vec![raw[0]];
        let mut left = 0.0f32;
        for i in 1..raw.len() {
            let (ax, ay) = raw[i - 1];
            let (bx, by) = raw[i];
            let l = ((bx - ax).powi(2) + (by - ay).powi(2)).sqrt();
            if l <= 1e-6 {
                continue;
            }
            let mut u = 0.0;
            while left + (l - u) >= 1.0 {
                u += 1.0 - left;
                left = 0.0;
                let f = u / l;
                trk.push((ax + (bx - ax) * f, ay + (by - ay) * f));
            }
            left += l - u;
        }
        if trk.len() < 2 {
            trk.push((cx + mo.0, cy + mo.1));
        }
        let len = (trk.len() - 1) as f32;

        // 轨道字符：从最近的日志里取可见 ASCII 字符
        let mut src: Vec<char> = Vec::new();
        for l in app.logs.iter().rev().take(30) {
            src.extend(l.text.chars().filter(|c| c.is_ascii_graphic()));
            if src.len() > 4000 {
                break;
            }
        }
        if src.is_empty() {
            src = "rainbow::core INFO duck=ok".chars().filter(|c| c.is_ascii_graphic()).collect();
        }
        let (sw, sh) = (app.scr.w as i32, app.scr.h as i32);
        let mut cells: Vec<(i32, i32, f32, char)> = Vec::new();
        let mut k = 0usize;
        for (i, &(x, y)) in trk.iter().enumerate() {
            let (gx, gy) = (x.round() as i32, (y / 2.0).floor() as i32);
            if gx < 0 || gy < 1 || gx >= sw || gy >= sh {
                continue;
            }
            if cells.last().map_or(false, |c| c.0 == gx && c.1 == gy) {
                continue;
            }
            let ch = if k % 4 == 3 { '·' } else { src[k % src.len()] };
            k += 1;
            cells.push((gx, gy, i as f32, ch));
        }

        app.duck.visible = true;
        app.duck.asleep = false;
        app.duck.glow_to = 0.5;
        app.duck.x = cx;
        app.duck.tx = cx;
        app.duck.y = cy;
        app.duck.ty = cy;
        app.duck.right = true;
        app.duck.bubble = None;
        app.push(log_line("INFO", "chain::shooter", "珠链.exe 已加载：自动瞄准 v0.3（只会凑三个）", Kind::Info));
        app.set_title("鸭鸭.exe — 珠链");

        let mut me = Chain {
            t: 0.0,
            step: 0,
            cx,
            cy,
            small,
            mo,
            dh,
            ex,
            trk,
            len,
            cells,
            r,
            d,
            beads: Vec::new(),
            queue: Vec::new(),
            next_id: 1,
            shot: None,
            cd: 0.0,
            hold: 0,
            flies: Vec::new(),
            pops: Vec::new(),
            joins: Vec::new(),
            best: 0,
            logged_combo: false,
            eaten: 0,
            jiggle: 0.0,
            say: None,
        };
        let c = me.next_color(app, 255);
        me.beads.push(Bead { id: 0, c, s: -2.0 * d, g: 1.0 });
        me
    }

    fn pos(&self, s: f32) -> (f32, f32) {
        let s = s.clamp(0.0, self.len);
        let i = (s.floor() as usize).min(self.trk.len() - 2);
        let f = s - i as f32;
        let (a, b) = (self.trk[i], self.trk[i + 1]);
        (a.0 + (b.0 - a.0) * f, a.1 + (b.1 - a.1) * f)
    }

    fn mouth(&self, right: bool) -> (f32, f32) {
        (self.cx + if right { self.mo.0 } else { -self.mo.0 }, self.cy + self.mo.1)
    }

    /// 小屏自己画气泡：App 的鸭子气泡按 18 列宽算位置，40x12 上会落到状态栏那一行
    fn say(&mut self, app: &mut App, text: &str, dur: f32) {
        if self.small {
            self.say = Some((text.to_string(), self.t + dur));
        } else {
            app.quack(text, dur);
        }
    }

    /// 生成链条颜色：大多是 1–2 颗一组，夹着能触发连击的「X X Y Y X」「X X Z Z Y Y Z X」
    fn next_color(&mut self, app: &mut App, tail: u8) -> u8 {
        if self.queue.is_empty() {
            let pick = |app: &mut App, not: &[u8]| loop {
                let c = app.rng.int(COLORS.len()) as u8;
                if !not.contains(&c) {
                    break c;
                }
            };
            let x = pick(app, &[tail]);
            let roll = app.rng.f();
            if roll < 0.3 {
                let y = pick(app, &[x]);
                self.queue.extend([x, x, y, y, x]);
            } else if roll < 0.42 {
                let z = pick(app, &[x]);
                let y = pick(app, &[x, z]);
                self.queue.extend([x, x, z, z, y, y, z, x]);
            } else {
                let n = 1 + app.rng.int(2);
                self.queue.extend(std::iter::repeat(x).take(n));
            }
            // 队列是从链尾往外倒着长的
            self.queue.reverse();
        }
        self.queue.pop().unwrap_or(0)
    }

    fn idx(&self, id: u32) -> Option<usize> {
        self.beads.iter().position(|b| b.id == id)
    }

    fn adjacent(&self, i: usize) -> bool {
        // i 和 i+1 是否贴着
        self.beads[i + 1].s - self.beads[i].s <= self.d * 1.15
    }

    /// 检查 i 所在的同色段，够 3 个就消掉。combo = 这是第几段连击
    fn check(&mut self, app: &mut App, i: usize, combo: u32) -> bool {
        if i >= self.beads.len() {
            return false;
        }
        let c = self.beads[i].c;
        let mut a = i;
        while a > 0 && self.beads[a - 1].c == c && self.adjacent(a - 1) {
            a -= 1;
        }
        let mut b = i;
        while b + 1 < self.beads.len() && self.beads[b + 1].c == c && self.adjacent(b) {
            b += 1;
        }
        if b - a + 1 < 3 {
            return false;
        }
        let (mx, my) = self.pos(self.beads[(a + b) / 2].s);
        let cap = if app.hi { 70 } else { 10 };
        for k in a..=b {
            let s = self.beads[k].s;
            if s < 0.0 || self.flies.len() >= cap {
                continue;
            }
            let (x, y) = self.pos(s);
            let (ex, ey) = (x - self.cx, y - self.cy);
            let l = (ex * ex + ey * ey).sqrt().max(1.0);
            let sp = app.rng.range(28.0, 55.0);
            self.flies.push(Fly {
                x,
                y,
                vx: ex / l * sp + app.rng.range(-12.0, 12.0),
                vy: ey / l * sp - app.rng.range(8.0, 25.0),
                shade: [0u8, 2, 3, 4][app.rng.int(4)],
                born: self.t,
                ph: app.rng.f() * 2.0,
                orbit: None,
            });
        }
        self.beads.drain(a..=b);
        if combo >= 2 {
            self.pops.push(Pop { x: mx, y: my, t0: self.t, text: format!("×{} 连击", combo) });
            self.best = self.best.max(combo);
            if !self.logged_combo {
                self.logged_combo = true;
                app.push(log_line("INFO", "chain::gc", &format!("×{} 连击：链表合并后又触发了一次 GC", combo), Kind::Fixed));
            }
        }
        if a > 0 && a < self.beads.len() {
            self.joins.push((self.beads[a - 1].id, self.beads[a].id, combo + 1));
        }
        true
    }

    /// 自动瞄准：优先能消的、能连击的、离嘴近的
    fn aim(&mut self, app: &mut App) -> Option<(u32, u8)> {
        let n = self.beads.len();
        let mut best: Option<(f32, u32, u8)> = None;
        let mut a = 0;
        while a < n {
            let c = self.beads[a].c;
            let mut b = a;
            while b + 1 < n && self.beads[b + 1].c == c && self.adjacent(b) {
                b += 1;
            }
            let s = self.beads[b].s;
            if self.beads[a].s >= 2.0 && s < self.len - self.d * 2.0 {
                let l = b - a + 1;
                let mut sc = s / self.len * 40.0 + app.rng.f() * 18.0;
                if l >= 2 {
                    sc += 100.0;
                    if a > 0 && b + 1 < n && self.beads[a - 1].c == self.beads[b + 1].c {
                        sc += 70.0;
                    }
                }
                if best.map_or(true, |x| sc > x.0) {
                    best = Some((sc, self.beads[b].id, c));
                }
            }
            a = b + 1;
        }
        best.map(|(_, id, c)| (id, c))
    }

    fn land(&mut self, app: &mut App, tid: u32, c: u8) {
        if let Some(i) = self.idx(tid) {
            let s = self.beads[i].s;
            let id = self.next_id;
            self.next_id += 1;
            self.beads.insert(i + 1, Bead { id, c, s, g: 0.0 });
            self.check(app, i + 1, 1);
        }
    }

    /// 链头该在的位置（占轨道总长的比例）
    fn front_goal(&self) -> f32 {
        let t = self.t;
        let l = self.len;
        if t < T_IN0 {
            -self.d
        } else if t < T_IN1 {
            smooth(T_IN0, T_IN1, t) * 0.36 * l
        } else if t < T_NEAR {
            l * (0.36 + 0.48 * (t - T_IN1) / (T_NEAR - T_IN1))
        } else {
            l * 0.84 + (l - self.d * 0.8 - l * 0.84) * smooth(T_NEAR, T_SLURP - 0.2, t)
        }
    }

    fn sp(&self, i: usize) -> f32 {
        // 新插进来的珠子 g 从 0 长到 1：两侧间距各 d/2 → d，前段被慢慢挤开而不是瞬移
        self.d * 0.5 * (self.beads[i - 1].g + self.beads[i].g)
    }

    fn mask_e(app: &App, x: i32, y: i32) -> f32 {
        let (w, h) = (app.scr.w as f32, app.scr.h as f32);
        let ex = (x as f32 + 0.5 - w / 2.0) / (w / 2.0);
        let ey = (y as f32 + 0.5 - h / 2.0) / (h / 2.0);
        (ex * ex + ey * ey).sqrt()
    }

    /// 日志只保留椭圆距离 < lim 的部分，其余清空
    fn logs_masked(app: &mut App, lim: f32) {
        app.draw_logs(9.0, 0, 0.0);
        let (w, h) = (app.scr.w as i32, app.scr.h as i32);
        for y in 0..h {
            for x in 0..w {
                if Self::mask_e(app, x, y) >= lim {
                    app.scr.put(x, y, ' ', GRAY, Some(BLACK));
                }
            }
        }
    }

    fn draw_bead(app: &mut App, x: f32, y: f32, c: Rgb, r: i32, g: f32, wig: bool) {
        let r = if g < 0.5 { (r - 1).max(1) } else { r };
        let (ox, oy) = (x.round() as i32, y.round() as i32);
        let light = mix(c, WHITE, 0.45);
        let dark = scale(c, 0.7);
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy > r * r + r {
                    continue;
                }
                let col = if dy < 0 && dx.abs() >= 1 {
                    light // 收起来的翅膀
                } else if dy >= r - 1 && r >= 2 {
                    dark
                } else {
                    c
                };
                app.layer.set(ox + dx, oy + dy, col);
            }
        }
        // 两根小触角
        let a = if wig { 2 } else { 1 };
        let ac = scale(c, 0.8);
        app.layer.set(ox - a, oy - r - 1, ac);
        app.layer.set(ox + a, oy - r - 1, ac);
    }
}

impl Interlude for Chain {
    fn name(&self) -> &'static str {
        "珠链"
    }

    fn update(&mut self, app: &mut App, dt: f32) -> bool {
        self.t += dt;
        let t = self.t;
        let d = self.d;

        // ── 链条前进 ──
        if !self.beads.is_empty() {
            let slurp = t >= T_SLURP;
            let front = self.beads.last().map(|b| b.s).unwrap_or(0.0);
            let v = if slurp {
                (60.0 + 1100.0 * (t - T_SLURP)) * (self.len / 900.0).clamp(0.2, 1.0)
            } else {
                let vmax = if t < T_IN1 + 0.3 {
                    900.0
                } else if t >= T_NEAR {
                    self.len * 0.3 + 8.0
                } else {
                    self.len * 0.12 + 8.0
                };
                ((self.front_goal() - front) * 4.0).clamp(0.0, vmax)
            };
            self.beads[0].s += v * dt;
            for b in self.beads.iter_mut() {
                b.g = (b.g + dt * 7.0).min(1.0);
            }
            let vs = (self.len * 0.25).max(60.0);
            for i in 1..self.beads.len() {
                let goal = self.beads[i - 1].s + self.sp(i);
                let s = self.beads[i].s;
                if s < goal {
                    self.beads[i].s = goal;
                } else if s > goal + 0.01 {
                    self.beads[i].s = (s - vs * dt).max(goal);
                }
            }
            // 倒吸接上后看看能不能连击
            let joins = std::mem::take(&mut self.joins);
            let mut keep = Vec::new();
            for (bid, fid, combo) in joins {
                match (self.idx(bid), self.idx(fid)) {
                    (Some(bi), Some(fi)) if fi == bi + 1 => {
                        if self.beads[fi].s - self.beads[bi].s <= self.sp(fi) + 0.05 {
                            if self.beads[fi].c == self.beads[bi].c {
                                self.check(app, fi, combo);
                            }
                        } else {
                            keep.push((bid, fid, combo));
                        }
                    }
                    _ => {}
                }
            }
            self.joins.extend(keep);
            // 吃进嘴里
            if slurp {
                while self.beads.last().map_or(false, |b| b.s >= self.len - 0.5) {
                    self.beads.pop();
                    self.eaten += 1;
                }
                if t > T_BURP - 0.2 {
                    self.eaten += self.beads.len() as u32;
                    self.beads.clear();
                }
                self.jiggle = if self.beads.is_empty() { 0.0 } else { ((t * 13.0) as i32 % 2) as f32 };
            } else if let Some(b) = self.beads.last_mut() {
                b.s = b.s.min(self.len - d * 0.6);
            }
        }
        // 外圈续上新珠子
        if t < T_FEED {
            while self.beads.first().map_or(false, |b| b.s > -d) {
                let tail = self.beads[0].c;
                let s = self.beads[0].s - d;
                let c = self.next_color(app, tail);
                let id = self.next_id;
                self.next_id += 1;
                self.beads.insert(0, Bead { id, c, s, g: 1.0 });
            }
        }

        // ── 鸭鸭射击 ──
        if (T_SHOOT0..T_SHOOT1).contains(&t) {
            self.cd -= dt;
            // 链条太短就先不打——留着给结尾吸溜
            let vis = self.beads.iter().filter(|b| b.s >= 0.0).count() as f32;
            let keep = (self.len / self.d * 0.45).max(8.0);
            if self.shot.is_none() && self.cd <= 0.0 && vis >= keep {
                if let Some((tid, c)) = self.aim(app) {
                    let (tx, _) = self.idx(tid).map(|i| self.pos(self.beads[i].s)).unwrap_or((self.cx, self.cy));
                    let right = tx >= self.cx;
                    app.duck.right = right;
                    let (mx, my) = self.mouth(right);
                    self.shot = Some(Shot { x: mx, y: my, c, target: tid });
                    self.cd = if app.hi { 0.5 } else { 0.62 };
                    self.hold = app.rng.int(COLORS.len()) as u8;
                }
            }
        }
        if let Some(mut sh) = self.shot.take() {
            match self.idx(sh.target) {
                Some(i) => {
                    let (tx, ty) = self.pos(self.beads[i].s);
                    let (ex, ey) = (tx - sh.x, ty - sh.y);
                    let l = (ex * ex + ey * ey).sqrt();
                    let step = (self.len * 0.4).max(160.0) * dt;
                    if l <= step + 1.0 {
                        self.land(app, sh.target, sh.c);
                    } else {
                        sh.x += ex / l * step;
                        sh.y += ey / l * step;
                        self.shot = Some(sh);
                    }
                }
                None => {}
            }
        }

        // ── 飞蛾 ──
        let (cx, cy) = (self.cx, self.cy);
        let (orx, ory) = ((self.dh.0 + 9.0) * self.ex.min(1.5), self.dh.1 + if self.small { 4.0 } else { 6.0 });
        let (w, hp) = (app.w(), app.hp());
        self.flies.retain_mut(|f| {
            f.ph += dt * 9.0;
            match f.orbit.as_mut() {
                Some(a) => {
                    *a += dt * 1.5;
                    let (gx, gy) = (cx + a.cos() * orx, cy + a.sin() * ory);
                    let k = (dt * 3.0).min(1.0);
                    f.x += (gx - f.x) * k;
                    f.y += (gy - f.y) * k;
                    true
                }
                None => {
                    f.vx *= 1.0 + dt * 1.2;
                    f.vy *= 1.0 + dt * 1.2;
                    f.x += f.vx * dt;
                    f.y += f.vy * dt;
                    f.x > -6.0 && f.x < w + 6.0 && f.y > -6.0 && f.y < hp + 6.0 && t - f.born < 3.0
                }
            }
        });
        self.pops.retain(|p| t - p.t0 < 1.1);

        // ── 剧情 ──
        if self.step == 0 && t >= T_DUD {
            self.step += 1;
            app.duck.right = true;
            self.say(app, "噗……", 0.9);
            app.push(log_line("ERROR", "chain::shooter", "BubblePoolExhausted：泡泡打光了，下一发要等 GC", Kind::Error));
        }
        if self.step == 1 && t >= T_SLURP {
            self.step += 1;
            app.duck.right = true;
            self.say(app, "嘎！", 0.5);
            app.push(log_line("WARN", "duck::mouth", "链表即将溢出到嘴里——改用 O(1) 一口吞下", Kind::Info));
        }
        if self.step == 2 && t >= T_SLURP + 0.5 {
            self.step += 1;
            self.say(app, "吸溜——", 1.1);
        }
        if self.step == 3 && t >= T_BURP {
            self.step += 1;
            self.say(app, "嗝", 0.6);
            let n = (self.eaten as usize).clamp(6, if app.hi { 14 } else { 7 });
            let (mx, my) = self.mouth(true);
            for i in 0..n {
                let a = i as f32 / n as f32 * TAU;
                self.flies.push(Fly {
                    x: mx,
                    y: my,
                    vx: a.cos() * 60.0,
                    vy: a.sin() * 40.0 - 20.0,
                    shade: [0u8, 2, 3, 4, 1][i % 5],
                    born: t,
                    ph: i as f32 * 0.7,
                    orbit: Some(a + 0.4),
                });
            }
        }
        if self.step == 4 && t >= T_GC {
            self.step += 1;
            app.push(log_line("WARN", "duck::gc", "回收失败：这些 bug 还被鸭鸭引用着（refcount > 0）", Kind::Info));
        }
        if self.step == 5 && t >= T_LOVE {
            self.step += 1;
            self.say(app, "嘎，舍不得 free", 1.3);
        }
        app.duck.x = cx;
        app.duck.tx = cx;
        app.duck.y = cy + self.jiggle;
        app.duck.ty = app.duck.y;
        t >= T_END
    }

    fn draw(&mut self, app: &mut App) {
        let t = self.t;
        let roll = smooth(T_ROLL0, T_ROLL1, t);
        let out = smooth(T_OUT0, T_END - 0.08, t);
        // 日志：开场从外往里收掉，收尾从中心铺回来
        if roll < 1.0 {
            Self::logs_masked(app, (1.0 - roll) * 1.5);
        } else if out > 0.0 {
            Self::logs_masked(app, out * 1.5);
        }
        let olim = out * 1.5;
        let hidden = |app: &App, x: f32, y: f32| out > 0.0 && Self::mask_e(app, x as i32, (y / 2.0) as i32) < olim;

        // 轨道（日志字符卷成的螺旋）
        let smax = roll * self.len * 1.05;
        let smin = if t >= T_SLURP { self.beads.first().map_or(self.len + 1.0, |b| b.s.max(0.0)) } else { 0.0 };
        for i in 0..self.cells.len() {
            let (x, y, s, ch) = self.cells[i];
            if s > smax || s < smin {
                continue;
            }
            if out > 0.0 && Self::mask_e(app, x, y) < olim {
                continue;
            }
            let c = rb(Band::Dim, x as f32, y as f32 * 2.0);
            app.scr.put_c(x, y, ch, c, None);
        }

        // 珠子
        let hi = app.hi;
        for i in 0..self.beads.len() {
            let b = &self.beads[i];
            if b.s < 0.0 || b.s > self.len {
                continue;
            }
            let (x, y) = self.pos(b.s);
            if hidden(app, x, y) {
                continue;
            }
            if self.small && y - ((self.r + 1) as f32) < 2.0 {
                continue;
            }
            let wig = hi && ((t * 3.0 + b.id as f32 * 0.37) as i32) % 4 == 0;
            Self::draw_bead(app, x, y, COLORS[b.c as usize], self.r, b.g, wig);
        }

        // 飞行中的彩虹泡：先画，贴着身体的部分会被后面的鸭鸭挡住
        if let Some(sh) = &self.shot {
            let (x, y, c) = (sh.x, sh.y, COLORS[sh.c as usize]);
            let r = self.r;
            let (ox, oy) = (x.round() as i32, y.round() as i32);
            for dy in -r..=r {
                for dx in -r..=r {
                    let q = dx * dx + dy * dy;
                    if q > r * r + r {
                        continue;
                    }
                    let col = if (dx, dy) == (-1, -1) { WHITE } else if q >= r * r - r + 1 { mix(c, WHITE, 0.5) } else { c };
                    app.layer.set(ox + dx, oy + dy, col);
                }
            }
            // 彩虹尾巴
            let (mx, my) = self.mouth(app.duck.right);
            let (ex, ey) = (x - mx, y - my);
            let l = (ex * ex + ey * ey).sqrt().max(1.0);
            let n = if hi { 4 } else { 2 };
            for k in 1..=n {
                let back = (r + 2 + k * 3) as f32;
                if back > l {
                    break;
                }
                let (px, py) = (x - ex / l * back, y - ey / l * back);
                app.layer.set(px.round() as i32, py.round() as i32, rb(Band::Text, px, py));
            }
        }

        // 鸭鸭：珠子、泡泡都盖不住它
        if t >= T_DUCK && out < 0.12 {
            if self.small {
                mini_duck(app, app.duck.x, app.duck.y, app.duck.right, 1);
            } else {
                app.draw_glow(app.duck.glow * smooth(T_DUCK, T_DUCK + 0.6, t));
                app.draw_duck(None);
            }
        }

        // 嘴边含着的下一发（小屏放到嘴外，免得压住鸭鸭）
        let shooting = (T_SHOOT0..T_SHOOT1).contains(&t);
        if shooting && self.shot.is_none() {
            let (mx, my) = self.mouth(app.duck.right);
            let dir = if app.duck.right { 1.0 } else { -1.0 };
            let d = if self.small { 4.0 } else { 1.0 };
            Self::draw_bead(app, mx + dir * d, my, COLORS[self.hold as usize], (self.r - 1).max(1), 1.0, false);
        }

        // 没打出来的那一发（小屏往外挪，别压到鸭鸭）
        if (T_DUD..T_DUD + 0.5).contains(&t) {
            let (mx0, my) = self.mouth(true);
            let mx = mx0 + if self.small { 6.5 } else { 2.0 };
            let u = (t - T_DUD) / 0.5;
            if u < 0.45 {
                let rr = 1 + (u * 4.0) as i32;
                for k in 0..8 {
                    let a = k as f32 / 8.0 * TAU;
                    app.layer.set((mx + a.cos() * rr as f32).round() as i32, (my + a.sin() * rr as f32).round() as i32, [200, 200, 210]);
                }
            } else {
                for k in 0..3 {
                    let a = k as f32 * 2.1 - 1.0;
                    let rr = 3.0 + u * 5.0;
                    app.layer.set((mx + a.cos() * rr).round() as i32, (my + a.sin() * rr).round() as i32, [120, 120, 130]);
                }
            }
        }

        // 飞蛾
        for i in 0..self.flies.len() {
            let f = &self.flies[i];
            if hidden(app, f.x, f.y) {
                continue;
            }
            let (x, y, s, fl) = (f.x, f.y.max(3.0), f.shade, f.ph);
            let rain = if hi { (1.0 - (t - f.born) / 0.9).clamp(0.0, 1.0) } else { 0.0 };
            let fl = if hi { fl } else { fl * 0.5 };
            moth(app, x, y, s, fl, rain);
        }
    }

    fn draw_over(&mut self, app: &mut App) {
        let t = self.t;
        if t >= T_ROLL1 - 0.3 && t < T_OUT0 {
            let s = if t >= T_BURP {
                " 珠链 — 已清空 — 回收失败 ".to_string()
            } else if t >= T_SLURP {
                format!(" 珠链 — 剩 {} 颗 — 吸溜中 ", self.beads.iter().filter(|b| b.s >= 0.0).count())
            } else if t >= T_DUD {
                format!(" 珠链 — 剩 {} 颗 — 泡泡 0 ", self.beads.iter().filter(|b| b.s >= 0.0).count())
            } else {
                let n = self.beads.iter().filter(|b| b.s >= 0.0).count();
                if self.best >= 2 {
                    format!(" 珠链 — 剩 {} 颗 — 最高 ×{} ", n, self.best)
                } else {
                    format!(" 珠链 — 剩 {} 颗 ", n)
                }
            };
            let w = app.scr.w as i32;
            let x = ((w - str_w(&s) as i32) / 2).max(0);
            app.scr.text(x, 0, &s, [235, 235, 240], Some(HUD_BG));
        }
        if self.small {
            if let Some((text, until)) = &self.say {
                if t < *until {
                    let w = app.scr.w as i32;
                    let tw = (str_w(text) as i32 + 3).min((w - 2).max(3));
                    let x = (self.cx as i32 + 4).clamp(1, (w - tw - 1).max(1));
                    let y = 1;
                    for i in 0..tw {
                        app.scr.put(x + i, y, ' ', BLACK, Some([250, 250, 245]));
                    }
                    app.scr.text(x + 1, y, text, [20, 20, 30], Some([250, 250, 245]));
                }
            }
        }
        for p in &self.pops {
            let age = t - p.t0;
            let y = ((p.y / 2.0) as i32 - 2 - (age * 3.0) as i32).max(1);
            let x = p.x as i32 - str_w(&p.text) as i32 / 2;
            let fg = if age < 0.15 { WHITE } else { [255, 226, 120] };
            app.scr.text(x, y, &p.text, fg, Some([50, 30, 60]));
        }
    }
}
