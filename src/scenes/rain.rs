//! 插曲：字符雨。
//! 日志开始「漏水」：字一个个从行里掉下来，拖着自己日志里的字（中文、hex、模块名，竖着读得通）汇成黑白灰的雨；
//! 几只飞蛾在雨里逆流往上游，总被冲下来；大鸭鸭从左边漂过，碰到的雨柱变成彩虹并保持彩虹；
//! 雨全变彩虹后「停了」：雨滴落到底部堆起来，再从底往上一层层堆回成日志——「雨停了，日志已晾干」。
//!
//! 带宽：雨按 2 格一条「雨道」排列；拖尾颜色按离头的距离分 5 档，头往下走一格只有档位交界处的几个格子变色；
//! 静止的日志、落地的字堆、彩虹拖尾（调色板色）都不产生额外重写。省流量档只开一半雨道。
use super::*;

const T_DUCK0: f32 = 5.0; // 大鸭鸭从左边进场
const T_DUCK1: f32 = 11.4; // 大鸭鸭从右边出场
const T_STOP: f32 = 12.2; // 雨停
const T_BUILD: f32 = 13.7; // 开始一层层堆回日志
const BUILD_LEN: f32 = 2.6;
const T_END: f32 = 17.9;

struct Drop {
    lane: usize,
    y: f32,
    v: f32,
    acc: f32,
    vmax: f32,
    len: f32,
    /// 从行里漏下来的那个字：头一直是它
    fixed: Option<char>,
    src: usize,
    k: usize,
    hr: i32,
    start: i32,
}

struct Bug {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    flap: f32,
    shade: u8,
    dye: f32,
    ph: f32,
    tumble: f32,
}

pub struct Rain {
    t: f32,
    w: usize,
    h: usize,
    /// 还挂在行里的日志字（只记宽字符的本体格）
    grid: Vec<Option<(char, Col)>>,
    alive: Vec<u32>,
    leak_acc: f32,
    leaked: usize,
    pool: Vec<Vec<char>>,
    drops: Vec<Drop>,
    /// 每条雨道每行留下的字
    trail: Vec<char>,
    cool: Vec<f32>,
    dyed: Vec<bool>,
    n_dyed: usize,
    grains: Vec<Vec<(f32, f32, char)>>,
    heap: Vec<char>,
    pile: Vec<usize>,
    bugs: Vec<Bug>,
    knocks: usize,
    step: usize,
    stopped: bool,
}

fn lanes_of(w: usize) -> usize {
    (w + 1) / 2
}

impl Rain {
    pub fn new(app: &mut App) -> Self {
        let (w, h) = (app.scr.w, app.scr.h);
        // 把此刻的日志画面拍下来，作为开场「还没漏的字」，和主线无缝衔接
        app.scr.clear();
        app.draw_logs(9.0, 0, 0.0);
        app.last_top = None;
        let mut grid = vec![None; w * h];
        let mut alive = Vec::new();
        for (i, c) in app.scr.cells.iter().enumerate() {
            if c.w != 2 && c.ch != ' ' {
                grid[i] = Some((c.ch, c.fg));
                alive.push(i as u32);
            }
        }
        // 雨滴用日志自己的字：取最近几十行（去掉空格），竖着读也是一段日志
        let mut pool: Vec<Vec<char>> = app
            .logs
            .iter()
            .rev()
            .take(80)
            .map(|l| l.text.chars().filter(|c| !c.is_whitespace()).collect::<Vec<char>>())
            .filter(|v| v.len() > 8)
            .collect();
        if pool.is_empty() {
            pool.push("duck::core鸭鸭正在认真听你讲代码trace=0badf00d".chars().collect());
        }
        let n = if app.hi { 6 } else { 3 };
        let mut bugs = Vec::new();
        for m in app.moths.iter().filter(|m| m.alive).take(n) {
            let ph = app.rng.range(0.0, TAU);
            bugs.push(Bug { x: m.x, y: m.y, vx: m.vx * 0.3, vy: m.vy * 0.3, flap: m.flap, shade: m.shade, dye: 0.0, ph, tumble: 0.0 });
        }
        while bugs.len() < n {
            let x = app.rng.range(4.0, app.w() - 4.0);
            let y = app.hp() + app.rng.range(2.0, 20.0);
            let shade = app.rng.int(5) as u8;
            let ph = app.rng.range(0.0, TAU);
            bugs.push(Bug { x, y, vx: 0.0, vy: -20.0, flap: 0.0, shade, dye: 0.0, ph, tumble: 0.0 });
        }
        let ln = lanes_of(w);
        let mut r = Rain {
            t: 0.0,
            w,
            h,
            grid,
            alive,
            leak_acc: 0.0,
            leaked: 0,
            pool,
            drops: Vec::new(),
            trail: vec!['\0'; ln * h],
            cool: (0..ln).map(|_| app.rng.range(0.0, 1.5)).collect(),
            dyed: vec![false; ln],
            n_dyed: 0,
            grains: (0..ln).map(|_| Vec::new()).collect(),
            heap: vec!['\0'; ln * h],
            pile: vec![0; ln],
            bugs,
            knocks: 0,
            step: 0,
            stopped: false,
        };
        r.alive.reverse();
        app.set_title("鸭鸭.exe — 日志在漏水");
        r
    }

