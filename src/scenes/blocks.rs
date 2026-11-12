//! 插曲：整理 bug——只借「块从上方落下、可旋转平移、拼满一行就消掉」的通用玩法。
//! 场地是整个终端的宽度（右边留给指挥的鸭子），落下来的是 Bug 卷成的怪块：
//! 3/5/6 格随机连成的不规则多联块。
//! 飞蛾黑白灰，格子里是 卸 和 }{ 小翅膀；一格 = 2 列 × 1 行。好几块同时在不同的「车道」里落，AI 挑块挑落点。
//! 拼满一行：闪白 → 变成一行彩虹日志 → 往上飞回日志流。
//! 结尾：从天上掉下一块巨大的鸭子形状的块，它的肚子底下正好是 bug 堆的反面——连 bug 堆深处的空洞都填上了。
//! 一次消掉整堆，满屏彩虹字溶解回日志，bug 们飞回鸭子身边。
//!
//! 性能：背景是压暗的静态日志；每帧变的只有落着的几块、正在闪/飞的几行。终局溶解每格只变一次。
use super::*;

const FY0: i32 = 1; // 场地从第 1 行开始（第 0 行是标签）
const T_PLAY: f32 = 1.4;
const T_STOP: f32 = 14.6;
const FLASH: f32 = 0.36;
const D_WAIT: f32 = 0.35; // 鸭子块出现前的停顿
const D_FALL: f32 = 0.8;
const B_END: f32 = 2.75;
const O_END: f32 = 0.8;
const ORANGE: Rgb = [255, 160, 40];

/// 飞蛾三种灰：(底色, 花纹色)
const SHADES: [(Rgb, Rgb); 3] = [([196, 196, 204], [84, 82, 98]), ([124, 122, 136], [34, 32, 44]), ([48, 46, 58], [162, 160, 176])];
const GLYPH: [(char, char); 4] = [('ж', ' '), (' ', 'ж'), ('}', '{'), ('ж', 'ж')];

const CHAT: [&str; 6] = ["往左——再左一点", "转一下！转！", "别留洞啊", "这块长得像我", "那行就差一格", "稳住，能消"];
const BOOM_TXT: &[u8] = b"quack(); fix(bug); sort(logs); gc(); ";

#[derive(PartialEq, Clone, Copy)]
enum Ph {
    Play,
    Drain,
    Duck,
    Boom,
    Out,
}

struct Row {
    c: Vec<u8>,
    id: u32,
}
struct Clear {
    id: u32,
    t0: f32,
    text: String,
}
struct FlyLine {
    y0: f32,
    t0: f32,
    text: String,
}
struct Fall {
    lane: usize,
    rots: Vec<Vec<(i32, i32)>>,
    r: usize,
    x: i32,
    y: f32,
    iy: i32,
    tr: usize,
    tx: i32,
    ly: i32,
    shade: u8,
    glyph: Vec<u8>,
    acc: f32,
    drop: bool,
}
struct Lane {
    x0: i32,
    x1: i32,
    cd: f32,
}
struct Bit {
    x: i32,
    y: i32,
    ch: char,
    die: f32,
}
struct Fly {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    shade: u8,
    ph: f32,
    born: f32,
}

pub struct Blocks {
    t: f32,
    mt: f32,
    ph: Ph,
    rng: Rng,
    fw: i32,
    fh: i32,
    x0: i32,
    cw: i32,
    ch: i32,
    jh: i32,
    rows: Vec<Row>,
    next_id: u32,
    top: Vec<i32>,
    rowcnt: Vec<i32>,
    lanes: Vec<Lane>,
    falls: Vec<Fall>,
    clears: Vec<Clear>,
    lines: Vec<FlyLine>,
    n: u32,
    msg_k: u32,
    slab: Vec<(i32, i32, u8)>,
    slab_n: i32,
    bits: Vec<Bit>,
    flies: Vec<Fly>,
    small: bool,
    duck_at: (f32, f32),
    dimtab: Vec<Rgb>,
    chat: Vec<&'static str>,
    chat_i: usize,
    first_clear: bool,
    say: Option<(String, f32)>,
}

fn norm(c: &mut [(i32, i32)]) {
    let mx = c.iter().map(|p| p.0).min().unwrap_or(0);
    let my = c.iter().map(|p| p.1).min().unwrap_or(0);
    for p in c.iter_mut() {
        p.0 -= mx;
        p.1 -= my;
    }
}

/// 随机长一个 n 格的连通形状（不在任何「标准形状表」里挑，纯随机生长）
fn grow(rng: &mut Rng, n: usize) -> Vec<(i32, i32)> {
    let mut c = vec![(0, 0)];
    let mut guard = 0;
    while c.len() < n && guard < 200 {
        guard += 1;
        let (bx, by) = c[rng.int(c.len())];
        let (dx, dy) = [(1, 0), (-1, 0), (0, 1), (0, -1)][rng.int(4)];
        let p = (bx + dx, by + dy);
        if !c.contains(&p) {
            c.push(p);
        }
    }
    norm(&mut c);
    c
}

