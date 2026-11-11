//! 插曲：扫 Bug —— 只借「翻格子、数字提示周围藏着几只 bug」的玩法。
//! 日志上弹出一个终端窗口，鸭鸭当光标一跳一跳地真推理（单格约束 + 子集推理 + 全局计数），
//! 推到最后只剩五五开，猜错，踩中的不是雷而是一朵花；藏着的 bug 全变成真飞蛾飞出来围着鸭鸭：
//! 扫 bug 失败 → 交友成功。
//!
//! 整局在 new() 里一次算完（棋盘、动作序列、每格翻开/插标记的时刻），画面是时间的纯函数：
//! 棋盘静态，只有变化的格子会写到终端。
use super::*;

const COVER_A: Rgb = [52, 55, 80];
const COVER_B: Rgb = [43, 45, 66];
const OPEN_BG: Rgb = [15, 15, 24];
const WIN_BG: Rgb = [11, 11, 18];
const WIN_FG: Rgb = [140, 142, 178];
const FLASH: Rgb = [92, 94, 146];
const EXPOSED: Rgb = [74, 58, 96];
const NEVER: f32 = f32::INFINITY;
const BUG_CH: char = 'ж';

// ───────────────────────── 棋盘与推理 ─────────────────────────

enum Act {
    Flag(usize),
    /// 鸭鸭站在 at 上啄一下，翻开 cells（含洪水填充的距离，用来做涟漪）
    Peck(usize, Vec<(usize, u16)>),
}

struct Board {
    w: usize,
    h: usize,
    nb: usize,
    bug: Vec<bool>,
    num: Vec<u8>,
    st: Vec<u8>, // 0 盖着，1 翻开，2 插了标记
}