    /// 终端中途改了尺寸：缓冲重来（字符雨本身无状态可言，雨照下）
    fn refit(&mut self, app: &App) {
        let (w, h) = (app.scr.w, app.scr.h);
        if w == self.w && h == self.h {
            return;
        }
        let ln = lanes_of(w);
        self.w = w;
        self.h = h;
        self.grid = vec![None; w * h];
        self.alive.clear();
        self.drops.clear();
        self.trail = vec!['\0'; ln * h];
        self.cool = vec![0.5; ln];
        let all = self.t > T_DUCK1;
        self.dyed = vec![all; ln];
        self.grains = (0..ln).map(|_| Vec::new()).collect();
        self.heap = vec!['\0'; ln * h];
        self.pile = vec![0; ln];
    }

    fn lane_on(&self, app: &App, lane: usize) -> bool {
        app.hi || lane % 2 == 0
    }

    fn spawn(&mut self, app: &mut App, lane: usize, y: f32, fixed: Option<char>) {
        let cap = if app.hi { 420 } else { 170 };
        if self.drops.len() >= cap {
            return;
        }
        let r = &mut app.rng;
        let src = r.int(self.pool.len());
        let k = r.int(self.pool[src].len());
        let hs = (self.h as f32 / 45.0).clamp(0.45, 1.0);
        let (v, acc, vmax) = if fixed.is_some() { (r.range(2.0, 6.0), r.range(18.0, 34.0), r.range(16.0, 28.0)) } else { (r.range(13.0, 30.0), 0.0, 0.0) };
        let len = r.range(6.0, 20.0) * hs;
        let hr = y.floor() as i32;
        let start = if fixed.is_some() { hr } else { i32::MIN };
        self.drops.push(Drop { lane, y, v, acc, vmax, len, fixed, src, k, hr, start });
    }

    /// 雨头经过的地方，还挂在行里的字被冲走
    fn wash(&mut self, x: i32, row: i32) {
        if row < 0 || row >= self.h as i32 {
            return;
        }
        let w = self.w as i32;
        for xx in [x - 1, x, x + 1] {
            if xx < 0 || xx >= w {
                continue;
            }
            let i = (row * w + xx) as usize;
            if let Some((ch, _)) = self.grid[i] {
                if xx == x - 1 && char_w(ch) < 2 {
                    continue;
                }
                self.grid[i] = None;
                self.leaked += 1;
            }
        }
    }