fn rots_of(c: &[(i32, i32)]) -> Vec<Vec<(i32, i32)>> {
    let mut out: Vec<Vec<(i32, i32)>> = Vec::new();
    let mut cur = c.to_vec();
    for _ in 0..4 {
        let mut s = cur.clone();
        s.sort();
        if !out.iter().any(|o| {
            let mut so = o.clone();
            so.sort();
            so == s
        }) {
            out.push(cur.clone());
        }
        cur = cur.iter().map(|&(x, y)| (y, -x)).collect();
        norm(&mut cur);
    }
    out
}

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
                Col::T(rgb) => scale(rgb, 0.2),
            });
            if let Col::T(b) = c.bg {
                if b != BLACK {
                    c.bg = Col::T(scale(b, 0.2));
                }
            }
        }
    }
}

/// 彩虹字（调色板色，终端自己流动），黑底；超过 xmax 就停
fn rtext(app: &mut App, x: i32, y: i32, s: &str, xmax: i32) -> i32 {
    let mut x = x;
    for ch in s.chars() {
        let cw = char_w(ch) as i32;
        if x + cw > xmax {
            break;
        }
        app.scr.put_c(x, y, ch, rb(Band::Text, x as f32, y as f32), Some(Col::T(BLACK)));
        x += cw;
    }
    x
}

impl Blocks {
    pub fn new(app: &mut App) -> Self {
        let (w, h) = (app.scr.w as i32, app.scr.h as i32);
        let hp = h as f32 * 2.0;
        let small = w < 100 || h < 30;
        let (hw, hh) = app.duck_half();
        let dz = if small { 12 } else { (hw * 2.0) as i32 + 8 };
        let big = w >= 150 && h >= 40;
        let (cw, ch) = if big { (4, 2) } else { (2, 1) };
        let (fw, x0) = if big {
            let fw = 14;
            (fw, ((w - (fw * cw + 24 + 38)) / 2).max(8))
        } else {
            let fw = ((w - dz) / cw).max(4);
            (fw, ((w - dz - fw * cw) / 2).max(0))
        };
        let fh = ((h - FY0) / ch).max(4);
        let mut rng = Rng(app.rng.next() | 1);

        // 车道：大屏每条约 5 格宽，几块同时落
        let lw = if big { 5 } else if small { 7 } else { 9 };
        let mut nl = (fw / lw).max(1);
        if !app.hi {
            nl = nl.min(4);
        }
        let lanes: Vec<Lane> = (0..nl)
            .map(|i| Lane { x0: fw * i / nl, x1: fw * (i + 1) / nl, cd: T_PLAY + (i % 5) as f32 * 0.12 + rng.range(0.0, 0.2) })
            .collect();

        // 开场的 bug 堆：上面几行有能填的坑，下面每行都藏着被盖住的空洞（AI 消不掉，留给结尾）
        let jh = ((fh as f32 * if small { 0.3 } else { 0.36 }) as i32).max(2);
        let bd = (jh / 2).clamp(1, 5);
        let mut depth = vec![0i32; fw as usize];
        let mut c = 0;
        while c < fw {
            let seg = 1 + rng.int(4) as i32;
            let d = if rng.f() < 0.35 { 0 } else { rng.int(bd as usize + 1) as i32 };
            for k in c..(c + seg).min(fw) {
                depth[k as usize] = d;
            }
            c += seg;
        }
        let mut rows: Vec<Row> = (0..fh).map(|i| Row { c: vec![0; fw as usize], id: i as u32 }).collect();
        for r in (fh - jh)..fh {
            for cc in 0..fw {
                if r >= fh - jh + depth[cc as usize] {
                    if big {
                        let shade = (((cc / 6) as u32 ^ (r / 4) as u32).wrapping_mul(2654435761) >> 7) % 3;
                        let g = (((cc as u32 / 2) ^ r as u32) % 4) as u8;
                        rows[r as usize].c[cc as usize] = 1 + shade as u8 * 4 + g;
                    } else {
                        let shade = (((cc / 3) as u32 ^ (r / 2) as u32).wrapping_mul(2654435761) >> 7) % 3;
                        rows[r as usize].c[cc as usize] = 1 + shade as u8 * 4 + rng.int(4) as u8;
                    }
                }
            }
        }
        for r in (fh - jh + bd)..fh {
            let nh = 1 + rng.int(2 + (fw / 30) as usize);
            for _ in 0..nh {
                let cc = rng.int(fw as usize);
                if r > 0 && rows[r as usize - 1].c[cc] != 0 {
                    rows[r as usize].c[cc] = 0;
                }
            }
            if rows[r as usize].c.iter().all(|&v| v != 0) {
                let cc = rng.int(fw as usize);
                rows[r as usize].c[cc] = 0;
            }
        }

        let mut chat: Vec<&'static str> = CHAT.to_vec();
        for i in (1..chat.len()).rev() {
            let j = rng.int(i + 1);
            chat.swap(i, j);
        }

        let duck_at = if small {
            ((w - dz / 2) as f32, (hp * 0.45).max(6.0))
        } else if big {
            ((x0 + fw * cw + 24) as f32, (hp * 0.4).max(hh + 10.0))
        } else {
            ((w - dz / 2) as f32, (hp * 0.4).max(hh + 10.0))
        };
        app.moths.clear();
        app.duck.visible = true;
        app.duck.asleep = false;
        app.duck.glow_to = 0.55;
        app.duck.x = duck_at.0;
        app.duck.tx = duck_at.0;
        app.duck.y = duck_at.1;
        app.duck.ty = duck_at.1;
        app.duck.right = false;
        app.duck.bubble = None;
        app.push(log_line("INFO", "bug::sort", "整理 bug.exe 已加载：自动摆放 AI v0.3（会旋转，不会累）", Kind::Info));
        app.set_title("鸭子.exe — 整理 bug");

        let mut b = Blocks {
            t: 0.0,
            mt: 0.0,
            ph: Ph::Play,
            rng,
            fw,
            fh,
            x0,
            cw,
            ch,
            jh,
            rows,
            next_id: fh as u32,
            top: vec![0; fw as usize],
            rowcnt: vec![0; fh as usize],
            lanes,
            falls: Vec::new(),
            clears: Vec::new(),
            lines: Vec::new(),
            n: 0,
            msg_k: 0,
            slab: Vec::new(),
            slab_n: 0,
            bits: Vec::new(),
            flies: Vec::new(),
            small,
            duck_at,
            dimtab: (0..256u32)
                .map(|i| {
                    let i = i as u8;
                    let k = if i >= Band::Dim as u8 && i < Band::Duck as u8 { 0.45 } else { 0.19 };
                    scale(pal_rgb(i, app.pal_t()), k)
                })
                .collect(),
            chat,
            chat_i: 0,
            first_clear: false,
            say: None,
        };
        b.recompute();
        b
    }

