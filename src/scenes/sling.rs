//! 插曲：弹弓——只借「拉开弹弓、抛物线发射、砸塌一座结构」的玩法。
//! 日志沉下去当地面；左边一把像素弹弓，右边是 Bug 用烂日志（ERROR/WARN/TODO/FIXME…）搭的城堡，各层蹲着飞蛾。
//! 第一发擦边只崩掉一角（飞蛾嘲笑），第二发正中，城堡一层层塌成满地字符碎块；
//! 结尾飞蛾把碎块一块块叼起来，拼成一行绿色的 [FIXED]——「谢谢你帮我们重构」。
//!
//! 性能：静止的城堡不做物理；碎块有上限，落地即静止（堆高表）；天空大面积纯黑。
use super::*;

const T_SINK0: f32 = 0.1;
const T_SINK1: f32 = 1.3;
const T_SL0: f32 = 1.1; // 弹弓从地里长出来
const T_SL1: f32 = 1.8;
const T_BUILD0: f32 = 1.3; // 城堡一块块落下来
const T_MOTH_IN: f32 = 2.3;
const T_DUCK0: f32 = 2.8; // 鸭鸭掉进皮兜
const T_DUCK1: f32 = 3.15;
const PULL: f32 = 1.1;
const T_PULL1: f32 = 3.4;
const T_FIRE1: f32 = 4.8;
const FT1: f32 = 0.8;
const T_HIT1: f32 = T_FIRE1 + FT1;
const BACK_T: f32 = 1.25; // 擦边后弹回皮兜
const T_PULL2: f32 = 7.7;
const T_FIRE2: f32 = 9.3;
const FT2: f32 = 0.72;
const T_HIT2: f32 = T_FIRE2 + FT2;
const DROP_T: f32 = 0.6;
const T_COLLECT: f32 = 13.6;
const GO: f32 = 0.42;
const CARRY: f32 = 0.6;
const TRIP: f32 = 1.06;
const T_DUCK_SAY: f32 = 16.9;
const T_MOTH_SAY: f32 = 18.1;
const T_RISE0: f32 = 20.3;
const T_RISE1: f32 = 21.0;
const T_END: f32 = 21.2;
const G: f32 = 46.0; // 碎块重力（行/秒²）

const BEAMS: [&str; 10] = [
    "ERROR NullPointerException at castle.rs:42",
    "WARN deprecated since 2019, removing soon",
    "TODO: fix before release (2017)",
    "panic: entered unreachable code",
    "ERROR segmentation fault (core dumped)",
    "WARN retry 3/3 failed, retrying anyway",
    "undefined is not a function",
    "ERROR deadlock detected in lock_all()",
    "WARN works on my machine",
    "FIXME: magic number 37, do not touch",
];
const BEAMS_SM: [&str; 8] = ["ERR ", "WARN ", "TODO ", "FIX ", "BUG ", "NULL ", "PANIC ", "GC "];
const COLS: [&str; 8] = ["FIXME", "HACK", "TODO", "XXX", "BUG", "TEMP", "LEGACY", "WTF"];
const FIXED_TXT: [&str; 4] = [
    "[FIXED] castle.rs 重构完成 0 warning",
    "[FIXED] castle.rs 重构完成",
    "[FIXED] 重构完成",
    "[FIXED]",
];

struct Rn(u64);
impl Rn {
    fn f(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
    fn r(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f()
    }
    fn i(&mut self, n: usize) -> usize {
        ((self.f() * n as f32) as usize).min(n.saturating_sub(1))
    }
}

struct Blk {
    cells: Vec<(i32, i32, char, Col)>,
    x: i32,
    y: i32,
    st: u8, // 0 静止 1 震动待塌 2 下落 3 没了
    fx: f32,
    fy: f32,
    vx: f32,
    vy: f32,
    wait: f32,
    tin: f32,
}

struct Pc {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    ch: char,
    col: Col,
    rest: bool,
    trip: i32,
}

struct Mo {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    tx: f32,
    ty: f32,
    shade: u8,
    ph: f32,
    perch: i32,
    mode: u8, // 0 飞入 1 蹲着 2 飞 3 搬砖
    start: f32,
    sx: f32,
    sy: f32,
}

struct Trip {
    p: usize,
    s: f32,
    from: (f32, f32),
    to: (f32, f32),
    ch: char,
    col: i32,
}

pub struct Sling {
    t: f32,
    ev: u8,
    rng: Rn,
    sw: usize,
    sh: usize,
    gy: i32,
    k: i32,
    cap: usize,
    stem: i32,
    tipl: (f32, f32),
    tipr: (f32, f32),
    qrest: (f32, f32),
    qpull: (f32, f32),
    doff: (f32, f32),
    tg1: (f32, f32),
    tg2: (f32, f32),
    land: (f32, f32),
    gpx: f32,
    gb: f32,
    cx0: i32,
    cx1: i32,
    top: i32,
    hit1: (f32, f32),
    hit2: (f32, f32),
    rad2: f32,
    blks: Vec<Blk>,
    occ: Vec<i32>,
    heap: Vec<u8>,
    ps: Vec<Pc>,
    moths: Vec<Mo>,
    laugh: Vec<usize>,
    trips: Vec<Trip>,
    line_y: i32,
    line_x1: i32,
    fixed: String,
    score: u32,
    disp: f32,
    snap: Vec<Cell>,
    snap_w: usize,
    snap_h: usize,
    rise: Option<Vec<Cell>>,
    clip: i32,
}

fn ease_out(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    1.0 - (1.0 - x) * (1.0 - x)
}
fn sm(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}
fn lerp2(a: (f32, f32), b: (f32, f32), f: f32) -> (f32, f32) {
    (a.0 + (b.0 - a.0) * f, a.1 + (b.1 - a.1) * f)
}
/// 从 a 出发、T 秒后正好落到 b 的抛物线，s 秒时的位置
fn bal(a: (f32, f32), b: (f32, f32), tt: f32, g: f32, s: f32) -> (f32, f32) {
    let vy0 = (b.1 - a.1 - 0.5 * g * tt * tt) / tt;
    (a.0 + (b.0 - a.0) * s / tt, a.1 + vy0 * s + 0.5 * g * s * s)
}

/// 白底气泡（3 行），尾巴朝下指向 (cx, by+3)
fn say(app: &mut App, cx: i32, by: i32, text: &str) {
    let w = app.scr.w as i32;
    let tw = str_w(text) as i32 + 4;
    let bx = (cx - tw / 2).clamp(0, (w - tw).max(0));
    let by = by.max(if w < 70 { 1 } else { 0 });
    let bgc = [250, 250, 245];
    for row in 0..3 {
        for x in 0..tw {
            app.scr.put(bx + x, by + row, ' ', BLACK, Some(bgc));
        }
    }
    app.scr.text(bx + 2, by + 1, text, [20, 20, 30], Some(bgc));
    app.scr.put(cx.clamp(bx + 1, bx + tw - 2), by + 3, '▀', bgc, None);
}

impl Sling {
    pub fn new(app: &mut App) -> Self {
        let seed = app.rng.next() | 1;
        app.duck.visible = false;
        app.duck.bubble = None;
        app.set_title("鸭鸭.exe — 弹弓 · 目标：bug 城堡");
        app.scr.clear();
        app.draw_logs(9.0, 0, 0.0);
        let snap = app.scr.cells.clone();
        app.scr.clear();
        app.scr.scroll = 0;
        app.last_top = None;
        let mut s = Sling {
            t: 0.0,
            ev: 0,
            rng: Rn(seed),
            sw: 0,
            sh: 0,
            gy: 0,
            k: 1,
            cap: 200,
            stem: 0,
            tipl: (0.0, 0.0),
            tipr: (0.0, 0.0),
            qrest: (0.0, 0.0),
            qpull: (0.0, 0.0),
            doff: (0.0, 0.0),
            tg1: (0.0, 0.0),
            tg2: (0.0, 0.0),
            land: (0.0, 0.0),
            gpx: 1.0,
            gb: 1.0,
            cx0: 0,
            cx1: 0,
            top: 0,
            hit1: (0.0, 0.0),
            hit2: (0.0, 0.0),
            rad2: 3.0,
            blks: Vec::new(),
            occ: Vec::new(),
            heap: Vec::new(),
            ps: Vec::new(),
            moths: Vec::new(),
            laugh: Vec::new(),
            trips: Vec::new(),
            line_y: 0,
            line_x1: 0,
            fixed: String::new(),
            score: 0,
            disp: 0.0,
            snap_w: app.scr.w,
            snap_h: app.scr.h,
            snap,
            rise: None,
            clip: i32::MAX,
        };
        s.layout(app);
        s
    }