    fn update_rain(&mut self, app: &mut App, dt: f32) {
        let (w, h) = (self.w, self.h);
        let ln = lanes_of(w);
        // 开场漏水：先是零星几滴，越漏越快；4.6 秒后剩下的字很快全掉光
        if !self.alive.is_empty() && self.t < T_STOP {
            let force = self.t >= 4.6;
            let per_s = if force { self.alive.len() as f32 * 2.5 } else { ln as f32 * (0.18 + 0.9 * smooth(0.3, 4.4, self.t)) };
            let fixed_cap = if app.hi { 200 } else { 70 };
            self.leak_acc += per_s * dt;
            let mut tries = 0;
            while self.leak_acc >= 1.0 && !self.alive.is_empty() && tries < 400 {
                tries += 1;
                let j = app.rng.int(self.alive.len());
                let i = self.alive[j] as usize;
                let Some((ch, _)) = self.grid[i] else {
                    self.alive.swap_remove(j);
                    continue;
                };
                let (x, y) = (i % w, i / w);
                let lane = x / 2;
                let n_fixed = self.drops.iter().filter(|d| d.fixed.is_some()).count();
                // 同一条雨道太挤、或已经有很多滴在掉时，这个字先留着（被雨冲走或晚点再掉）
                let ok = self.lane_on(app, lane) && n_fixed < fixed_cap && !self.drops.iter().any(|d| d.lane == lane && (d.hr - y as i32).abs() < 8);
                if !ok && !force {
                    continue;
                }
                self.leak_acc -= 1.0;
                self.alive.swap_remove(j);
                self.grid[i] = None;
                self.leaked += 1;
                if ok {
                    self.spawn(app, lane, y as f32, Some(ch));
                }
            }
            self.leak_acc = self.leak_acc.min(5.0);
        }
        // 从天上下的雨：密度在开场 1~5 秒里慢慢起来
        if self.t < T_STOP {
            let dens = smooth(1.0, 5.0, self.t);
            for lane in 0..ln {
                if !self.lane_on(app, lane) {
                    continue;
                }
                self.cool[lane] -= dt;
                if self.cool[lane] <= 0.0 {
                    self.cool[lane] = if app.hi { app.rng.range(0.6, 1.9) } else { app.rng.range(0.8, 2.4) };
                    if app.rng.f() < dens {
                        let y = -app.rng.range(0.0, 6.0);
                        self.spawn(app, lane, y, None);
                    }
                }
            }
        }
        // 雨滴往下走：头离开的格子留下一个字
        let mut i = 0;
        while i < self.drops.len() {
            let d = &mut self.drops[i];
            if d.acc > 0.0 {
                d.v = (d.v + d.acc * dt).min(d.vmax);
            }
            d.y += d.v * dt;
            let nr = d.y.floor() as i32;
            let (lane, x) = (d.lane, d.lane as i32 * 2);
            let mut rows = Vec::new();
            while d.hr < nr {
                let src = &self.pool[d.src];
                let ch = src[d.k % src.len()];
                d.k += 1;
                rows.push((d.hr, ch));
                d.hr += 1;
            }
            let dead = d.y - d.len > h as f32 + 1.0;
            for (r, ch) in rows {
                if r >= 0 && (r as usize) < h {
                    self.trail[lane * h + r as usize] = ch;
                }
                self.wash(x, r + 1);
            }
            if dead {
                self.drops.swap_remove(i);
            } else {
                i += 1;
            }
        }
    }

    /// 雨停：每滴雨（头 + 一截拖尾）拆成单个的字，自由落体堆到底部
    fn stop_rain(&mut self) {
        let h = self.h as i32;
        let drops = std::mem::take(&mut self.drops);
        for d in drops {
            let n = (d.len as i32).min(7);
            for k in 0..n {
                let r = d.hr - k;
                if r < d.start || r < -4 || r >= h {
                    continue;
                }
                let ch = if k == 0 {
                    match d.fixed {
                        Some(c) => c,
                        None => {
                            let s = &self.pool[d.src];
                            s[d.k % s.len()]
                        }
                    }
                } else if r >= 0 {
                    self.trail[d.lane * self.h + r as usize]
                } else {
                    '\0'
                };
                if ch != '\0' {
                    self.grains[d.lane].push((r as f32, d.v * 0.7, ch));
                }
            }
        }
        for g in self.grains.iter_mut() {
            g.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        }
        self.grid.iter_mut().for_each(|c| *c = None);
        self.alive.clear();
    }