    /// 小屏自己画气泡：App 的气泡按大鸭子定位，40x12 上会落到标题那一行
    fn say(&mut self, app: &mut App, text: &str, dur: f32) {
        if self.small {
            self.say = Some((text.to_string(), self.t + dur));
        } else {
            app.quack(text, dur);
        }
    }

    fn recompute(&mut self) {
        let (fw, fh) = (self.fw as usize, self.fh as usize);
        for c in 0..fw {
            self.top[c] = (0..fh).find(|&r| self.rows[r].c[c] != 0).unwrap_or(fh) as i32;
        }
        for r in 0..fh {
            self.rowcnt[r] = self.rows[r].c.iter().filter(|&&v| v != 0).count() as i32;
        }
    }

    fn fits(&self, lane: usize, cells: &[(i32, i32)], x: i32, y: i32) -> bool {
        let l = &self.lanes[lane];
        cells.iter().all(|&(cx, cy)| {
            let (c, r) = (x + cx, y + cy);
            c >= l.x0 && c < l.x1 && r < self.fh && (r < 0 || self.rows[r as usize].c[c as usize] == 0)
        })
    }

    /// 从车道里最低的表面格子开始，沿表面长出 n 格：每格要么正压在堆上，要么压在本块上（落下去严丝合缝）
    fn fitted(&mut self, l0: i32, l1: i32, n: usize, lowest: bool) -> Option<Vec<(i32, i32)>> {
        let mx = (l0..l1).map(|c| self.top[c as usize]).max()?;
        let pool: Vec<i32> = (l0..l1).filter(|&c| if lowest { self.top[c as usize] == mx } else { self.top[c as usize] >= mx - 1 }).collect();
        let c0 = pool[self.rng.int(pool.len())];
        let start = (c0, self.top[c0 as usize] - 1);
        if start.1 < 0 {
            return None;
        }
        let mut set = vec![start];
        for _ in 0..40 {
            if set.len() >= n {
                break;
            }
            let mut opts = Vec::new();
            for &(x, y) in &set {
                for (dx, dy) in [(1, 0), (-1, 0), (0, -1), (0, 1)] {
                    let p = (x + dx, y + dy);
                    if p.0 < l0 || p.0 >= l1 || p.1 < 0 || p.1 >= self.top[p.0 as usize] || set.contains(&p) || opts.contains(&p) {
                        continue;
                    }
                    if p.1 + 1 == self.top[p.0 as usize] || set.contains(&(p.0, p.1 + 1)) {
                        opts.push(p);
                    }
                }
            }
            if opts.is_empty() {
                break;
            }
            // 偏爱往低处、往旁边长（填平），偶尔往上冒一格
            opts.sort_by_key(|p| -p.1);
            let pick = if self.rng.f() < 0.75 { self.rng.int(opts.len().min(2)) } else { self.rng.int(opts.len()) };
            set.push(opts[pick]);
        }
        if set.len() < 3 {
            return None;
        }
        norm(&mut set);
        Some(set)
    }

