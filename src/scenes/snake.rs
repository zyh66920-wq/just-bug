//! 插曲：贪吃 Bug —— 飞蛾首尾相接排成一条「虫」，自动寻路吃彩虹豆，每吃一颗从尾巴长出一节彩虹；
//! 最后贪心把自己盘成死结、无路可走，鸭鸭游过来「嘎」地一啄，虫子散成一只只飞蛾四散飞走。
//!
//! 画法：网格一格 3×3 像素（像素层，3 字符宽 × 1.5 行）：2×2 是身体，1 像素缝只在前后相连的两节之间填上，
//! 所以盘起来的螺旋也看得出一圈一圈。身体每一节的颜色跟着「格子被走进去的那一步」走，蛇移动时只有头、尾、
//! 彩虹分界这几格在变；日志按调色板压暗，静止零成本。
use super::*;
use std::collections::VecDeque;

const DIRS: [(i32, i32); 4] = [(1, 0), (0, 1), (-1, 0), (0, -1)];
const L0: usize = 8;
const COIL_AT: f32 = 13.0;
const TRAP_AT: f32 = 16.0;
const FORCE_AT: f32 = 17.5;

#[derive(Clone, Copy)]
struct Seg {
    x: i32,
    y: i32,
    e: u32, // 走进这一格时的步数：颜色由它决定，蛇移动时不变
}
struct Bean {
    x: i32,
    y: i32,
    born: f32,
    hue: f32,
}
struct Fly {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    flap: f32,
    shade: u8,
    rainbow: f32,
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Gather,
    Run,
    Coil,
    Stuck,
    Burst,
    Out,
}

pub struct Snake {
    t: f32,
    mt: f32,
    mode: Mode,
    rng: Rng,
    cols: i32,
    rows: i32,
    ox: i32,
    oy: i32,
    blocked: Vec<bool>,
    occ: Vec<bool>,
    body: VecDeque<Seg>, // front = 尾，back = 头
    dir: (i32, i32),
    grow: u32,
    rainbow: usize,
    stamp: u32,
    acc: f32,
    beans: Vec<Bean>,
    bean_n: usize,
    bean_cd: f32,
    gather: Vec<(f32, f32, f32)>, // 起点 x、y、到达时刻
    run_at: f32,
    coil: VecDeque<(i32, i32)>,
    knot: (f32, f32),
    knot_r: f32,
    swim: Option<(f32, f32, f32, f32, bool)>,
    k: i32,
    dimtab: Vec<Rgb>,
    flies: Vec<Fly>,
    pops: Vec<(i32, i32, f32)>,
    small: bool,
    corner: (f32, f32),
    chat: Vec<&'static str>,
    chat_i: usize,
    ev: usize,
    flash: f32,
}

const CHAT: &[&str] = &["嘎？", "它在吃彩虹", "别咬到自己", "O(n) 的胃口", "长度焦虑了属于是", "这条 bug 在排队"];

