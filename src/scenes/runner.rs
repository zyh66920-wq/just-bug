//! 插曲：鸭鸭跑酷——只借「横版无尽跑、自动跳障碍、越跑越快」的玩法。
//! 日志整页沉下去，最后一行留下来当地平线（一直向左滚的日志），没沉下去的日志碎片挂在天上当云；
//! 鸭鸭在日志上跑，跳过趴在地上的飞蛾、压扁自己躲过低飞的飞蛾；
//! 最后来了一堵跳不过去的 Bug 墙——「检测到一个无法跳过的 bug」——墙顶的飞蛾把鸭鸭抬了过去。
//!
//! 性能：天空大面积纯黑静止；每帧真正在变的只有地平线那一行、几朵云、鸭鸭和飞蛾。
use super::*;

// 时间轴（插曲开始后第几秒）
const T_SINK0: f32 = 0.15;
const T_SINK1: f32 = 1.35;
const T_ENTER0: f32 = 1.3;
const T_ENTER1: f32 = 2.4;
const T_RUN0: f32 = 1.7;
const T_RUN1: f32 = 2.5;
const T_B0: f32 = 13.6; // 开始刹车（墙来了）
const T_STOP: f32 = 15.4;
const T_HOP: f32 = 16.2; // 不死心地跳一下
const HOP_T: f32 = 0.36;
const T_WAVE: f32 = 16.9; // 墙上的飞蛾认出了鸭鸭
const T_HELP0: f32 = 17.3;
const T_HELP1: f32 = 18.0;
const T_LIFT0: f32 = 18.0;
const T_LIFT1: f32 = 20.1;
const T_INF: f32 = 18.9;
const T_SCAT0: f32 = 20.5;
const T_RISE0: f32 = 21.3;
const T_RISE1: f32 = 22.0;
const T_END: f32 = 22.4;
const DT_TAB: f32 = 0.005;

// 障碍种类
const G1: u8 = 0; // 一只趴着
const G2: u8 = 1; // 两只摞起来
const PY: u8 = 2; // 三只叠成小堆
const FLY: u8 = 3; // 低空飞来
const T3: u8 = 4; // 三只摞成塔

/// (鸭鸭和它正面相遇的时刻, 种类, 是否险过)
const PLAN: [(f32, u8, bool); 10] = [
    (3.6, G1, false),
    (4.9, G2, false),
    (6.1, FLY, false),
    (7.2, PY, false),
    (8.3, G1, true),
    (9.3, FLY, false),
    (10.2, T3, false),
    (11.0, G1, false),
    (11.8, FLY, false),
    (12.7, PY, true),
];

const ORANGE: Rgb = [255, 160, 40];
const OUTLINE: Rgb = [14, 12, 24];

struct Ob {
    kind: u8,
    tp: f32,
    x: f32, // 世界坐标（中心，列）
    close: bool,
    shade: [u8; 3],
    ph: f32,
    j0: f32, // 起跳时刻
    jt: f32, // 滞空时间
    jh: f32, // 跳多高（像素）
    d0: f32, // 蹲下区间
    d1: f32,
}

struct Cloud {
    cells: Vec<Cell>,
    x: f32,
    row: i32,
    f: f32, // 视差系数
}

struct Wm {
    c: i32,
    r: i32,
    shade: u8,
    ph: f32,
    vx: f32,
    vy: f32,
    helper: i32, // -1 普通墙砖；否则是托鸭鸭的第几号
}

pub struct Runner {
    t: f32,
    step: usize,
    title_cd: f32,
    seed: u64,
    sw: usize,
    sh: usize,
    k: i32,
    gy: i32,
    gtop: i32,
    dx: f32,
    u: f32,
    tab: Vec<f32>,
    obs: Vec<Ob>,
    wall_x: f32,
    wnc: i32,
    wnr: i32,
    wcw: i32,
    wch: i32,
    land_x: f32,
    lift_l: f32,
    wall: Vec<Wm>,
    snap: Vec<Cell>,
    snap_w: usize,
    snap_h: usize,
    rise: Option<Vec<Cell>>,
    ground: Vec<(char, Col)>,
    clouds: Vec<Cloud>,
}

/// 一行日志按 draw_logs 的配色拆成「每列一格」，宽字符右半格记成 '\0'
fn line_cols(l: &Line, out: &mut Vec<(char, Col)>) {
    let ly = (l.no % 10_000) as f32;
    let mut lx = 0usize;
    for (ci, ch) in l.text.chars().enumerate() {
        let cw = char_w(ch);
        let e = l.eat.get(lx).copied().unwrap_or(0);
        let tail = ci >= l.tail && l.kind != Kind::Fatal;
        let mut fg: Col = match l.kind {
            Kind::Error => RED.into(),
            Kind::Fatal => WHITE.into(),
            Kind::Fixed => GREEN.into(),
            _ => rb(if tail { Band::Dim } else { Band::Text }, lx as f32, ly),
        };
        if tail {
            if let Col::T(c) = fg {
                fg = Col::T(scale(c, 0.42));
            }
        }
        if e == 1 {
            fg = Col::T(if tail { [62, 62, 62] } else { [138, 138, 138] });
        }
        if e == 2 {
            for _ in 0..cw {
                out.push((' ', fg));
            }
        } else {
            out.push((ch, fg));
            if cw == 2 {
                out.push(('\0', fg));
            }
        }
        lx += cw;
    }
}

