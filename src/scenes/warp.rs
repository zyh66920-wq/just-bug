//! 插曲：星空跃迁 —— 经典「星空飞行」屏保 + 「跳进超空间」的通用意象。
//!
//! 日志的字从屏幕中心往外散成星星 → 缓慢星空飞行，鸭鸭当领航员、6 只 bug 排成 V 字编队当僚机 →
//! 倒计时 3·2·1 → 星星拉成放射光线、白蓝转彩虹 → 白光一闪跳出超空间 → 回到原来的日志：
//! 「当前位置 = 127.0.0.1（共跃迁 0 光年）」，随行 bug 一只没甩掉。
//!
//! 性能：星星数随屏幕面积缩放并封顶；亮度、颜色都分档量化（不动的星星零成本）；
//! 光线拖尾封顶字数；彩虹用调色板色（按到中心的距离取色，调色板流动 = 光圈往外涌，免费）。
use super::*;

const T_CRUISE: f32 = 2.0; // 日志散成星星
const T_COUNT: f32 = 7.5; // 倒计时 3·2·1
const T_WARP: f32 = 10.5; // 跃迁
const T_DECEL: f32 = 14.0; // 光线缩回
const T_FLASH: f32 = 14.4; // 白光一闪，回到日志
const T_END: f32 = 18.6;

const WHITE_BLUE: Rgb = [168, 198, 255];
const DIGITS: [[&str; 7]; 3] = [
    ["..#..", ".##..", "..#..", "..#..", "..#..", "..#..", ".###."],
    [".###.", "#...#", "....#", "..##.", ".#...", "#....", "#####"],
    [".###.", "#...#", "....#", "..##.", "....#", "#...#", ".###."],
];
struct Star {
    x: f32,
    y: f32,
    z: f32,
    /// 来自日志的字：原字符与原颜色；None = 普通星星
    log: Option<(char, Col)>,
    /// 日志字在屏幕上的原位置（未被擦除波扫到之前不动）
    home: (f32, f32),
    released: bool,
    age: f32,
}

struct Wing {
    x: f32,
    y: f32,
    flap: f32,
    shade: u8,
    slot: Option<(f32, f32)>, // 编队位置（相对鸭鸭，字符 x / 像素 y）；None = 多出来的，飞走
}

pub struct Warp {
    stars: Vec<Star>,
    n_star: usize,
    wings: Vec<Wing>,
    doppler: usize,
    rainbow: f32, // 飞蛾被跃迁染上的彩虹程度
}

fn hash(a: u32, b: u32) -> u32 {
    let mut h = a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0xC2B2_AE3D);
    h ^ (h >> 13)
}

impl Warp {
    pub fn new(app: &mut App) -> Self {
        let (w, h) = (app.scr.w, app.scr.h);
        let area = w * h;
        let n_star = (area / 45).clamp(24, 340);
        let mut stars = Vec::with_capacity(n_star + 700);
        for _ in 0..n_star {
            let z = app.rng.range(0.08, 1.0);
            let (x, y) = (app.rng.range(-1.0, 1.0), app.rng.range(-1.0, 1.0));
            stars.push(Star { x, y, z, log: None, home: (0.0, 0.0), released: true, age: 9.0 });
        }
        // 从当前日志画面里抽一部分字，等擦除波扫过时变成往外飞的星星
        app.scr.clear();
        app.draw_logs(9.0, 0, 0.0);
        let cap = (area / 22).min(if app.hi { 700 } else { 280 });
        let lit: Vec<usize> = (0..area).filter(|&i| {
            let c = app.scr.cells[i];
            c.ch != ' ' && c.w != 2
        }).collect();
        let p = cap as f32 / lit.len().max(1) as f32;
        for &i in &lit {
            if app.rng.f() >= p {
                continue;
            }
            let c = app.scr.cells[i];
            let (sx, sy) = ((i % w) as f32 + 0.5, (i / w) as f32 + 0.5);
            stars.push(Star { x: 0.0, y: 0.0, z: 1.0, log: Some((c.ch, c.fg)), home: (sx, sy), released: false, age: 0.0 });
        }
        app.scr.clear();
        // 现有的飞蛾：前 6 只进编队，多的飞走；不够 6 只从左边补
        let mut wings = Vec::new();
        for (i, m) in app.moths.iter().enumerate() {
            wings.push(Wing { x: m.x, y: m.y, flap: m.flap, shade: m.shade, slot: if i < 6 { Some((0.0, 0.0)) } else { None } });
        }
        while wings.len() < 6 {
            let y = app.rng.range(0.2, 0.8) * app.hp();
            let shade = app.rng.int(5) as u8;
            wings.push(Wing { x: -4.0 - app.rng.range(0.0, 20.0), y, flap: app.rng.range(0.0, 2.0), shade, slot: Some((0.0, 0.0)) });
        }
        app.duck.visible = true;
        app.duck.asleep = false;
        app.set_title("鸭鸭.exe — 星际领航");
        Warp { stars, n_star, wings, doppler: 0, rainbow: 0.0 }
    }