    fn layout(&mut self, app: &mut App) {
        let (w, h) = (app.scr.w as i32, app.scr.h as i32);
        self.sw = app.scr.w;
        self.sh = app.scr.h;
        self.cap = if app.hi { 240 } else { 110 };
        let gy = (h - 2).max(4);
        self.gy = gy;
        let k = if h >= 30 && w >= 100 { 2 } else { 1 };
        self.k = k;
        let kf = k as f32;
        let hp = (2 * gy) as f32;
        // ── 弹弓
        let sx = ((w as f32 * 0.14) as i32).max(8);
        self.stem = sx;
        let yj = (2 * gy - 1 - 6 * k) as f32;
        self.tipl = ((sx - 4 * k) as f32, yj - 5.0 * kf);
        self.tipr = ((sx + 1 + 4 * k) as f32, yj - 5.0 * kf);
        self.qrest = (sx as f32 + 0.5, yj - 5.0 * kf + 2.0);
        self.qpull = (self.qrest.0 - 7.0 * kf, self.qrest.1 + 3.5 * kf);
        self.doff = (4.0 * kf, -kf);
        self.gpx = hp * 1.9;
        self.gb = hp * 1.1;
        // ── 城堡
        let cw = ((w as f32 * 0.3) as i32).clamp(14, 56);
        let cx1 = w - 3;
        let cx0 = cx1 - cw + 1;
        self.cx0 = cx0;
        self.cx1 = cx1;
        let avail = gy - 4;
        let nl = if avail >= 34 { 4 } else if avail >= 14 { 3 } else { 2 };
        let lh = (((avail as f32 * 0.75) / nl as f32 - 1.0) as i32).clamp(2, 7);
        let step = (cw / (2 * nl + 1)).max(2);
        self.blks.clear();
        let mut perch_spots: Vec<(f32, f32, i32, i32)> = Vec::new(); // (px, py, x 列, 下面那一行)
        let mut beam_i = self.rng.i(BEAMS.len());
        let mut col_i = self.rng.i(COLS.len());
        let (mut x0_top, mut x1_top, mut r_top) = (cx0, cx1, gy - 1);
        let (mut x0_0, mut b_0) = (cx0, gy - 1);
        for i in 0..nl {
            let x0 = cx0 + i * step;
            let x1 = cx1 - i * step;
            let wd = x1 - x0 + 1;
            if wd < 4 {
                break;
            }
            let b = gy - 1 - i * (lh + 1);
            let r = b - lh;
            if r < 2 {
                break;
            }
            let tb = T_BUILD0 + i as f32 * 0.32;
            let ncol = (wd / 14 + 1).max(2);
            let mut cxs = Vec::new();
            for j in 0..ncol {
                let x = x0 + j * (wd - 1) / (ncol - 1);
                cxs.push(x);
                let word: Vec<char> = COLS[col_i % COLS.len()].chars().chain(std::iter::once('·')).collect();
                col_i += 1;
                let mut cells = Vec::new();
                for d in 0..lh {
                    let ch = word[d as usize % word.len()];
                    cells.push((0, d, ch, rb(Band::Text, x as f32 * 2.0, (r + 1 + d) as f32)));
                }
                self.blks.push(Blk { cells, x, y: r + 1, st: 0, fx: 0.0, fy: 0.0, vx: 0.0, vy: 0.0, wait: 0.0, tin: tb + j as f32 * 0.05 });
            }
            // 梁：一整行烂日志，切成几段
            let mut txt = String::new();
            if w < 70 {
                while (txt.chars().count() as i32) < wd {
                    txt.push_str(BEAMS_SM[beam_i % BEAMS_SM.len()]);
                    beam_i += 1;
                }
            } else {
                while (txt.chars().count() as i32) < wd {
                    txt.push_str(BEAMS[beam_i % BEAMS.len()]);
                    txt.push_str("  ");
                    beam_i += 1;
                }
            }
            let chars: Vec<char> = txt.chars().take(wd as usize).collect();
            let kind = if chars.starts_with(&['E', 'R', 'R']) { 1 } else if chars.starts_with(&['W', 'A', 'R', 'N']) { 2 } else { 0 };
            let nseg = (wd / 11).max(1);
            for sg in 0..nseg {
                let a = sg * wd / nseg;
                let e = (sg + 1) * wd / nseg;
                let mut cells = Vec::new();
                for c in a..e {
                    let x = x0 + c;
                    let col: Col = match kind {
                        1 => RED.into(),
                        2 => [255, 190, 80].into(),
                        _ => rb(Band::Text, x as f32, r as f32),
                    };
                    cells.push((c - a, 0, chars[c as usize], col));
                }
                self.blks.push(Blk { cells, x: x0 + a, y: r, st: 0, fx: 0.0, fy: 0.0, vx: 0.0, vy: 0.0, wait: 0.0, tin: tb + 0.2 + sg as f32 * 0.05 });
            }
            // 蹲在这一层里面（两根柱子之间的地板上）
            if cxs.len() >= 2 && cxs[1] - cxs[0] >= 6 {
                let x = (cxs[0] + cxs[1]) / 2;
                perch_spots.push((x as f32, (2 * (b + 1) - 2) as f32, x, b + 1));
            }
            if wd >= 30 && cxs.len() >= 3 {
                let x = (cxs[cxs.len() - 2] + cxs[cxs.len() - 1]) / 2;
                perch_spots.push((x as f32, (2 * (b + 1) - 2) as f32, x, b + 1));
            }
            // 露在外面的梁头上
            if step >= 5 && i + 1 < nl {
                perch_spots.push(((x0 + step / 2) as f32, (2 * r - 2) as f32, x0 + step / 2, r));
                perch_spots.push(((x1 - step / 2) as f32, (2 * r - 2) as f32, x1 - step / 2, r));
            }
            x0_top = x0;
            x1_top = x1;
            r_top = r;
            if i == 0 {
                x0_0 = x0;
                b_0 = b;
            }
        }
        // 屋顶
        let n_roof = ((x1_top - x0_top) / 9).clamp(1, 3);
        for j in 0..n_roof {
            let x = x0_top + (j + 1) * (x1_top - x0_top) / (n_roof + 1);
            perch_spots.push((x as f32, (2 * r_top - 2) as f32, x, r_top));
        }
        self.top = r_top;
        self.rebuild_occ();
        // ── 目标点
        self.hit1 = (x1_top as f32 - 1.0, r_top as f32);
        self.tg1 = (x1_top as f32 - 2.0 - 3.0 * kf, (2 * r_top) as f32 - 2.0 * kf);
        let mid = b_0 - lh / 2;
        self.hit2 = (x0_0 as f32 + 1.0, mid as f32);
        self.rad2 = lh as f32 * 0.8 + 1.5;
        self.tg2 = (x0_0 as f32 - 4.0 * kf, (2 * mid) as f32);
        self.land = (x0_0 as f32 - 9.0 * kf, (2 * gy - 1) as f32 - 3.5 * kf - 1.0);
        // ── 飞蛾
        self.moths.clear();
        let mcap = if app.hi { 12 } else { 8 };
        let n = perch_spots.len();
        let mut order: Vec<usize> = (0..n).collect();
        // 屋顶的优先（在后面），保证嘲笑的那几只在
        order.reverse();
        for (idx, &si) in order.iter().take(mcap).enumerate() {
            let (px, py, x, row) = perch_spots[si];
            let perch = if row >= gy { -1 } else { self.occ_at(x, row) };
            let shade = [0u8, 4, 2, 0, 3, 4, 1, 2][self.rng.i(8)];
            let sx = w as f32 + 4.0 + self.rng.r(0.0, 20.0);
            let sy = self.rng.r(0.0, hp * 0.4);
            self.moths.push(Mo {
                x: sx,
                y: sy,
                vx: 0.0,
                vy: 0.0,
                tx: px,
                ty: py,
                shade,
                ph: self.rng.r(0.0, 6.28),
                perch,
                mode: 0,
                start: T_MOTH_IN + idx as f32 * 0.08,
                sx: 0.0,
                sy: 0.0,
            });
        }
        self.heap = vec![0; w.max(1) as usize];
        self.ps.clear();
        self.trips.clear();
        self.laugh.clear();
        self.ev = 0;
    }