fn moth_cols(shade: u8) -> (Rgb, Rgb) {
    let wing = match shade {
        0 | 1 => [238, 238, 238],
        2 => [175, 175, 175],
        3 => [120, 120, 120],
        _ => [44, 44, 52],
    };
    let body = if shade == 4 { [235, 235, 235] } else { scale(wing, 0.4) };
    (wing, body)
}

/// 放大 k 倍的飞蛾（5k × 3k 像素），(left, top) 是左上像素；clip 以下不画（别压到地平线那一行字）
fn moth_px(app: &mut App, left: i32, top: i32, k: i32, shade: u8, frame: usize, clip: i32) {
    let (wing, body) = moth_cols(shade);
    for (j, row) in MOTH[frame & 1].iter().enumerate() {
        for (i, ch) in row.chars().enumerate() {
            let c = match ch {
                '#' => wing,
                'o' => body,
                _ => continue,
            };
            for sy in 0..k {
                for sx in 0..k {
                    let y = top + j as i32 * k + sy;
                    if y < clip {
                        app.layer.set(left + i as i32 * k + sx, y, c);
                    }
                }
            }
        }
    }
}

fn bubble(app: &mut App, ax: i32, top: i32, text: &str, right: bool) {
    let w = app.scr.w as i32;
    let tw = str_w(text) as i32 + 4;
    let bx = if right { ax } else { ax - tw };
    let bx = bx.clamp(0, (w - tw).max(0));
    let top = top.max(1);
    let bgc = [250, 250, 245];
    for row in 0..3 {
        for x in 0..tw {
            app.scr.put(bx + x, top + row, ' ', BLACK, Some(bgc));
        }
    }
    app.scr.text(bx + 2, top + 1, text, [20, 20, 30], Some(bgc));
    let tx = (if right { ax + 1 } else { ax - 2 }).clamp(bx + 1, bx + tw - 2);
    app.scr.put(tx, top + 3, '▀', bgc, None);
}

fn ease_out(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    1.0 - (1.0 - x) * (1.0 - x)
}

impl Runner {
    pub fn new(app: &mut App) -> Self {
        let seed = app.rng.next();
        app.duck.visible = false;
        app.duck.bubble = None;
        app.set_title("鸭鸭跑酷 — 0000");
        // 拍一张「现在的日志画面」，开场让它整页往下沉
        app.scr.clear();
        app.draw_logs(9.0, 0, 0.0);
        let snap = app.scr.cells.clone();
        app.scr.clear();
        app.scr.scroll = 0;
        app.last_top = None;
        // 地平线：最后一行日志接着往前几行，拼成一条足够长的滚动带
        let mut ground = Vec::new();
        let n = app.logs.len();
        if n == 0 {
            line_cols(&log_line("INFO", "duck::run", "鸭鸭.exe 进入跑酷模式  trace=00000000  duck=ok", Kind::Info), &mut ground);
        } else {
            let mut i = 0;
            while ground.len() < 2600 && i < 4 * n.max(1) + 20 {
                line_cols(&app.logs[n - 1 - (i % n)], &mut ground);
                ground.extend([(' ', Col::T(GRAY)); 3]);
                i += 1;
            }
        }
        let mut r = Runner {
            t: 0.0,
            step: 0,
            title_cd: 0.0,
            seed,
            sw: 0,
            sh: 0,
            k: 1,
            gy: 0,
            gtop: 0,
            dx: 0.0,
            u: 1.0,
            tab: Vec::new(),
            obs: Vec::new(),
            wall_x: 0.0,
            wnc: 1,
            wnr: 2,
            wcw: 6,
            wch: 3,
            land_x: 0.0,
            lift_l: 0.0,
            wall: Vec::new(),
            snap,
            snap_w: app.scr.w,
            snap_h: app.scr.h,
            rise: None,
            ground,
            clouds: Vec::new(),
        };
        r.plan(app);
        r.make_clouds(app);
        r
    }