    fn speed(pt: f32) -> f32 {
        if pt < T_CRUISE {
            0.14 + 0.85 * (1.0 - smooth(0.0, T_CRUISE, pt))
        } else if pt < T_WARP {
            0.14 + 0.16 * smooth(T_COUNT, T_WARP, pt)
        } else if pt < T_DECEL {
            0.3 + 4.0 * smooth(T_WARP, T_WARP + 1.6, pt)
        } else {
            4.3 * (1.0 - smooth(T_DECEL, T_FLASH - 0.05, pt)) + 0.05
        }
    }

    /// 拖尾长度（深度单位）：巡航时为 0（点），倒计时开始微微拉长，跃迁时拉成光线，减速时缩回
    fn trail(pt: f32) -> f32 {
        if pt < T_COUNT + 1.5 {
            0.0
        } else {
            Self::speed(pt) * 0.11
        }
    }

    /// 擦除波半径（视觉单位：x 一格 = 1，y 一格 = 2）
    fn wipe(app: &App, pt: f32) -> f32 {
        let (cx, cy) = (app.w() / 2.0, app.scr.h as f32 / 2.0);
        let max_r = (cx * cx + 4.0 * cy * cy).sqrt() + 3.0;
        max_r * smooth(0.0, 1.8, pt).powf(1.3)
    }

    /// 鸭鸭驾驶位（像素坐标）：消失点正下方，留出头顶的星空
    fn cockpit(app: &App) -> (f32, f32) {
        let (_, hh) = app.duck_half();
        let hp = app.hp();
        // 跳出超空间后挪到上半屏，把底部的「跃迁报告」日志让出来
        let y = if app.pt >= T_FLASH { hp * 0.36 } else { hp / 2.0 + hh + 4.0 };
        let y = y.min(hp - hh - 1.0).max(hh + 1.0);
        (app.w() / 2.0, y)
    }

    /// V 字编队：从鸭鸭尾巴往左后方张开，上下各 3 只
    fn slot(app: &App, k: usize) -> (f32, f32) {
        let s = app.s() as f32;
        let (hw, hh) = app.duck_half();
        let (_, dy) = Self::cockpit(app);
        let i = (k / 2 + 1) as f32;
        let side = if k % 2 == 0 { -1.0 } else { 1.0 };
        let ox = -(hw * 0.55 + 7.5 * s * i);
        let oy = hh * 0.35 + 5.0 * s * i;
        let room = if side < 0.0 { dy - 3.0 } else { app.hp() - 3.0 - dy };
        let k = (room / (hh * 0.35 + 15.0 * s)).min(1.0).max(0.2);
        (ox, side * oy * k)
    }
}