impl Snake {
    pub fn new(app: &mut App) -> Self {
        let (w, h) = (app.scr.w as i32, app.scr.h as i32);
        let hp = h * 2;
        // 大屏一格 6×6 像素（4×4 身体 + 2 像素缝），小屏 3×3（2×2 + 1）
        let k = if w >= 100 && h >= 30 { 2 } else { 1 };
        let cs = 3 * k;
        let cols = ((w + k) / cs).max(4);
        let rows = ((hp - 2 + k) / cs).max(3);
        let ox = (w - (cols * cs - k)) / 2;
        let oy = 2 + (hp - 2 - (rows * cs - k)) / 2;
        let mut rng = Rng(app.rng.next() | 1);
        let small = w < 110 || h < 30;

        // 鸭鸭泊在右下角围观；它占的格子蛇不进去
        let (hw, hh) = app.duck_half();
        let corner = if small { (w as f32 - 7.0, hp as f32 - 6.0) } else { (w as f32 - hw - 3.0, hp as f32 - hh - 3.0) };
        let (bx0, bx1, by0, by1) = if small {
            (corner.0 - 7.0, corner.0 + 6.0, corner.1 - 6.0, corner.1 + 5.0)
        } else {
            (corner.0 - hw - 2.0, corner.0 + hw + 2.0, corner.1 - hh - 3.0, corner.1 + hh + 3.0)
        };
        let n = (cols * rows) as usize;
        let mut blocked = vec![false; n];
        for gy in 0..rows {
            for gx in 0..cols {
                let (px, py) = ((ox + gx * cs) as f32, (oy + gy * cs) as f32);
                let (kk, bs) = (k as f32, 2.0 * k as f32);
                if px + bs + kk > bx0 && px - kk <= bx1 && py + bs + kk > by0 && py - kk <= by1 {
                    blocked[(gy * cols + gx) as usize] = true;
                }
            }
        }

        // 开场：一排飞蛾首尾相接，尾在左、头在右
        let len = L0.min((cols - 4).max(3) as usize);
        let gy0 = rows / 2 - if small { 1 } else { 0 };
        let gx0 = ((cols - len as i32) / 3).max(1);
        let mut body = VecDeque::new();
        let mut occ = vec![false; n];
        for k in 0..len {
            let (x, y) = (gx0 + k as i32, gy0);
            let i = (y * cols + x) as usize;
            blocked[i] = false;
            occ[i] = true;
            body.push_back(Seg { x, y, e: k as u32 });
        }
        let mut gather = Vec::new();
        let old: Vec<(f32, f32)> = app.moths.iter().map(|m| (m.x, m.y)).collect();
        app.moths.clear();
        for k in 0..len {
            let (sx, sy) = match old.get(k) {
                Some(&p) => p,
                None => {
                    let (fw, fh) = (w as f32, hp as f32);
                    match rng.int(4) {
                        0 => (-3.0, rng.range(0.0, fh)),
                        1 => (fw + 3.0, rng.range(0.0, fh)),
                        2 => (rng.range(0.0, fw), -3.0),
                        _ => (rng.range(0.0, fw), fh + 3.0),
                    }
                }
            };
            gather.push((sx, sy, 0.45 + k as f32 * 0.09 + 1.1));
        }
        let run_at = gather.last().map(|g| g.2).unwrap_or(1.5) + 0.35;

        let mut chat: Vec<&'static str> = CHAT.to_vec();
        for i in (1..chat.len()).rev() {
            let j = rng.int(i + 1);
            chat.swap(i, j);
        }

        app.duck.visible = true;
        app.duck.asleep = false;
        app.duck.glow_to = 0.55;
        app.duck.tx = corner.0;
        app.duck.ty = corner.1;
        app.duck.bubble = None;
        app.push(log_line("INFO", "snake::ai", "贪吃 bug.exe 已加载：自动寻路 BFS + 防自杀检查 v0.1", Kind::Info));
        app.set_title("鸭鸭.exe — 贪吃 bug");

        Snake {
            t: 0.0,
            mt: 0.0,
            mode: Mode::Gather,
            rng,
            cols,
            rows,
            ox,
            oy,
            blocked,
            occ,
            body,
            dir: (1, 0),
            grow: 0,
            rainbow: 0,
            stamp: len as u32,
            acc: 0.0,
            beans: Vec::new(),
            bean_n: ((cols * rows) as usize / (if k == 2 { 90 } else { 260 })).clamp(4, 10),
            bean_cd: 0.0,
            gather,
            run_at,
            coil: VecDeque::new(),
            knot: (0.0, 0.0),
            knot_r: 0.0,
            swim: None,
            k,
            dimtab: (0..256u32)
                .map(|i| {
                    let i = i as u8;
                    let k = if i >= Band::Dim as u8 && i < Band::Duck as u8 { 0.62 } else { 0.28 };
                    scale(pal_rgb(i, app.pal_t()), k)
                })
                .collect(),
            flies: Vec::new(),
            pops: Vec::new(),
            small,
            corner,
            chat,
            chat_i: 0,
            ev: 0,
            flash: -1.0,
        }
    }

    // ───── 网格 ─────
    fn inb(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.cols && y < self.rows
    }
    fn idx(&self, x: i32, y: i32) -> usize {
        (y * self.cols + x) as usize
    }
    /// 格子左上角像素
    fn px(&self, x: i32, y: i32) -> (i32, i32) {
        (self.ox + x * 3 * self.k, self.oy + y * 3 * self.k)
    }
    /// 格子中心像素（给飞蛾、结的位置用）
    fn center(&self, x: i32, y: i32) -> (f32, f32) {
        let (px, py) = self.px(x, y);
        (px as f32 + self.k as f32, py as f32 + self.k as f32)
    }
    fn head(&self) -> Seg {
        *self.body.back().unwrap()
    }
    fn tail_free(&self) -> bool {
        self.grow == 0 && self.body.len() > 1
    }
    fn passable(&self, x: i32, y: i32) -> bool {
        if !self.inb(x, y) {
            return false;
        }
        let i = self.idx(x, y);
        if self.blocked[i] {
            return false;
        }
        if !self.occ[i] {
            return true;
        }
        let t = self.body[0];
        self.tail_free() && t.x == x && t.y == y
    }
    fn bean_at(&self, x: i32, y: i32) -> Option<usize> {
        self.beans.iter().position(|b| b.x == x && b.y == y)
    }