    /// 按当前屏幕尺寸排好整局：速度表、障碍、跳跃时机、Bug 墙
    fn plan(&mut self, app: &App) {
        let (w, h) = (app.scr.w as i32, app.scr.h as i32);
        self.sw = app.scr.w;
        self.sh = app.scr.h;
        let k = if w >= 150 && h >= 40 { 2 } else { 1 };
        self.k = k;
        self.gy = h - 1;
        self.gtop = self.gy * 2;
        let tw = 10 * k;
        self.dx = (w as f32 * 0.25).max(tw as f32 / 2.0 + 2.0);
        self.u = (w as f32 / 240.0).clamp(0.4, 1.4);
        let n = (T_END / DT_TAB) as usize + 4;
        self.tab.clear();
        let mut s = 0.0;
        for i in 0..n {
            self.tab.push(s);
            let t = i as f32 * DT_TAB;
            s += 0.5 * (self.v_at(t) + self.v_at(t + DT_TAB)) * DT_TAB;
        }
        let mut rng = Rng(self.seed | 1);
        self.obs.clear();
        for &(tp, kind, close) in PLAN.iter() {
            let (wo, he) = Self::ob_size(kind, k);
            let v = self.v_at(tp) + if kind == FLY { self.vf() } else { 0.0 };
            let o = ((tw + wo) as f32 / 2.0 - 0.5 * k as f32).max(1.0) / v.max(1.0);
            // 像真人玩：大多提前一点点，险过的那次起跳晚了
            let delta = if close { 0.07 } else { rng.range(-0.05, 0.02) };
            let margin = if close { 0.0 } else { 2.0 };
            let mut ob = Ob {
                kind,
                tp,
                x: self.s_at(tp) + self.dx,
                close,
                shade: [rng.int(4) as u8, rng.int(4) as u8, rng.int(4) as u8],
                ph: rng.f(),
                j0: -9.0,
                jt: 1.0,
                jh: 0.0,
                d0: -9.0,
                d1: -9.0,
            };
            if kind == FLY {
                ob.d0 = tp - o - 0.12 + delta * 0.5;
                ob.d1 = tp + o + 0.06;
            } else {
                let a = delta.abs() + o;
                let jt = (a * 2.0 / 0.7).clamp(0.45, 1.05);
                let rr = (2.0 * a / jt).min(0.92);
                ob.jt = jt;
                // 小屏天不够高：别跳出屏幕
                ob.jh = ((he + 1) as f32 / (1.0 - rr * rr) + margin).min((self.gtop - 8 * k - 4).max(he + 2) as f32);
                ob.j0 = tp + delta - jt / 2.0;
            }
            self.obs.push(ob);
        }
        // Bug 墙
        self.wnc = if w < 60 { 1 } else if k == 2 { 3 } else { 2 };
        self.wcw = 5 * k + 1;
        self.wch = 3 * k;
        let duck_full = 8 * k + 2;
        let hel = 3 * k + 1;
        let hw_max = self.gtop - (duck_full + hel + 4);
        let hw_t = (self.gtop as f32 * 0.62) as i32;
        self.wnr = (hw_t.min(hw_max) / self.wch).max(2);
        let stop_x = self.dx + tw as f32 / 2.0 + (4 * k + 4) as f32;
        self.wall_x = self.s_at(T_STOP) + stop_x;
        let ww = (self.wnc * self.wcw - 1) as f32;
        self.land_x = (stop_x + ww + (3 * k + 3) as f32 + tw as f32 / 2.0).min(w as f32 - tw as f32 / 2.0 - 1.0);
        self.lift_l = (3 * k * self.wnr + 2) as f32;
        self.wall.clear();
        for r in 0..self.wnr {
            for c in 0..self.wnc {
                let shade = if rng.f() < 0.3 { 4 } else { rng.int(4) as u8 };
                self.wall.push(Wm {
                    c,
                    r,
                    shade,
                    ph: rng.f() * 2.0,
                    vx: rng.range(-1.0, 1.0) * 45.0 * self.u,
                    vy: -rng.range(45.0, 90.0),
                    helper: if r == self.wnr - 1 { c } else { -1 },
                });
            }
        }
    }

    /// 天上的云：从日志截图里挑几段 trace 碎片，开场它们不跟着沉，就挂在原处
    fn make_clouds(&mut self, app: &App) {
        let (w, sh) = (self.snap_w as i32, self.snap_h as i32);
        let rows = ((self.gy as f32 * 0.42) as i32).min(sh - 1);
        if rows < 2 || w < 20 {
            return;
        }
        let mut rng = Rng(self.seed.rotate_left(17) | 1);
        let want = (w / 30).clamp(2, 8);
        for _ in 0..want {
            for _ in 0..30 {
                let row = 2 + rng.int((rows - 1) as usize) as i32;
                let len = 8 + rng.int(11) as i32;
                if len >= w {
                    continue;
                }
                let cx = rng.int((w - len) as usize) as i32;
                let base = (row * w + cx) as usize;
                let seg = &self.snap[base..base + len as usize];
                let ok = seg.iter().all(|c| c.w == 0 && c.ch.is_ascii())
                    && seg[0].ch != ' '
                    && seg[len as usize - 1].ch != ' ';
                let clash = self.clouds.iter().any(|c| c.row == row && (c.x as i32 - cx).abs() < 18);
                if ok && !clash {
                    let f = if app.hi { rng.range(0.1, 0.3) } else { 0.0 };
                    self.clouds.push(Cloud { cells: seg.to_vec(), x: cx as f32, row, f });
                    break;
                }
            }
        }
    }