impl Interlude for Warp {
    fn name(&self) -> &'static str {
        "星空跃迁"
    }

    fn update(&mut self, app: &mut App, dt: f32) -> bool {
        let pt = app.pt;
        let (w, h) = (app.w(), app.scr.h as f32);
        let (cx, cy) = (w / 2.0, h / 2.0);

        // ── 剧本 ──
        if app.ev(0, 2.4) {
            app.quack("嘎！领航员就位", 1.8);
        }
        if app.ev(1, 4.6) {
            app.quack("僚机 bug 全员到齐", 1.8);
        }
        if app.ev(2, 6.4) {
            app.quack("坐稳了", 1.0);
        }
        for (k, n) in [(3usize, 3), (4, 2), (5, 1)] {
            if app.ev(k, T_COUNT + (3 - n) as f32) {
                app.set_title(&format!("跃迁倒计时 {}…", n));
            }
        }
        if app.ev(6, T_WARP) {
            app.set_title("跃迁中 >>>>>>>>");
        }
        if pt >= T_WARP && pt < T_DECEL {
            // 多普勒嘎：越跑越长
            let n = (((pt - T_WARP) / 0.32) as usize + 1).min(10);
            if n != self.doppler {
                self.doppler = n;
                app.quack(&format!("嘎{}", "～".repeat(n)), T_DECEL - 0.1 - pt);
            }
        }
        let mut snap = false;
        if app.ev(7, T_FLASH) {
            // 屏幕正白着：鸭鸭和编队瞬移到新位置，看不出跳变
            snap = true;
            app.duck.bubble = None;
            app.push(log_line("INFO", "warp::drive", "跃迁完成：当前位置 = 127.0.0.1（共跃迁 0 光年）", Kind::Fixed));
            app.push(log_line("WARN", "warp::nav", "领航员把目的地填成了 localhost", Kind::Info));
            app.set_title("跃迁完成（0 光年）");
        }
        if app.ev(8, T_FLASH + 0.7) {
            app.quack("嘎？", 2.0);
        }
        if app.ev(9, T_FLASH + 2.0) {
            app.push(log_line("INFO", "bug::radar", "随行 bug 6/6，一只都没甩掉", Kind::Info));
        }

        // ── 星星 ──
        let v = Self::speed(pt);
        let r = Self::wipe(app, pt);
        for st in self.stars.iter_mut() {
            if !st.released {
                let (ex, ey) = (st.home.0 - cx, (st.home.1 - cy) * 2.0);
                if (ex * ex + ey * ey).sqrt() < r {
                    // 被擦除波扫到：把屏幕位置反投影回 3D，从这里开始往外飞
                    let z = 0.55 + hash(st.home.0 as u32, st.home.1 as u32) as f32 / u32::MAX as f32 * 0.35;
                    st.x = (st.home.0 - cx) / cx * z;
                    st.y = (st.home.1 - cy) / cy * z;
                    st.z = z;
                    st.released = true;
                }
                continue;
            }
            st.age += dt;
            st.z -= v * dt;
            let (sx, sy) = (cx + st.x / st.z * cx, cy + st.y / st.z * cy);
            if st.z < 0.02 || (sx - cx).abs() > cx + 1.0 || (sy - cy).abs() > cy + 1.0 {
                if st.log.is_some() {
                    st.z = -1.0; // 标记删除
                    continue;
                }
                st.x = app.rng.range(-1.0, 1.0);
                st.y = app.rng.range(-1.0, 1.0);
                st.z = if pt < T_WARP { app.rng.range(0.92, 1.0) } else { app.rng.range(0.7, 1.0) };
                st.age = 0.0;
            }
        }
        self.stars.retain(|s| s.z > -0.5);

        // ── 鸭鸭：飞到驾驶位，面朝右 ──
        let (dx, dy) = Self::cockpit(app);
        app.duck.tx = dx;
        app.duck.ty = dy;
        if snap {
            app.duck.x = dx;
            app.duck.y = dy;
        }
        app.duck.visible = true;
        app.duck.asleep = false;
        if pt > 2.2 {
            app.duck.right = true;
        }

        // ── 僚机 ──
        let mut k = 0;
        let (ddx, ddy) = (app.duck.x, app.duck.y);
        let slots: Vec<(f32, f32)> = (0..6).map(|i| Self::slot(app, i)).collect();
        for wg in self.wings.iter_mut() {
            wg.flap += dt * if pt >= T_WARP && pt < T_FLASH { 16.0 } else { 9.0 };
            match wg.slot {
                Some(_) => {
                    let (ox, oy) = slots[k.min(5)];
                    let bob = (app.t * 2.0 + k as f32 * 1.3).sin() * 1.2;
                    let (tx, ty) = (ddx + ox, ddy + oy + bob);
                    let follow = if pt < 1.0 { 0.6 } else { 2.0 };
                    let kk = if snap { 1.0 } else { (follow * dt).min(1.0) };
                    wg.x += (tx - wg.x) * kk;
                    wg.y += (ty - wg.y) * kk;
                    k += 1;
                }
                None => {
                    // 多出来的 bug：从中心往外散开飞走
                    let (ex, ey) = (wg.x - cx, wg.y - cy * 2.0);
                    let d = (ex * ex + ey * ey).sqrt().max(1.0);
                    wg.x += ex / d * 40.0 * dt;
                    wg.y += ey / d * 40.0 * dt;
                }
            }
        }
        let (wpx, hpx) = (w, app.hp());
        self.wings.retain(|wg| wg.slot.is_some() || (wg.x > -6.0 && wg.x < wpx + 6.0 && wg.y > -6.0 && wg.y < hpx + 6.0));
        // 跃迁时 bug 被染成彩虹，出来后慢慢褪色
        let target = if pt >= T_WARP && pt < T_FLASH { 0.75 } else { 0.0 };
        let rate = if target > self.rainbow { 0.6 } else { 0.45 };
        self.rainbow += (target - self.rainbow).clamp(-rate * dt, rate * dt);

        if pt >= T_END {
            // 把编队交还给主线（飞蛾从原地继续）
            app.moths.clear();
            for wg in self.wings.iter().filter(|w| w.slot.is_some()) {
                app.moths.push(Moth { x: wg.x, y: wg.y, vx: 0.0, vy: 0.0, flap: wg.flap % 2.0, shade: wg.shade, tx: wg.x, ty: wg.y, delay: 0.0, color: 0.0, alive: true });
            }
            return true;
        }
        false
    }

    fn draw(&mut self, app: &mut App) {
        let pt = app.pt;
        let hi = app.hi;
        let (w, h) = (app.w(), app.scr.h as f32);
        let (cx, cy) = (w / 2.0, h / 2.0);
        let pal_t = app.pal_t();

        // ── 背景：开场的日志 + 擦除波；结尾回到日志 ──
        if pt < T_CRUISE {
            app.draw_logs(9.0, 0, 0.0);
            let r = Self::wipe(app, pt);
            let (sw, sh) = (app.scr.w as i32, app.scr.h as i32);
            for y in 0..sh {
                let ey = (y as f32 + 0.5 - cy) * 2.0;
                for x in 0..sw {
                    let ex = x as f32 + 0.5 - cx;
                    let d = (ex * ex + ey * ey).sqrt();
                    if d < r {
                        app.scr.put_c(x, y, ' ', Col::T(GRAY), Some(Col::T(BLACK)));
                    } else if hi && d < r + 2.5 {
                        // 波前一圈字被照亮
                        let i = (y * sw + x) as usize;
                        let c = app.scr.cells[i];
                        if c.ch != ' ' {
                            let k = 1.0 - (d - r) / 2.5;
                            app.scr.cells[i].fg = Col::T(mix(resolve(c.fg, pal_t), WHITE, (k * 4.0).round() / 4.0));
                        }
                    }
                }
            }
        } else if pt >= T_FLASH {
            app.draw_logs(9.0, 0, 0.0);
        }

        // ── 星星 / 光线 ──
        if pt < T_FLASH {
            let r = Self::wipe(app, pt);
            let trail = Self::trail(pt);
            let cap = if hi { 16 } else { 7 };
            let rb_k = smooth(T_WARP + 0.4, T_WARP + 2.2, pt);
            let rb_k = if hi { (rb_k * 8.0).round() / 8.0 } else if rb_k > 0.5 { 1.0 } else { 0.0 };
            let limit = if hi { usize::MAX } else { self.n_star / 2 };
            for (i, st) in self.stars.iter().enumerate() {
                if !st.released || (st.log.is_none() && i >= limit) {
                    continue;
                }
                let (hx, hy) = (cx + st.x / st.z * cx, cy + st.y / st.z * cy);
                if st.log.is_none() && pt < T_CRUISE {
                    let (ex, ey) = (hx - cx, (hy - cy) * 2.0);
                    if (ex * ex + ey * ey).sqrt() > r {
                        continue;
                    }
                }
                let near = ((1.0 - st.z) * 1.3).clamp(0.0, 1.0);
                // 日志字：先保持原样飞一小段，再变成星星
                if let Some((ch, col)) = st.log {
                    if st.age < 0.35 {
                        app.scr.put_c(hx as i32, hy as i32, ch, col, None);
                        continue;
                    } else if st.age < 0.6 {
                        app.scr.put_c(hx as i32, hy as i32, '*', Col::T(WHITE), None);
                        continue;
                    }
                }
                let (tx, ty) = if trail > 0.0 {
                    let z2 = (st.z + trail).min(1.2);
                    (cx + st.x / z2 * cx, cy + st.y / z2 * cy)
                } else {
                    (hx, hy)
                };
                let (mut ddx, mut ddy) = (tx - hx, ty - hy);
                let mut n = ddx.abs().max(ddy.abs()).round() as i32;
                if n > cap {
                    let k = cap as f32 / n as f32;
                    ddx *= k;
                    ddy *= k;
                    n = cap;
                }
                if n < 1 {
                    // 点状星星：近大远小、近亮远暗（亮度分档）
                    let ch = match near {
                        b if b > 0.8 => '*',
                        b if b > 0.55 => '+',
                        b if b > 0.25 => '·',
                        _ => '.',
                    };
                    let lv = if hi { (near * 6.0).round() / 6.0 } else { (near * 3.0).round() / 3.0 };
                    let c = scale(mix(WHITE_BLUE, WHITE, lv), 0.5 + 0.5 * lv);
                    let col = if rb_k >= 1.0 { ring(hx, hy, cx, cy, false) } else { Col::T(c) };
                    app.scr.put_c(hx as i32, hy as i32, ch, col, None);
                    continue;
                }
                let a = (2.0 * ddy).atan2(ddx).rem_euclid(std::f32::consts::PI);
                let pi8 = std::f32::consts::PI / 8.0;
                let ch = if a < pi8 || a >= 7.0 * pi8 {
                    '-'
                } else if a < 3.0 * pi8 {
                    '\\'
                } else if a < 5.0 * pi8 {
                    '|'
                } else {
                    '/'
                };
                let lv = if hi { (near * 4.0).round() / 4.0 } else { 1.0 };
                for j in 0..=n {
                    let f = j as f32 / n as f32;
                    let (px, py) = (hx + ddx * f, hy + ddy * f);
                    let tail = f > 0.5;
                    let col = if rb_k >= 1.0 {
                        ring(px, py, cx, cy, tail)
                    } else {
                        let base = mix(WHITE_BLUE, WHITE, lv);
                        let base = if rb_k > 0.0 { mix(base, resolve(ring(px, py, cx, cy, false), pal_t), rb_k) } else { base };
                        Col::T(scale(base, if tail { 0.45 } else { 0.6 + 0.4 * lv }))
                    };
                    app.scr.put_c(px as i32, py as i32, if j == 0 && n > 2 { '*' } else { ch }, col, None);
                }
            }
        }

        // ── 跳出前：中心的出口白光迅速张开 ──
        if pt >= T_DECEL && pt < T_FLASH {
            let k = ((pt - T_DECEL) / (T_FLASH - T_DECEL)).clamp(0.0, 1.0);
            let k = if hi { k } else { (k * 5.0).floor() / 5.0 };
            let r = (cx * cx + 4.0 * cy * cy).sqrt() * 0.45 * k * k * k;
            if r > 0.5 {
                let (x0, x1) = ((cx - r) as i32 - 1, (cx + r) as i32 + 1);
                let (y0, y1) = ((cy - r / 2.0) as i32 - 1, (cy + r / 2.0) as i32 + 1);
                for y in y0..=y1 {
                    for x in x0..=x1 {
                        let (ex, ey) = (x as f32 + 0.5 - cx, (y as f32 + 0.5 - cy) * 2.0);
                        let d = (ex * ex + ey * ey).sqrt() / r;
                        if d < 1.0 {
                            let q = ((1.0 - d) * 4.0).ceil() / 4.0;
                            if let Some(bg) = app.scr.bg_at(x, y) {
                                *bg = Col::T(mix(WHITE_BLUE, WHITE, q));
                            }
                        }
                    }
                }
            }
        }

        // ── 僚机 + 鸭鸭（像素层） ──
        let warp = pt >= T_WARP && pt < T_FLASH;
        let tint = if hi { self.rainbow } else { (self.rainbow * 3.0).round() / 3.0 };
        for (i, wg) in self.wings.iter().enumerate() {
            // 跃迁时 bug 们被气流吹得一抖一抖（满效果才抖，每秒 12 下）
            let shake = if warp && hi { ((hash((app.t * 12.0) as u32, i as u32) % 3) as f32 - 1.0) * 0.6 } else { 0.0 };
            moth(app, wg.x + shake, wg.y, wg.shade, wg.flap, tint);
        }
        app.draw_duck(None);

        // ── 倒计时大字 ──
        if pt >= T_COUNT && pt < T_WARP {
            let n = (3 - ((pt - T_COUNT) as i32).min(2)) as usize; // 3,2,1
            let g = &DIGITS[n - 1];
            let s = app.s();
            let gw = 5 * s;
            let top = ((cy * 2.0) * 0.42 - (7 * s) as f32).max(2.0) as i32;
            let frac = (pt - T_COUNT).fract();
            let c = if hi { mix([255, 230, 120], WHITE_BLUE, frac) } else { [255, 230, 120] };
            blit_glyph(&mut app.layer, g, cx as i32 - gw / 2, top, s, c);
        }
    }

    fn draw_over(&mut self, app: &mut App) {
        let pt = app.pt;
        let hi = app.hi;
        let (w, h) = (app.scr.w as i32, app.scr.h as i32);
        let cx = w / 2;
        // HUD：底部一行仪表
        if pt >= 3.0 && pt < T_FLASH {
            let full = "领航员：鸭鸭  │  僚机：bug ×6  │  目的地：127.0.0.1  │  燃料：彩虹 100%";
            let mid = "目的地：127.0.0.1  │  燃料：彩虹";
            let short = "→ 127.0.0.1";
            let s = [full, mid, short].into_iter().find(|s| (str_w(s) as i32) + 4 <= w).unwrap_or(short);
            let col = if pt >= T_WARP { [255, 200, 120] } else { [120, 170, 210] };
            app.scr.text(cx - str_w(s) as i32 / 2, h - 1, s, col, Some(BLACK));
        }
        if pt >= T_COUNT && pt < T_WARP {
            let s = app.s();
            let top = ((h as f32) * 0.42 - (7 * s) as f32).max(2.0) as i32 / 2;
            let label = "跃迁倒计时";
            app.scr.text(cx - str_w(label) as i32 / 2, (top - 2).max(0), label, [200, 210, 230], Some(BLACK));
        }
        // 白光一闪：全白 0.1 秒，再淡出露出日志
        if pt >= T_FLASH && pt < T_FLASH + 0.8 {
            let f = 1.0 - smooth(T_FLASH + 0.08, T_FLASH + 0.8, pt);
            let f = if hi { (f * 14.0).round() / 14.0 } else { (f * 4.0).round() / 4.0 };
            if f <= 0.0 {
                return;
            }
            let pal_t = app.pal_t();
            for c in app.scr.cells.iter_mut() {
                if f >= 0.99 {
                    c.ch = ' ';
                    c.w = 0;
                    c.bg = Col::T(WHITE);
                    c.fg = Col::T(WHITE);
                } else {
                    c.bg = Col::T(mix(resolve(c.bg, pal_t), WHITE, f));
                    c.fg = Col::T(mix(resolve(c.fg, pal_t), WHITE, f));
                }
            }
        }
    }
}

/// 彩虹光圈：按到中心的视觉距离取调色板色；调色板一流动，整片光线的彩虹就往外涌（零带宽）
fn ring(x: f32, y: f32, cx: f32, cy: f32, dim: bool) -> Col {
    let (ex, ey) = (x - cx, (y - cy) * 2.0);
    let d = (ex * ex + ey * ey).sqrt();
    let idx = ((d * 0.45) as u32 % PN as u32) as u8;
    Col::P(if dim { Band::Dim as u8 } else { Band::Text as u8 } + idx)
}