    fn update_grains(&mut self, dt: f32, front_top: i32) {
        let h = self.h;
        for lane in 0..self.grains.len() {
            let gs = &mut self.grains[lane];
            let mut floor = h as i32 - 1 - self.pile[lane] as i32;
            let mut prev: Option<f32> = None;
            let mut i = 0;
            while i < gs.len() {
                let (y, v, ch) = gs[i];
                let v = v + 46.0 * dt;
                let mut ny = y + v * dt;
                if let Some(p) = prev {
                    ny = ny.min(p - 1.0);
                }
                if prev.is_none() && ny >= floor as f32 {
                    if floor >= 0 && floor < front_top {
                        self.heap[lane * h + floor as usize] = ch;
                        self.pile[lane] += 1;
                    }
                    floor -= 1;
                    gs.remove(i);
                    continue;
                }
                if ny.floor() as i32 >= front_top {
                    gs.remove(i);
                    continue;
                }
                gs[i] = (ny, v, ch);
                prev = Some(ny);
                i += 1;
            }
        }
    }

    fn update_bugs(&mut self, app: &mut App, dt: f32) {
        let (w, hp) = (app.w(), app.hp());
        let (dx, dy) = (app.duck.x, app.duck.y);
        let t = self.t;
        let raining = !self.stopped;
        let dens = smooth(0.5, 5.0, t);
        for i in 0..self.bugs.len() {
            let roll = app.rng.f();
            let kick = app.rng.range(50.0, 85.0);
            let b = &mut self.bugs[i];
            b.ph += dt;
            if raining {
                // 逆流往上游；越往上雨越猛，总会被冲下来
                let high = smooth(hp * 0.6, hp * 0.12, b.y);
                let p = dens * (0.25 + 1.6 * high) * dt;
                b.tumble -= dt;
                if b.tumble <= 0.0 && roll < p {
                    b.vy = kick;
                    b.tumble = 0.55;
                    self.knocks += 1;
                }
                if b.tumble <= 0.0 {
                    b.vy += (-26.0 - b.vy) * 2.8 * dt;
                } else {
                    b.vy += (10.0 - b.vy) * 1.2 * dt;
                }
                let mut ax = (b.ph * 1.7).sin() * 7.0;
                let (ex, ey) = (dx - b.x, dy - b.y);
                let dd = (ex * ex + ey * ey).sqrt();
                if dd < 70.0 && dd > 1.0 {
                    ax += ex / dd * 9.0;
                }
                b.vx += (ax - b.vx) * 2.0 * dt;
                let lane = (b.x.max(0.0) as usize / 2).min(self.dyed.len() - 1);
                if self.dyed[lane] {
                    b.dye = (b.dye + dt * 0.5).min(1.0);
                }
            } else {
                // 雨停了：围着鸭鸭的光转，身上的彩虹慢慢晾掉
                let tx = dx + (t * 1.1 + b.ph * 3.0).cos() * 28.0;
                let ty = dy - 10.0 + (t * 1.6 + b.ph * 2.0).sin() * 9.0;
                b.vx += ((tx - b.x) * 1.6 - b.vx) * 2.2 * dt;
                b.vy += ((ty - b.y) * 1.6 - b.vy) * 2.2 * dt;
                b.dye = (b.dye - dt * 0.4).max(0.0);
                b.tumble = 0.0;
            }
            b.x += b.vx * dt;
            b.y += b.vy * dt;
            if raining {
                if b.y > hp - 3.0 {
                    b.y = hp - 3.0;
                    b.vy = -30.0;
                    b.tumble = 0.0;
                }
                b.x = b.x.clamp(3.0, w - 3.0);
            }
            b.flap += dt * if b.tumble > 0.0 { 5.0 } else { 15.0 };
        }
    }
}