impl Board {
    fn new(w: usize, h: usize, nb: usize, first: usize, rng: &mut Rng) -> Self {
        let n = w * h;
        let (fx, fy) = ((first % w) as i32, (first / w) as i32);
        let mut cand: Vec<usize> =
            (0..n).filter(|&c| ((c % w) as i32 - fx).abs() > 1 || ((c / w) as i32 - fy).abs() > 1).collect();
        let mut bug = vec![false; n];
        for _ in 0..nb.min(cand.len()) {
            let k = rng.int(cand.len());
            bug[cand.swap_remove(k)] = true;
        }
        let mut b = Board { w, h, nb: nb.min(n), bug, num: vec![0; n], st: vec![0; n] };
        for c in 0..n {
            let (nbr, k) = b.nbrs(c);
            b.num[c] = nbr[..k].iter().filter(|&&d| b.bug[d]).count() as u8;
        }
        b
    }
    fn nbrs(&self, c: usize) -> ([usize; 8], usize) {
        let (x, y) = ((c % self.w) as i32, (c / self.w) as i32);
        let mut out = [0usize; 8];
        let mut k = 0;
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (nx, ny) = (x + dx, y + dy);
                if (dx != 0 || dy != 0) && nx >= 0 && ny >= 0 && (nx as usize) < self.w && (ny as usize) < self.h {
                    out[k] = ny as usize * self.w + nx as usize;
                    k += 1;
                }
            }
        }
        (out, k)
    }
    /// 翻开一格（0 就洪水填充），返回新翻开的格子和离起点的步数
    fn reveal(&mut self, c: usize) -> Vec<(usize, u16)> {
        let mut out = Vec::new();
        if self.st[c] != 0 || self.bug[c] {
            return out;
        }
        self.st[c] = 1;
        out.push((c, 0));
        let mut i = 0;
        while i < out.len() {
            let (c, d) = out[i];
            i += 1;
            if self.num[c] != 0 {
                continue;
            }
            let (nbr, k) = self.nbrs(c);
            for &e in &nbr[..k] {
                if self.st[e] == 0 && !self.bug[e] {
                    self.st[e] = 1;
                    out.push((e, d + 1));
                }
            }
        }
        out
    }
    /// 一轮推理：(可以连开的数字格及其周围, 单独可开的格子, 确定是 bug 的格子)
    fn deduce(&self) -> (Vec<(usize, Vec<usize>)>, Vec<usize>, Vec<usize>) {
        let n = self.w * self.h;
        let mut fidx = vec![usize::MAX; n];
        let mut fr: Vec<(usize, Vec<usize>, i32)> = Vec::new();
        for c in 0..n {
            if self.st[c] != 1 || self.num[c] == 0 {
                continue;
            }
            let (nbr, k) = self.nbrs(c);
            let mut u = Vec::new();
            let mut f = 0;
            for &d in &nbr[..k] {
                match self.st[d] {
                    0 => u.push(d),
                    2 => f += 1,
                    _ => {}
                }
            }
            if !u.is_empty() {
                fidx[c] = fr.len();
                fr.push((c, u, self.num[c] as i32 - f));
            }
        }
        let mut safe = vec![false; n];
        let mut bad = vec![false; n];
        let mut grouped = vec![false; n];
        let mut groups = Vec::new();
        // 单格约束
        for (c, u, r) in &fr {
            if *r == 0 {
                for &d in u {
                    safe[d] = true;
                    grouped[d] = true;
                }
                groups.push((*c, u.clone()));
            } else if *r == u.len() as i32 {
                for &d in u {
                    bad[d] = true;
                }
            }
        }
        // 子集推理：A 的未知格是 B 的子集，差集里的 bug 数 = rB - rA
        for (a, ua, ra) in &fr {
            let (ax, ay) = ((a % self.w) as i32, (a / self.w) as i32);
            for dy in -2..=2 {
                for dx in -2..=2 {
                    let (bx, by) = (ax + dx, ay + dy);
                    if (dx == 0 && dy == 0) || bx < 0 || by < 0 || bx as usize >= self.w || by as usize >= self.h {
                        continue;
                    }
                    let bi = fidx[by as usize * self.w + bx as usize];
                    if bi == usize::MAX {
                        continue;
                    }
                    let (_, ub, rbv) = &fr[bi];
                    if ua.len() >= ub.len() || !ua.iter().all(|x| ub.contains(x)) {
                        continue;
                    }
                    let d: Vec<usize> = ub.iter().copied().filter(|x| !ua.contains(x)).collect();
                    let rr = rbv - ra;
                    if rr == 0 {
                        for &x in &d {
                            safe[x] = true;
                        }
                    } else if rr == d.len() as i32 {
                        for &x in &d {
                            bad[x] = true;
                        }
                    }
                }
            }
        }
        // 全局计数（只在别的都推不动时用）
        if !safe.iter().any(|&s| s) && !bad.iter().any(|&b| b) {
            let flagged = self.st.iter().filter(|&&s| s == 2).count();
            let cov: Vec<usize> = (0..n).filter(|&c| self.st[c] == 0).collect();
            let rem = self.nb.saturating_sub(flagged);
            if !cov.is_empty() {
                if rem == 0 {
                    for &c in &cov {
                        safe[c] = true;
                    }
                } else if rem == cov.len() {
                    for &c in &cov {
                        bad[c] = true;
                    }
                }
            }
        }
        let singles = (0..n).filter(|&c| safe[c] && !grouped[c] && !bad[c]).collect();
        let bugs = (0..n).filter(|&c| bad[c] && !safe[c]).collect();
        (groups, singles, bugs)
    }
    fn dist2(&self, a: usize, b: usize) -> i32 {
        let (dx, dy) = ((a % self.w) as i32 - (b % self.w) as i32, (a / self.w) as i32 - (b / self.w) as i32);
        dx * dx + dy * dy
    }
    /// 从 first 开局一直推到推不动。返回 (首点翻开的格子, 动作序列)；剩下 st==0 的就是只能猜的格子
    fn solve(&mut self, first: usize) -> (Vec<(usize, u16)>, Vec<Act>) {
        let opened = self.reveal(first);
        let mut acts = Vec::new();
        let mut cur = first;
        loop {
            let (groups, singles, bugs) = self.deduce();
            if groups.is_empty() && singles.is_empty() && bugs.is_empty() {
                break;
            }
            let mut cands: Vec<(usize, Option<Vec<usize>>)> = Vec::new();
            cands.extend(bugs.into_iter().map(|c| (c, None)));
            cands.extend(groups.into_iter().map(|(a, u)| (a, Some(u))));
            cands.extend(singles.into_iter().map(|c| (c, Some(vec![c]))));
            // 像高手一样就近处理：每次挑离鸭鸭最近的一步
            while !cands.is_empty() {
                let k = (0..cands.len()).min_by_key(|&i| self.dist2(cur, cands[i].0)).unwrap();
                let (at, what) = cands.swap_remove(k);
                match what {
                    None => {
                        if self.st[at] == 0 {
                            self.st[at] = 2;
                            acts.push(Act::Flag(at));
                            cur = at;
                        }
                    }
                    Some(cells) => {
                        let mut got = Vec::new();
                        for c in cells {
                            got.extend(self.reveal(c));
                        }
                        if !got.is_empty() {
                            acts.push(Act::Peck(at, got));
                            cur = at;
                        }
                    }
                }
            }
        }
        (opened, acts)
    }
}

// ───────────────────────── 场景 ─────────────────────────

/// 鸭鸭的路径点（格子坐标；j 可以是负数＝在窗口上方）
struct Way {
    t: f32,
    i: f32,
    j: f32,
    hop: f32,
    peck: bool,
    fall: bool,
    right: bool,
}

struct Fly {
    cell: usize,
    launch: f32,
    on: bool,
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    ph: f32,
    om: f32,
    rx: f32,
    shade: u8,
}