    fn bfs(&self, occ: &[bool], extra: Option<usize>, s: usize) -> Vec<i32> {
        let n = (self.cols * self.rows) as usize;
        let mut d = vec![-1i32; n];
        d[s] = 0;
        let mut q = VecDeque::new();
        q.push_back(s);
        while let Some(i) = q.pop_front() {
            let (x, y) = (i as i32 % self.cols, i as i32 / self.cols);
            for (dx, dy) in DIRS {
                let (nx, ny) = (x + dx, y + dy);
                if !self.inb(nx, ny) {
                    continue;
                }
                let j = self.idx(nx, ny);
                if d[j] >= 0 || self.blocked[j] || (occ[j] && Some(j) != extra) {
                    continue;
                }
                d[j] = d[i] + 1;
                q.push_back(j);
            }
        }
        d
    }

    /// 假装走一步 (x, y)：返回（走完后的占用、尾巴格、头能不能还摸到尾巴、头能到的格子数）
    fn simulate(&self, x: i32, y: i32) -> (bool, usize) {
        let mut o = self.occ.clone();
        let moves = self.tail_free();
        let nt = if moves {
            let t = self.body[0];
            o[self.idx(t.x, t.y)] = false;
            self.body[1]
        } else {
            self.body[0]
        };
        let n = self.idx(x, y);
        o[n] = true;
        let ti = self.idx(nt.x, nt.y);
        let d = self.bfs(&o, Some(ti), n);
        (d[ti] >= 0, d.iter().filter(|&&v| v >= 0).count())
    }

    /// 正常模式：BFS 找最近的豆，第一步过安全检查（走完还能追到自己尾巴）；不安全就挑最宽敞的路
    fn choose(&self) -> Option<(i32, i32)> {
        let h = self.head();
        let extra = if self.tail_free() { let t = self.body[0]; Some(self.idx(t.x, t.y)) } else { None };
        let d = self.bfs(&self.occ, extra, self.idx(h.x, h.y));
        let target = self
            .beans
            .iter()
            .filter(|b| self.t - b.born > 0.25)
            .map(|b| (d[self.idx(b.x, b.y)], b.x, b.y))
            .filter(|v| v.0 > 0)
            .min_by_key(|v| v.0);
        if let Some((mut k, mut x, mut y)) = target {
            while k > 1 {
                for (dx, dy) in DIRS {
                    let (nx, ny) = (x + dx, y + dy);
                    if self.inb(nx, ny) && d[self.idx(nx, ny)] == k - 1 {
                        x = nx;
                        y = ny;
                        k -= 1;
                        break;
                    }
                }
            }
            if self.simulate(x, y).0 {
                return Some((x, y));
            }
        }
        let mut best: Option<((bool, usize, i32), (i32, i32))> = None;
        for (dx, dy) in DIRS.iter() {
            let (nx, ny) = (h.x + dx, h.y + dy);
            if !self.passable(nx, ny) {
                continue;
            }
            let (safe, area) = self.simulate(nx, ny);
            // 同样安全宽敞时，优先直走，少拐弯
            let straight = if (*dx, *dy) == self.dir { 1 } else { 0 };
            let key = (safe, area, straight);
            if best.map_or(true, |b| key > b.0) {
                best = Some((key, (nx, ny)));
            }
        }
        best.map(|b| b.1)
    }

    /// 兜底的「贪心自杀」：挑最窄的路钻（盘不出死结时用）
    fn trap_move(&self) -> Option<(i32, i32)> {
        let h = self.head();
        DIRS.iter()
            .map(|(dx, dy)| (h.x + dx, h.y + dy))
            .filter(|&(x, y)| self.passable(x, y))
            .min_by_key(|&(x, y)| self.simulate(x, y).1)
    }