    fn rebuild_occ(&mut self) {
        let (w, h) = (self.sw as i32, self.sh as i32);
        self.occ.clear();
        self.occ.resize((w * h).max(0) as usize, -1);
        for (i, b) in self.blks.iter().enumerate() {
            if b.st != 0 {
                continue;
            }
            for c in &b.cells {
                let (x, y) = (b.x + c.0, b.y + c.1);
                if x >= 0 && y >= 0 && x < w && y < h {
                    self.occ[(y * w + x) as usize] = i as i32;
                }
            }
        }
    }
    fn occ_at(&self, x: i32, y: i32) -> i32 {
        let (w, h) = (self.sw as i32, self.sh as i32);
        if x < 0 || y < 0 || x >= w || y >= h {
            return -1;
        }
        self.occ[(y * w + x) as usize]
    }

    fn spawn(&mut self, x: f32, y: f32, vx: f32, vy: f32, ch: char, col: Col) {
        if self.ps.len() >= self.cap || ch == ' ' {
            return;
        }
        let y = y.min((self.gy - 1) as f32);
        self.ps.push(Pc { x, y, vx, vy, ch, col, rest: false, trip: -1 });
    }

    fn shatter(&mut self, bi: usize, vx: f32, vy: f32) {
        let cells = std::mem::take(&mut self.blks[bi].cells);
        let (bx, by) = if self.blks[bi].st == 2 { (self.blks[bi].fx.round(), self.blks[bi].fy.floor()) } else { (self.blks[bi].x as f32, self.blks[bi].y as f32) };
        for c in cells {
            let (a, b) = (self.rng.r(-5.0, 5.0), self.rng.r(-7.0, 0.0));
            self.spawn(bx + c.0 as f32, by + c.1 as f32, vx + a, vy + b, c.2, c.3);
            self.score += 10;
        }
        self.score += 50;
        self.blks[bi].st = 3;
    }