    /// AI：几种候选怪块 × 每个朝向 × 每个落点，按「消行、填满度、新空洞、高度」打分
    fn plan(&mut self, lane: usize) -> Option<Fall> {
        let (l0, l1) = (self.lanes[lane].x0, self.lanes[lane].x1);
        let fw = self.fw as f32;
        let mut best: Option<(f32, usize, usize, i32, i32)> = None;
        let mut cands = Vec::new();
        // 一半候选是 bug 们照着坑「卷」出来的形状（贴着表面长，不留洞），一半纯随机
        for k in 0..4 {
            let n = [3usize, 5, 6][self.rng.int(3)];
            let shape = if k < 2 { self.fitted(l0, l1, n, k == 0) } else { None };
            let shape = shape.unwrap_or_else(|| grow(&mut self.rng, n));
            cands.push(rots_of(&shape));
        }
        for (si, rots) in cands.iter().enumerate() {
            for (ri, cells) in rots.iter().enumerate() {
                let cw = cells.iter().map(|p| p.0).max().unwrap_or(0) + 1;
                for x in l0..=(l1 - cw) {
                    let mut y = i32::MAX;
                    for &(cx, cy) in cells {
                        y = y.min(self.top[(x + cx) as usize] - 1 - cy);
                    }
                    if y < 0 {
                        continue;
                    }
                    let mut s = 0.0;
                    let mut lines = 0;
                    let mut ys: Vec<(i32, i32)> = Vec::new();
                    for &(_, cy) in cells {
                        match ys.iter_mut().find(|p| p.0 == y + cy) {
                            Some(p) => p.1 += 1,
                            None => ys.push((y + cy, 1)),
                        }
                    }
                    for &(r, k) in &ys {
                        let rc = self.rowcnt[r as usize];
                        if rc + k >= self.fw {
                            lines += 1;
                        }
                        s += (rc as f32 / fw).powi(2) * k as f32 * 1.6;
                    }
                    let mut holes = 0;
                    for &(cx, cy) in cells {
                        let c = x + cx;
                        if cells.iter().any(|&(ox, oy)| ox == cx && oy > cy) {
                            continue;
                        }
                        holes += self.top[c as usize] - (y + cy) - 1;
                    }
                    let topy = y + cells.iter().map(|p| p.1).min().unwrap_or(0);
                    s += lines as f32 * 12.0 - holes as f32 * 4.5 - (self.fh - topy) as f32 * 0.35 + self.rng.f() * 0.4;
                    if best.map_or(true, |b| s > b.0) {
                        best = Some((s, si, ri, x, y));
                    }
                }
            }
        }
        let (_, si, ri, tx, ly) = best?;
        let rots = cands.swap_remove(si);
        let r0 = self.rng.int(rots.len());
        let cells = &rots[r0];
        let cw = cells.iter().map(|p| p.0).max().unwrap_or(0) + 1;
        let chh = cells.iter().map(|p| p.1).max().unwrap_or(0) + 1;
        let x = ((l0 + l1) / 2 - cw / 2).clamp(l0, (l1 - cw).max(l0));
        let n = cells.len();
        let glyph = (0..n).map(|_| [0u8, 0, 1, 1, 2, 3][self.rng.int(6)]).collect();
        Some(Fall {
            lane,
            rots,
            r: r0,
            x,
            y: -chh as f32,
            iy: -chh,
            tr: ri,
            tx,
            ly,
            shade: self.rng.int(2) as u8,
            glyph,
            acc: 0.0,
            drop: false,
        })
    }

    fn line_text(&mut self, width: i32) -> String {
        let mut s = String::new();
        while (str_w(&s) as i32) < width {
            self.msg_k += 1;
            let hex = self.rng.int(0xFFFF);
            let m = match (self.msg_k + self.rng.int(3) as u32) % 6 {
                0 => format!("[INFO] bug::gc 已归档 bug#0x{:04X}，排得整整齐齐", hex),
                1 => "[DEBUG] duck::lint 这一行没毛病".to_string(),
                2 => format!("[INFO] log::sink 飞蛾 ×{} 已写回 stdout", 6 + hex % 30),
                3 => format!("[OK] bug#0x{:04X} 已按时间戳排序", hex),
                4 => "[WARN] bug::gc 有只 bug 想插队，已驳回".to_string(),
                _ => format!("[INFO] 第 {} 行日志整理完毕", self.n + 1),
            };
            s.push_str(&m);
            s.push_str("   ");
        }
        s
    }