pub struct Sweeper {
    t: f32,
    bw: i32,
    bh: i32,
    s: i32,
    nb: usize,
    bug: Vec<bool>,
    num: Vec<u8>,
    open_t: Vec<f32>,
    flag_t: Vec<f32>,
    moth_t: Vec<f32>,
    guess: usize,
    ways: Vec<Way>,
    t_click: f32,
    stuck_t: f32,
    peck_t: f32,
    close_t: f32,
    end_t: f32,
    under: Option<Vec<Cell>>,
    flies: Vec<Fly>,
    told: usize,
    duck: (f32, f32, bool),
}

const T_POP: f32 = 0.36;
const T_LAND: f32 = 1.15;
const T_CLICK: f32 = 1.3;
const COLLAPSE: f32 = 0.45;

impl Sweeper {
    pub fn new(app: &mut App) -> Self {
        let (w, h) = (app.scr.w as i32, app.scr.h as i32);
        let s = if w >= 24 * 4 + 8 && h >= 12 * 2 + 9 { 2 } else { 1 };
        let bw = ((w - 6) / (2 * s)).clamp(6, 24);
        let bh = ((h - 6) / s).clamp(4, 12);
        let n = (bw * bh) as usize;
        let nb = ((n as f32 * 0.135).round() as usize).max(3);
        let (bwu, bhu) = (bw as usize, bh as usize);

        // 生成多盘挑一盘：首点安全、大部分能推理、最后卡在一小撮五五开上
        let mut best: Option<(i32, Board, usize, Vec<(usize, u16)>, Vec<Act>)> = None;
        for _ in 0..70 {
            let fx = bwu / 2 + app.rng.int(bwu / 3 + 1) - bwu / 6;
            let fy = bhu / 2 + app.rng.int(bhu / 3 + 1) - bhu / 6;
            let first = fy.min(bhu - 1) * bwu + fx.min(bwu - 1);
            let mut b = Board::new(bwu, bhu, nb, first, &mut app.rng);
            let (opened, acts) = b.solve(first);
            let stuck = b.st.iter().filter(|&&x| x == 0).count() as i32;
            let na = acts.len() as i32;
            let mut score = -(na - 65).abs() - 4 * (stuck - 3).abs() + (opened.len() as i32).min(40) / 4;
            if stuck < 2 {
                score -= 1000;
            }
            if stuck > 10 {
                score -= 200;
            }
            if best.as_ref().map_or(true, |bb| score > bb.0) {
                best = Some((score, b, first, opened, acts));
            }
            if score > -10 {
                break;
            }
        }
        let (_, mut b, first, opened, acts) = best.unwrap();

        // ── 排时间表 ──
        let mut open_t = vec![NEVER; n];
        let mut flag_t = vec![NEVER; n];
        let mut far = 0u16;
        for &(c, d) in &opened {
            open_t[c] = T_CLICK + d as f32 * 0.035;
            far = far.max(d);
        }
        let play_start = T_CLICK + far as f32 * 0.035 + 0.35;
        let na = acts.len().max(1);
        let play_len = (na as f32 * 0.14).clamp(4.0, 10.5);
        let pos = |c: usize| ((c % bwu) as f32, (c / bwu) as f32);
        let (fi, fj) = pos(first);
        let mut ways = vec![
            Way { t: 0.55, i: fi, j: -9.0, hop: 0.0, peck: false, fall: false, right: true },
            Way { t: T_LAND, i: fi, j: fj, hop: T_LAND - 0.55, peck: false, fall: true, right: true },
            Way { t: T_CLICK, i: fi, j: fj, hop: 0.0, peck: true, fall: false, right: true },
        ];
        let mut prev = T_CLICK;
        for (k, a) in acts.iter().enumerate() {
            let ta = play_start + play_len * ((k + 1) as f32 / na as f32).powf(0.8);
            let at = match a {
                Act::Flag(c) => {
                    flag_t[*c] = ta;
                    *c
                }
                Act::Peck(at, cells) => {
                    for &(c, d) in cells {
                        open_t[c] = open_t[c].min(ta + d as f32 * 0.03);
                    }
                    *at
                }
            };
            let (i, j) = pos(at);
            ways.push(Way { t: ta, i, j, hop: (0.8 * (ta - prev)).min(0.17), peck: true, fall: false, right: true });
            prev = ta;
        }
        let stuck_t = prev + 0.5;

        // ── 结尾：五五开里挑一个 bug 去踩，另一个安全格当犹豫对象 ──
        let stuck: Vec<usize> = (0..n).filter(|&c| b.st[c] == 0).collect();
        let last = ways.last().map(|w| (w.j as usize * bwu + w.i as usize).min(n - 1)).unwrap_or(first);
        let guess = stuck
            .iter()
            .copied()
            .filter(|&c| b.bug[c])
            .min_by_key(|&c| b.dist2(c, last))
            .or_else(|| (0..n).find(|&c| b.bug[c] && b.st[c] == 0))
            .or_else(|| (0..n).find(|&c| b.bug[c]))
            .unwrap_or(first);
        let alt = stuck.iter().copied().filter(|&c| c != guess && !b.bug[c]).min_by_key(|&c| b.dist2(c, guess));
        let alt = alt.or_else(|| stuck.iter().copied().find(|&c| c != guess)).unwrap_or(guess);
        let (gi, gj) = pos(guess);
        let (ai, aj) = pos(alt);
        let peck_t = stuck_t + 2.2;
        for (k, (i, j)) in [(ai, aj), (gi, gj), (ai, aj), (gi, gj)].into_iter().enumerate() {
            let t = stuck_t + 0.45 + k as f32 * 0.5;
            ways.push(Way { t, i, j, hop: 0.28, peck: false, fall: false, right: true });
        }
        ways.push(Way { t: peck_t, i: gi, j: gj, hop: 0.0, peck: true, fall: false, right: true });
        // 吓一跳，跳到花旁边让位（和 flower_geo 同一套夹取：花 3.5 格宽、6 格高）
        let (bwf, bhf) = (bw as f32, bh as f32);
        let fi = gi.clamp(1.25, (bwf - 2.25).max(1.25));
        let base = gj.max(5.0).min(bhf - 1.0);
        let off = ((8.5 * s as f32 + 3.0) / (2 * s) as f32).ceil();
        let side = if fi + off <= bwf - 1.0 || fi - off < 0.0 { fi + off } else { fi - off };
        let sj = (base + 1.0).min(bhf - 1.0);
        ways.push(Way { t: peck_t + 0.55, i: side.clamp(0.0, bwf - 1.0), j: sj, hop: 0.32, peck: false, fall: false, right: true });
        // 朝向：往右跳就朝右，原地就保持
        let mut r = true;
        for k in 0..ways.len() {
            if k > 0 {
                let d = ways[k].i - ways[k - 1].i;
                if d > 0.01 {
                    r = true;
                } else if d < -0.01 {
                    r = false;
                }
            }
            ways[k].right = r;
        }
        let side_k = ways.len() - 1;
        ways[side_k].right = gi > ways[side_k].i; // 跳开后回头看花

        // 所有 bug 变飞蛾：先露出来，再按离踩中点的远近依次飞出
        let mut moth_t = vec![NEVER; n];
        let mut flies = Vec::new();
        for c in 0..n {
            if b.bug[c] && c != guess {
                let d = (b.dist2(c, guess) as f32).sqrt();
                moth_t[c] = peck_t + 1.35 + d * 0.035 + app.rng.range(0.0, 0.15);
            }
        }
        moth_t[guess] = peck_t + 1.1;
        let mut order: Vec<usize> = (0..n).filter(|&c| b.bug[c]).collect();
        order.sort_by(|&a, &b2| moth_t[a].partial_cmp(&moth_t[b2]).unwrap());
        let nfly = order.len();
        let kk = 0.6 + 0.4 * s as f32;
        for (idx, c) in order.into_iter().enumerate() {
            // 三圈同向旋转的椭圆轨道，同一圈里均匀分布：看得出是「围成一圈」
            let ring = idx % 3;
            let per = ((nfly + 2 - ring) / 3).max(1);
            flies.push(Fly {
                cell: c,
                launch: moth_t[c],
                on: false,
                x: 0.0,
                y: 0.0,
                vx: 0.0,
                vy: 0.0,
                ph: (idx / 3) as f32 / per as f32 * TAU + ring as f32 * 0.7 + app.rng.range(-0.15, 0.15),
                om: [2.1, 1.7, 1.4][ring],
                rx: (18.0 + ring as f32 * 9.0) * kk,
                shade: app.rng.int(5) as u8,
            });
        }
        let close_t = peck_t + 4.2;
        b.st.clear();

        app.info("launcher", "启动 扫bug.exe —— 鸭鸭：就玩一局");
        app.set_title("鸭鸭.exe — 扫 Bug");
        app.duck.bubble = None;
        Sweeper {
            t: 0.0,
            bw,
            bh,
            s,
            nb: b.nb,
            bug: b.bug,
            num: b.num,
            open_t,
            flag_t,
            moth_t,
            guess,
            ways,
            t_click: T_CLICK,
            stuck_t,
            peck_t,
            close_t,
            end_t: close_t + COLLAPSE + 1.2,
            under: None,
            flies,
            told: 0,
            duck: (0.0, 0.0, true),
        }
    }