    fn vf(&self) -> f32 {
        14.0 * self.u
    }

    fn v_at(&self, t: f32) -> f32 {
        let (v0, v1) = (46.0 * self.u, 108.0 * self.u);
        if t < T_RUN0 {
            0.0
        } else if t < T_RUN1 {
            v0 * smooth(T_RUN0, T_RUN1, t)
        } else if t < T_B0 {
            v0 + (v1 - v0) * (t - T_RUN1) / (T_B0 - T_RUN1)
        } else if t < T_STOP {
            v1 * (1.0 - (t - T_B0) / (T_STOP - T_B0))
        } else {
            0.0
        }
    }

    fn s_at(&self, t: f32) -> f32 {
        if self.tab.is_empty() {
            return 0.0;
        }
        let f = (t / DT_TAB).max(0.0);
        let i = (f as usize).min(self.tab.len() - 2);
        let a = (f - i as f32).clamp(0.0, 1.0);
        self.tab[i] + (self.tab[i + 1] - self.tab[i]) * a
    }

    /// (宽, 有效高) 像素
    fn ob_size(kind: u8, k: i32) -> (i32, i32) {
        match kind {
            G1 => (5 * k, 3 * k),
            G2 => (5 * k, 6 * k),
            PY => (10 * k + 1, 6 * k),
            T3 => (5 * k, 9 * k),
            _ => (5 * k, 3 * k),
        }
    }

    fn score(&self) -> u32 {
        (self.s_at(self.t.min(T_STOP)) / 3.0) as u32
    }

    fn passed(&self) -> usize {
        self.obs.iter().filter(|o| o.tp < self.t).count()
    }

    fn fly_bottom(&self) -> i32 {
        let k = self.k;
        self.gtop - 1 - ((3.5 * k as f32).round() as i32 + k + 3)
    }

    /// 鸭鸭此刻：(中心 x, 离地像素, 横向缩放, 纵向缩放, 脚的帧, 腿伸长, 是否可见)
    fn duck_pose(&self, t: f32) -> (f32, f32, f32, f32, i32, i32, bool) {
        if t < T_ENTER0 || t >= T_RISE1 {
            return (0.0, 0.0, 1.0, 1.0, 0, 0, false);
        }
        let tw = (10 * self.k) as f32;
        let k = self.k as f32;
        let l0 = 3.0 * k + 1.0;
        let mut x = self.dx;
        let mut lift = 0.0f32;
        let (mut sx, mut sy) = (1.0f32, 1.0f32);
        if t < T_ENTER1 {
            x = -tw + (self.dx + tw) * smooth(T_ENTER0, T_ENTER1, t);
        }
        if t < T_STOP + 0.5 {
            for o in &self.obs {
                if o.kind == FLY {
                    let s = smooth(o.d0 - 0.07, o.d0, t) * (1.0 - smooth(o.d1, o.d1 + 0.07, t));
                    if s > 0.0 {
                        sx = 1.0 + 0.3 * s;
                        sy = 1.0 - 0.5 * s;
                    }
                } else if t >= o.j0 && t < o.j0 + o.jt + 0.1 {
                    let tau = (t - o.j0) / o.jt;
                    if tau < 1.0 {
                        lift = lift.max(o.jh * 4.0 * tau * (1.0 - tau));
                        if tau < 0.15 {
                            sx = 0.92;
                            sy = 1.1;
                        }
                    } else {
                        sx = 1.12;
                        sy = 0.82;
                    }
                }
            }
            if t > T_B0 + 0.5 && t < T_STOP + 0.25 && lift == 0.0 {
                sx = 1.08;
                sy = 0.9;
            }
        }
        if t >= T_HOP && t < T_HOP + HOP_T {
            let tau = (t - T_HOP) / HOP_T;
            lift = (3.0 * k + 2.0) * 4.0 * tau * (1.0 - tau);
        }
        if t >= T_HELP0 && t < T_LIFT0 {
            lift = l0 * smooth(T_HELP1 - 0.3, T_HELP1, t);
        } else if t >= T_LIFT0 && t < T_LIFT1 {
            let f = (t - T_LIFT0) / (T_LIFT1 - T_LIFT0);
            x = self.dx + (self.land_x - self.dx) * smooth(0.25, 0.85, f);
            lift = l0 + (self.lift_l - l0) * (smooth(0.0, 0.35, f) - smooth(0.75, 1.0, f));
        } else if t >= T_LIFT1 {
            x = self.land_x;
            lift = l0 * (1.0 - smooth(T_LIFT1, T_LIFT1 + 0.3, t));
        }
        let running = t < T_STOP && lift == 0.0;
        let (frame, leg) = if running {
            let ph = (t * 6.0 + self.s_at(t) / (8.0 * k)) as i32 & 1;
            if sy < 0.8 { (1 + ph, 0) } else { (1 + ph, ph) }
        } else {
            (0, 0)
        };
        (x, lift, sx, sy, frame, leg, true)
    }

