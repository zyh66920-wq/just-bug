//! 插曲：扑腾鸭——只借「点一下往上扑、重力下坠、钻过上下障碍之间的缝」的玩法。
//! 日志画面往两边裂开，留下一根根「日志柱」（竖排的日志字），柱子上下成对、中间留缝，从右往左移；
//! 鸭鸭在左侧 1/3 处自动扑腾穿缝，缝里停着的飞蛾给它让路、击掌；
//! 最后一对柱子的缝窄得过不去——柱子其实是飞蛾伪装的，哗地散开排成两列夹道，放鸭鸭过去。
//!
//! 性能：柱子是稀疏字符、用调色板暗彩虹（颜色跟着字走），每帧只有柱子边缘和鸭鸭、飞蛾在变；省流量档柱子更稀更少。
use super::*;

// 时间轴（插曲开始后第几秒）
const T_SPLIT0: f32 = 0.1;
const T_SPLIT1: f32 = 1.2;
const T_GAP0: f32 = 0.7;
const T_GAP1: f32 = 1.5;
const T_IN0: f32 = 0.9;
const T_IN1: f32 = 1.9;
const T_GO: f32 = 1.5;
const T_B0: f32 = 12.3; // 最后一对柱子来了，开始减速
const T_STOP: f32 = 13.4;
const T_SHAKE: f32 = 14.3;
const T_SCAT: f32 = 14.9;
const T_RUN2: f32 = 15.5; // 世界重新开动
const T_CAP2: f32 = 16.5;
const T_CLOSE0: f32 = 17.6;
const T_CLOSE1: f32 = 18.2;
const T_END: f32 = 18.4;
const DT_TAB: f32 = 0.005;
const PHYS_DT: f32 = 1.0 / 120.0;

const EYE: Rgb = [12, 10, 20];
const BEAK: Rgb = [255, 160, 40];
const OUTLINE: Rgb = [14, 12, 24];

struct Pm {
    off: f32,
    shade: u8,
    ph: f32,
    top: bool,
    five: bool,
}

struct Pillar {
    x: f32,  // 世界坐标（左列）
    gc: f32, // 缝中心（像素）
    gh: f32, // 缝高（像素）
    close: bool,
    last: bool,
    snap: bool, // 开场就站在屏幕上、由日志截图切出来的
    cells: Vec<Option<(char, Col)>>,
    moths: Vec<Pm>,
}

struct Bit {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    shade: u8,
    ph: f32,
    top: bool,
    slot: f32,
    ang: f32,
    rad: f32,
}

pub struct Flap {
    t: f32,
    seed: u64,
    sw: usize,
    sh: usize,
    k: i32,
    pw: i32,
    dh: f32,
    half: f32,
    dx: f32,
    v0: f32,
    v1: f32,
    g: f32,
    fv: f32,
    tab: Vec<f32>,
    pillars: Vec<Pillar>,
    snap: Vec<Cell>,
    snap_w: usize,
    snap_h: usize,
    endsnap: Option<Vec<Cell>>,
    end_w: usize,
    end_h: usize,
    y: f32,
    vy: f32,
    acc: f32,
    flap_t: f32,
    passed: usize,
    close_t: f32,
    five: Option<(i32, i32)>,
    bits: Vec<Bit>,
    scattered: bool,
    s_scat: f32,
    step: u8,
}

fn ease_out(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    1.0 - (1.0 - x) * (1.0 - x)
}