    fn cw(&self) -> i32 {
        2 * self.s
    }
    /// 窗口外框 (x0, y0, x1, y1) 和棋盘左上角 (bx, by)，每帧按当前屏幕居中
    fn lay(&self, app: &App) -> (i32, i32, i32, i32, i32, i32) {
        let (w, h) = (app.scr.w as i32, app.scr.h as i32);
        let gap = if h >= self.bh * self.s + 6 { 1 } else { 0 };
        let ww = self.bw * self.cw() + 4;
        let wh = self.bh * self.s + 3 + gap;
        let (x0, y0) = ((w - ww) / 2, (h - wh) / 2);
        (x0, y0, x0 + ww - 1, y0 + wh - 1, x0 + 2, y0 + 2 + gap)
    }
    /// 鸭鸭站在格子 (i, j) 上方时的像素中心
    fn duck_px(&self, app: &App, i: f32, j: f32) -> (f32, f32) {
        let (_, y0, .., bx, by) = self.lay(app);
        let k = self.s;
        let x = bx as f32 + i * self.cw() as f32 + self.cw() as f32 / 2.0;
        let top = by as f32 + j * self.s as f32; // 格子顶行（字符）
        let half = ((7 * k) / 2) as f32;
        let above = top * 2.0 - 1.0 - (7 * k) as f32 + half;
        // 站在格子上方会挡住标题栏时，改站到格子下方（抬头看它）
        if j >= 0.0 && (above - half - 1.0) / 2.0 < (y0 + 1) as f32 {
            return (x, (top + self.s as f32) * 2.0 + 1.0 + half);
        }
        (x, above)
    }
    /// 鸭鸭此刻的位置与朝向；None 表示还没出场
    fn duck_at(&self, app: &App, t: f32) -> Option<(f32, f32, bool)> {
        let ws = &self.ways;
        if t < ws[0].t {
            return None;
        }
        let idx = ws.iter().rposition(|w| w.t <= t).unwrap_or(0);
        let cur = &ws[idx];
        let (mut x, mut y) = self.duck_px(app, cur.i, cur.j);
        let mut right = cur.right;
        if let Some(nx) = ws.get(idx + 1) {
            if nx.hop > 0.0 && t > nx.t - nx.hop {
                let u = ((t - (nx.t - nx.hop)) / nx.hop).clamp(0.0, 1.0);
                let (x2, y2) = self.duck_px(app, nx.i, nx.j);
                let k = self.s as f32;
                if nx.fall {
                    let e = u * u;
                    x += (x2 - x) * e;
                    y += (y2 - y) * e;
                } else {
                    let dist = ((x2 - x).powi(2) + (y2 - y).powi(2)).sqrt();
                    let arc = (3.0 * k + dist * 0.22).min(9.0 * k);
                    x += (x2 - x) * u;
                    y += (y2 - y) * u - arc * 4.0 * u * (1.0 - u);
                }
                right = nx.right;
            }
        }
        if cur.peck && t - cur.t < 0.07 {
            y += 1.0;
        }
        Some((x, y, right))
    }