    /// 规划死结：从头前方开始，一圈圈往里盘满一个 a×a 的方块（a² ≤ 长度），终点四面都是自己
    fn plan(&mut self) -> bool {
        let len = self.body.len() + self.grow as usize;
        let h = self.head();
        let d = self.dir;
        for a in (3..=5i32).rev() {
            if (a * a) as usize > len {
                continue;
            }
            for side in [1i32, -1] {
                let mut runs = vec![a];
                for r in (1..a).rev() {
                    runs.push(r);
                    runs.push(r);
                }
                let (mut x, mut y, mut dir) = (h.x, h.y, d);
                let mut path = Vec::new();
                let mut ok = true;
                'outer: for r in runs {
                    for _ in 0..r {
                        x += dir.0;
                        y += dir.1;
                        if !self.inb(x, y) || self.blocked[self.idx(x, y)] || self.occ[self.idx(x, y)] {
                            ok = false;
                            break 'outer;
                        }
                        path.push((x, y));
                    }
                    dir = if side > 0 { (-dir.1, dir.0) } else { (dir.1, -dir.0) };
                }
                if ok {
                    let (mut sx, mut sy) = (0.0, 0.0);
                    for &(px, py) in &path {
                        let (qx, qy) = self.center(px, py);
                        sx += qx;
                        sy += qy;
                    }
                    self.knot = (sx / path.len() as f32, sy / path.len() as f32);
                    self.knot_r = (a * 3 * self.k) as f32 / 2.0;
                    let last = *path.last().unwrap();
                    if self.bean_at(last.0, last.1).is_none() {
                        self.beans.push(Bean { x: last.0, y: last.1, born: self.t, hue: self.rng.range(0.0, 84.0) });
                    }
                    self.coil = path.into_iter().collect();
                    return true;
                }
            }
        }
        false
    }

    fn advance(&mut self, x: i32, y: i32) {
        let h = self.head();
        if self.grow > 0 {
            self.grow -= 1;
        } else if let Some(t) = self.body.pop_front() {
            let i = self.idx(t.x, t.y);
            self.occ[i] = false;
        }
        let i = self.idx(x, y);
        self.occ[i] = true;
        self.body.push_back(Seg { x, y, e: self.stamp });
        self.stamp += 1;
        self.dir = (x - h.x, y - h.y);
        if let Some(b) = self.bean_at(x, y) {
            self.beans.swap_remove(b);
            self.grow += 1;
            self.rainbow += 1;
            self.bean_cd = self.bean_cd.max(0.35);
            let (px, py) = self.px(x, y);
            self.pops.push((px, py, self.t));
        }
    }

    /// 走一步；返回 false 表示卡死
    fn tick(&mut self) -> bool {
        if self.mode == Mode::Run && self.t >= COIL_AT && self.t < TRAP_AT && self.plan() {
            self.mode = Mode::Coil;
            self.ev = self.ev.max(100);
        }
        if self.mode == Mode::Coil {
            let Some((x, y)) = self.coil.pop_front() else { return false };
            if !self.passable(x, y) {
                return false;
            }
            self.advance(x, y);
            return !self.coil.is_empty();
        }
        if self.t >= FORCE_AT {
            return false;
        }
        let mv = if self.t >= TRAP_AT { self.trap_move() } else { self.choose() };
        match mv {
            Some((x, y)) => {
                self.advance(x, y);
                true
            }
            None => false,
        }
    }

    fn spawn_bean(&mut self) {
        let h = self.head();
        for _ in 0..40 {
            let x = self.rng.int(self.cols as usize) as i32;
            let y = self.rng.int(self.rows as usize) as i32;
            let i = self.idx(x, y);
            if self.blocked[i] || self.occ[i] || self.bean_at(x, y).is_some() || (x - h.x).abs() + (y - h.y).abs() < 4 {
                continue;
            }
            let hue = self.rng.range(0.0, 84.0);
            self.beans.push(Bean { x, y, born: self.t, hue });
            return;
        }
    }

    fn enter_stuck(&mut self, app: &mut App) {
        if self.mode == Mode::Run || self.knot == (0.0, 0.0) {
            let h = self.head();
            self.knot = self.center(h.x, h.y);
            self.knot_r = 2.0 * self.k as f32;
        }
        self.mode = Mode::Stuck;
        self.mt = 0.0;
        self.coil.clear();
        self.pops.clear();
        self.flash = self.t;
        app.push(log_line("ERROR", "snake::ai", "无路可走：贪吃 bug 把自己绕成了死结（deadlock）", Kind::Info));
    }

    fn burst(&mut self, app: &mut App) {
        let n = self.body.len();
        let (kx, ky) = self.knot;
        for (i, s) in self.body.iter().enumerate() {
            let (x, y) = self.center(s.x, s.y);
            let (mut ex, mut ey) = (x - kx, y - ky);
            let a = self.rng.range(0.0, TAU);
            ex += a.cos() * 2.0;
            ey += a.sin() * 2.0;
            let d = (ex * ex + ey * ey).sqrt().max(0.5);
            let sp = self.rng.range(45.0, 95.0);
            let rbw = i < self.rainbow && i + 1 < n;
            let shade = if i + 1 == n { 0 } else if rbw { 0 } else { [0u8, 2, 0, 3][(s.e % 4) as usize] };
            self.flies.push(Fly {
                x,
                y,
                vx: ex / d * sp,
                vy: ey / d * sp * 0.75 - 8.0,
                flap: self.rng.range(0.0, 4.0),
                shade,
                rainbow: if rbw { 1.0 } else { 0.0 },
            });
        }
        self.body.clear();
        self.occ.fill(false);
        self.beans.clear();
        self.mode = Mode::Burst;
        self.mt = 0.0;
    }

    fn seg_col(&self, i: usize) -> Col {
        let s = self.body[i];
        if self.flash >= 0.0 && self.t - self.flash < 0.22 {
            return [255, 96, 96].into();
        }
        if i < self.rainbow && i + 1 < self.body.len() {
            rb(Band::Text, s.e as f32 * 7.0, 0.0)
        } else {
            match s.e % 4 {
                0 | 2 => [226, 226, 230].into(),
                1 => [158, 158, 166].into(),
                _ => [92, 92, 100].into(),
            }
        }
    }
}