fn hash(a: i32, b: i32, c: i32) -> u32 {
    let mut h = (a as u32).wrapping_mul(0x9E37_79B1) ^ (b as u32).wrapping_mul(0x85EB_CA77) ^ (c as u32).wrapping_mul(0xC2B2_AE3D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^ (h >> 13)
}

fn bubble(app: &mut App, ax: i32, top: i32, text: &str) {
    let w = app.scr.w as i32;
    let tw = str_w(text) as i32 + 4;
    let bx = ax.clamp(0, (w - tw).max(0));
    let top = top.max(1);
    let bgc = [250, 250, 245];
    for row in 0..3 {
        for x in 0..tw {
            app.scr.put(bx + x, top + row, ' ', BLACK, Some(bgc));
        }
    }
    app.scr.text(bx + 2, top + 1, text, [20, 20, 30], Some(bgc));
    let tx = (ax + 1).clamp(bx + 1, bx + tw - 2);
    app.scr.put(tx, top + 3, '▀', bgc, None);
}

/// 一行居中的字幕（黑底，免得被柱子的字搅乱）
fn caption(app: &mut App, y: i32, s: &str, rainbow: bool, c: Rgb) {
    let w = app.scr.w as i32;
    let mut x = ((w - str_w(s) as i32) / 2).max(0);
    app.scr.put(x - 1, y, ' ', BLACK, Some(BLACK));
    for ch in s.chars() {
        let fg: Col = if rainbow { rb(Band::Text, x as f32, y as f32) } else { c.into() };
        app.scr.put_c(x, y, ch, fg, Some(Col::T(BLACK)));
        x += char_w(ch) as i32;
    }
    app.scr.put(x, y, ' ', BLACK, Some(BLACK));
}

impl Flap {
    pub fn new(app: &mut App) -> Self {
        let seed = app.rng.next() | 1;
        app.duck.visible = false;
        app.duck.bubble = None;
        app.set_title("扑腾鸭 — 穿过 ×0");
        // 拍一张「现在的日志画面」：开场往两边裂开，留下的竖条就是第一批日志柱
        app.scr.clear();
        app.draw_logs(9.0, 0, 0.0);
        let snap = app.scr.cells.clone();
        app.scr.clear();
        app.scr.scroll = 0;
        app.last_top = None;
        let mut f = Flap {
            t: 0.0,
            seed,
            sw: 0,
            sh: 0,
            k: 1,
            pw: 2,
            dh: 9.0,
            half: 6.0,
            dx: 0.0,
            v0: 0.0,
            v1: 0.0,
            g: 0.0,
            fv: 0.0,
            tab: Vec::new(),
            pillars: Vec::new(),
            snap,
            snap_w: app.scr.w,
            snap_h: app.scr.h,
            endsnap: None,
            end_w: 0,
            end_h: 0,
            y: 0.0,
            vy: 0.0,
            acc: 0.0,
            flap_t: 1.0,
            passed: 0,
            close_t: -9.0,
            five: None,
            bits: Vec::new(),
            scattered: false,
            s_scat: 0.0,
            step: 0,
        };
        f.plan(app);
        f.y = f.pillars.first().map(|p| p.gc).unwrap_or(app.hp() / 2.0);
        f
    }

    fn v_at(&self, t: f32) -> f32 {
        if t < T_GO {
            0.0
        } else if t < T_B0 {
            let lin = self.v0 + (self.v1 - self.v0) * ((t - T_GO) / (T_B0 - T_GO));
            lin * smooth(T_GO, T_GO + 0.7, t)
        } else if t < T_STOP {
            self.v1 * (1.0 - (t - T_B0) / (T_STOP - T_B0)).powf(1.5)
        } else if t < T_RUN2 {
            0.0
        } else {
            self.v1 * smooth(T_RUN2, T_RUN2 + 0.6, t)
        }
    }

    fn s_at(&self, t: f32) -> f32 {
        if self.tab.is_empty() {
            return 0.0;
        }
        let f = (t.max(0.0) / DT_TAB).min((self.tab.len() - 1) as f32);
        let i = f as usize;
        let j = (i + 1).min(self.tab.len() - 1);
        let k = f - i as f32;
        self.tab[i] * (1.0 - k) + self.tab[j] * k
    }

    /// 按当前尺寸（和种子）排好所有柱子；换尺寸时重排
    fn plan(&mut self, app: &App) {
        let (w, h) = (app.scr.w, app.scr.h);
        self.sw = w;
        self.sh = h;
        let wf = w as f32;
        let hp = h as f32 * 2.0;
        let k = app.s();
        self.k = k;
        self.pw = if w >= 160 { 4 } else if w >= 70 { 3 } else { 2 };
        let pw = self.pw;
        self.dh = (7 * k + 2) as f32;
        self.half = (5 * k + 1) as f32;
        self.dx = (wf / 3.0).round();
        let sp = (wf * 0.2).max(2.6 * (10 * k + 2) as f32 + pw as f32);
        let spacing = if app.hi { sp } else { sp * 1.5 };
        self.v0 = sp / 1.6;
        self.v1 = sp / 1.15;
        self.g = 18.0 * self.dh;
        self.fv = (2.0 * self.g * 0.55 * self.dh).sqrt();
        self.tab.clear();
        let (mut s, mut t) = (0.0, 0.0);
        while t <= T_END + 0.3 {
            self.tab.push(s);
            s += self.v_at(t) * DT_TAB;
            t += DT_TAB;
        }
        let fin = (self.dx + self.half + 3.0 + self.s_at(T_STOP)).round();

        let mut pool: Vec<Vec<char>> = app
            .logs
            .iter()
            .rev()
            .take(40)
            .map(|l| l.text.chars().filter(|c| c.is_ascii_graphic()).collect::<Vec<char>>())
            .filter(|v| v.len() >= 8)
            .collect();
        if pool.is_empty() {
            pool.push("INFOduck::flaptrace=0xDEADBEEFbug=okmoth=1quack()".chars().collect());
        }
        let mut rng = Rng(self.seed);
        let dh = self.dh;
        let (lo, hi) = (2.0 + 1.3 * dh, hp - 1.3 * dh);
        let (lo, hi) = if lo < hi { (lo, hi) } else { (hp * 0.5, hp * 0.5 + 0.001) };
        let mut xs = Vec::new();
        let mut x = (wf * 0.45).max(self.dx + self.half + 8.0).round();
        while x <= fin - spacing * 0.9 {
            xs.push(x);
            x = (x + spacing).round();
        }
        xs.push(fin);
        let n = xs.len();
        let snap_ok = self.snap_w == w && self.snap_h == h && self.snap.len() == w * h;
        let dens = if app.hi { 0.74 } else { 0.5 };
        let mut gc = rng.range(lo, hi);
        self.pillars.clear();
        for (i, &px) in xs.iter().enumerate() {
            let last = i == n - 1;
            if i > 0 && !last {
                gc = (gc + rng.range(-1.1, 1.1) * dh).clamp(lo, hi);
            }
            let close = !last && (i == 2 || i == 4);
            let gh = if last { 0.45 * dh } else if close { 1.75 * dh } else { 2.4 * dh };
            let from_snap = snap_ok && px >= 0.0 && px as usize + pw as usize <= w;
            let mut cells = vec![None; pw as usize * h];
            for c in 0..pw as usize {
                let line = &pool[rng.int(pool.len())];
                let o = rng.int(line.len());
                for r in 1..h {
                    let pick = line[(o + r) % line.len()];
                    cells[r * pw as usize + c] = if from_snap {
                        let cell = self.snap[r * w + px as usize + c];
                        if cell.ch == ' ' {
                            None
                        } else if cell.w != 0 {
                            Some((pick, cell.fg))
                        } else {
                            Some((cell.ch, cell.fg))
                        }
                    } else if rng.f() < dens {
                        Some((pick, rb(if c == 0 { Band::Text } else { Band::Dim }, (i * 7 + c * 3) as f32, r as f32)))
                    } else {
                        None
                    };
                }
            }
            let nm = if last || i == 0 { 0 } else if i % 2 == 1 { 2 } else { 1 };
            let nm = if app.hi { nm } else { nm.min(1) };
            let mut moths = Vec::new();
            let mut top = rng.f() < 0.5;
            for j in 0..nm {
                moths.push(Pm { off: rng.range(-0.6, 0.6), shade: rng.int(5) as u8, ph: rng.range(0.0, 10.0), top, five: j == 0 && i % 2 == 1 });
                top = !top;
            }
            self.pillars.push(Pillar { x: px, gc, gh, close, last, snap: from_snap, cells, moths });
        }
    }

    /// 自动驾驶的目标高度：下一对还没完全穿过的柱子的缝
    fn target(&self, s: f32) -> f32 {
        let edge = self.dx - self.half - 1.0;
        for p in &self.pillars {
            if p.x - s + self.pw as f32 >= edge {
                return p.gc + 0.25 * self.dh;
            }
        }
        self.sh as f32
    }

    fn passed_at(&self, s: f32) -> usize {
        let edge = self.dx - self.half;
        self.pillars.iter().filter(|p| !p.last && p.x - s + (self.pw as f32) < edge).count()
    }

    fn duck_x(&self, t: f32) -> f32 {
        let f = ease_out((t - T_IN0) / (T_IN1 - T_IN0));
        -2.0 * self.half + (self.dx + 2.0 * self.half) * f
    }

    fn spawn_bits(&mut self, app: &App, s: f32) {
        self.scattered = true;
        self.s_scat = s;
        let Some(p) = self.pillars.iter().find(|p| p.last) else { return };
        let pw = self.pw as usize;
        let sx0 = (p.x - s).round();
        let cap = if app.hi { 60 } else { 20 };
        let filled: Vec<(usize, usize)> = (1..self.sh)
            .flat_map(|r| (0..pw).map(move |c| (r, c)))
            .filter(|&(r, c)| p.cells.get(r * pw + c).map_or(false, |x| x.is_some()))
            .collect();
        let stride = ((filled.len() + cap - 1) / cap.max(1)).max(1);
        let mut rng = Rng(self.seed ^ 0xB0B);
        let dh = self.dh;
        let gc = p.gc;
        let (mut nt, mut nb) = (0.0f32, 0.0f32);
        let mut bits = Vec::new();
        for (idx, &(r, c)) in filled.iter().enumerate() {
            if idx % stride != 0 {
                continue;
            }
            let y = r as f32 * 2.0 + 1.0;
            let top = y < gc;
            let slot = if top {
                nt += 1.0;
                nt
            } else {
                nb += 1.0;
                nb
            };
            let sp = rng.range(2.5, 7.0) * dh;
            bits.push(Bit {
                x: sx0 + c as f32 + 0.5,
                y,
                vx: rng.range(-1.0, 2.0) * dh,
                vy: if top { -sp } else { sp },
                shade: rng.int(5) as u8,
                ph: rng.range(0.0, 10.0),
                top,
                slot,
                ang: rng.range(0.0, std::f32::consts::TAU),
                rad: rng.range(1.2, 2.6) * dh,
            });
        }
        // 夹道队形：每一排以柱子为中心左右铺开
        for b in &mut bits {
            let n = if b.top { nt } else { nb };
            b.slot = sx0 + (b.slot - n / 2.0) * 6.0 + if b.top { 0.0 } else { 3.0 };
        }
        self.bits = bits;
    }

    fn step_phys(&mut self, s: f32) {
        let tgt = self.target(s);
        self.vy = (self.vy + self.g * PHYS_DT).min(self.fv * 1.4);
        self.y += self.vy * PHYS_DT;
        if self.y > tgt && self.vy > 0.0 {
            self.vy = -self.fv;
            self.flap_t = 0.0;
        }
        self.flap_t += PHYS_DT;
        let hp = self.sh as f32 * 2.0;
        let (a, b) = (self.dh * 0.5 + 2.0, hp - self.dh * 0.5 - 1.0);
        self.y = if a < b { self.y.clamp(a, b) } else { hp * 0.5 };
    }

    /// 带姿态的小鸭鸭：上扑时身体拉长、翅膀扬起，下落时微微低头
    fn duck_px(&self, app: &mut App, cx: f32, cy: f32) {
        let k = self.k;
        let kf = k as f32;
        let hi = app.hi;
        let tq = (app.t * 14.0).floor() / 10.0;
        let ft = self.flap_t;
        let stretch = if ft < 0.1 {
            1.22
        } else if ft < 0.25 {
            1.22 - (ft - 0.1) / 0.15 * 0.22
        } else if self.vy > 0.5 * self.fv {
            0.94
        } else {
            1.0
        };
        let tilt = (self.vy / self.fv).clamp(-0.7, 1.0) * 0.2 * kf;
        let wing: &[(i32, i32)] = if ft < 0.12 {
            &[(1, 0), (1, 1), (2, 1), (2, 2), (3, 2), (3, 3)]
        } else if ft < 0.24 {
            &[(1, 3), (2, 3), (3, 3), (4, 3)]
        } else {
            &[(2, 4), (3, 4), (4, 4), (3, 5), (4, 5)]
        };
        let gw = 10 * k + 2;
        let pad = 3 * k + 2;
        let gh = 7 * k + 2 + pad * 2;
        let ox = cx.round() as i32 - 5 * k - 1;
        let oy = cy.round() as i32 - gh / 2;
        let top = (gh as f32 - 7.0 * kf * stretch) / 2.0;
        let mut buf: Vec<Option<Col>> = vec![None; (gw * gh) as usize];
        let plot = |buf: &mut Vec<Option<Col>>, i: i32, j: i32, c: Option<Col>| {
            let dy = (i as f32 - 4.5) * tilt;
            let y0 = (top + j as f32 * kf * stretch + dy).round() as i32;
            let y1 = (top + (j + 1) as f32 * kf * stretch + dy).round() as i32;
            for sx in 0..k {
                let x = i * k + sx + 1;
                for y in y0..y1.max(y0 + 1) {
                    if y >= 1 && y < gh - 1 {
                        let col = c.unwrap_or_else(|| {
                            if hi {
                                mix(lolcat((ox + x) as f32 * 0.8, (oy + y) as f32 * 0.4, tq), WHITE, 0.25).into()
                            } else {
                                rb(Band::Duck, x as f32 * 1.6 / kf, (y - pad) as f32 * 0.8 / kf)
                            }
                        });
                        buf[(y * gw + x) as usize] = Some(col);
                    }
                }
            }
        };
        for (j, row) in MINI_DUCK.iter().enumerate() {
            for (i, ch) in row.chars().enumerate() {
                let c = match ch {
                    '.' => continue,
                    'e' => Some(Col::T(EYE)),
                    'B' => Some(Col::T(BEAK)),
                    _ => None,
                };
                plot(&mut buf, i as i32, j as i32, c);
            }
        }
        for &(i, j) in wing {
            plot(&mut buf, i, j, Some(rb(Band::Dim, i as f32 * 3.0, j as f32 * 2.0)));
        }
        for y in 0..gh {
            for x in 0..gw {
                let c = buf[(y * gw + x) as usize].or_else(|| {
                    let near = [(-1, 0), (1, 0), (0, -1), (0, 1)].iter().any(|(ddx, ddy)| {
                        let (nx, ny) = (x + ddx, y + ddy);
                        nx >= 0 && ny >= 0 && nx < gw && ny < gh && buf[(ny * gw + nx) as usize].is_some()
                    });
                    near.then(|| Col::T(OUTLINE))
                });
                if let Some(c) = c {
                    app.layer.set(ox + x, oy + y, c);
                }
            }
        }
    }

    /// 日志截图左右两半，各自往外平移 off 列；skip 的列不画（留给日志柱）；blanks=true 时连空格也画（当幕布）
    fn halves(app: &mut App, snap: &[Cell], sw: usize, sh: usize, off: i32, skip: &dyn Fn(i32) -> bool, blanks: bool) {
        let (w, h) = (app.scr.w as i32, app.scr.h.min(sh));
        let mid = sw as i32 / 2;
        for r in 0..h {
            for x in 0..sw as i32 {
                if skip(x) {
                    continue;
                }
                let c = snap[r * sw + x as usize];
                if c.w == 2 || (!blanks && c.ch == ' ' && c.bg == Col::T(BLACK)) {
                    continue;
                }
                let nx = if x < mid { x - off } else { x + off };
                if nx >= 0 && nx < w {
                    app.scr.put_c(nx, r as i32, c.ch, c.fg, Some(c.bg));
                }
            }
        }
    }
}

impl Interlude for Flap {
    fn name(&self) -> &'static str {
        "扑腾鸭"
    }

    fn update(&mut self, app: &mut App, dt: f32) -> bool {
        self.t += dt;
        let t = self.t;
        if app.scr.w != self.sw || app.scr.h != self.sh {
            self.plan(app);
        }
        if t >= T_IN0 {
            self.acc += dt;
            let mut tt = t - self.acc;
            while self.acc >= PHYS_DT {
                self.acc -= PHYS_DT;
                tt += PHYS_DT;
                let s = self.s_at(tt);
                self.step_phys(s);
            }
        }
        let s = self.s_at(t);
        // 穿过计数 / 险过
        let n = self.passed_at(s);
        if n > self.passed {
            for i in self.passed..n.min(self.pillars.len()) {
                if self.pillars[i].close {
                    self.close_t = t;
                }
            }
            self.passed = n;
            if t < T_SCAT {
                app.set_title(&format!("扑腾鸭 — 穿过 ×{}", n));
            }
        }
        if self.step == 0 && t >= T_STOP + 0.2 {
            self.step = 1;
            app.set_title("扑腾鸭 — 前方缝隙：0.4 只鸭宽");
        }
        // 散成飞蛾
        if !self.scattered && t >= T_SCAT {
            self.spawn_bits(app, s);
            app.set_title("扑腾鸭 — 穿过 ×∞（碰撞检测已被 bug 关闭）");
        }
        if self.scattered {
            let ddt = dt.min(0.05);
            let ds = s - self.s_scat;
            let (dx, dy, dh) = (self.dx, self.y, self.dh);
            let ycorr = self.pillars.iter().find(|p| p.last).map_or(dy, |p| p.gc);
            for (i, b) in self.bits.iter_mut().enumerate() {
                b.ph += ddt * (9.0 + (i % 5) as f32);
                if t < T_SCAT + 0.35 {
                    let d = (-2.0 * ddt).exp();
                    b.vx *= d;
                    b.vy *= d;
                } else {
                    let (tx, ty) = if t < T_CAP2 {
                        let lane = if b.top { -1.0 } else { 1.0 };
                        (b.slot - ds, ycorr + lane * (1.35 * dh + (i % 2) as f32 * 3.0))
                    } else {
                        let a = b.ang + t * 1.6;
                        (dx + a.cos() * b.rad * 1.8, dy + a.sin() * b.rad)
                    };
                    b.vx += ((tx - b.x) * 24.0 - b.vx * 8.0) * ddt;
                    b.vy += ((ty - b.y) * 24.0 - b.vy * 8.0) * ddt;
                }
                b.x += b.vx * ddt;
                b.y += b.vy * ddt;
            }
        }
        if self.step == 1 && t >= T_CAP2 {
            self.step = 2;
            app.push(log_line("WARN", "flap::collide", "碰撞检测已被禁用（操作人：bug；理由：那是我对象）", Kind::Info));
            app.push(log_line("INFO", "bug::pillar", &format!("日志柱解散，重新编组为飞蛾 ×{}", self.bits.len()), Kind::Info));
            app.push(log_line("INFO", "duck::flap", &format!("本局穿过 ×{}，最后一关由 bug 放行（成绩仅供参考）", self.passed + 1), Kind::Fixed));
        }
        t >= T_END
    }

    fn draw(&mut self, app: &mut App) {
        let t = self.t;
        if t >= T_CLOSE1 {
            app.draw_logs(9.0, 0, 0.0);
            return;
        }
        let hi = app.hi;
        let tq = if hi { t } else { (t * 15.0).floor() / 15.0 };
        let (w, h) = (app.scr.w as i32, app.scr.h as i32);
        let pw = self.pw;
        let s = self.s_at(t);

        // ── 开场：日志往两边裂开，日志柱留在原地
        if tq < T_SPLIT1 && self.snap_w == app.scr.w && self.snap_h == app.scr.h {
            let f = ((tq - T_SPLIT0) / (T_SPLIT1 - T_SPLIT0)).clamp(0.0, 1.0);
            let off = ((w / 2 + 2) as f32 * f * f).round() as i32;
            let cols: Vec<i32> = self.pillars.iter().filter(|p| p.snap).map(|p| p.x as i32).collect();
            let skip = move |x: i32| cols.iter().any(|&c| x >= c && x < c + pw);
            let snap = std::mem::take(&mut self.snap);
            Self::halves(app, &snap, self.snap_w, self.snap_h, off, &skip, false);
            self.snap = snap;
        }

        // ── 日志柱
        let gap_open = smooth(T_GAP0, T_GAP1, tq);
        let shake = ((t - T_SHAKE) / (T_SCAT - T_SHAKE)).clamp(0.0, 1.0);
        let jt = (t * 14.0) as i32;
        let mut five = None;
        let duck_x = self.duck_x(t);
        for (pi, p) in self.pillars.iter().enumerate() {
            if p.last && self.scattered {
                continue;
            }
            let sx0 = (p.x - s).round() as i32;
            if sx0 + pw < 0 || sx0 >= w {
                continue;
            }
            let g = if p.snap { gap_open } else { 1.0 };
            let gh = p.gh * g;
            let (gt, gb) = (p.gc - gh / 2.0, p.gc + gh / 2.0);
            let has_gap = g > 0.03;
            let (te, bs) = if has_gap { ((gt / 2.0).floor() as i32, (gb / 2.0).ceil() as i32) } else { (h, h) };
            let shaking = p.last && t >= T_SHAKE;
            for r in 1..h {
                if r >= te && r < bs {
                    continue;
                }
                let jx = if shaking && (hash(r, jt, pi as i32) % 100) < (shake * 70.0) as u32 { (hash(r, jt, 7) % 3) as i32 - 1 } else { 0 };
                for c in 0..pw {
                    let x = sx0 + c + jx;
                    if x < 0 || x >= w {
                        continue;
                    }
                    if has_gap && (r == te - 1 || r == bs) && g > 0.5 {
                        app.scr.put_c(x, r, '═', rb(Band::Text, (pi * 5 + c as usize) as f32, r as f32), None);
                        continue;
                    }
                    if let Some(Some((ch, col))) = p.cells.get(r as usize * pw as usize + c as usize) {
                        let mut ch = *ch;
                        if shaking && (hash(r, c, jt / 2) % 100) < (shake * 85.0) as u32 {
                            ch = ['w', 'v', 'W', '^'][(hash(r, c, pi as i32) % 4) as usize];
                        }
                        app.scr.put_c(x, r, ch, *col, None);
                    }
                }
            }
            // 缝里停着的飞蛾：鸭鸭来了就贴边让路，领头那只伸手击掌
            if g > 0.9 {
                for m in &p.moths {
                    let mx = sx0 as f32 + pw as f32 / 2.0 + m.off;
                    let d = (mx - self.dx).abs();
                    let near = 1.0 - smooth(self.half + 2.0, self.half + 10.0, d);
                    let base = if m.top { gt + 1.5 } else { gb - 1.5 };
                    let my = if m.five {
                        let hf = 1.0 - smooth(1.0, self.half, d);
                        if hf > 0.3 {
                            five = Some((mx.round() as i32, ((base + (self.y - base) * 0.3 * hf) / 2.0) as i32));
                        }
                        base + (self.y - base) * 0.3 * hf
                    } else {
                        base + if m.top { -3.0 } else { 3.0 } * near
                    };
                    moth(app, mx, my, m.shade, m.ph + tq * (3.0 + 12.0 * near), 0.0);
                }
            }
        }
        self.five = five;

        // ── 收尾幕布位置（像素层要躲开）
        let close_off = if t >= T_CLOSE0 {
            let f = ((tq - T_CLOSE0) / (T_CLOSE1 - T_CLOSE0 - 0.05)).clamp(0.0, 1.0);
            Some(((w / 2 + 2) as f32 * (1.0 - ease_out(f))).round() as i32)
        } else {
            None
        };
        let visible = |x: f32| match close_off {
            Some(o) => x >= (w / 2 - o) as f32 && x < (w / 2 + o) as f32,
            None => true,
        };

        // ── 飞蛾群（散开的柱子）
        let rainbow = if hi && t >= T_CAP2 { 0.45 } else { 0.0 };
        for b in &self.bits {
            if visible(b.x) {
                moth(app, b.x, b.y, b.shade, b.ph, rainbow);
            }
        }

        // ── 鸭鸭
        if t >= T_IN0 && visible(duck_x) {
            self.duck_px(app, duck_x, self.y);
        }

        // ── 收尾：日志从两边合拢
        if let Some(off) = close_off {
            if self.endsnap.is_none() {
                let keep = app.scr.cells.clone();
                app.scr.clear();
                app.draw_logs(9.0, 0, 0.0);
                self.endsnap = Some(app.scr.cells.clone());
                self.end_w = app.scr.w;
                self.end_h = app.scr.h;
                app.scr.cells = keep;
                app.scr.scroll = 0;
                app.last_top = None;
            }
            if self.end_w == app.scr.w && self.end_h == app.scr.h {
                let snap = self.endsnap.take().unwrap_or_default();
                Self::halves(app, &snap, self.end_w, self.end_h, off, &|_| false, true);
                self.endsnap = Some(snap);
            }
        }
    }

    fn draw_over(&mut self, app: &mut App) {
        let t = self.t;
        if t < T_IN0 || t >= T_CLOSE0 {
            return;
        }
        let (w, h) = (app.scr.w as i32, app.scr.h as i32);
        // ── 顶栏
        let right = if t >= T_SCAT { "穿过 ×∞".to_string() } else { format!("穿过 ×{}", self.passed) };
        let rw = str_w(&right) as i32;
        app.scr.text(w - rw - 2, 0, &right, [210, 210, 220], Some(BLACK));
        if w > rw + 14 {
            let mut x = 2;
            for ch in "扑腾鸭".chars() {
                app.scr.put(x, 0, ch, rb(Band::Text, x as f32 * 3.0, 0.0), Some(BLACK));
                x += 2;
            }
        }
        let dx = self.duck_x(t);
        let duck_top = ((self.y - self.dh * 0.7) / 2.0) as i32;
        // ── 险过 / 击掌
        if t - self.close_t < 0.8 {
            app.scr.text(dx as i32 + self.half as i32 + 1, (duck_top - 1).max(1), "险！", [255, 200, 90], Some(BLACK));
        }
        if let Some((fx, fy)) = self.five.filter(|_| h >= 16) {
            app.scr.text(fx - 2, (fy - 2).clamp(1, h - 1), "啪！", WHITE, Some(BLACK));
        }
        // ── 结尾文案
        let cap_y = if h > 14 { 2 } else { 1 };
        let bt = if duck_top - 4 <= cap_y { ((self.y + self.dh * 0.7) / 2.0) as i32 + 1 } else { duck_top - 4 };
        if t >= T_STOP + 0.2 && t < T_SCAT {
            let s = if w >= 60 { "WARN  前方缝隙 < 一只鸭宽，预计碰撞" } else { "WARN 缝太窄" };
            caption(app, cap_y, s, false, [255, 190, 80]);
        }
        if t >= T_STOP + 0.6 && t < T_SCAT {
            bubble(app, dx as i32 - 2, bt, "吸肚子……也不行");
        }
        if t >= T_SCAT + 0.2 && t < T_RUN2 + 0.5 {
            bubble(app, dx as i32 - 2, bt, "嘎？！");
        }
        if t >= T_SCAT + 0.15 && t < T_CAP2 {
            caption(app, cap_y, "柱子：其实我们是 bug", true, WHITE);
        }
        if t >= T_CAP2 {
            caption(app, cap_y, "这关的碰撞检测被 bug 关了", true, WHITE);
        }
    }
}