    fn draw_cell(&self, app: &mut App, i: i32, j: i32, bx: i32, by: i32) {
        let t = self.t;
        let c = (j * self.bw + i) as usize;
        let (cw, s) = (self.cw(), self.s);
        let (x0, y0) = (bx + i * cw, by + j * s);
        let cover = if (i + j) % 2 == 0 { COVER_A } else { COVER_B };
        let (gx, gy) = (x0 + cw / 2 - 1, y0 + (s - 1) / 2);
        let block = |app: &mut App, bg: Rgb| {
            for y in 0..s {
                for x in 0..cw {
                    app.scr.put(x0 + x, y0 + y, ' ', bg, Some(bg));
                }
            }
        };
        if self.bug[c] {
            if t >= self.moth_t[c] || (c == self.guess && t >= self.peck_t) {
                // bug 搬走了 / 被踩中的那格（上面长花）：空格子
                self.draw_open(app, c, x0, y0, OPEN_BG);
            } else if t >= self.moth_t[c] - 0.4 {
                block(app, EXPOSED);
                app.scr.put(gx, gy, BUG_CH, [235, 235, 240], Some(EXPOSED));
            } else if t >= self.flag_t[c] {
                let bg = if t - self.flag_t[c] < 0.12 { FLASH } else { cover };
                block(app, bg);
                app.scr.put(gx, gy, BUG_CH, [238, 238, 245], Some(bg));
            } else {
                block(app, cover);
            }
            return;
        }
        let ot = self.open_t[c];
        if t < ot {
            block(app, cover);
            return;
        }
        let bg = if t - ot < 0.1 { FLASH } else { OPEN_BG };
        if self.num[c] > 0 {
            block(app, bg);
            let d = (b'0' + self.num[c]) as char;
            app.scr.put_c(gx, gy, d, rb(Band::Text, gx as f32, gy as f32 * 1.5), Some(Col::T(bg)));
        } else {
            self.draw_open(app, c, x0, y0, bg);
        }
    }