    fn lock(&mut self, f: &Fall) {
        let cells = &f.rots[f.r];
        let mut touched: Vec<i32> = Vec::new();
        for (k, &(cx, cy)) in cells.iter().enumerate() {
            let (c, r) = (f.x + cx, f.iy + cy);
            if r < 0 || r >= self.fh {
                continue;
            }
            self.rows[r as usize].c[c as usize] = 1 + f.shade * 4 + f.glyph[k];
            if !touched.contains(&r) {
                touched.push(r);
            }
        }
        for r in touched {
            let row = &self.rows[r as usize];
            if row.c.iter().all(|&v| v != 0) && !self.clears.iter().any(|c| c.id == row.id) {
                let id = row.id;
                let text = self.line_text(self.fw * self.cw);
                self.clears.push(Clear { id, t0: self.t, text });
            }
        }
        self.lanes[f.lane].cd = self.rng.range(0.02, 0.1);
        self.recompute();
    }

    fn build_slab(&mut self) {
        let st = self.top.iter().copied().min().unwrap_or(self.fh);
        let s = ((self.fw as f32 * 0.45 / 10.0) as i32).min(st / 7).max(1);
        let dr0 = (st - 7 * s).max(1);
        let dc0 = (self.fw - 10 * s) / 2;
        self.slab.clear();
        for (j, row) in MINI_DUCK.iter().enumerate() {
            for (i, ch) in row.chars().enumerate() {
                let kind = match ch {
                    '#' => 0u8,
                    'e' => 1,
                    'B' => 2,
                    _ => continue,
                };
                for sy in 0..s {
                    for sx in 0..s {
                        let c = dc0 + i as i32 * s + sx;
                        if c >= 0 && c < self.fw {
                            self.slab.push((c, dr0 + j as i32 * s + sy, kind));
                        }
                    }
                }
            }
        }
        for r in st..self.fh {
            for c in 0..self.fw {
                if self.rows[r as usize].c[c as usize] == 0 {
                    self.slab.push((c, r, 3));
                }
            }
        }
        self.slab_n = self.fh - st;
    }

    fn boom(&mut self, app: &mut App) {
        let mut cells: Vec<(i32, i32)> = Vec::new();
        for r in 0..self.fh {
            for c in 0..self.fw {
                if self.rows[r as usize].c[c as usize] != 0 {
                    cells.push((c, r));
                }
            }
        }
        for &(c, r, _) in &self.slab {
            if r >= 0 {
                cells.push((c, r));
            }
        }
        let fh = self.fh as f32;
        self.bits.clear();
        for &(c, r) in &cells {
            let (sx, sy) = (self.x0 + c * self.cw, FY0 + r * self.ch);
            for k in 0..(self.cw * self.ch) {
                let i = ((sx + k) as usize + r as usize * 7) % BOOM_TXT.len();
                let die = 0.95 + (r as f32 / fh) * 0.9 + self.rng.f() * 0.3;
                self.bits.push(Bit { x: sx + k % self.cw, y: sy + k / self.cw, ch: BOOM_TXT[i] as char, die });
            }
        }
        for row in self.rows.iter_mut() {
            row.c.iter_mut().for_each(|v| *v = 0);
        }
        self.slab.clear();
        self.recompute();
        self.n += self.slab_n.max(0) as u32;
        // bug 们从溶解的彩虹字里飞出来，回鸭子身边
        let nf = if app.hi { 7 } else { 3 };
        for i in 0..nf {
            if cells.is_empty() {
                break;
            }
            let (c, r) = cells[self.rng.int(cells.len())];
            self.flies.push(Fly {
                x: (self.x0 + c * self.cw) as f32,
                y: ((FY0 + r * self.ch) * 2) as f32,
                vx: self.rng.range(-20.0, 20.0),
                vy: self.rng.range(-40.0, -15.0),
                shade: [0u8, 2, 3, 4][self.rng.int(4)],
                ph: self.rng.range(0.0, 6.0),
                born: 1.0 + i as f32 * 0.12,
            });
        }
        app.push(log_line(
            "INFO",
            "duck::gc",
            &format!("一只鸭子整理了全部 bug：{} 行，0 warning，空洞也顺手填了", self.slab_n),
            Kind::Fixed,
        ));
        self.say(app, "嘎（一行不剩）", 1.4);
    }
}