    /// 没东西托着的静止块开始下落
    fn settle(&mut self) {
        let gy = self.gy;
        loop {
            self.rebuild_occ();
            let mut ch = false;
            for i in 0..self.blks.len() {
                if self.blks[i].st != 0 {
                    continue;
                }
                let b = &self.blks[i];
                let sup = b.cells.iter().any(|c| {
                    let (x, y) = (b.x + c.0, b.y + c.1 + 1);
                    if y >= gy {
                        return true;
                    }
                    let o = self.occ_at(x, y);
                    o >= 0 && o != i as i32
                });
                if !sup {
                    let b = &mut self.blks[i];
                    b.st = 2;
                    b.fx = b.x as f32;
                    b.fy = b.y as f32;
                    b.vx = 0.0;
                    b.vy = 0.0;
                    ch = true;
                }
            }
            if !ch {
                break;
            }
        }
        self.rebuild_occ();
    }

    fn hit(&mut self, ix: f32, iy: f32, rad: f32, pw: f32, shock: bool) {
        for bi in 0..self.blks.len() {
            if self.blks[bi].st != 0 {
                continue;
            }
            let (bx, by) = (self.blks[bi].x, self.blks[bi].y);
            let n0 = self.blks[bi].cells.len();
            let cells = std::mem::take(&mut self.blks[bi].cells);
            let mut keep = Vec::new();
            let mut lost = 0;
            for c in cells {
                let (cx, cy) = ((bx + c.0) as f32, (by + c.1) as f32);
                let (dx, dy) = ((cx - ix) * 0.5, cy - iy);
                let d = (dx * dx + dy * dy).sqrt();
                if d < rad {
                    lost += 1;
                    let (nx, ny) = if d > 0.01 { (dx / d, dy / d) } else { (1.0, 0.0) };
                    let sp = pw * self.rng.r(0.5, 1.0) * (1.3 - d / rad);
                    let up = self.rng.r(3.0, 10.0);
                    self.spawn(cx, cy, nx * sp * 2.0 + pw * 0.5, ny * sp - up, c.2, c.3);
                    self.score += 10;
                } else {
                    keep.push(c);
                }
            }
            let small = lost > 0 && keep.len() * 10 < n0 * 4;
            self.blks[bi].cells = keep;
            if self.blks[bi].cells.is_empty() {
                self.blks[bi].st = 3;
            } else if small {
                self.shatter(bi, pw * 0.4, -4.0);
            } else if shock {
                let b = &self.blks[bi];
                let n = b.cells.len() as f32;
                let (sx, sy) = b.cells.iter().fold((0.0, 0.0), |a, c| (a.0 + c.0 as f32, a.1 + c.1 as f32));
                let (mx, my) = (b.x as f32 + sx / n, b.y as f32 + sy / n);
                let d = (((mx - ix) * 0.5).powi(2) + (my - iy).powi(2)).sqrt();
                let wait = 0.05 + d * 0.035 + self.rng.r(0.0, 0.12);
                let vx = pw * 0.55 * (1.0 - d / 30.0).max(0.2) + self.rng.r(0.0, 4.0);
                let vy = -self.rng.r(0.0, 7.0) * (1.0 - d / 30.0).max(0.0);
                let b = &mut self.blks[bi];
                b.st = 1;
                b.wait = wait;
                b.vx = vx;
                b.vy = vy;
            }
        }
        self.settle();
    }

    fn land_col(&mut self, x: f32) -> (f32, f32) {
        let w = self.heap.len() as i32;
        let c0 = (x.round() as i32).clamp(0, w - 1);
        let mut best = c0;
        for d in [0, 1, -1, 2, -2, 3, -3] {
            let c = c0 + d;
            if c >= 0 && c < w && self.heap[c as usize] < 3 {
                best = c;
                break;
            }
        }
        let hgt = self.heap[best as usize];
        self.heap[best as usize] = (hgt + 1).min(6);
        (best as f32, (self.gy - 1 - hgt.min(3) as i32) as f32)
    }

    fn step(&mut self, dt: f32, t: f32) {
        let gy = self.gy;
        let w = self.sw as f32;
        // 震动待塌
        let mut changed = false;
        for b in self.blks.iter_mut() {
            if b.st == 1 {
                b.wait -= dt;
                if b.wait <= 0.0 {
                    b.st = 2;
                    b.fx = b.x as f32;
                    b.fy = b.y as f32;
                    changed = true;
                }
            }
        }
        if changed {
            self.settle();
        }
        // 下落的整块
        for i in 0..self.blks.len() {
            if self.blks[i].st != 2 {
                continue;
            }
            let landed = {
                let b = &mut self.blks[i];
                b.vy += G * dt;
                b.fx += b.vx * dt;
                b.fy += b.vy * dt;
                b.vx *= 1.0 - 0.4 * dt;
                let b = &self.blks[i];
                let (fx, fy) = (b.fx.round() as i32, b.fy);
                let mut hit = false;
                let mut maxr = i32::MIN;
                for c in &b.cells {
                    let y = (fy + c.1 as f32).floor() as i32;
                    maxr = maxr.max(y);
                    if y >= gy || self.occ_at(fx + c.0, y) >= 0 {
                        hit = true;
                    }
                }
                if hit && maxr >= gy {
                    self.blks[i].fy -= (maxr - gy + 1) as f32;
                }
                hit
            };
            if landed {
                let (vx, vy) = (self.blks[i].vx * 0.5, -self.blks[i].vy.abs() * 0.28);
                self.shatter(i, vx, vy);
            }
        }
        // 碎块
        for i in 0..self.ps.len() {
            if self.ps[i].rest || self.ps[i].trip >= 0 {
                continue;
            }
            let p = &mut self.ps[i];
            p.vy += G * dt;
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            if p.x < 0.0 || p.x > w - 1.0 {
                p.x = p.x.clamp(0.0, w - 1.0);
                p.vx = -p.vx * 0.4;
            }
            let c = (p.x.round() as usize).min(self.heap.len() - 1);
            let floor = (gy - 1 - self.heap[c].min(3) as i32) as f32;
            if p.y >= floor && p.vy > 0.0 {
                if p.vy > 9.0 {
                    p.y = floor;
                    p.vy *= -0.32;
                    p.vx *= 0.6;
                } else {
                    let x = p.x;
                    let (lx, ly) = self.land_col(x);
                    let p = &mut self.ps[i];
                    p.x = lx;
                    p.y = ly;
                    p.rest = true;
                }
            }
        }
        // 飞蛾
        for i in 0..self.moths.len() {
            let m = &self.moths[i];
            if m.mode == 1 && m.perch >= 0 && self.blks[m.perch as usize].st != 0 {
                let (a, b) = (self.rng.r(-30.0, 40.0), self.rng.r(-60.0, -35.0));
                let (hx, hy) = (m.tx + self.rng.r(-6.0, 6.0), (2 * self.top) as f32 - self.rng.r(6.0, 14.0));
                let m = &mut self.moths[i];
                m.mode = 2;
                m.vx = a;
                m.vy = b;
                m.tx = hx;
                m.ty = hy.max(3.0);
            }
            let m = &mut self.moths[i];
            if (m.mode == 0 && t >= m.start) || m.mode == 2 {
                let ax = (m.tx - m.x) * 16.0 - m.vx * 6.5;
                let ay = (m.ty - m.y) * 16.0 - m.vy * 6.5;
                m.vx += ax * dt;
                m.vy += ay * dt;
                m.x += m.vx * dt;
                m.y += m.vy * dt;
                if m.mode == 0 && (m.tx - m.x).abs() + (m.ty - m.y).abs() < 0.8 && m.vx.abs() + m.vy.abs() < 4.0 {
                    m.mode = 1;
                    m.x = m.tx;
                    m.y = m.ty;
                }
            }
        }
    }