    /// 翻开的空格：露出底下被窗口盖住的日志碎片（暗彩虹，调色板色，流动免费）
    fn draw_open(&self, app: &mut App, c: usize, x0: i32, y0: i32, bg: Rgb) {
        let (cw, s) = (self.cw(), self.s);
        let rw = self.bw * cw;
        let (i, j) = ((c % self.bw as usize) as i32, (c / self.bw as usize) as i32);
        for y in 0..s {
            let mut x = 0;
            while x < cw {
                let (ux, uy) = (i * cw + x, j * s + y);
                let cell = self.under.as_ref().and_then(|u| u.get((uy * rw + ux) as usize).copied());
                let mut ch = ' ';
                let mut fg = Col::T(bg);
                let mut wide = false;
                if let Some(cl) = cell {
                    match cl.w {
                        // 碎片里的数字换成点，免得和真正的提示数字混淆
                        0 => {
                            ch = if cl.ch.is_ascii_digit() { '·' } else { cl.ch };
                            fg = cl.fg;
                        }
                        1 if x + 1 < cw => {
                            ch = cl.ch;
                            fg = cl.fg;
                            wide = true;
                        }
                        _ => {}
                    }
                }
                app.scr.put_c(x0 + x, y0 + y, ch, fg, Some(Col::T(bg)));
                x += if wide { 2 } else { 1 };
            }
        }
    }

    /// 花的像素几何：(中心 x 像素, 底部 y 像素, 放大倍数)。和 new() 里鸭鸭让位的算法一致
    fn flower_geo(&self, app: &App) -> (i32, i32, i32) {
        let (.., bx, by) = self.lay(app);
        let k = self.s;
        let (gi, gj) = ((self.guess % self.bw as usize) as f32, (self.guess / self.bw as usize) as f32);
        let fi = gi.clamp(1.25, (self.bw as f32 - 2.25).max(1.25));
        let base = gj.max(5.0).min(self.bh as f32 - 1.0);
        let cx = bx + (fi * self.cw() as f32) as i32 + self.cw() / 2;
        let bot = (by + (base as i32 + 1) * self.s) * 2 - 1;
        (cx, bot, k)
    }

    /// 踩中的格子里长出一朵彩虹花（像素层，和鸭鸭、飞蛾同一画风）：茎 → 叶 → 花苞 → 盛开
    fn draw_flower(&self, app: &mut App) {
        let u = (self.t - self.peck_t - 0.1) / 0.8;
        if u < 0.0 {
            return;
        }
        const ART: [&str; 12] = [
            ".PP.PP.", "PPPPPPP", "PPYYYPP", ".PYYYP.", "PPYYYPP", "PPPPPPP", ".PP.PP.", "...G...", ".L.G...", "LL.G.LL",
            "...G.L.", "...G...",
        ];
        const BUD: [&str; 5] = ["..bbb..", ".bbbbb.", "..bbb..", "...G...", "...G..."];
        let (cx, bot, k) = self.flower_geo(app);
        let (w, h) = (7 * k + 2, 12 * k + 2);
        let (ox, oy) = (cx - (7 * k) / 2 - 1, bot - 12 * k);
        let mut buf: Vec<Option<Col>> = vec![None; (w * h) as usize];
        let show = |r: usize| -> Option<&'static str> {
            if u >= 0.8 {
                Some(ART[r])
            } else if u >= 0.55 && (2..7).contains(&r) {
                Some(BUD[r - 2])
            } else if (u >= 0.3 && r >= 7) || r >= 10 {
                Some(ART[r])
            } else {
                None
            }
        };
        for r in 0..12 {
            let Some(row) = show(r) else { continue };
            for (i, ch) in row.chars().enumerate() {
                let c: Col = match ch {
                    'P' => rb(Band::Duck, (ox + i as i32 * k) as f32 * 1.6, (oy + r as i32 * k) as f32 * 0.8),
                    'Y' => Col::T([255, 214, 70]),
                    'b' => Col::T([255, 150, 200]),
                    'G' => Col::T([84, 200, 112]),
                    'L' => Col::T([56, 165, 88]),
                    _ => continue,
                };
                for sy in 0..k {
                    for sx in 0..k {
                        let (px, py) = (1 + i as i32 * k + sx, 1 + r as i32 * k + sy);
                        buf[(py * w + px) as usize] = Some(c);
                    }
                }
            }
        }
        for y in 0..h {
            for x in 0..w {
                let c = buf[(y * w + x) as usize].or_else(|| {
                    let near = [(-1, 0), (1, 0), (0, -1), (0, 1)].iter().any(|(dx, dy)| {
                        let (nx, ny) = (x + dx, y + dy);
                        nx >= 0 && ny >= 0 && nx < w && ny < h && buf[(ny * w + nx) as usize].is_some()
                    });
                    near.then(|| Col::T([14, 12, 24]))
                });
                if let Some(c) = c {
                    app.layer.set(ox + x, oy + y, c);
                }
            }
        }
    }

    fn bubble(&self) -> Option<&'static str> {
        let t = self.t;
        if t > 0.9 && t < 1.9 {
            Some("就一局！")
        } else if t > self.stuck_t + 0.3 && t < self.peck_t - 0.05 {
            Some("五五开……")
        } else if t > self.peck_t + 0.12 && t < self.peck_t + 1.3 {
            Some("嘎？！")
        } else if t > self.peck_t + 2.1 && t < self.close_t + 0.2 {
            Some("……那交个朋友？")
        } else {
            None
        }
    }
}