    /// 带描边、能压扁拉长、会迈步的小鸭鸭（造型同 MINI_DUCK）
    fn duck_px(&self, app: &mut App, cx: f32, feet_bottom: i32, sx: f32, sy: f32, frame: i32, leg: i32) {
        let k = self.k;
        let (bw, bh) = (10 * k, 7 * k);
        let tw = ((bw as f32 * sx).round() as i32).max(4);
        let th = ((bh as f32 * sy).round() as i32).max(3);
        let fh = k + leg;
        let body_bot = feet_bottom - fh;
        let left = (cx - tw as f32 / 2.0).round() as i32;
        let top = body_bot - th + 1;
        let (gw, gh) = (tw + 2, th + fh + 2);
        let (ox, oy) = (left - 1, top - 1);
        let hi = app.hi;
        let tq = (app.t * 14.0).floor() / 10.0;
        let mut buf: Vec<Option<Col>> = vec![None; (gw * gh) as usize];
        for j in 0..th {
            for i in 0..tw {
                let sr = ((j * bh / th) / k).min(6) as usize;
                let sc = ((i * bw / tw) / k).min(9) as usize;
                let ch = MINI_DUCK[sr].as_bytes()[sc];
                if ch == b'.' {
                    continue;
                }
                let (px, py) = (i + 1, j + 1);
                let c: Col = match ch {
                    b'e' => [12, 10, 20].into(),
                    b'B' => ORANGE.into(),
                    _ if hi => mix(lolcat((ox + px) as f32 * 0.8, (oy + py) as f32 * 0.4, tq), WHITE, 0.25).into(),
                    _ => rb(Band::Duck, px as f32 * 1.6 / k as f32, py as f32 * 0.8 / k as f32),
                };
                buf[(py * gw + px) as usize] = Some(c);
            }
        }
        // 两只脚：0 都着地；1/2 轮流迈步（抬起的那只短一截）
        let feet = [((tw as f32 * 0.33) as i32, frame != 2), ((tw as f32 * 0.55) as i32, frame != 1)];
        for (fx, down) in feet {
            let rows = if down { fh } else { fh / 2 };
            for r in 0..rows {
                for q in 0..k {
                    let (px, py) = (fx + 1 + q, th + 1 + r);
                    if px < gw && py < gh {
                        buf[(py * gw + px) as usize] = Some(ORANGE.into());
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
                    near.then(|| Col::T(OUTLINE))
                });
                if let Some(c) = c {
                    if oy + y < self.gtop {
                        app.layer.set(ox + x, oy + y, c);
                    }
                }
            }
        }
    }