    fn flee_all(&mut self) {
        let hp_top = (2 * self.top) as f32;
        let w = self.sw as f32;
        for i in 0..self.moths.len() {
            let (a, b) = (self.rng.r(-20.0, 60.0), self.rng.r(-70.0, -40.0));
            let hx = self.rng.r(self.cx0 as f32 - 12.0, w - 4.0);
            let hy = self.rng.r(4.0, (hp_top - 4.0).max(5.0));
            let m = &mut self.moths[i];
            m.mode = 2;
            m.vx = a;
            m.vy = b;
            m.tx = hx;
            m.ty = hy;
        }
    }

    fn plan_collect(&mut self) {
        let w = self.sw as i32;
        let mn = self.moths.len().max(1);
        let mut txt = FIXED_TXT[3];
        for s in FIXED_TXT {
            let n = s.chars().filter(|c| *c != ' ').count();
            if (str_w(s) as i32) <= w - 4 && (str_w(s) as i32) <= (self.cx1 - self.cx0 + 1).max(16) + 10 && n <= 3 * mn {
                txt = s;
                break;
            }
        }
        self.fixed = txt.to_string();
        let tw = str_w(txt) as i32;
        self.line_y = if self.top - 2 >= 6 { self.top } else { self.gy - 1 };
        let center = (self.cx0 + self.cx1) / 2;
        let x0 = (center - tw / 2).clamp(1, (w - tw - 1).max(1));
        self.line_x1 = x0 + tw;
        for m in self.moths.iter_mut() {
            m.sx = m.x;
            m.sy = m.y;
            m.mode = 3;
        }
        let mut x = x0;
        let mut i = 0usize;
        let ly = self.line_y;
        for ch in txt.chars() {
            let cw = char_w(ch) as i32;
            if ch != ' ' {
                // 离这个位置最近、还没被认领的静止碎块
                let mut best = usize::MAX;
                let mut bd = f32::MAX;
                for (pi, p) in self.ps.iter().enumerate() {
                    if p.rest && p.trip < 0 {
                        let d = (p.x - x as f32).abs() + self.rng.f() * 3.0;
                        if d < bd {
                            bd = d;
                            best = pi;
                        }
                    }
                }
                if best == usize::MAX {
                    let px = self.rng.r(self.cx0 as f32, self.cx1 as f32);
                    self.ps.push(Pc { x: px.round(), y: (self.gy - 1) as f32, vx: 0.0, vy: 0.0, ch: '·', col: Col::T(GRAY), rest: true, trip: -1 });
                    best = self.ps.len() - 1;
                }
                self.ps[best].trip = self.trips.len() as i32;
                let (m, j) = (i % mn, i / mn);
                let p = &self.ps[best];
                self.trips.push(Trip {
                    p: best,
                    s: T_COLLECT + j as f32 * TRIP + m as f32 * 0.07,
                    from: (p.x, p.y * 2.0 - 1.5),
                    to: (x as f32, (2 * ly - 2) as f32),
                    ch,
                    col: x,
                });
                i += 1;
            }
            x += cw;
        }
    }

    /// 搬砖阶段飞蛾 m 的位置（像素）
    fn mpos(&self, m: usize, t: f32) -> (f32, f32) {
        let mn = self.moths.len().max(1);
        let mo = &self.moths[m];
        let mut prev = (mo.sx, mo.sy);
        let mut j = m;
        while j < self.trips.len() {
            let tr = &self.trips[j];
            if t < tr.s {
                // 等着出发：在原地飞一会儿再慢慢靠近
                return prev;
            }
            let u = t - tr.s;
            if u < GO {
                let f = sm(u / GO);
                let p = lerp2(prev, tr.from, f);
                return (p.0, p.1 - (f * std::f32::consts::PI).sin() * 5.0);
            }
            if u < GO + CARRY {
                let f = sm((u - GO) / CARRY);
                let p = lerp2(tr.from, tr.to, f);
                return (p.0, p.1 - (f * std::f32::consts::PI).sin() * 8.0);
            }
            prev = tr.to;
            j += mn;
        }
        prev
    }

    fn q(&self, t: f32) -> (f32, f32) {
        let pull = |s: f32| lerp2(self.qrest, self.qpull, ease_out(s / PULL));
        let snap = |s: f32| {
            if s > 0.8 {
                return self.qrest;
            }
            let a = -0.5 * (-5.0 * s).exp() * (24.0 * s).cos();
            (self.qrest.0 + (self.qpull.0 - self.qrest.0) * a, self.qrest.1 + (self.qpull.1 - self.qrest.1) * a)
        };
        if t < T_PULL1 {
            self.qrest
        } else if t < T_FIRE1 {
            pull(t - T_PULL1)
        } else if t < T_PULL2 {
            snap(t - T_FIRE1)
        } else if t < T_FIRE2 {
            pull(t - T_PULL2)
        } else {
            snap(t - T_FIRE2)
        }
    }