impl Interlude for Rain {
    fn name(&self) -> &'static str {
        "字符雨"
    }

    fn update(&mut self, app: &mut App, dt: f32) -> bool {
        self.t += dt;
        let t = self.t;
        self.refit(app);
        let (hw, _) = app.duck_half();
        let (w, hp) = (app.w(), app.hp());

        // ── 鸭鸭 ──
        if t < T_DUCK0 {
            app.duck.tx = -hw * 3.0;
        } else if t < T_STOP {
            let k = ((t - T_DUCK0) / (T_DUCK1 - T_DUCK0)).clamp(0.0, 1.0);
            let k = k * 0.85 + smooth(0.0, 1.0, k) * 0.15;
            let x = -hw * 1.4 + (w + hw * 2.8) * k;
            let y = (hp * 0.45 + (t * 1.2).sin() * hp * 0.1).clamp(hw * 0.8 + 2.0, hp - hw * 0.8 - 2.0);
            app.duck.x = x;
            app.duck.tx = x;
            app.duck.y = y;
            app.duck.ty = y;
            app.duck.right = true;
            // 鸭鸭碰到的雨柱染成彩虹，永久
            let (a, b) = (((x - hw * 0.95) / 2.0).floor(), ((x + hw * 0.95) / 2.0).ceil());
            for lane in a.max(0.0) as usize..(b.max(0.0) as usize).min(self.dyed.len()) {
                if !self.dyed[lane] {
                    self.dyed[lane] = true;
                    self.n_dyed += 1;
                }
            }
            if t > T_DUCK1 {
                for d in self.dyed.iter_mut().filter(|d| !**d) {
                    *d = true;
                    self.n_dyed += 1;
                }
            }
        } else {
            // 停在日志结尾那句话的右边，别挡住「雨停了，日志已晾干」
            app.duck.tx = (56.0 + hw).max(w * 0.6).min(w - hw * 0.8);
            app.duck.ty = app.lane_y();
        }

        // ── 事件 ──
        let ev = |s: &mut Self, k: usize, at: f32| -> bool {
            if s.step == k && s.t >= at {
                s.step += 1;
                true
            } else {
                false
            }
        };
        if ev(self, 0, 0.5) {
            app.quack("漏水了！", 1.3);
        }
        if ev(self, 1, T_DUCK0) {
            app.set_title("鸭鸭.exe — 彩虹雨");
        }
        if ev(self, 2, T_DUCK0 + 1.6) {
            app.quack("鸭鸭防水", 1.8);
        }
        if ev(self, 3, T_DUCK0 + 4.4) {
            app.quack("嘎", 0.9);
        }
        if ev(self, 4, T_STOP) {
            self.stopped = true;
            self.stop_rain();
            app.set_title("鸭鸭.exe — 雨停了");
        }
        if ev(self, 5, T_BUILD - 0.05) {
            app.push(log_line("WARN", "rain::leak", &format!("日志漏水 {} 字，已全部回收", self.leaked), Kind::Info));
            app.push(log_line("INFO", "duck::dye", &format!("{} 根雨柱已染成彩虹（鸭鸭：顺路）", self.n_dyed), Kind::Info));
            app.push(log_line("WARN", "bug::swim", &format!("飞蛾逆流而上 {} 次，进度 0%", self.knocks.max(3)), Kind::Info));
            app.push(log_line("INFO", "weather::rain", "雨停了，日志已晾干", Kind::Info));
            app.set_title("鸭鸭.exe — 晾日志");
            app.quack("收日志咯", 1.4);
        }
        if ev(self, 6, T_BUILD + BUILD_LEN + 0.3) {
            app.quack("晾干了，嘎", 1.3);
        }

        // ── 雨 / 字堆 ──
        if !self.stopped {
            self.update_rain(app, dt);
        } else {
            let ft = self.front_top();
            self.update_grains(dt, ft);
        }
        self.update_bugs(app, dt);

        if t >= T_END {
            // 飞蛾交还主线（位置接上，不闪）
            app.moths.clear();
            for b in &self.bugs {
                app.moths.push(Moth { x: b.x, y: b.y, vx: b.vx, vy: b.vy, flap: b.flap, shade: b.shade, tx: b.x, ty: b.y, delay: 0.0, color: 0.0, alive: true });
            }
            return true;
        }
        false
    }

    fn draw(&mut self, app: &mut App) {
        let (w, h) = (self.w, self.h);
        if app.scr.w != w || app.scr.h != h {
            return;
        }
        let pt = app.pal_t();
        let ft = self.front_top();

        // 1. 堆回来的日志（从底往上一层层），刚落好的那一层闪一下白
        if ft < h as i32 {
            app.draw_logs(9.0, 0, 0.0);
            for y in 0..(ft.max(0) as usize) {
                app.scr.cells[y * w..(y + 1) * w].fill(BLANK);
            }
            if app.hi {
                let per = BUILD_LEN / h as f32;
                for y in ft.max(0) as usize..h {
                    let age = self.t - (T_BUILD + (h - 1 - y) as f32 * per);
                    let k = 1.0 - age / 0.15;
                    if k > 0.0 {
                        for c in app.scr.cells[y * w..(y + 1) * w].iter_mut() {
                            if c.ch != ' ' {
                                c.fg = Col::T(mix(resolve(c.fg, pt), WHITE, k * 0.8));
                            }
                        }
                    }
                }
            }
        }

        // 2. 还挂在行里的日志字
        if !self.alive.is_empty() {
            for (i, c) in self.grid.iter().enumerate() {
                if let Some((ch, col)) = *c {
                    app.scr.put_c((i % w) as i32, (i / w) as i32, ch, col, None);
                }
            }
        }

        // 3. 地上的字堆（彩虹，静止）
        if self.stopped {
            for lane in 0..self.pile.len() {
                for k in 0..self.pile[lane] {
                    let y = h - 1 - k;
                    if (y as i32) >= ft {
                        continue;
                    }
                    let ch = self.heap[lane * h + y];
                    if ch != '\0' {
                        let x = lane as i32 * 2;
                        app.scr.put_c(x, y as i32, ch, rb(Band::Text, x as f32, y as f32), None);
                    }
                }
                for &(gy, _, ch) in &self.grains[lane] {
                    let y = gy.floor() as i32;
                    if y >= 0 && y < ft {
                        let x = lane as i32 * 2;
                        app.scr.put_c(x, y, ch, rb(Band::Text, x as f32, y as f32), None);
                    }
                }
            }
        }

        // 4. 雨
        for d in &self.drops {
            let x = d.lane as i32 * 2;
            let rainbow = self.dyed[d.lane];
            let n = d.len as i32;
            for k in 0..=n {
                let r = d.hr - k;
                if r < d.start {
                    break;
                }
                if r < 0 || r >= h as i32 {
                    continue;
                }
                let ch = if k == 0 {
                    match d.fixed {
                        Some(c) => c,
                        None => {
                            let s = &self.pool[d.src];
                            s[d.k % s.len()]
                        }
                    }
                } else {
                    self.trail[d.lane * h + r as usize]
                };
                if ch == '\0' {
                    continue;
                }
                let col: Col = if k == 0 {
                    WHITE.into()
                } else if rainbow {
                    rb(if k <= 6 { Band::Text } else { Band::Dim }, x as f32, r as f32)
                } else {
                    Col::T(match k {
                        1..=2 => [196, 196, 196],
                        3..=6 => [132, 132, 132],
                        7..=11 => [84, 84, 84],
                        _ => [50, 50, 50],
                    })
                };
                app.scr.put_c(x, r, ch, col, None);
            }
        }

        // 5. 大鸭鸭 + 飞蛾（像素层）
        app.draw_glow(app.duck.glow);
        app.draw_duck(None);
        for b in &self.bugs {
            let rbw = if app.hi { b.dye } else { (b.dye * 4.0).round() / 4.0 };
            moth(app, b.x, b.y, b.shade, b.flap, rbw);
        }
    }
}

impl Rain {
    /// 堆回日志的进度：这一行及以下已经是日志
    fn front_top(&self) -> i32 {
        if self.t < T_BUILD {
            return self.h as i32;
        }
        let k = ((self.t - T_BUILD) / BUILD_LEN).clamp(0.0, 1.0);
        self.h as i32 - (k * self.h as f32).ceil() as i32
    }
}