impl Interlude for Blocks {
    fn name(&self) -> &'static str {
        "整理 Bug"
    }

    fn update(&mut self, app: &mut App, dt: f32) -> bool {
        self.t += dt;
        self.mt += dt;
        let t = self.t;

        // 闪完的行：飞回日志流
        while let Some(i) = self.clears.iter().position(|c| t - c.t0 >= FLASH) {
            let cl = self.clears.swap_remove(i);
            if let Some(idx) = self.rows.iter().position(|r| r.id == cl.id) {
                self.rows.remove(idx);
                let id = self.next_id;
                self.next_id += 1;
                self.rows.insert(0, Row { c: vec![0; self.fw as usize], id });
                self.lines.push(FlyLine { y0: (FY0 + idx as i32 * self.ch) as f32, t0: t, text: cl.text });
                self.n += 1;
                self.recompute();
                let falls = std::mem::take(&mut self.falls);
                let mut falls2 = Vec::with_capacity(falls.len());
                for mut f in falls {
                    while !self.fits(f.lane, &f.rots[f.r], f.x, f.iy) && f.iy > -20 {
                        f.iy -= 1;
                        f.y = f.iy as f32;
                    }
                    falls2.push(f);
                }
                self.falls = falls2;
                if !self.first_clear {
                    self.first_clear = true;
                    self.say(app, "嘎！消了一行", 1.0);
                }
            }
        }
        self.lines.retain(|l| t - l.t0 < 1.6);

        match self.ph {
            Ph::Play | Ph::Drain => {
                if self.ph == Ph::Play && t >= T_STOP {
                    self.ph = Ph::Drain;
                    self.mt = 0.0;
                    self.say(app, "停！都别动", 1.0);
                }
                // 出新块
                if self.ph == Ph::Play {
                    let lim = (self.fh / 4).max(3);
                    for li in 0..self.lanes.len() {
                        if self.falls.iter().any(|f| f.lane == li) {
                            continue;
                        }
                        self.lanes[li].cd -= dt;
                        let (l0, l1) = (self.lanes[li].x0, self.lanes[li].x1);
                        let low = (l0..l1).map(|c| self.top[c as usize]).min().unwrap_or(0);
                        if self.lanes[li].cd <= 0.0 && low > lim {
                            if let Some(f) = self.plan(li) {
                                self.falls.push(f);
                            } else {
                                self.lanes[li].cd = 0.5;
                            }
                        }
                    }
                }
                // 落
                let mv = if app.hi { 0.035 } else { 0.05 };
                let falls = std::mem::take(&mut self.falls);
                let mut keep = Vec::with_capacity(falls.len());
                for mut f in falls {
                    f.acc += dt;
                    while f.acc >= mv {
                        f.acc -= mv;
                        if f.r != f.tr {
                            let nr = (f.r + 1) % f.rots.len();
                            if self.fits(f.lane, &f.rots[nr], f.x, f.iy) {
                                f.r = nr;
                            }
                        } else if f.x != f.tx {
                            let nx = f.x + (f.tx - f.x).signum();
                            if self.fits(f.lane, &f.rots[f.r], nx, f.iy) {
                                f.x = nx;
                            }
                        }
                        if f.r == f.tr && f.x == f.tx {
                            f.drop = true;
                        }
                    }
                    f.y += dt * if f.drop { 90.0 } else { 20.0 };
                    let mut landed = false;
                    while f.y.floor() as i32 > f.iy {
                        if self.fits(f.lane, &f.rots[f.r], f.x, f.iy + 1) {
                            f.iy += 1;
                        } else {
                            landed = true;
                            break;
                        }
                    }
                    if landed {
                        self.lock(&f);
                    } else {
                        keep.push(f);
                    }
                }
                self.falls = keep;
                // 鸭子指挥
                for (k, at) in [3.4f32, 7.2, 11.0].iter().enumerate() {
                    if self.chat_i == k && t >= *at && self.ph == Ph::Play {
                        let s = self.chat[k % self.chat.len()];
                        self.say(app, s, 1.3);
                        self.chat_i += 1;
                    }
                }
                if self.ph == Ph::Drain && ((self.falls.is_empty() && self.clears.is_empty()) || self.mt > 2.2) {
                    self.falls.clear();
                    self.clears.clear();
                    self.build_slab();
                    self.ph = Ph::Duck;
                    self.mt = 0.0;
                    app.push(log_line("WARN", "bug::sched", "检测到超大块：形状像一只鸭子，形状表里没有这个", Kind::Info));
                    self.say(app, "看我的", 1.0);
                }
            }
            Ph::Duck => {
                if self.mt >= D_WAIT + D_FALL {
                    self.boom(app);
                    self.ph = Ph::Boom;
                    self.mt = 0.0;
                }
            }
            Ph::Boom => {
                if self.mt >= B_END {
                    self.ph = Ph::Out;
                    self.mt = 0.0;
                    app.push(log_line("INFO", "bug::home", "bug 们已被领回鸭子身边（鸭子说这是 feature）", Kind::Info));
                    self.say(app, "收工，回家", 1.0);
                }
            }
            Ph::Out => {}
        }

        // 飞回鸭子身边的 bug
        let (dx, dy) = (app.duck.x, app.duck.y);
        let ft = if self.ph == Ph::Out { B_END + self.mt } else { self.mt };
        for (i, f) in self.flies.iter_mut().enumerate() {
            if ft < f.born {
                continue;
            }
            let a = i as f32 * 0.9 + t * 1.8;
            let (tx, ty) = (dx + a.cos() * 22.0, dy + a.sin() * 12.0);
            f.vx += ((tx - f.x) * 4.0 - f.vx * 1.8) * dt;
            f.vy += ((ty - f.y) * 4.0 - f.vy * 1.8) * dt;
            f.x += f.vx * dt;
            f.y += f.vy * dt;
            f.ph += dt * 14.0;
        }

        let title = match self.ph {
            Ph::Boom | Ph::Out => format!("鸭子.exe — 整理 bug（一只鸭子整理了全部 {} 行）", self.n),
            _ => format!("鸭子.exe — 整理 bug（已整理日志 {} 行）", self.n),
        };
        app.set_title(&title);

        if self.ph == Ph::Out && self.mt >= O_END {
            app.moths.clear();
            for f in self.flies.iter().take(10) {
                app.spawn_moth(f.x, f.y, f.vx * 0.5, f.vy * 0.5);
                if let Some(m) = app.moths.last_mut() {
                    m.shade = f.shade;
                }
            }
            return true;
        }
        false
    }

    fn draw(&mut self, app: &mut App) {
        let t = self.t;
        app.draw_logs(9.0, 0, 0.0);
        let din = smooth(0.0, 0.9, t) * 1.15;
        let dout = if self.ph == Ph::Out { smooth(0.0, O_END * 0.85, self.mt) * 1.15 } else { 0.0 };
        if dout < 1.1 {
            let tab = std::mem::take(&mut self.dimtab);
            dim_logs(app, &tab, dout, din);
            self.dimtab = tab;
        }
        let (x0, fw, fh) = (self.x0, self.fw, self.fh);

        // bug 堆（开场从下往上长出来）
        let rv = smooth(0.25, 1.3, t) * (self.jh + 1) as f32;
        for r in 0..fh {
            if t < 1.3 && ((fh - r) as f32) > rv {
                continue;
            }
            let sy = FY0 + r * self.ch;
            let row = &self.rows[r as usize];
            if let Some(cl) = self.clears.iter().find(|c| c.id == row.id) {
                if t - cl.t0 < 0.1 {
                    for y in sy..sy + self.ch {
                        for x in x0..x0 + fw * self.cw {
                            app.scr.put(x, y, ' ', WHITE, Some(WHITE));
                        }
                    }
                } else {
                    let text = cl.text.clone();
                    let e = rtext(app, x0, sy, &text, x0 + fw * self.cw);
                    for y in sy..sy + self.ch {
                        let x0b = if y == sy { e } else { x0 };
                        for x in x0b..x0 + fw * self.cw {
                            app.scr.put(x, y, ' ', WHITE, Some(BLACK));
                        }
                    }
                }
                continue;
            }
            for c in 0..fw {
                let v = row.c[c as usize];
                if v != 0 {
                    put_cell(app, x0 + c * self.cw, sy, v, self.cw, self.ch);
                }
            }
        }
        // 落点预览：告诉观众这块要落在哪
        for f in &self.falls {
            for &(cx, cy) in f.rots[f.r].iter() {
                let r = f.ly + cy;
                if r < 0 || r >= self.fh {
                    continue;
                }
                let sx = x0 + (f.tx + cx) * self.cw;
                let sy = FY0 + r * self.ch;
                for dy in 0..self.ch {
                    for dx in 0..self.cw {
                        app.scr.put(sx + dx, sy + dy, ' ', WHITE, Some([38, 38, 50]));
                    }
                }
            }
        }
        // 正在落的块
        for f in &self.falls {
            for (k, &(cx, cy)) in f.rots[f.r].iter().enumerate() {
                let r = f.iy + cy;
                if r >= 0 {
                    put_cell(app, x0 + (f.x + cx) * self.cw, FY0 + r * self.ch, 1 + f.shade * 4 + f.glyph[k], self.cw, self.ch);
                }
            }
        }
        // 鸭子块
        if self.ph == Ph::Duck {
            let u = ((self.mt - D_WAIT) / D_FALL).clamp(0.0, 1.0);
            let off = (-(fh as f32 + 1.0) * (1.0 - u * u)).round() as i32;
            for &(c, r, kind) in &self.slab {
                let row = r + off;
                if row < 0 {
                    continue;
                }
                let (sx, sy) = (x0 + c * self.cw, FY0 + row * self.ch);
                for dy in 0..self.ch {
                    for dx in 0..self.cw {
                        let bg = match kind {
                            0 => rb(Band::Duck, (sx + dx) as f32, (sy + dy) as f32),
                            1 => Col::T([16, 14, 24]),
                            2 => Col::T(ORANGE),
                            _ => rb(Band::Duck, (sx + dx) as f32, (sy + dy) as f32),
                        };
                        app.scr.put_c(sx + dx, sy + dy, ' ', Col::T(BLACK), Some(bg));
                    }
                }
            }
        }
        // 满屏彩虹字，从上往下溶解回日志
        if self.ph == Ph::Boom {
            let a = self.mt;
            for i in 0..self.bits.len() {
                let b = &self.bits[i];
                if a >= b.die {
                    continue;
                }
                let (x, y, ch) = (b.x, b.y, b.ch);
                if a < 0.14 {
                    app.scr.put(x, y, ' ', WHITE, Some(WHITE));
                } else {
                    app.scr.put_c(x, y, ch, rb(Band::Text, x as f32, y as f32), Some(Col::T(BLACK)));
                }
            }
        }

        // 指挥鸭子
        if self.small {
            let (x, y) = (app.duck.x, app.duck.y);
            mini_duck(app, x, y, false, 1);
        } else {
            app.draw_glow(app.duck.glow);
            app.draw_duck(None);
        }
        let ft = if self.ph == Ph::Out { B_END + self.mt } else if self.ph == Ph::Boom { self.mt } else { -1.0 };
        for i in 0..self.flies.len() {
            let f = &self.flies[i];
            if ft < f.born {
                continue;
            }
            let (x, y, s, p) = (f.x, f.y, f.shade, f.ph);
            moth(app, x, y, s, p, 0.0);
        }
        let _ = self.duck_at;
    }

    fn draw_over(&mut self, app: &mut App) {
        let t = self.t;
        let (x0, fw) = (self.x0, self.fw);
        // 消掉的行变成彩虹日志往上飞
        for i in 0..self.lines.len() {
            let a = t - self.lines[i].t0;
            let y = (self.lines[i].y0 - (6.0 * a + 42.0 * a * a)).round() as i32;
            if y < FY0 {
                continue;
            }
            let text = self.lines[i].text.clone();
            rtext(app, x0, y, &text, x0 + fw * self.cw);
        }
        // 顶上的小标签
        if self.ph != Ph::Out {
            let s = if self.small {
                if self.ph == Ph::Boom {
                    format!(" 鸭子.exe · 一口气 {} 行", self.slab_n)
                } else {
                    format!(" 鸭子.exe · 整理 {} 行", self.n)
                }
            } else if self.ph == Ph::Boom {
                format!(" 鸭子.exe — 整理 bug · 一口气整理了 {} 行", self.slab_n)
            } else {
                format!(" 鸭子.exe — 整理 bug · 已整理日志 {} 行", self.n)
            };
            let x = (x0 + (fw * self.cw - str_w(&s) as i32) / 2).max(0);
            app.scr.text(x, 0, &s, [235, 235, 240], Some([28, 26, 40]));
        }
        if self.small {
            if let Some((text, until)) = &self.say {
                if t < *until {
                    let w = app.scr.w as i32;
                    let tw = (str_w(text) as i32 + 3).min((w - 2).max(3));
                    let x = (w - tw - 1).max(1);
                    let y = 1;
                    for i in 0..tw {
                        app.scr.put(x + i, y, ' ', BLACK, Some([250, 250, 245]));
                    }
                    app.scr.text(x + 1, y, text, [20, 20, 30], Some([250, 250, 245]));
                }
            }
        }
    }
}

fn put_cell(app: &mut App, x: i32, y: i32, v: u8, cw: i32, ch: i32) {
    let k = (v - 1) as usize;
    let (bg, fg) = SHADES[(k / 4).min(2)];
    let (a, b) = GLYPH[k % 4];
    if cw == 2 && ch == 1 {
        app.scr.put(x, y, a, fg, Some(bg));
        app.scr.put(x + 1, y, b, fg, Some(bg));
        return;
    }
    let face = mix(bg, WHITE, 0.10);
    let low = scale(bg, 0.84);
    for dy in 0..ch {
        let c = if dy == 0 { face } else { low };
        for dx in 0..cw {
            app.scr.put(x + dx, y + dy, ' ', BLACK, Some(c));
        }
    }
    let gx = x + (cw - 2) / 2;
    let gy = y + ch / 2;
    let gf = scale(bg, 0.55);
    app.scr.put(gx, gy, a, gf, None);
    app.scr.put(gx + 1, gy, b, gf, None);
}