    /// 鸭鸭的中心像素、朝向
    fn duck(&self, t: f32) -> Option<(f32, f32, bool)> {
        let rest = (self.qrest.0 + self.doff.0, self.qrest.1 + self.doff.1);
        let p0 = (self.qpull.0 + self.doff.0, self.qpull.1 + self.doff.1);
        let loaded = |t: f32| {
            let q = self.q(t);
            (q.0 + self.doff.0, q.1 + self.doff.1, true)
        };
        if t < T_DUCK0 {
            None
        } else if t < T_DUCK1 {
            let f = (t - T_DUCK0) / (T_DUCK1 - T_DUCK0);
            Some((rest.0, -10.0 + (rest.1 + 10.0) * f * f, true))
        } else if t < T_FIRE1 {
            Some(loaded(t))
        } else if t < T_HIT1 {
            let p = bal(p0, self.tg1, FT1, self.gpx, t - T_FIRE1);
            Some((p.0, p.1, true))
        } else if t < T_HIT1 + BACK_T {
            let s = t - T_HIT1;
            let p = bal(self.tg1, rest, BACK_T, self.gb, s);
            Some((p.0, p.1.max(4.0), ((s * 9.0) as i32) % 2 == 1))
        } else if t < T_FIRE2 {
            Some(loaded(t))
        } else if t < T_HIT2 {
            let p = bal(p0, self.tg2, FT2, self.gpx, t - T_FIRE2);
            Some((p.0, p.1, true))
        } else if t < T_HIT2 + DROP_T {
            let s = t - T_HIT2;
            let p = bal(self.tg2, self.land, DROP_T, self.gb, s);
            Some((p.0, p.1, ((s * 9.0) as i32) % 2 == 0))
        } else {
            Some((self.land.0, self.land.1, true))
        }
    }

    fn px(&self, app: &mut App, x: i32, y: i32, c: impl Into<Col>) {
        if y < self.clip && y < 2 * self.gy {
            app.layer.set(x, y, c);
        }
    }
    fn line(&self, app: &mut App, a: (f32, f32), b: (f32, f32), c: Rgb) {
        let n = ((b.0 - a.0).abs().max((b.1 - a.1).abs()).ceil() as i32).max(1);
        for i in 0..=n {
            let p = lerp2(a, b, i as f32 / n as f32);
            self.px(app, p.0.round() as i32, p.1.round() as i32, c);
        }
    }

    fn draw_rows(&self, app: &mut App, snap: &[Cell], r0: usize, r1: usize, shift: i32, clip: i32, solid: bool) {
        let (w, sw) = (self.snap_w.min(app.scr.w), self.snap_w);
        for r in r0..r1.min(self.snap_h) {
            let y = r as i32 + shift;
            if y < 0 || y >= clip {
                continue;
            }
            for x in 0..w {
                let c = snap[r * sw + x];
                if c.w == 2 || (!solid && c.ch == ' ' && c.bg == Col::T(BLACK)) {
                    continue;
                }
                app.scr.put_c(x as i32, y, c.ch, c.fg, Some(c.bg));
            }
        }
    }
}

impl Interlude for Sling {
    fn name(&self) -> &'static str {
        "弹弓"
    }