/// 放大 k 倍的飞蛾（k = 1 就是普通飞蛾），(cx, cy) 是中心像素
fn moth_k(app: &mut App, cx: f32, cy: f32, shade: u8, flap: f32, k: i32) {
    let k = k.max(1);
    let wing: Rgb = match shade {
        0 | 1 => [238, 238, 238],
        2 => [175, 175, 175],
        3 => [120, 120, 120],
        _ => [22, 22, 22],
    };
    let body = if shade == 4 { [235, 235, 235] } else { [70, 66, 80] };
    let fr = &MOTH[((flap as i32) & 1) as usize];
    let (ox, oy) = (cx.round() as i32 - 5 * k / 2, cy.round() as i32 - 3 * k / 2);
    // 先描一圈深色边，压在白色身体上也认得出头
    let (gw, gh) = (5 * k + 2, 3 * k + 2);
    let mut buf: Vec<Option<Rgb>> = vec![None; (gw * gh) as usize];
    for (j, row) in fr.iter().enumerate() {
        for (i, ch) in row.chars().enumerate() {
            let c = match ch {
                '#' => wing,
                'o' => body,
                _ => continue,
            };
            for sy in 0..k {
                for sx in 0..k {
                    buf[((j as i32 * k + sy + 1) * gw + i as i32 * k + sx + 1) as usize] = Some(c);
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
                near.then_some([18, 16, 28])
            });
            if let Some(c) = c {
                app.layer.set(ox + x - 1, oy + y - 1, c);
            }
        }
    }
}

/// 日志压暗退到背景：彩虹字换成插曲开始那一刻的颜色、压到三成左右的静态真彩色（不再流动，静止时零成本）。
/// (x + 2y)/span 在 [lo, hi) 之间的格子才压暗，用来做斜着扫过去的转场。
fn dim_logs(app: &mut App, tab: &[Rgb], lo: f32, hi: f32) {
    let (w, h) = (app.scr.w, app.scr.h);
    let span = (w + 2 * h) as f32;
    for y in 0..h {
        for x in 0..w {
            let f = (x as f32 + 2.0 * y as f32) / span;
            if f < lo || f >= hi {
                continue;
            }
            let c = &mut app.scr.cells[y * w + x];
            c.fg = Col::T(match c.fg {
                Col::P(i) => tab[i as usize],
                Col::T(rgb) => scale(rgb, 0.3),
            });
            if let Col::T(b) = c.bg {
                if b != BLACK {
                    c.bg = Col::T(scale(b, 0.3));
                }
            }
        }
    }
}