    /// 墙砖 / 托举飞蛾此刻的 (左, 上, 翅膀帧)；None 表示已经飞出屏幕
    fn wall_moth(&self, m: &Wm, t: f32, s: f32, hi: bool) -> Option<(i32, i32, usize)> {
        let k = self.k;
        let mut left = self.wall_x - s + (m.c * self.wcw) as f32;
        let mut top = (self.gtop - 1 - m.r * self.wch - 3 * k + 1) as f32;
        let idle = if hi { 1.6 } else { 0.8 };
        let mut frame = ((t * idle + m.ph) as i32 & 1) as usize;
        let w0 = (t - T_WAVE) * 9.0 - (m.c + (self.wnr - 1 - m.r)) as f32 * 0.6;
        if w0 > 0.0 && t < T_WAVE + 1.4 {
            frame = (w0 as i32 & 1) as usize;
        }
        let fast = ((t * if hi { 10.0 } else { 6.0 } + m.ph) as i32 & 1) as usize;
        if m.helper >= 0 {
            let nh = self.wnc;
            let slot = |x: f32, lift: f32| -> (f32, f32) {
                let mid = (m.helper as f32 - (nh - 1) as f32 / 2.0) * (5 * k + 1) as f32;
                let sl = x + mid - 2.5 * k as f32;
                (sl, (self.gtop - 1) as f32 - lift + 1.0)
            };
            if t >= T_HELP0 && t < T_LIFT1 + 0.1 {
                let (dx, lift, ..) = self.duck_pose(t);
                let (sl, st) = slot(dx, lift);
                if t < T_HELP1 {
                    let f = smooth(T_HELP0, T_HELP1, t);
                    left = left + (sl - left) * f;
                    top = top + (st - top) * f - (f * std::f32::consts::PI).sin() * 10.0;
                } else {
                    left = sl;
                    top = st + ((t * 7.0 + m.ph * 3.0).sin() * 0.6).round();
                }
                frame = fast;
            } else if t >= T_LIFT1 + 0.1 {
                let (dx, lift, ..) = self.duck_pose(T_LIFT1 + 0.1);
                let (sl, st) = slot(dx, lift);
                // 放下鸭鸭后往两边散开（画在鸭鸭身后）
                let tau = t - T_LIFT1 - 0.1;
                let side = m.helper as f32 - (nh - 1) as f32 / 2.0;
                let side = if side == 0.0 { if m.vx < 0.0 { -0.4 } else { 0.4 } } else { side };
                left = sl + side * 70.0 * self.u.max(0.6) * tau;
                top = st - 30.0 * tau - 50.0 * tau * tau;
                frame = fast;
            }
        } else if t >= T_SCAT0 {
            let tau = (t - T_SCAT0 - (m.r as f32 * 0.04 + m.c as f32 * 0.03)).max(0.0);
            if tau > 0.0 {
                left += m.vx * tau;
                top += m.vy * tau - 80.0 * tau * tau;
                frame = fast;
            }
        }
        let (l, tp) = (left.round() as i32, top.round() as i32);
        // 横向越界交给 Layer::set 裁剪；完全飞出顶部才算没了
        if tp + 3 * k < 0 {
            return None;
        }
        Some((l, tp, frame))
    }

    fn draw_rows(&self, app: &mut App, snap: &[Cell], shift: i32, holes: bool) {
        let (w, h) = (self.snap_w.min(app.scr.w), self.snap_h.min(app.scr.h));
        let sw = self.snap_w;
        for r in 0..(self.gy.max(0) as usize).min(h) {
            let y = r as i32 + shift;
            if y >= self.gy || y < 0 {
                continue;
            }
            for x in 0..w {
                let c = snap[r * sw + x];
                if c.w == 2 || (c.ch == ' ' && c.bg == Col::T(BLACK)) {
                    continue;
                }
                if holes && self.clouds.iter().any(|cl| cl.row == r as i32 && (x as i32) >= cl.x as i32 && (x as i32) < cl.x as i32 + cl.cells.len() as i32) {
                    continue;
                }
                app.scr.put_c(x as i32, y, c.ch, c.fg, Some(c.bg));
            }
        }
    }
}