    fn update(&mut self, app: &mut App, dt: f32) -> bool {
        self.t += dt;
        let t = self.t;
        if app.scr.w != self.sw || app.scr.h != self.sh {
            self.layout(app);
        }
        let dt = dt.min(0.05);
        let n = (dt / 0.017).ceil().max(1.0) as i32;
        for _ in 0..n {
            self.step(dt / n as f32, t);
        }
        let gap = self.score as f32 - self.disp;
        if gap > 0.0 {
            self.disp = (self.disp + (gap * dt * 7.0).max(dt * 120.0)).min(self.score as f32);
        }
        if self.ev == 0 && t >= T_HIT1 {
            self.ev = 1;
            let (ix, iy) = self.hit1;
            let before: Vec<bool> = self.moths.iter().map(|m| m.mode == 1).collect();
            self.hit(ix, iy, 2.3, 9.0, false);
            // 被波及的飞蛾惊飞一下，然后回来笑
            let ipx = (ix, iy * 2.0);
            for i in 0..self.moths.len() {
                let m = &self.moths[i];
                let near = (m.x - ipx.0).abs() < 12.0 && (m.y - ipx.1).abs() < 10.0;
                if near || (before[i] && m.perch >= 0 && self.blks[m.perch as usize].st != 0) {
                    let (a, b) = (self.rng.r(-10.0, 40.0), self.rng.r(-60.0, -35.0));
                    let m = &mut self.moths[i];
                    m.mode = 2;
                    m.vx = a;
                    m.vy = b;
                    m.ty = (m.ty - 6.0).max(3.0);
                    m.tx -= 2.0;
                    self.laugh.push(i);
                }
            }
            if self.laugh.is_empty() && !self.moths.is_empty() {
                self.laugh.push(0);
            }
            app.set_title("鸭鸭.exe — 弹弓 · 第 1 发：擦边");
        }
        if self.ev == 1 && t >= T_HIT2 {
            self.ev = 2;
            let (ix, iy) = self.hit2;
            let r = self.rad2;
            self.hit(ix, iy, r, 16.0, true);
            self.flee_all();
            app.set_title("鸭鸭.exe — 弹弓 · 第 2 发：正中！");
        }
        if self.ev == 2 && t >= T_COLLECT {
            self.ev = 3;
            // 还在空中的统统落地
            for i in 0..self.blks.len() {
                if self.blks[i].st == 1 || self.blks[i].st == 2 {
                    self.shatter(i, 0.0, 0.0);
                }
            }
            for i in 0..self.ps.len() {
                if !self.ps[i].rest {
                    let x = self.ps[i].x;
                    let (lx, ly) = self.land_col(x);
                    let p = &mut self.ps[i];
                    p.x = lx;
                    p.y = ly;
                    p.rest = true;
                }
            }
            self.plan_collect();
            app.set_title("鸭鸭.exe — 弹弓 · bug 们在……重构？");
        }
        if self.ev == 3 && t >= T_COLLECT + 2.4 {
            self.ev = 4;
            for p in self.ps.iter_mut() {
                if p.trip < 0 {
                    p.col = Col::T([58, 58, 68]);
                }
            }
        }
        if self.ev == 4 && t >= T_RISE0 {
            self.ev = 5;
            app.push(log_line("ERROR", "bug::castle", "遭到不明鸭类弹射，结构完整性 0%", Kind::Error));
            app.push(log_line("INFO", "bug::castle", &format!("飞蛾顺手重构了 castle.rs（拆除分 {}）", self.score), Kind::Info));
            app.push(log_line("FIXED", "castle.rs", "重构完成，主要贡献者：一只来拆迁的鸭", Kind::Fixed));
        }
        if t >= T_END {
            let lx = self.land.0.clamp(4.0, app.w() - 4.0);
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
        let gy = self.gy;
        let h = app.scr.h as i32;
        let k = self.k;
        let kf = k as f32;
        // ── 地面：日志最底下两行留着；上面的整页沉下去
        let snap = std::mem::take(&mut self.snap);
        self.draw_rows(app, &snap, gy as usize, self.snap_h, 0, h, false);
        if t < T_SINK1 {
            let f = ((t - T_SINK0) / (T_SINK1 - T_SINK0)).clamp(0.0, 1.0);
            let shift = (gy as f32 * f * f).round() as i32;
            self.draw_rows(app, &snap, 0, gy as usize, shift, gy, false);
        }
        self.snap = snap;

        // ── 碎块（被叼着的在 draw_over 画）
        for p in &self.ps {
            if p.trip >= 0 {
                let tr = &self.trips[p.trip as usize];
                if t >= tr.s + GO {
                    continue;
                }
            }
            app.scr.put_c(p.x.round() as i32, p.y.floor() as i32, p.ch, p.col, None);
        }
        // ── 城堡
        let jit = if hi { ((t * 28.0) as i32) & 1 } else { 0 };
        for b in &self.blks {
            match b.st {
                0 => {
                    if t < b.tin {
                        continue;
                    }
                    let p = ((t - b.tin) / 0.3).min(1.0);
                    let off = ((1.0 - p) * (1.0 - p) * (b.y + 4) as f32).round() as i32;
                    for c in &b.cells {
                        app.scr.put_c(b.x + c.0, b.y + c.1 - off, c.2, c.3, None);
                    }
                }
                1 => {
                    for c in &b.cells {
                        app.scr.put_c(b.x + c.0 + jit, b.y + c.1, c.2, c.3, None);
                    }
                }
                2 => {
                    let (fx, fy) = (b.fx.round() as i32, b.fy.floor() as i32);
                    for c in &b.cells {
                        app.scr.put_c(fx + c.0, fy + c.1, c.2, c.3, None);
                    }
                }
                _ => {}
            }
        }
        // ── 预测轨迹（虚线）
        for (t0, tf, tg, ft) in [(T_PULL1, T_FIRE1, self.tg1, FT1), (T_PULL2, T_FIRE2, self.tg2, FT2)] {
            if t >= t0 + 0.2 && t < tf {
                let reveal = ease_out((t - t0 - 0.2) / (PULL - 0.2));
                let p0 = (self.qpull.0 + self.doff.0, self.qpull.1 + self.doff.1);
                let n = 18;
                let march = if hi { (t * 7.0) as usize } else { 0 };
                for i in 2..=n {
                    if i as f32 / n as f32 > reveal || (i + march) % 3 == 0 {
                        continue;
                    }
                    let p = bal(p0, tg, ft, self.gpx, ft * i as f32 / n as f32);
                    app.scr.put(p.0.round() as i32, (p.1 / 2.0).floor() as i32, '·', [190, 190, 210], None);
                }
            }
        }
        // ── 回到日志：整页从下面升上来
        self.clip = i32::MAX;
        if t >= T_RISE0 {
            if self.rise.is_none() {
                let keep = app.scr.cells.clone();
                app.scr.clear();
                app.draw_logs(9.0, 0, 0.0);
                self.rise = Some(app.scr.cells.clone());
                app.scr.cells = keep;
                app.scr.scroll = 0;
                app.last_top = None;
            }
            let f = ease_out((t - T_RISE0) / (T_RISE1 - T_RISE0 - 0.1));
            let shift = (h as f32 * (1.0 - f)).round() as i32;
            let snap = self.rise.take().unwrap_or_default();
            let (sw0, sh0) = (self.snap_w, self.snap_h);
            self.snap_w = app.scr.w;
            self.snap_h = app.scr.h;
            if snap.len() == self.snap_w * self.snap_h {
                self.draw_rows(app, &snap, 0, self.snap_h, shift, h, true);
            }
            self.snap_w = sw0;
            self.snap_h = sh0;
            self.rise = Some(snap);
            self.clip = 2 * shift;
        }

        // ══ 像素层
        // 弹弓
        if t >= T_SL0 {
            let f = ease_out((t - T_SL0) / (T_SL1 - T_SL0));
            let off = ((1.0 - f) * (11 * k + 2) as f32).round() as i32;
            let sx = self.stem;
            let base = 2 * gy - 1;
            let yj = base - 6 * k;
            let metal = [128, 138, 170];
            let shine = [196, 206, 236];
            for y in yj..=base {
                self.px(app, sx, y + off, shine);
                self.px(app, sx + 1, y + off, metal);
            }
            let (tl, tr) = (self.tipl, self.tipr);
            let o = off as f32;
            self.line(app, (sx as f32, yj as f32 + o), (tl.0, tl.1 + o), shine);
            self.line(app, (sx as f32 + 1.0, yj as f32 + o), (tr.0, tr.1 + o), metal);
            if k == 2 {
                self.line(app, (sx as f32 + 1.0, yj as f32 + o), (tl.0 + 1.0, tl.1 + o), metal);
                self.line(app, (sx as f32 + 2.0, yj as f32 + o), (tr.0 + 1.0, tr.1 + o), metal);
            }
            // 底座
            for x in -2..=3 {
                self.px(app, sx + x, base + off, [90, 96, 120]);
            }
            if off == 0 {
                let band = [235, 95, 140];
                let q = self.q(t);
                let pq = (q.0, q.1);
                self.line(app, tl, pq, band);
                for dy in -1..=1 {
                    self.px(app, pq.0.round() as i32 - 1, pq.1.round() as i32 + dy, [120, 50, 80]);
                    self.px(app, pq.0.round() as i32, pq.1.round() as i32 + dy, [120, 50, 80]);
                }
                if let Some((dx, dy, right)) = self.duck(t) {
                    let dy = if k == 1 { dy.max(6.0) } else { dy };
                    if (dy / 2.0) as i32 + 2 < self.clip / 2 {
                        mini_duck(app, dx, dy, right, k);
                    }
                }
                self.line(app, tr, pq, band);
            }
        }
        // 飞行拖尾
        for (tf, th, tg, ft) in [(T_FIRE1, T_HIT1, self.tg1, FT1), (T_FIRE2, T_HIT2, self.tg2, FT2)] {
            if t >= tf && t < th + 0.15 {
                let p0 = (self.qpull.0 + self.doff.0, self.qpull.1 + self.doff.1);
                let nt = if hi { 12 } else { 6 };
                for j in 1..=nt {
                    let s = (t - tf).min(ft) - j as f32 * 0.028 - 0.03;
                    if s < 0.0 {
                        break;
                    }
                    let p = bal(p0, tg, ft, self.gpx, s);
                    let c = rb(Band::Duck, j as f32 * 3.0, 0.0);
                    let x = p.0.round() as i32 - 5 * k;
                    self.px(app, x, p.1.round() as i32, c);
                    if j < nt / 2 {
                        self.px(app, x, p.1.round() as i32 + 1, c);
                    }
                }
            }
        }
        // 飞蛾
        let laughing = t >= T_HIT1 + 0.6 && t < T_PULL2 + 0.6;
        for (i, m) in self.moths.iter().enumerate() {
            if m.mode == 0 && t < m.start {
                continue;
            }
            let (mut x, mut y) = if m.mode == 3 { self.mpos(i, t) } else { (m.x, m.y) };
            let mut flap = t * 12.0 + m.ph;
            match m.mode {
                1 => {
                    flap = if hi && ((t * 0.7 + m.ph) as i32) % 5 == 0 { t * 8.0 } else { 0.0 };
                }
                2 if laughing && self.laugh.contains(&i) => {
                    flap = t * 20.0;
                    if hi {
                        y -= ((t * 10.0 + m.ph).sin().abs()) * 2.0;
                    }
                }
                2 | 3 if hi => {
                    y += (t * 2.5 + m.ph).sin() * 1.2;
                    x += (t * 1.7 + m.ph).cos() * 0.8;
                }
                _ => {}
            }
            if (y / 2.0) as i32 + 1 < self.clip / 2 && y < (2 * gy) as f32 {
                moth(app, x, y, m.shade, flap, 0.0);
            }
        }
        let _ = kf;
    }

    fn draw_over(&mut self, app: &mut App) {
        let t = self.t;
        if t >= T_RISE0 || t < T_SL0 {
            return;
        }
        let w = app.scr.w as i32;
        let k = self.k;
        // ── 计分
        let shots = 2 - (t >= T_FIRE1) as i32 - (t >= T_FIRE2) as i32;
        let sc = format!("拆除分 {:06}", self.disp as u32);
        let rw = str_w(&sc) as i32;
        let up = (self.disp as u32) < self.score;
        app.scr.text(w - rw - 2, 0, &sc, if up { [255, 220, 90] } else { [200, 200, 215] }, None);
        let left = format!("剩余鸭鸭 ×{}", shots);
        if w > rw + str_w(&left) as i32 + 6 {
            app.scr.text(2, 0, &left, [200, 200, 215], None);
        }
        // ── 撞击闪光
        for (th, hp) in [(T_HIT1, self.hit1), (T_HIT2, self.hit2)] {
            let s = t - th;
            if (0.0..0.35).contains(&s) {
                let r = 1.0 + s * 14.0;
                let n = 10;
                for i in 0..n {
                    let a = i as f32 / n as f32 * 6.283;
                    let x = (hp.0 + a.cos() * r * 2.0).round() as i32;
                    let y = (hp.1 + a.sin() * r).round() as i32;
                    if y >= 0 && y < self.gy {
                        app.scr.put(x, y, if i % 2 == 0 { '✶' } else { '*' }, [255, 230, 120], None);
                    }
                }
            }
        }
        // ── 被叼着的 / 拼好的字
        for (ti, tr) in self.trips.iter().enumerate() {
            let u = t - tr.s;
            if u < GO {
                continue;
            }
            if u < GO + CARRY {
                let p = &self.ps[tr.p];
                // 跟着叼它的飞蛾
                let mi = ti % self.moths.len().max(1);
                let (mx, my) = self.mpos(mi, t);
                app.scr.put_c(mx.round() as i32, ((my + 2.0) / 2.0).floor() as i32, p.ch, p.col, None);
            } else {
                let flash = app.hi && u < GO + CARRY + 0.12;
                app.scr.put(tr.col, (tr.to.1 as i32 + 2) / 2, tr.ch, if flash { WHITE } else { GREEN }, None);
            }
        }
        // ── 气泡
        let duck_top = |s: &Self, t: f32| s.duck(t).map(|(x, y, _)| (x as i32, ((y - 3.5 * k as f32 - 1.0) / 2.0) as i32));
        if t >= T_HIT1 + 0.5 && !self.laugh.is_empty() {
            let lines = [(T_HIT1 + 0.5, T_HIT1 + 1.9, "擦边球～"), (T_HIT1 + 1.3, T_PULL2 + 0.5, "就这？")];
            for (n, (a, b, s)) in lines.iter().enumerate() {
                if t >= *a && t < *b {
                    let mi = self.laugh[n.min(self.laugh.len() - 1)];
                    let m = &self.moths[mi];
                    let off = if n == 1 && self.laugh.len() == 1 { 6 } else { 0 };
                    say(app, m.x as i32 + off, (m.y / 2.0) as i32 - 5 - off as i32 / 3, s);
                }
            }
        }
        if t >= T_PULL2 + 0.3 && t < T_FIRE2 {
            if let Some((x, top)) = duck_top(self, t) {
                say(app, x, top - 4, "嘎！这次认真了");
            }
        }
        // 晕
        if t >= T_HIT2 + DROP_T && t < T_HIT2 + DROP_T + 2.2 {
            if let Some((x, top)) = duck_top(self, t) {
                let a = t * 6.0;
                for i in 0..3 {
                    let b = a + i as f32 * 2.094;
                    app.scr.put(x + (b.cos() * 3.0 * k as f32).round() as i32, top - 1 + (b.sin() * 0.6).round() as i32, '✦', [255, 230, 120], None);
                }
            }
        }
        let duck_end = if w < 70 { T_MOTH_SAY } else { T_RISE0 };
        if t >= T_DUCK_SAY && t < duck_end {
            if let Some((x, top)) = duck_top(self, t) {
                say(app, x, top - 4, "嘎？我是来拆的……");
            }
        }
        if t >= T_MOTH_SAY && !self.fixed.is_empty() {
            let cx = self.line_x1 - 6;
            let by = self.line_y - 6;
            let by = if by < 0 { self.line_y + 1 } else { by };
            say(app, cx, by, "谢谢你帮我们重构");
        }
    }
}