impl Interlude for Snake {
    fn name(&self) -> &'static str {
        "贪吃 Bug"
    }

    fn update(&mut self, app: &mut App, dt: f32) -> bool {
        self.t += dt;
        self.mt += dt;
        let t = self.t;
        match self.mode {
            Mode::Gather | Mode::Run | Mode::Coil => {
                if self.mode == Mode::Gather && t >= self.run_at {
                    self.mode = Mode::Run;
                }
                // 豆子陆续冒出来
                self.bean_cd -= dt;
                if t > 1.0 && self.mode != Mode::Coil && self.beans.len() < self.bean_n && self.bean_cd <= 0.0 {
                    self.spawn_bean();
                    self.bean_cd = if t < 3.0 { 0.12 } else { 0.25 };
                }
                if self.mode != Mode::Gather {
                    let extra = (self.body.len() as f32 - L0 as f32).max(0.0);
                    let sps = if self.mode == Mode::Coil { 13.0 } else { (12.0 + extra * 0.45).min(20.0) };
                    self.acc += dt * sps;
                    while self.acc >= 1.0 {
                        self.acc -= 1.0;
                        if !self.tick() {
                            self.enter_stuck(app);
                            break;
                        }
                    }
                }
                // 鸭鸭在角落围观，偶尔冒泡
                app.duck.right = false;
                for (k, at) in [3.2f32, 6.8, 10.2].iter().enumerate() {
                    if self.chat_i == k && t >= *at && self.mode != Mode::Coil && self.mode != Mode::Stuck {
                        let s = self.chat[k % self.chat.len()];
                        app.quack(s, 1.4);
                        self.chat_i += 1;
                    }
                }
                if self.ev >= 100 && self.ev < 101 {
                    self.ev = 101;
                    app.push(log_line("WARN", "snake::ai", "发现一颗超香的豆，防自杀检查已临时关闭（贪心模式）", Kind::Info));
                    app.quack("喂……", 1.0);
                }
            }
            Mode::Stuck => {
                if self.ev < 200 && self.mt >= 0.45 {
                    self.ev = 200;
                    // 鸭鸭游到结旁边，嘴对着结；结贴着右边就从左边过去
                    let (hw, hh) = if self.small { (5.0, 4.0) } else { app.duck_half() };
                    let w = app.w();
                    let rx = self.knot.0 + self.knot_r + hw * 0.8;
                    let (tx, face_right) = if rx <= w - hw * 0.6 { (rx, false) } else { ((self.knot.0 - self.knot_r - hw * 0.8).max(hw * 0.5), true) };
                    let ty = (self.knot.1 - hh * 0.25).max(hh + 8.0).min(app.hp() - hh);
                    self.swim = Some((app.duck.x, app.duck.y, tx, ty, face_right));
                }
                // 自己驱动鸭鸭（x 和 tx 一起设，App 的缓动就不会把它掉头）
                if let Some((fx, fy, tx, ty, face)) = self.swim {
                    let u = smooth(0.45, 1.55, self.mt);
                    let lunge = if self.mt > 1.7 && self.mt < 1.95 { ((self.mt - 1.7) / 0.25 * std::f32::consts::PI).sin() * 4.0 } else { 0.0 };
                    let x = fx + (tx - fx) * u + if face { lunge } else { -lunge };
                    let y = fy + (ty - fy) * u;
                    app.duck.x = x;
                    app.duck.tx = x;
                    app.duck.y = y;
                    app.duck.ty = y;
                    app.duck.right = face;
                }
                if self.ev < 201 && self.mt >= 1.75 {
                    self.ev = 201;
                    app.quack("嘎！", 0.9);
                }
                if self.mt >= 1.9 {
                    self.burst(app);
                }
            }
            Mode::Burst => {
                if self.ev < 300 && self.mt >= 0.45 {
                    self.ev = 300;
                    app.push(log_line("INFO", "snake::ai", "贪吃 bug 已解开（被鸭啄开的）", Kind::Info));
                }
                if self.ev < 301 && self.mt >= 1.0 {
                    self.ev = 301;
                    app.quack("写循环要留出口", 1.3);
                    app.duck.tx = self.corner.0;
                    app.duck.ty = self.corner.1;
                }
                if self.mt >= 1.7 {
                    self.mode = Mode::Out;
                    self.mt = 0.0;
                }
            }
            Mode::Out => {}
        }
        // 散开的飞蛾
        let (w, hp) = (app.w(), app.hp());
        for (i, f) in self.flies.iter_mut().enumerate() {
            f.x += f.vx * dt;
            f.y += f.vy * dt + (t * 7.0 + i as f32).sin() * dt * 6.0;
            f.vx *= 1.0 + dt * 0.25;
            f.vy *= 1.0 + dt * 0.25;
            f.flap += dt * 14.0;
        }
        self.flies.retain(|f| f.x > -6.0 && f.x < w + 6.0 && f.y > -6.0 && f.y < hp + 6.0);
        self.pops.retain(|p| t - p.2 < 0.6);

        // 标题栏
        let title = match self.mode {
            Mode::Stuck => format!("鸭鸭.exe — 贪吃 bug（长度 {}，死锁）", self.body.len()),
            Mode::Burst | Mode::Out => "鸭鸭.exe — 贪吃 bug（已解开）".to_string(),
            _ => format!("鸭鸭.exe — 贪吃 bug（长度 {}）", self.body.len()),
        };
        app.set_title(&title);

        if self.mode == Mode::Out && self.mt >= 1.15 {
            // 还在屏幕里的飞蛾交还给主线，接着围着鸭鸭转
            app.moths.clear();
            for f in self.flies.iter().take(10) {
                app.spawn_moth(f.x, f.y, f.vx * 0.5, f.vy * 0.5);
                if let Some(m) = app.moths.last_mut() {
                    m.shade = f.shade;
                    m.color = f.rainbow;
                }
            }
            return true;
        }
        false
    }

    fn draw(&mut self, app: &mut App) {
        let t = self.t;
        app.draw_logs(9.0, 0, 0.0);
        // 进场：斜着扫一遍压暗；退场：反着扫回来
        let din = smooth(0.0, 0.9, t) * 1.15;
        let dout = if self.mode == Mode::Out { smooth(0.0, 0.8, self.mt) * 1.15 } else { 0.0 };
        if self.mode != Mode::Out || dout < 1.1 {
            let tab = std::mem::take(&mut self.dimtab);
            dim_logs(app, &tab, dout, din);
            self.dimtab = tab;
        }

        let k = self.k;
        let bs = 2 * k;
        // 豆子：小屏是十字小星星（眨眼变斜十字）；大屏是 4×4 去角的圆豆（眨眼时亮一个高光）
        for b in &self.beans {
            let (px, py) = self.px(b.x, b.y);
            let age = t - b.born;
            let c = rb(Band::Duck, b.hue, 0.0);
            let wink = (t * 0.7 + b.hue * 0.37).fract() < 0.12;
            if k == 1 {
                let (cx, cy) = (px + 1, py + 1);
                app.layer.set(cx, cy, c);
                if age < 0.25 {
                    continue;
                }
                let arms: [(i32, i32); 4] = if wink { [(-1, -1), (1, -1), (-1, 1), (1, 1)] } else { [(-1, 0), (1, 0), (0, -1), (0, 1)] };
                for (dx, dy) in arms {
                    app.layer.set(cx + dx, cy + dy, c);
                }
            } else {
                for dy in 0..4 {
                    for dx in 0..4 {
                        let corner = (dx == 0 || dx == 3) && (dy == 0 || dy == 3);
                        let inner = (1..3).contains(&dx) && (1..3).contains(&dy);
                        if corner || (age < 0.25 && !inner) {
                            continue;
                        }
                        app.layer.set(px + dx, py + dy, c);
                    }
                }
                if wink && age >= 0.25 {
                    app.layer.set(px + 1, py + 1, [255, 255, 250]);
                }
            }
        }

        // 身体：bs×bs 一节，前后相连的两节之间补上缝
        let n = self.body.len();
        let gathering = self.mode == Mode::Gather;
        let shown = |i: usize| !gathering || t >= self.gather[i.min(self.gather.len() - 1)].2;
        for i in 0..n.saturating_sub(1) {
            if !shown(i) {
                continue;
            }
            let s = self.body[i];
            let (px, py) = self.px(s.x, s.y);
            let c = self.seg_col(i);
            // 节与节之间的缝用暗一档的同色，看得出一节一节
            let cj = match c {
                Col::P(p) if p >= Band::Text as u8 && p < Band::Dim as u8 => Col::P(p + PN),
                Col::T(rgb) => Col::T(scale(rgb, 0.6)),
                o => o,
            };
            for dy in 0..bs {
                for dx in 0..bs {
                    app.layer.set(px + dx, py + dy, c);
                }
            }
            if !shown(i + 1) {
                continue;
            }
            let nx = self.body[i + 1];
            let (qx, qy) = self.px(nx.x, nx.y);
            let (gx, gy, gw, gh) = match (nx.x - s.x, nx.y - s.y) {
                (1, 0) => (px + bs, py, k, bs),
                (-1, 0) => (qx + bs, py, k, bs),
                (0, 1) => (px, py + bs, bs, k),
                (0, -1) => (px, qy + bs, bs, k),
                _ => (0, 0, 0, 0),
            };
            for dy in 0..gh {
                for dx in 0..gw {
                    app.layer.set(gx + dx, gy + dy, cj);
                }
            }
        }
        // 头：领头的那只飞蛾（大屏放大一倍），每走一步扇一下翅膀，触角朝前
        if n > 0 && shown(n - 1) {
            let h = self.body[n - 1];
            let (cx, cy) = self.center(h.x, h.y);
            let shade = if self.flash >= 0.0 && t - self.flash < 0.22 { 4 } else { 0 };
            let (dx, dy) = self.dir;
            let ant: Rgb = [210, 210, 220];
            for j in 0..k {
                let (ax, ay) = (cx as i32 + dx * (2 * k + j), cy as i32 + dy * (2 * k + j) - if dy == 0 { 0 } else { 0 });
                let off = k + j;
                if dx != 0 {
                    app.layer.set(ax, ay - off, ant);
                    app.layer.set(ax, ay + off - 1 + (k == 1) as i32, ant);
                } else {
                    app.layer.set(ax - off, ay, ant);
                    app.layer.set(ax + off - 1 + (k == 1) as i32, ay, ant);
                }
            }
            moth_k(app, cx, cy, shade, h.e as f32, k);
        }
        // 开场：飞蛾们飞过来排队
        if gathering {
            for (k, &(sx, sy, at)) in self.gather.iter().enumerate() {
                if t >= at || k >= n {
                    continue;
                }
                let s = self.body[k];
                let (tx, ty) = self.center(s.x, s.y);
                let u = smooth(at - 1.1, at, t);
                let (ex, ey) = (tx - sx, ty - sy);
                let arc = (u * std::f32::consts::PI).sin() * 10.0;
                let l = (ex * ex + ey * ey).sqrt().max(1.0);
                let (x, y) = (sx + ex * u - ey / l * arc, sy + ey * u + ex / l * arc);
                let shade = if k + 1 == n { 0 } else { [0u8, 2, 0, 3][(s.e % 4) as usize] };
                moth(app, x, y, shade, t * 12.0 + k as f32, 0.0);
            }
        }

        // 鸭鸭
        if self.small {
            let (x, y, r) = (app.duck.x, app.duck.y + (t * 2.3).sin() * 1.0, app.duck.right);
            mini_duck(app, x, y, r, 1);
        } else {
            app.draw_glow(app.duck.glow);
            app.draw_duck(None);
        }

        // 四散的飞蛾
        for i in 0..self.flies.len() {
            let f = &self.flies[i];
            let (x, y, s, fl, r) = (f.x, f.y, f.shade, f.flap, f.rainbow);
            moth(app, x, y, s, fl, r);
        }
    }

    fn draw_over(&mut self, app: &mut App) {
        let t = self.t;
        // 顶上的小标签
        if self.mode != Mode::Out {
            let s = match self.mode {
                Mode::Stuck => format!(" 贪吃 bug — 长度 {} — 死锁 ", self.body.len()),
                Mode::Burst => " 贪吃 bug — 已解开 ".to_string(),
                _ => format!(" 贪吃 bug — 长度 {} ", self.body.len()),
            };
            let w = app.scr.w as i32;
            let x = ((w - str_w(&s) as i32) / 2).max(0);
            app.scr.text(x, 0, &s, [235, 235, 240], Some([28, 26, 40]));
        }
        // 吃到豆：冒个 +1
        for &(px, py, t0) in &self.pops {
            let y = py / 2 - 2 - ((t - t0) * 3.0) as i32;
            app.scr.text(px + self.k * 3, y, "+1", [255, 240, 200], None);
        }
        // 卡住：头上冒问号
        if self.mode == Mode::Stuck && !self.body.is_empty() {
            let h = self.head();
            let (px, py) = self.px(h.x, h.y);
            let s = if (self.mt * 4.0) as i32 % 2 == 0 { "?!" } else { "!?" };
            app.scr.text(px, py / 2 - 1, s, [255, 230, 120], Some([40, 20, 20]));
        }
    }
}