impl Interlude for Runner {
    fn name(&self) -> &'static str {
        "鸭鸭跑酷"
    }

    fn update(&mut self, app: &mut App, dt: f32) -> bool {
        self.t += dt;
        let t = self.t;
        if app.scr.w != self.sw || app.scr.h != self.sh {
            self.plan(app);
            self.clouds.retain(|c| c.row < self.gy - 2);
        }
        if t >= T_ENTER0 && t < T_STOP {
            self.title_cd -= dt;
            if self.title_cd <= 0.0 {
                self.title_cd = 0.5;
                let s = format!("鸭鸭跑酷 — {:04}", self.score());
                app.set_title(&s);
            }
        }
        if self.step == 0 && t >= T_STOP + 0.1 {
            self.step = 1;
            app.set_title("鸭鸭跑酷 — 检测到一个无法跳过的 bug");
        }
        if self.step == 1 && t >= T_INF {
            self.step = 2;
            app.set_title("鸭鸭跑酷 — ∞（这关没法过，但 bug 放水了）");
        }
        if self.step == 2 && t >= T_RISE0 {
            self.step = 3;
            let n = self.score();
            app.push(log_line("WARN", "bug::wall", "前方出现无法跳过的 bug（高 9999px，已报告产品）", Kind::Info));
            app.push(log_line("INFO", "bug::wall", "bug 自愿让行：已把鸭鸭抬过去，不收费", Kind::Info));
            app.push(log_line("INFO", "duck::run", &format!("本局跑了 {} 列日志，得分 ∞（bug 放水，成绩仅供参考）", n * 3), Kind::Info));
        }
        if t >= T_END {
            let lx = self.land_x;
            app.duck.x = lx;
            app.duck.tx = lx;
            app.duck.y = app.lane_y();
            app.duck.ty = app.duck.y;
            app.duck.right = true;
            let w = app.w();
            for (i, m) in app.moths.iter_mut().enumerate() {
                m.x = (lx + (i as f32 * 7.3) % 40.0 - 20.0).clamp(0.0, w);
                m.y = -4.0 - i as f32 * 2.0;
                m.tx = m.x;
                m.ty = m.y;
                m.vx = 0.0;
                m.vy = 0.0;
            }
            return true;
        }
        false
    }

    fn draw(&mut self, app: &mut App) {
        let t = self.t;
        if t >= T_RISE1 {
            app.draw_logs(9.0, 0, 0.0);
            return;
        }
        let hi = app.hi;
        let tq = if hi { t } else { (t * 15.0).floor() / 15.0 };
        let s = self.s_at(t);
        let (w, gy, gtop, k) = (app.scr.w as i32, self.gy, self.gtop, self.k);

        // ── 背景：整页日志沉下去 / 收尾时长回来
        if tq < T_SINK1 {
            let f = ((tq - T_SINK0) / (T_SINK1 - T_SINK0)).clamp(0.0, 1.0);
            let shift = (gy as f32 * f * f).round() as i32;
            let snap = std::mem::take(&mut self.snap);
            self.draw_rows(app, &snap, shift, true);
            self.snap = snap;
        }
        if t >= T_RISE0 {
            if self.rise.is_none() {
                app.scr.clear();
                app.draw_logs(9.0, 0, 0.0);
                self.rise = Some(app.scr.cells.clone());
                self.snap_w = app.scr.w;
                self.snap_h = app.scr.h;
                app.scr.clear();
                app.scr.scroll = 0;
                app.last_top = None;
            }
            let f = ease_out((tq - T_RISE0) / (T_RISE1 - T_RISE0 - 0.1));
            let shift = (gy as f32 * (1.0 - f)).round() as i32;
            let snap = self.rise.take().unwrap_or_default();
            self.draw_rows(app, &snap, shift, false);
            let sw = self.snap_w;
            if (gy as usize) < self.snap_h {
                for x in 0..sw.min(app.scr.w) {
                    let c = snap[gy as usize * sw + x];
                    if c.w != 2 {
                        app.scr.put_c(x as i32, gy, c.ch, c.fg, Some(c.bg));
                    }
                }
            }
            self.rise = Some(snap);
        } else {
            // ── 云：挂在天上的日志碎片，慢速视差（省流量档不动）
            for cl in &self.clouds {
                let span = (w + 30) as f32;
                let x0 = ((cl.x + 15.0 - s * cl.f).rem_euclid(span) - 15.0).round() as i32;
                for (i, c) in cl.cells.iter().enumerate() {
                    if c.ch != ' ' {
                        app.scr.put_c(x0 + i as i32, cl.row, c.ch, c.fg, None);
                    }
                }
            }
            // ── 地平线：最后一行日志，向左滚
            let len = self.ground.len().max(1);
            let off = s.floor() as usize;
            for x in 0..w {
                let (ch, c) = self.ground[(x as usize + off) % len];
                if ch == '\0' {
                    if x == 0 {
                        app.scr.put_c(0, gy, ' ', c, None);
                    }
                    continue;
                }
                app.scr.put_c(x, gy, ch, c, None);
            }
        }

        // ── 障碍
        if t >= T_ENTER0 && t < T_STOP + 1.0 {
            for (oi, o) in self.obs.iter().enumerate() {
                let mut cx = o.x - s;
                if o.kind == FLY {
                    cx -= self.vf() * (t - o.tp);
                }
                let (wo, _) = Self::ob_size(o.kind, k);
                let left = (cx - wo as f32 / 2.0).round() as i32;
                if left > w + 2 || left + wo < -2 {
                    continue;
                }
                // 开场时已经在屏幕里的那几只：从天上扑棱着落到跑道上
                let mut rest = if ((t * 0.7 + o.ph).fract()) < 0.12 { 1 } else { 0 };
                let mut b = gtop - 1; // 精灵底边
                if o.x - wo as f32 / 2.0 < w as f32 {
                    let t0 = T_ENTER0 + oi as f32 * 0.15;
                    let f = smooth(t0, t0 + 0.9, t);
                    if f < 1.0 {
                        b -= ((1.0 - f) * (gtop + 12 * k) as f32) as i32;
                        rest = ((t * if hi { 9.0 } else { 5.0 }) as i32 & 1) as usize;
                    }
                }
                let top_of = |bottom: i32| bottom - 3 * k + 1;
                match o.kind {
                    G1 => moth_px(app, left, top_of(b), k, o.shade[0], rest, gtop),
                    G2 => {
                        moth_px(app, left, top_of(b), k, o.shade[0], 0, gtop);
                        moth_px(app, left, top_of(b - 3 * k), k, o.shade[1], rest, gtop);
                    }
                    T3 => {
                        moth_px(app, left, top_of(b), k, o.shade[0], 0, gtop);
                        moth_px(app, left, top_of(b - 3 * k), k, o.shade[1], 0, gtop);
                        moth_px(app, left, top_of(b - 6 * k), k, o.shade[2], rest, gtop);
                    }
                    PY => {
                        moth_px(app, left, top_of(b), k, o.shade[0], 0, gtop);
                        moth_px(app, left + 5 * k + 1, top_of(b), k, o.shade[1], 0, gtop);
                        moth_px(app, left + (5 * k + 1) / 2, top_of(b - 3 * k), k, o.shade[2], rest, gtop);
                    }
                    _ => {
                        let fr = ((t * if hi { 9.0 } else { 5.0 } + o.ph * 2.0) as i32 & 1) as usize;
                        let fb = self.fly_bottom();
                        moth_px(app, left, fb - 3 * k + 1, k, o.shade[0].min(2), fr, gtop);
                    }
                }
            }
        }


        // ── Bug 墙（托举的飞蛾只在鸭鸭脚下，鸭鸭画在最上面）
        let wl = self.wall_x - s;
        if wl < w as f32 + 2.0 {
            for m in &self.wall {
                if let Some((l, tp, fr)) = self.wall_moth(m, t, s, hi) {
                    moth_px(app, l, tp, k, m.shade, fr, gtop);
                }
            }
        }
        // ── 鸭鸭
        let (x, lift, sx, sy, frame, leg, vis) = self.duck_pose(t);
        if vis {
            self.duck_px(app, x, gtop - 1 - lift.round() as i32, sx, sy, frame, leg);
        }
    }

    fn draw_over(&mut self, app: &mut App) {
        let t = self.t;
        if t < T_ENTER0 || t >= T_RISE0 {
            return;
        }
        let w = app.scr.w as i32;
        let k = self.k;
        // ── 计分条
        let inf = t >= T_INF;
        let right = if inf {
            "越过 bug ×∞   得分 ∞".to_string()
        } else {
            format!("越过 bug ×{}   {:04}", self.passed(), self.score())
        };
        let rw = str_w(&right) as i32;
        app.scr.text(w - rw - 2, 0, &right, [210, 210, 220], None);
        let name = "鸭鸭跑酷";
        if w > rw + 16 {
            let mut x = 2;
            for ch in name.chars() {
                app.scr.put(x, 0, ch, rb(Band::Text, x as f32 * 3.0, 0.0), None);
                x += 2;
            }
        }
        // ── 刹车时脚下的灰
        let (dx, lift, ..) = self.duck_pose(t);
        if t > T_B0 + 0.4 && t < T_STOP + 0.3 && lift == 0.0 {
            let tw = 10 * k;
            let fl = ((t * 12.0) as i32) & 1;
            for i in 0..3 {
                let ch = if (i + fl) & 1 == 0 { '≈' } else { '~' };
                let c = scale([200, 200, 200], 1.0 - i as f32 * 0.3);
                app.scr.put((dx as i32) - tw / 2 - 2 - i * 2, self.gy - 1, ch, c, None);
            }
        }
        // ── 险过
        for o in &self.obs {
            if o.close && t > o.tp - 0.05 && t < o.tp + 0.8 {
                let y = ((self.gtop - 1) as f32 - o.jh - (8 * k) as f32) / 2.0 - 2.0;
                app.scr.text(self.dx as i32 + 2, (y as i32).max(1), "险！差 1px", [255, 200, 90], None);
            }
        }
        // ── 文案
        let cap_y = if self.gy > 14 { 2 } else { 1 };
        let center = |s: &str| (w - str_w(s) as i32) / 2;
        if t >= T_STOP + 0.2 && t < T_INF {
            let s = "WARN  检测到一个无法跳过的 bug";
            app.scr.text(center(s).max(0), cap_y, s, [255, 190, 80], None);
        }
        if t >= T_INF {
            let s = "这关没法过，但 bug 放水了";
            let mut x = center(s).max(0);
            for ch in s.chars() {
                app.scr.put(x, cap_y, ch, rb(Band::Text, x as f32, cap_y as f32), None);
                x += char_w(ch) as i32;
            }
        }
        // ── 气泡
        let duck_top_row = ((self.gtop - 1) as f32 - lift - (8 * k + 1) as f32) / 2.0;
        let say = |app: &mut App, s: &str| bubble(app, dx as i32 + 3 * k, duck_top_row as i32 - 4, s, true);
        if t >= T_STOP + 0.1 && t < T_HOP {
            say(app, "嘎？");
        } else if t >= T_HOP + HOP_T + 0.05 && t < T_WAVE + 0.3 {
            say(app, "嘎……");
        } else if t >= T_LIFT1 && t < T_SCAT0 + 0.7 {
            say(app, "嘎！谢啦");
        }
        if t >= T_WAVE + 0.2 && t < T_HELP1 {
            let wl = (self.wall_x - self.s_at(t)) as i32;
            let top = self.gtop - self.wnr * self.wch;
            bubble(app, wl + self.wnc * self.wcw / 2, top / 2 - 4, "上来！", true);
        }
    }
}