impl Interlude for Sweeper {
    fn name(&self) -> &'static str {
        "扫 Bug"
    }

    fn update(&mut self, app: &mut App, dt: f32) -> bool {
        self.t += dt;
        let t = self.t;
        if let Some(d) = self.duck_at(app, t) {
            self.duck = d;
        }
        if self.told == 0 && t > self.peck_t + 1.5 {
            self.told = 1;
            app.set_title("鸭鸭.exe — 扫 bug 失败，交友成功");
        }
        if self.told == 1 && t > self.close_t + COLLAPSE {
            self.told = 2;
            app.push(log_line("WARN", "bug.exe", "踩中 bug ×1：未爆炸，开了一朵花", Kind::Info));
            app.push(log_line("INFO", "duck::social", &format!("好友 +{}（全是 bug）", self.nb), Kind::Info));
        }
        // 飞蛾：弹簧追着「绕鸭鸭转的椭圆轨道」，一出巢就有往上的冲劲
        let (dx, dy, _) = self.duck;
        let (.., bx, by) = self.lay(app);
        let cap = if app.hi { usize::MAX } else { 14 };
        let dt = dt.min(0.05);
        let (cw, s, bwu) = (self.cw(), self.s, self.bw as usize);
        for (k, f) in self.flies.iter_mut().enumerate() {
            if k >= cap || t < f.launch {
                continue;
            }
            if !f.on {
                f.on = true;
                let (i, j) = ((f.cell % bwu) as i32, (f.cell / bwu) as i32);
                f.x = (bx + i * cw + cw / 2) as f32;
                f.y = ((by + j * s) * 2 + s) as f32;
                f.vx = (f.x - dx) * 0.6 + f.ph.cos() * 25.0;
                f.vy = -55.0 - f.om * 10.0;
            }
            let a = f.ph + t * f.om;
            let tx = dx + f.rx * a.cos();
            let ty = dy - 2.0 + f.rx * 0.42 * a.sin();
            f.vx += ((tx - f.x) * 14.0 - f.vx * 4.2) * dt;
            f.vy += ((ty - f.y) * 14.0 - f.vy * 4.2) * dt;
            f.x += f.vx * dt;
            f.y += f.vy * dt;
        }
        if t >= self.end_t {
            // 交接：鸭鸭从这里长回大鸭鸭；新朋友跟着回主线（排在最前，不会被截掉）
            app.duck.x = dx;
            app.duck.y = dy;
            app.duck.right = self.duck.2;
            app.duck.bubble = None;
            let mut keep: Vec<Moth> = self
                .flies
                .iter()
                .filter(|f| f.on)
                .take(10)
                .map(|f| Moth {
                    x: f.x,
                    y: f.y,
                    vx: f.vx,
                    vy: f.vy,
                    flap: (f.ph * 3.0) % 2.0,
                    shade: f.shade,
                    tx: f.x,
                    ty: f.y,
                    delay: 0.0,
                    color: 0.0,
                    alive: true,
                })
                .collect();
            keep.extend(app.moths.drain(..));
            app.moths = keep;
            return true;
        }
        false
    }

    fn draw(&mut self, app: &mut App) {
        let t = self.t;
        let back = t >= self.close_t + COLLAPSE;
        if back {
            app.draw_logs(9.0, 0, 0.0);
        } else {
            // 窗口外的日志变暗：直接换成暗彩虹色带（调色板色，静止零成本）
            let hi = app.hi;
            app.hi = false;
            app.draw_logs(9.0, 0, 1.0);
            app.hi = hi;
        }
        let (x0, y0, x1, y1, bx, by) = self.lay(app);
        if self.under.is_none() {
            let (rw, rh) = (self.bw * self.cw(), self.bh * self.s);
            let mut v = Vec::with_capacity((rw * rh) as usize);
            for y in 0..rh {
                for x in 0..rw {
                    let (sx, sy) = (bx + x, by + y);
                    let ok = sx >= 0 && sy >= 0 && (sx as usize) < app.scr.w && (sy as usize) < app.scr.h;
                    v.push(if ok { app.scr.cells[sy as usize * app.scr.w + sx as usize] } else { BLANK });
                }
            }
            self.under = Some(v);
        }
        let (mx, my) = ((x0 + x1) / 2, (y0 + y1) / 2);
        let (hw, hh) = ((x1 - x0) / 2, (y1 - y0) / 2);
        if t < T_POP {
            // 弹出：6 步放大
            let k = 0.3 + 0.7 * ((t / T_POP * 6.0).floor() / 6.0);
            let (a, b) = (((hw as f32) * k) as i32, ((hh as f32) * k).max(1.0) as i32);
            window(app, mx - a, my - b, mx + a + (x1 - x0) % 2, my + b + (y1 - y0) % 2, "", WIN_FG, WIN_BG);
        } else if t < self.close_t {
            let hung = t > self.stuck_t + 0.3 && t < self.peck_t;
            let title = if t > self.peck_t + 1.5 {
                "扫 bug 失败 → 交友成功"
            } else if hung {
                "扫 bug.exe（未响应）"
            } else {
                "扫 bug.exe"
            };
            window(app, x0, y0, x1, y1, title, WIN_FG, WIN_BG);
            let flagged = self.flag_t.iter().filter(|&&f| t >= f).count();
            let (left, right) = if t > self.peck_t + 1.5 {
                (format!("好友 +{}", self.nb), "游戏结束（大概）".to_string())
            } else if t > self.stuck_t {
                (format!("剩余 bug ×{}", self.nb.saturating_sub(flagged)), "推不动了，只能猜".to_string())
            } else {
                let el = (t - self.t_click).max(0.0);
                (format!("剩余 bug ×{}", self.nb.saturating_sub(flagged)), format!("用时 {:4.1}s", el))
            };
            let sy = y0 + 1;
            app.scr.text(x0 + 2, sy, &left, [215, 215, 232], Some(WIN_BG));
            let rx = x1 - 1 - str_w(&right) as i32;
            if rx > x0 + 2 + str_w(&left) as i32 + 1 {
                app.scr.text(rx, sy, &right, [120, 122, 156], Some(WIN_BG));
            }
            for j in 0..self.bh {
                for i in 0..self.bw {
                    self.draw_cell(app, i, j, bx, by);
                }
            }
        } else if !back {
            // 关窗：先压成一条线，再收成一个点
            let k = (((t - self.close_t) / COLLAPSE) * 6.0).floor() / 6.0;
            if k < 0.5 {
                let b = ((hh as f32) * (1.0 - k * 2.0)) as i32;
                if b >= 1 {
                    window(app, x0, my - b, x1, my + b, "", WIN_FG, WIN_BG);
                } else {
                    for x in x0..=x1 {
                        app.scr.put(x, my, '─', WIN_FG, Some(WIN_BG));
                    }
                }
            } else {
                let a = ((hw as f32) * (1.0 - (k - 0.5) * 2.0)) as i32;
                for x in mx - a..=mx + a {
                    app.scr.put(x, my, '─', [200, 200, 230], Some(WIN_BG));
                }
            }
        }
        // 像素层：花 + 鸭鸭 + 飞蛾
        if t < self.close_t {
            self.draw_flower(app);
        }
        if self.duck_at(app, t).is_some() {
            let (x, y, r) = self.duck;
            mini_duck(app, x, y, r, self.s);
        }
        let cap = if app.hi { usize::MAX } else { 14 };
        for f in self.flies.iter().take(cap) {
            if f.on {
                let flap = t * 13.0 + f.ph;
                moth(app, f.x, f.y, f.shade, flap, 0.0);
            }
        }
    }

    fn draw_over(&mut self, app: &mut App) {
        let Some(text) = self.bubble() else { return };
        if self.duck_at(app, self.t).is_none() {
            return;
        }
        let (dx, dy, right) = self.duck;
        let k = self.s as f32;
        let tw = str_w(text) as i32 + 4;
        let bgc = [250, 250, 245];
        // 头顶有地方（不压窗口标题栏）就放头顶，否则放在嘴巴那一侧
        let title_row = if self.t < self.close_t { self.lay(app).1 } else { -1 };
        let top = ((dy - 3.5 * k - 1.0) / 2.0).floor() as i32;
        if top - 4 > title_row && top - 4 >= 0 {
            let bx = if right { dx as i32 - 2 } else { dx as i32 + 2 - tw };
            let bx = bx.clamp(0, (app.scr.w as i32 - tw).max(0));
            let by = top - 4;
            for row in 0..3 {
                for x in 0..tw {
                    app.scr.put(bx + x, by + row, ' ', BLACK, Some(bgc));
                }
            }
            app.scr.text(bx + 2, by + 1, text, [20, 20, 30], Some(bgc));
            let tx = (dx as i32 + if right { 1 } else { -2 }).clamp(bx + 1, bx + tw - 2);
            app.scr.put(tx, by + 3, '▀', bgc, None);
            return;
        }
        let head = ((dy - 3.5 * k) / 2.0).floor() as i32 + 1;
        let reach = (5.0 * k) as i32 + 2;
        let mut right = right;
        if right && dx as i32 + reach + tw >= app.scr.w as i32 {
            right = false;
        } else if !right && dx as i32 - reach - tw < 0 {
            right = true;
        }
        let bx = if right { dx as i32 + reach + 1 } else { dx as i32 - reach - tw };
        let by = (head - 1).max(0);
        for row in 0..3 {
            for x in 0..tw {
                app.scr.put(bx + x, by + row, ' ', BLACK, Some(bgc));
            }
        }
        app.scr.text(bx + 2, by + 1, text, [20, 20, 30], Some(bgc));
        let (tx, tc) = if right { (bx - 1, '▐') } else { (bx + tw, '▌') };
        app.scr.put(tx, by + 1, tc, bgc, None);
    }
}
