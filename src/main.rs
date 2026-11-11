//! 鸭鸭与 Bug —— 一个毫无用处的终端屏保。
//! 双击运行：自动拉起全屏 Windows Terminal；任意键退出。
//! `--here` 在当前终端里直接跑；`--dump 文件.html --at 1,2,3 [--size 200x56] [--ending 0-3] [--seed N]`
//! 离线把指定时刻的画面渲染成网页（开发自查用）。
//!
//! 底线：只在自己窗口里演，不碰文件、不改系统；任意键随时退出。

use std::fmt::Write as _;
use std::io::Write;
use std::time::{Duration, Instant};

// ───────────────────────── 最小 Win32 控制台接口（零依赖） ─────────────────────────
#[allow(non_snake_case)]
mod win {
    pub type Handle = *mut core::ffi::c_void;
    pub const STD_INPUT: u32 = -10i32 as u32;
    pub const STD_OUTPUT: u32 = -11i32 as u32;
    #[repr(C)]
    #[derive(Default)]
    pub struct Csbi {
        pub size: [i16; 2],
        pub cursor: [i16; 2],
        pub attr: u16,
        pub win: [i16; 4],
        pub max: [i16; 2],
    }
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct InputRecord {
        pub kind: u16,
        pub _pad: u16,
        pub ev: [u32; 4],
    }
    #[repr(C)]
    #[derive(Default)]
    pub struct SystemTime {
        pub y: u16,
        pub mo: u16,
        pub dow: u16,
        pub d: u16,
        pub h: u16,
        pub mi: u16,
        pub s: u16,
        pub ms: u16,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        pub fn GetStdHandle(n: u32) -> Handle;
        pub fn GetConsoleMode(h: Handle, m: *mut u32) -> i32;
        pub fn SetConsoleMode(h: Handle, m: u32) -> i32;
        pub fn GetConsoleScreenBufferInfo(h: Handle, i: *mut Csbi) -> i32;
        pub fn GetNumberOfConsoleInputEvents(h: Handle, n: *mut u32) -> i32;
        pub fn ReadConsoleInputW(h: Handle, buf: *mut InputRecord, len: u32, read: *mut u32) -> i32;
        pub fn GetLocalTime(t: *mut SystemTime);
    }

    pub struct Console {
        hin: Handle,
        hout: Handle,
        min: u32,
        mout: u32,
    }
    impl Console {
        pub fn open() -> Console {
            unsafe {
                let (hin, hout) = (GetStdHandle(STD_INPUT), GetStdHandle(STD_OUTPUT));
                let (mut min, mut mout) = (0, 0);
                GetConsoleMode(hin, &mut min);
                GetConsoleMode(hout, &mut mout);
                SetConsoleMode(hout, mout | 0x1 | 0x4 | 0x8);
                SetConsoleMode(hin, 0x80);
                Console { hin, hout, min, mout }
            }
        }
        pub fn size(&self) -> (usize, usize) {
            let mut i = Csbi::default();
            unsafe {
                if GetConsoleScreenBufferInfo(self.hout, &mut i) == 0 {
                    return (120, 30);
                }
            }
            ((i.win[2] - i.win[0] + 1).max(10) as usize, (i.win[3] - i.win[1] + 1).max(5) as usize)
        }
        pub fn key_pressed(&self) -> bool {
            let mut pressed = false;
            unsafe {
                loop {
                    let mut n = 0;
                    if GetNumberOfConsoleInputEvents(self.hin, &mut n) == 0 || n == 0 {
                        break;
                    }
                    let mut buf = [InputRecord::default(); 16];
                    let mut got = 0;
                    if ReadConsoleInputW(self.hin, buf.as_mut_ptr(), 16, &mut got) == 0 {
                        break;
                    }
                    for r in &buf[..got as usize] {
                        if r.kind == 1 && r.ev[0] != 0 {
                            pressed = true;
                        }
                    }
                }
            }
            pressed
        }
        pub fn restore(&self) {
            unsafe {
                SetConsoleMode(self.hin, self.min);
                SetConsoleMode(self.hout, self.mout);
            }
        }
    }
    pub fn now_hms() -> String {
        let mut t = SystemTime::default();
        unsafe { GetLocalTime(&mut t) };
        format!("{:02}:{:02}:{:02}.{:03}", t.h, t.mi, t.s, t.ms)
    }
}

type Rgb = [u8; 3];
const BLACK: Rgb = [0, 0, 0];
const WHITE: Rgb = [255, 255, 255];
const GRAY: Rgb = [170, 170, 170];
const GREEN: Rgb = [90, 230, 120];
const RED: Rgb = [255, 70, 70];
const TAU: f32 = std::f32::consts::TAU;
/// 黑洞段粒子上限：每个移动的粒子都要往终端写字节
const PARTICLE_CAP: usize = 3300;

// ───────────────────────── 随机数 ─────────────────────────
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn f(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f()
    }
    fn int(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

// ───────────────────────── 颜色 ─────────────────────────
fn mix(a: Rgb, b: Rgb, k: f32) -> Rgb {
    let k = k.clamp(0.0, 1.0);
    [0, 1, 2].map(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * k) as u8)
}
fn scale(a: Rgb, k: f32) -> Rgb {
    [0, 1, 2].map(|i| (a[i] as f32 * k.clamp(0.0, 1.0)) as u8)
}
fn heat(i: f32) -> Rgb {
    const S: [(f32, Rgb); 5] = [
        (0.0, [50, 6, 4]),
        (0.3, [190, 45, 10]),
        (0.6, [255, 140, 35]),
        (0.85, [255, 220, 130]),
        (1.0, [255, 255, 245]),
    ];
    let i = i.clamp(0.0, 1.0);
    for w in S.windows(2) {
        if i <= w[1].0 {
            return mix(w[0].1, w[1].1, (i - w[0].0) / (w[1].0 - w[0].0));
        }
    }
    S[4].1
}
fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// ───────────────────────── 调色板彩虹 ─────────────────────────
// 性能核心：彩虹字只写一次，用 256 色调色板编号上色；彩虹「流动」靠每秒改 10 次调色板（144 个颜色，约 4KB），
// 终端会把屏幕上所有同编号的字原地变色，不用重写任何一个字。退出时 OSC 104 恢复原调色板。
#[derive(Clone, Copy, PartialEq, Debug)]
enum Col {
    T(Rgb), // 真彩色
    P(u8),  // 调色板编号
}
impl From<Rgb> for Col {
    fn from(c: Rgb) -> Col {
        Col::T(c)
    }
}
const PN: u8 = 48; // 每条色带的色相格数
#[derive(Clone, Copy)]
enum Band {
    Text = 16,  // 正文彩虹
    Dim = 64,   // 暗色彩虹（日志尾巴、崩溃屏背景、鸭翅膀）
    Duck = 112, // 提亮的彩虹（鸭身）
}
const PAL_LO: u8 = 16;
const PAL_HI: u8 = 160;
const RAINBOW_SPEED: f32 = 1.1;
/// 彩虹色带上 (x, y) 处的调色板编号（对角线条纹，和 lolcat 一样）
fn rb(band: Band, x: f32, y: f32) -> Col {
    let ph = ((x + y * 2.2) * 0.075 / TAU).rem_euclid(1.0);
    Col::P(band as u8 + ((ph * PN as f32) as u8).min(PN - 1))
}
/// 调色板第 idx 号在时刻 t 的真实颜色
fn pal_rgb(idx: u8, t: f32) -> Rgb {
    let (band, s) = if idx >= Band::Duck as u8 {
        (2, idx - Band::Duck as u8)
    } else if idx >= Band::Dim as u8 {
        (1, idx - Band::Dim as u8)
    } else {
        (0, idx.saturating_sub(Band::Text as u8))
    };
    let i = (s as f32 + 0.5) / PN as f32 * TAU - t * RAINBOW_SPEED;
    let f = |o: f32| ((i + o).sin() * 127.0 + 128.0) as u8;
    let c = [f(0.0), f(2.094), f(4.189)];
    match band {
        1 => scale(c, 0.42),
        2 => mix(c, WHITE, 0.22),
        _ => c,
    }
}
fn resolve(c: Col, t: f32) -> Rgb {
    match c {
        Col::T(r) => r,
        Col::P(i) => pal_rgb(i, t),
    }
}
/// 真彩色版彩虹（给本来就每帧在动的东西用：飞蛾、粒子），和调色板同一时刻同一颜色
fn lolcat(x: f32, y: f32, t: f32) -> Rgb {
    resolve(rb(Band::Text, x, y), t)
}

// ───────────────────────── 屏幕缓冲 ─────────────────────────
fn char_w(c: char) -> usize {
    let u = c as u32;
    let wide = (0x1100..=0x115F).contains(&u)
        || (0x2E80..=0xA4CF).contains(&u)
        || (0xAC00..=0xD7A3).contains(&u)
        || (0xF900..=0xFAFF).contains(&u)
        || (0xFE30..=0xFE4F).contains(&u)
        || (0xFF00..=0xFF60).contains(&u)
        || (0xFFE0..=0xFFE6).contains(&u);
    if wide { 2 } else { 1 }
}
fn str_w(s: &str) -> usize {
    s.chars().map(char_w).sum()
}

#[derive(Clone, Copy, PartialEq)]
struct Cell {
    ch: char,
    fg: Col,
    bg: Col,
    w: u8, // 0 普通，1 宽字符本体，2 宽字符右半格
}
const BLANK: Cell = Cell { ch: ' ', fg: Col::T(GRAY), bg: Col::T(BLACK), w: 0 };

struct Screen {
    w: usize,
    h: usize,
    cells: Vec<Cell>,
    prev: Vec<Cell>,
    force: bool,
    /// 本帧整屏要上移几行（日志滚动），flush 时用终端原生滚屏，只补写新行
    scroll: usize,
    /// 终端支持改调色板（Windows Terminal）；否则把调色板颜色按 t=0 换成静态真彩色
    pal: bool,
}
impl Screen {
    fn new(w: usize, h: usize) -> Self {
        Screen { w, h, cells: vec![BLANK; w * h], prev: vec![BLANK; w * h], force: true, scroll: 0, pal: true }
    }
    fn clear(&mut self) {
        self.cells.fill(BLANK);
    }
    fn break_at(&mut self, x: usize, y: usize) {
        let i = y * self.w + x;
        match self.cells[i].w {
            1 => {
                if x + 1 < self.w {
                    self.cells[i + 1].w = 0;
                    self.cells[i + 1].ch = ' ';
                }
            }
            2 => {
                if x > 0 {
                    self.cells[i - 1].w = 0;
                    self.cells[i - 1].ch = ' ';
                }
            }
            _ => return,
        }
        self.cells[i].w = 0;
        self.cells[i].ch = ' ';
    }
    fn put(&mut self, x: i32, y: i32, ch: char, fg: impl Into<Col>, bg: Option<Rgb>) {
        self.put_c(x, y, ch, fg.into(), bg.map(Col::T));
    }
    fn put_c(&mut self, x: i32, y: i32, ch: char, fg: Col, bg: Option<Col>) {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
            return;
        }
        let (x, y) = (x as usize, y as usize);
        let cw = char_w(ch);
        if cw == 2 && x + 1 >= self.w {
            return;
        }
        self.break_at(x, y);
        if cw == 2 {
            self.break_at(x + 1, y);
        }
        let i = y * self.w + x;
        let bg = bg.unwrap_or(self.cells[i].bg);
        self.cells[i] = Cell { ch, fg, bg, w: if cw == 2 { 1 } else { 0 } };
        if cw == 2 {
            self.cells[i + 1] = Cell { ch: ' ', fg, bg, w: 2 };
        }
    }
    fn text(&mut self, x: i32, y: i32, s: &str, fg: Rgb, bg: Option<Rgb>) {
        let mut x = x;
        for ch in s.chars() {
            self.put(x, y, ch, fg, bg);
            x += char_w(ch) as i32;
        }
    }
    fn bg_at(&mut self, x: i32, y: i32) -> Option<&mut Col> {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
            return None;
        }
        Some(&mut self.cells[y as usize * self.w + x as usize].bg)
    }
    fn sgr(&self, out: &mut String, fg: Option<Col>, bg: Option<Col>) {
        if fg.is_none() && bg.is_none() {
            return;
        }
        out.push_str("\x1b[");
        let mut first = true;
        for (c, base) in [(fg, 38), (bg, 48)] {
            let Some(c) = c else { continue };
            if !first {
                out.push(';');
            }
            first = false;
            let c = if self.pal { c } else { Col::T(resolve(c, 0.0)) };
            match c {
                Col::P(i) => {
                    let _ = write!(out, "{};5;{}", base, i);
                }
                Col::T(r) => {
                    let _ = write!(out, "{};2;{};{};{}", base, r[0], r[1], r[2]);
                }
            }
        }
        out.push('m');
    }
    fn flush(&mut self, out: &mut String) {
        out.push_str("\x1b[?2026h");
        let (mut cf, mut cb): (Option<Col>, Option<Col>) = (None, None);
        // 终端原生滚屏：先用纯黑背景把整屏上移，再把旧缓冲同步上移，后面的差量比较就只剩新行和覆盖物
        if self.scroll > 0 && self.scroll < self.h && !self.force {
            let k = self.scroll;
            self.sgr(out, None, Some(Col::T(BLACK)));
            cb = Some(Col::T(BLACK));
            let _ = write!(out, "\x1b[{}S", k);
            self.prev.copy_within(k * self.w.., 0);
            let n = self.prev.len();
            self.prev[n - k * self.w..].fill(BLANK);
        }
        self.scroll = 0;
        for y in 0..self.h {
            let row = y * self.w;
            let mut first = None;
            let mut last = 0;
            for x in 0..self.w {
                if self.force || self.cells[row + x] != self.prev[row + x] {
                    first.get_or_insert(x);
                    last = x;
                }
            }
            let Some(mut f) = first else { continue };
            while f > 0 && (self.cells[row + f].w == 2 || self.prev[row + f].w == 2) {
                f -= 1;
            }
            let mut l = last;
            if l + 1 < self.w && (self.cells[row + l].w == 1 || self.prev[row + l].w == 1) {
                l += 1;
            }
            // 只写变化的格子；没变的空档短就顺手重写，长就用光标跳过去
            let changed = |x: usize| -> bool {
                let i = row + x;
                self.force
                    || self.cells[i] != self.prev[i]
                    || (x + 1 < self.w && self.cells[i].w == 1 && self.cells[i + 1] != self.prev[i + 1])
            };
            let mut cursor: Option<usize> = None;
            let mut x = f;
            while x <= l {
                let c = self.cells[row + x];
                let step = if c.w == 1 { 2 } else { 1 };
                if !changed(x) {
                    let mut gap = 0;
                    while x + gap <= l && !changed(x + gap) {
                        gap += 1;
                    }
                    if x + gap > l {
                        break;
                    }
                    if gap > 3 {
                        x += gap;
                        // 同一行内用相对右移（\x1b[nC）比绝对定位短
                        if let Some(cx) = cursor {
                            if x > cx {
                                let _ = write!(out, "\x1b[{}C", x - cx);
                                cursor = Some(x);
                            }
                        }
                        continue;
                    }
                }
                if cursor != Some(x) {
                    let _ = write!(out, "\x1b[{};{}H", y + 1, x + 1);
                }
                cursor = Some(x + step);
                if c.w == 2 {
                    let nb = (cb != Some(c.bg)).then_some(c.bg);
                    self.sgr(out, None, nb);
                    cb = Some(c.bg);
                    out.push(' ');
                    x += 1;
                    continue;
                }
                let nf = (cf != Some(c.fg) && c.ch != ' ').then_some(c.fg);
                let nb = (cb != Some(c.bg)).then_some(c.bg);
                self.sgr(out, nf, nb);
                if let Some(f) = nf {
                    cf = Some(f);
                }
                cb = Some(c.bg);
                out.push(c.ch);
                x += step;
            }
        }
        out.push_str("\x1b[?2026l");
        self.prev.copy_from_slice(&self.cells);
        self.force = false;
    }
}

/// 像素层：一格字符 = 上下两个像素（用 ▀▄█ 合成），用来画鸭鸭、飞蛾和大号字
struct Layer {
    w: usize,
    h: usize,
    px: Vec<Option<Col>>,
}
impl Layer {
    fn new(w: usize, h: usize) -> Self {
        Layer { w, h: h * 2, px: vec![None; w * h * 2] }
    }
    fn clear(&mut self) {
        self.px.fill(None);
    }
    fn set(&mut self, x: i32, y: i32, c: impl Into<Col>) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
            self.px[y as usize * self.w + x as usize] = Some(c.into());
        }
    }
    fn composite(&self, scr: &mut Screen) {
        for cy in 0..scr.h {
            for cx in 0..scr.w {
                let top = self.px[(cy * 2) * self.w + cx];
                let bot = self.px[(cy * 2 + 1) * self.w + cx];
                let (x, y) = (cx as i32, cy as i32);
                match (top, bot) {
                    (None, None) => {}
                    (Some(a), Some(b)) if a == b => scr.put_c(x, y, '█', a, None),
                    (Some(a), Some(b)) => scr.put_c(x, y, '▀', a, Some(b)),
                    (Some(a), None) => scr.put_c(x, y, '▀', a, None),
                    (None, Some(b)) => scr.put_c(x, y, '▄', b, None),
                }
            }
        }
    }
}

/// 一次性把整条调色板写给终端（彩虹流动的一步）
fn palette_osc(out: &mut String, t: f32) {
    for i in PAL_LO..PAL_HI {
        let c = pal_rgb(i, t);
        let _ = write!(out, "\x1b]4;{};rgb:{:02x}/{:02x}/{:02x}\x1b\\", i, c[0], c[1], c[2]);
    }
}

// ───────────────────────── 美术资源 ─────────────────────────
/// 彩虹橡皮鸭（朝右）。# 身体  W 翅膀  e 眼睛  B/b 嘴
const DUCK: [&str; 14] = [
    "..........####....",
    ".........######...",
    "........########..",
    "........#####e##..",
    "........#######BBB",
    ".........####BBBB.",
    "#.........####bb..",
    "##.......######...",
    "###...##########..",
    "#################.",
    "####WWWWW#######..",
    ".####WWWW#######..",
    "..#############...",
    "....#########.....",
];
/// 飞蛾两帧（翅膀上/下），o 是身体
const MOTH: [[&str; 3]; 2] = [["#...#", "##o##", ".#.#."], [".....", "##o##", "##.##"]];
const COLON: [&str; 10] = ["...", "##.", "##.", "...", "...", "...", "...", "##.", "##.", "..."];
const LPAREN: [&str; 10] = ["..##", ".##.", "##..", "##..", "##..", "##..", "##..", "##..", ".##.", "..##"];
const RPAREN: [&str; 10] = ["##..", ".##.", "..##", "..##", "..##", "..##", "..##", "..##", ".##.", "##.."];

fn blit_glyph(layer: &mut Layer, g: &[&str], ox: i32, oy: i32, s: i32, c: Rgb) {
    for (j, row) in g.iter().enumerate() {
        for (i, ch) in row.chars().enumerate() {
            if ch == '#' {
                for dy in 0..s {
                    for dx in 0..s {
                        layer.set(ox + i as i32 * s + dx, oy + j as i32 * s + dy, c);
                    }
                }
            }
        }
    }
}

// ───────────────────────── 日志 ─────────────────────────
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Boot,
    Info,
    Error,
    Fatal,
    Fixed,
}
struct Line {
    text: String,
    kind: Kind,
    ok_col: usize,
    tail: usize,  // 从这个字符开始是暗色的 trace 尾巴（把屏幕右半边填满）
    no: u64,      // 全局递增的行号：彩虹按行号上色，滚动时颜色跟着文字走
    eat: Vec<u8>, // 每一列：0 正常，1 被吃成黑白，2 被咬掉
}

const BOOT: &[&str] = &[
    "正在初始化系统组件",
    "校验依赖完整性 (1024/1024)",
    "连接日志服务 127.0.0.1:8080",
    "加载配置 config.prod.yaml",
    "挂载彩虹驱动",
    "唤醒鸭鸭",
];

const FUNNY: &[(&str, &str, &str)] = &[
    ("INFO", "rainbow::core", "彩虹引擎 v0.2 启动完成"),
    ("INFO", "mem::dl", "正在下载更多内存 ... 87%"),
    ("WARN", "rainbow::core", "彩虹浓度超标 312%，建议佩戴墨镜"),
    ("DEBUG", "brain::parse", "第 4096 次尝试读懂需求文档"),
    ("INFO", "qa::magic", "已成功将 bug 重命名为 feature"),
    ("WARN", "mood::sched", "检测到今天是周一，士气 -40"),
    ("INFO", "coffee::io", "咖啡因注入完成，心率正常（大概）"),
    ("DEBUG", "rustc::trust", "正在说服编译器相信我"),
    ("INFO", "client::req", "甲方要的五彩斑斓的黑：渲染中"),
    ("WARN", "moyu::daemon", "摸鱼指数逼近临界值"),
    ("DEBUG", "var::alloc", "变量 temp2_final_真的最终版 已创建"),
    ("INFO", "duck::core", "鸭鸭正在认真听你讲代码"),
    ("DEBUG", "duck::core", "鸭鸭：嘎"),
    ("INFO", "duck::float", "浮力检测正常，鸭鸭不会沉"),
    ("WARN", "bug::radar", "雷达发现不明飞行物（有翅膀）"),
    ("DEBUG", "bug::nest", "某只 bug 声称自己是 feature"),
    ("INFO", "history", "1947-09-09 15:45 在 70 号继电器发现第一只 bug"),
    ("INFO", "duck::light", "鸭鸭亮度 9000 流明，飞蛾请勿靠近"),
    ("WARN", "bug::radar", "飞蛾正在向最亮的东西靠拢"),
];

fn stamp() -> String {
    win::now_hms()
}
fn log_line(level: &str, module: &str, msg: &str, kind: Kind) -> Line {
    Line { text: format!("{}  {:<5}  {:<13} {}", stamp(), level, module, msg), kind, ok_col: usize::MAX, tail: usize::MAX, no: 0, eat: Vec::new() }
}
fn boot_line(msg: &str) -> Line {
    let pad = 34usize.saturating_sub(str_w(msg));
    let head = format!("  {}{}", msg, ".".repeat(pad));
    let ok_col = head.chars().count() + 1;
    Line { text: format!("{} [ OK ]", head), kind: Kind::Boot, ok_col, tail: usize::MAX, no: 0, eat: Vec::new() }
}

// ───────────────────────── 角色 ─────────────────────────
struct Duck {
    x: f32, // 中心，像素坐标（y 已经是半格单位）
    y: f32,
    tx: f32,
    ty: f32,
    right: bool,
    asleep: bool,
    glow: f32,
    glow_to: f32,
    visible: bool,
    lane: f32,
    orb: f32,
    bubble: Option<(String, f32)>, // 文字，消失时间
    zz: Vec<(f32, f32, f32)>,      // 睡觉冒的 z：x y 年龄
    zz_cd: f32,
}

#[derive(Clone, Copy, PartialEq)]
enum MothMode {
    Orbit,
    Eat,
    Converge,
    Burst,
    Heart,
    Fix,
    Balloon,
    Crawl,
}
struct Moth {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    flap: f32,
    shade: u8,
    tx: f32,
    ty: f32,
    delay: f32,
    color: f32, // 被染成彩虹的程度
    alive: bool,
}

struct Particle {
    col: Col,
    hx: f32,
    hy: f32,
    ch: char,
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    sx: f32,
    sy: f32,
    delay: f32,
    gray: bool,
    alive: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Ending {
    Heart,
    BugWin,
    Fixed,
    Balloon,
}
const ENDINGS: [Ending; 4] = [Ending::Heart, Ending::BugWin, Ending::Fixed, Ending::Balloon];

#[derive(Clone, Copy, PartialEq, Debug)]
enum Phase {
    Boot,
    Sweep,
    Daily,
    Nap,
    Swarm,
    Hole,
    Collapse,
    Bang,
    End(Ending),
}

struct App {
    scr: Screen,
    layer: Layer,
    rng: Rng,
    logs: Vec<Line>,
    phase: Phase,
    pt: f32,
    t: f32,
    acc: f32,
    step: usize,
    parts: Vec<Particle>,
    eaten: usize,
    bitten: usize,
    duck: Duck,
    moths: Vec<Moth>,
    mmode: MothMode,
    fixed_n: usize,
    bag: Vec<Ending>,
    forced: Option<Ending>,
    returning: bool,
    title: String,
    title_dirty: bool,
    title_cool: f32,
    last_funny: usize,
    line_no: u64,
    last_top: Option<u64>,
}

impl App {
    fn new(w: usize, h: usize, seed: u64) -> Self {
        let mut a = App {
            scr: Screen::new(w, h),
            layer: Layer::new(w, h),
            rng: Rng(seed | 1),
            logs: Vec::new(),
            phase: Phase::Boot,
            pt: 0.0,
            t: 0.0,
            acc: 0.0,
            step: 0,
            parts: Vec::new(),
            eaten: 0,
            bitten: 0,
            duck: Duck {
                x: -40.0,
                y: 0.0,
                tx: 0.0,
                ty: 0.0,
                right: true,
                asleep: false,
                glow: 0.0,
                glow_to: 1.0,
                visible: false,
                lane: 0.0,
                orb: 0.0,
                bubble: None,
                zz: Vec::new(),
                zz_cd: 0.0,
            },
            moths: Vec::new(),
            mmode: MothMode::Orbit,
            fixed_n: 0,
            bag: Vec::new(),
            forced: None,
            returning: false,
            title: String::new(),
            title_dirty: false,
            title_cool: 0.0,
            last_funny: usize::MAX,
            line_no: 0,
            last_top: None,
        };
        a.set_title("C:\\Windows\\system32\\cmd.exe");
        a
    }
    fn resize(&mut self, w: usize, h: usize) {
        let (w, h) = (w.max(20), h.max(10));
        let pal = self.scr.pal;
        self.scr = Screen::new(w, h);
        self.scr.pal = pal;
        self.layer = Layer::new(w, h);
        self.last_top = None;
    }
    fn w(&self) -> f32 {
        self.scr.w as f32
    }
    fn hp(&self) -> f32 {
        self.scr.h as f32 * 2.0
    }
    fn s(&self) -> i32 {
        if self.scr.w >= 150 && self.scr.h >= 40 { 2 } else { 1 }
    }
    fn center(&self) -> (f32, f32) {
        (self.w() / 2.0, self.scr.h as f32)
    }
    fn set_title(&mut self, s: &str) {
        if self.title != s {
            self.title = s.to_string();
            self.title_dirty = true;
        }
    }
    fn go(&mut self, p: Phase) {
        self.phase = p;
        self.pt = 0.0;
        self.step = 0;
        self.acc = 0.0;
    }
    /// 阶段内按顺序触发的一次性事件
    fn ev(&mut self, k: usize, at: f32) -> bool {
        if self.step == k && self.pt >= at {
            self.step += 1;
            true
        } else {
            false
        }
    }
    fn push(&mut self, mut l: Line) {
        l.no = self.line_no;
        self.line_no += 1;
        if l.kind != Kind::Boot && l.tail == usize::MAX {
            l.tail = l.text.chars().count();
            let r = &mut self.rng;
            let mut s = String::from("   ");
            while s.len() < 190 {
                let seg = match r.int(6) {
                    0 => format!("trace={:08x} ", r.next() as u32),
                    1 => format!("span={:04x} ", r.int(65536)),
                    2 => format!("lat={}ms ", 1 + r.int(300)),
                    3 => format!("node=rb-{:02} ", r.int(48)),
                    4 => format!("rainbow={}% ", r.int(101)),
                    _ => format!("duck={} ", ["ok", "ok", "ok", "quack", "afk"][r.int(5)]),
                };
                s.push_str(&seg);
            }
            l.text.push_str(&s);
        }
        self.logs.push(l);
        if self.logs.len() > 400 {
            self.logs.drain(..200);
        }
    }
    fn info(&mut self, m: &str, s: &str) {
        self.push(log_line("INFO", m, s, Kind::Info));
    }
    fn push_random(&mut self) {
        if self.rng.f() < 0.3 {
            let mut i = self.rng.int(FUNNY.len());
            if i == self.last_funny {
                i = (i + 1) % FUNNY.len();
            }
            self.last_funny = i;
            let (lv, m, s) = FUNNY[i];
            self.push(log_line(lv, m, s, Kind::Info));
            return;
        }
        let r = &mut self.rng;
        let msg = match r.int(7) {
            0 => format!("加载模块 shader_{:03}.rbw ... ok", r.int(1000)),
            1 => format!("GET /api/v2/duck/{} 200 {}ms", r.int(99999), 3 + r.int(80)),
            2 => format!("同步彩虹缓存 [{}/4096]", r.int(4096)),
            3 => format!("gc: 回收 {} MB 无用的焦虑", 1 + r.int(512)),
            4 => format!("worker-{:02} 心跳正常", r.int(64)),
            5 => format!("校验 sha256:{:016x} ... ok", r.next()),
            _ => format!("渲染第 {} 帧彩虹，色差 0.{:03}", r.int(1_000_000), r.int(1000)),
        };
        let (lv, m) = if r.int(4) == 0 { ("DEBUG", "rainbow::gpu") } else { ("INFO", "rainbow::core") };
        self.push(log_line(lv, m, &msg, Kind::Info));
    }
    fn duck_half(&self) -> (f32, f32) {
        let s = self.s() as f32;
        (9.0 * s, 7.0 * s)
    }
    fn lane_y(&self) -> f32 {
        self.hp() - self.duck_half().1 - 6.0
    }
    fn quack(&mut self, text: &str, dur: f32) {
        self.duck.bubble = Some((text.to_string(), self.t + dur));
        let (dx, dy) = (self.duck.x, self.duck.y);
        for m in self.moths.iter_mut() {
            let (ex, ey) = (m.x - dx, m.y - dy);
            let d = (ex * ex + ey * ey).sqrt().max(0.5);
            if d < 45.0 {
                m.vx += ex / d * 90.0;
                m.vy += ey / d * 90.0;
            }
        }
    }
    fn spawn_moth_edge(&mut self) {
        let (w, h) = (self.w(), self.hp());
        let (x, y) = match self.rng.int(4) {
            0 => (-2.0, self.rng.range(0.0, h)),
            1 => (w + 2.0, self.rng.range(0.0, h)),
            2 => (self.rng.range(0.0, w), -2.0),
            _ => (self.rng.range(0.0, w), h + 2.0),
        };
        self.spawn_moth(x, y, 0.0, 0.0);
    }
    fn spawn_moth(&mut self, x: f32, y: f32, vx: f32, vy: f32) {
        let shade = self.rng.int(5) as u8;
        let flap = self.rng.range(0.0, 10.0);
        self.moths.push(Moth { x, y, vx, vy, flap, shade, tx: x, ty: y, delay: 0.0, color: 0.0, alive: true });
    }
    fn next_ending(&mut self) -> Ending {
        if let Some(e) = self.forced {
            return e;
        }
        if self.bag.is_empty() {
            let mut b = ENDINGS.to_vec();
            for i in (1..b.len()).rev() {
                let j = self.rng.int(i + 1);
                b.swap(i, j);
            }
            self.bag = b;
        }
        self.bag.pop().unwrap()
    }

    // ═════════════════════════ 每帧逻辑 ═════════════════════════
    fn update(&mut self, dt: f32) {
        self.t += dt;
        self.pt += dt;
        self.title_cool -= dt;
        match self.phase {
            Phase::Boot => {
                let due = ((self.pt - 0.6) / 0.42).floor();
                while (self.step as f32) <= due && self.step < BOOT.len() {
                    self.push(boot_line(BOOT[self.step]));
                    self.step += 1;
                }
                if self.pt > 0.6 + 0.42 * BOOT.len() as f32 + 0.5 {
                    self.go(Phase::Sweep);
                    self.set_title("正在加载彩虹...");
                }
            }
            Phase::Sweep => {
                if self.pt > 1.4 {
                    self.enter_daily();
                }
            }
            Phase::Daily => self.update_daily(dt),
            Phase::Nap => self.update_nap(dt),
            Phase::Swarm => {
                if self.ev(0, 0.2) {
                    self.push(log_line("ERROR", "bug::nest", "bug 密度过高，重力模块未响应", Kind::Error));
                    self.set_title("警告：bug 密度过高");
                }
                if self.ev(1, 1.6) {
                    self.push(log_line("FATAL", "cosmos::core", "bug 坍缩成奇点，请勿惊慌", Kind::Fatal));
                }
                if self.pt > 3.4 {
                    self.snapshot();
                    self.moths.clear();
                    let (cx, cy) = self.center();
                    self.duck.orb = (self.duck.y - cy).atan2(self.duck.x - cx);
                    self.go(Phase::Hole);
                }
            }
            Phase::Hole => {
                self.update_hole(dt);
                let (cx, cy) = self.center();
                let r = self.hole_radius() * 2.9;
                self.duck.orb += dt * 0.9;
                let (ox, oy) = (cx + r * self.duck.orb.cos(), cy + r * 0.24 * self.duck.orb.sin());
                self.duck.tx = ox;
                self.duck.ty = oy - self.duck_half().1 * 0.6;
                self.duck.right = self.duck.orb.sin() < 0.0;
                if self.ev(0, 3.2) {
                    self.quack("嘎", 1.4);
                }
                if self.eaten > 0 && self.title_cool <= 0.0 {
                    self.set_title(&format!("救命（已吞噬 {} 个字符）", self.eaten));
                    self.title_cool = 0.12;
                }
                if self.parts.iter().all(|p| !p.alive) || self.pt > 9.0 {
                    self.set_title("...");
                    self.go(Phase::Collapse);
                }
            }
            Phase::Collapse => {
                let (cx, cy) = self.center();
                if self.pt > 0.6 {
                    self.duck.tx = cx;
                    self.duck.ty = cy;
                }
                if self.ev(0, 1.2) {
                    self.quack("……", 1.0);
                }
                if self.ev(1, 2.5) {
                    self.quack("嘎！", 1.2);
                    self.set_title("嘎！");
                }
                if self.pt > 2.9 {
                    self.start_bang();
                    self.go(Phase::Bang);
                }
            }
            Phase::Bang => {
                self.update_bang(dt);
                if self.pt > 3.3 {
                    for l in self.logs.iter_mut() {
                        l.eat.clear();
                        match l.kind {
                            Kind::Error => *l = log_line("INFO", "bug::nest", "bug 密度恢复正常（原因不明）", Kind::Info),
                            Kind::Fatal => *l = log_line("INFO", "cosmos::core", "奇点已关闭，宇宙已恢复出厂设置", Kind::Info),
                            _ => {}
                        }
                    }
                    self.parts.clear();
                    let e = self.next_ending();
                    self.go(Phase::End(e));
                    self.start_ending(e);
                }
            }
            Phase::End(e) => self.update_ending(e, dt),
        }
        self.update_duck(dt);
        self.update_moths(dt);
    }

    fn enter_daily(&mut self) {
        self.go(Phase::Daily);
        // 长期挂机：f32 时间越大越不精确，每 50 分钟在新一轮开头回绕一次（此刻画面整体在换，看不出跳变）
        if self.t > 3000.0 {
            self.t -= 3000.0;
            if let Some((_, until)) = &mut self.duck.bubble {
                *until -= 3000.0;
            }
        }
        self.duck.lane %= std::f32::consts::TAU;
        for m in &mut self.moths {
            m.flap %= 2.0;
        }
        self.mmode = MothMode::Orbit;
        self.duck.asleep = false;
        self.duck.glow_to = 1.0;
        if !self.duck.visible || self.duck.x < -30.0 || self.duck.x > self.w() + 30.0 || self.duck.y < -20.0 {
            self.duck.visible = true;
            self.duck.x = -self.duck_half().0 * 2.5;
            self.duck.y = self.lane_y();
        }
        self.set_title("C:\\Windows\\system32\\cmd.exe");
    }

    fn update_daily(&mut self, dt: f32) {
        let h = self.scr.h;
        let rate = if self.logs.len() < h { 25.0 } else { 3.0 };
        self.acc += rate * dt;
        while self.acc >= 1.0 {
            self.acc -= 1.0;
            self.push_random();
        }
        self.duck.lane += dt * 0.16;
        self.duck.tx = self.w() / 2.0 + self.w() * 0.3 * (self.duck.lane - 1.2).sin();
        self.duck.ty = self.lane_y();
        if self.moths.len() < 10 && self.rng.f() < dt * 2.5 {
            self.spawn_moth_edge();
        }
        if self.returning && self.ev(0, 1.5) {
            self.info("duck::core", "鸭鸭回来了（若无其事）");
            self.returning = false;
        } else if self.step == 0 && self.pt > 1.5 {
            self.step = 1;
        }
        if self.ev(1, 4.5) {
            self.quack("嘎", 0.9);
        }
        if self.ev(2, 9.0) {
            self.quack("嘎", 0.9);
        }
        if self.pt > 11.0 {
            self.go(Phase::Nap);
            self.set_title("鸭鸭睡着了");
        }
    }

    fn update_nap(&mut self, dt: f32) {
        if self.ev(0, 0.5) {
            self.duck.asleep = true;
            self.duck.glow_to = 0.0;
            self.info("duck::core", "鸭鸭打了个盹，灯灭了");
            self.eaten = 0;
        }
        if self.ev(1, 1.3) {
            self.mmode = MothMode::Eat;
        }
        if self.pt > 1.3 && self.moths.len() < 90 && self.rng.f() < dt * 16.0 {
            self.spawn_moth_edge();
        }
        if self.pt > 1.5 {
            self.acc += dt / 1.5;
            if self.acc >= 1.0 {
                self.acc -= 1.0;
                let n = self.moths.len() + self.bitten;
                self.push(log_line("WARN", "bug::radar", &format!("发现 bug ×{}", n), Kind::Info));
            }
        }
        if self.title_cool <= 0.0 && self.pt > 1.3 {
            self.set_title(&format!("发现 bug ×{}", self.moths.len() + self.bitten));
            self.title_cool = 0.15;
        }
        if self.pt > 9.0 {
            self.go(Phase::Swarm);
            self.mmode = MothMode::Converge;
            self.duck.asleep = false;
            self.duck.glow_to = 1.0;
            self.duck.bubble = Some(("？".into(), self.t + 1.5));
        }
    }

    fn start_ending(&mut self, e: Ending) {
        match e {
            Ending::Heart => {
                self.mmode = MothMode::Heart;
                let (dx, dy) = (self.duck.x, self.duck.y);
                while self.moths.len() < 88 {
                    let a = self.rng.range(0.0, TAU);
                    let s = self.rng.range(30.0, 90.0);
                    self.spawn_moth(dx, dy, a.cos() * s, a.sin() * s);
                }
                self.set_title("bug ×0");
            }
            Ending::BugWin => {
                self.mmode = MothMode::Crawl;
                for l in self.logs.iter_mut() {
                    l.eat = vec![1; l.text.chars().count() * 2 + 2];
                }
                self.set_title(":(");
            }
            Ending::Fixed => {
                self.mmode = MothMode::Fix;
                self.fixed_n = 0;
                let n = self.moths.len();
                for (i, m) in self.moths.iter_mut().enumerate() {
                    m.delay = 0.6 + i as f32 / n.max(1) as f32 * 2.2;
                }
                self.quack("嘎！", 1.0);
            }
            Ending::Balloon => {
                self.mmode = MothMode::Balloon;
            }
        }
    }

    fn update_ending(&mut self, e: Ending, dt: f32) {
        let (cx, cy) = self.center();
        match e {
            Ending::Heart => {
                self.duck.tx = cx;
                self.duck.ty = cy;
                if self.ev(0, 1.0) {
                    self.info("cosmos::core", "刚才什么都没发生");
                }
                if self.pt > 2.4 {
                    let r = (self.pt - 2.4) * 70.0;
                    for m in self.moths.iter_mut() {
                        let d = ((m.x - cx).powi(2) + (m.y - cy).powi(2)).sqrt();
                        if d < r {
                            m.color = (m.color + dt * 3.0).min(1.0);
                        }
                    }
                }
                if self.ev(1, 3.0) {
                    let n = self.moths.len();
                    self.info("bug::nest", &format!("{} 只 bug 已被鸭鸭感化", n));
                    self.set_title("♥");
                }
                if self.pt > 7.5 {
                    self.enter_daily();
                }
            }
            Ending::BugWin => {
                // 0.6 秒后出现崩溃屏；4.2 秒鸭鸭从左边漂过来把它涂成彩虹
                self.duck.visible = self.pt > 4.0 && self.pt < 9.5;
                if self.ev(0, 4.0) {
                    self.duck.x = -self.duck_half().0 * 2.0;
                    self.duck.y = self.hp() * 0.78;
                    self.duck.right = true;
                }
                if self.pt > 4.0 {
                    let k = (self.pt - 4.0) / 4.5;
                    self.duck.tx = -self.duck_half().0 * 2.0 + k * (self.w() + self.duck_half().0 * 4.0);
                    self.duck.x = self.duck.tx;
                    self.duck.ty = self.hp() * 0.78;
                    let wipe = self.duck.x;
                    for m in self.moths.iter_mut() {
                        if m.x < wipe && m.color < 1.0 {
                            m.color = 1.0;
                        }
                    }
                    if self.ev(1, 6.5) {
                        self.set_title(":)");
                    }
                }
                if self.pt > 3.0 && self.moths.len() < 80 && self.rng.f() < dt * 12.0 {
                    self.spawn_moth_edge();
                }
                if self.pt > 11.5 {
                    for l in self.logs.iter_mut() {
                        l.eat.clear();
                    }
                    self.info("duck::core", "崩溃已被鸭鸭撤销");
                    self.duck.visible = false;
                    self.moths.retain(|m| m.color < 1.0);
                    self.enter_daily();
                }
            }
            Ending::Fixed => {
                self.duck.tx = cx;
                self.duck.ty = cy;
                if self.moths.is_empty() && self.step == 0 {
                    self.step = 1;
                    self.info("duck::core", "鸭鸭：嘎（深藏功与名）");
                    self.set_title("bug ×0");
                }
                if self.pt > 9.0 || (self.step == 1 && self.pt > 5.5) {
                    self.enter_daily();
                }
            }
            Ending::Balloon => {
                if self.pt < 2.2 {
                    self.duck.tx = cx;
                    self.duck.ty = cy;
                } else {
                    self.duck.ty -= dt * (8.0 + (self.pt - 2.2) * 10.0);
                    self.duck.y = self.duck.ty;
                    self.duck.x = cx + (self.pt * 1.3).sin() * 6.0;
                }
                if self.ev(0, 2.6) {
                    self.push(log_line("ERROR", "duck::core", "鸭鸭被 bug 带走了", Kind::Error));
                    self.set_title("鸭鸭呢？");
                }
                if self.pt > 8.0 {
                    for l in self.logs.iter_mut() {
                        if l.kind == Kind::Error {
                            l.kind = Kind::Info;
                        }
                    }
                    self.moths.clear();
                    self.duck.visible = false;
                    self.returning = true;
                    self.enter_daily();
                }
            }
        }
    }

    fn update_duck(&mut self, dt: f32) {
        let d = &mut self.duck;
        let k = (2.2 * dt).min(1.0);
        let ox = d.x;
        d.x += (d.tx - d.x) * k;
        d.y += (d.ty - d.y) * k;
        if !matches!(self.phase, Phase::Hole | Phase::End(Ending::BugWin)) {
            if d.x - ox > 0.4 * dt * 10.0 {
                d.right = true;
            } else if d.x - ox < -0.4 * dt * 10.0 {
                d.right = false;
            }
        }
        d.glow += (d.glow_to - d.glow) * (1.5 * dt).min(1.0);
        if let Some((_, until)) = &d.bubble {
            if self.t > *until {
                d.bubble = None;
            }
        }
        d.zz_cd -= dt;
        if d.asleep && d.zz_cd <= 0.0 {
            d.zz_cd = 0.8;
            d.zz.push((d.x + if d.right { 6.0 } else { -6.0 }, d.y - 14.0, 0.0));
        }
        for z in d.zz.iter_mut() {
            z.2 += dt;
            z.1 -= dt * 6.0;
            z.0 += (z.2 * 3.0).sin() * dt * 4.0;
        }
        d.zz.retain(|z| z.2 < 2.6);
    }

    /// 虫群：分离 + 各模式下的目标，粗糙版鸟群算法
    fn update_moths(&mut self, dt: f32) {
        let n = self.moths.len();
        if n == 0 {
            return;
        }
        let pos: Vec<(f32, f32)> = self.moths.iter().map(|m| (m.x, m.y)).collect();
        let (dcx, dcy) = (self.duck.x, self.duck.y);
        let (cx, cy) = self.center();
        let hp = self.hp();
        let (hw, hh) = self.duck_half();
        let start = self.logs.len().saturating_sub(self.scr.h);
        let pt = self.pt;
        let mode = self.mmode;
        let heart_k = hw * 0.125 * (1.0 + 0.05 * (self.t * 5.0).sin());
        let mut fixed_lines = 0;
        for i in 0..n {
            let mut m = std::mem::replace(
                &mut self.moths[i],
                Moth { x: 0.0, y: 0.0, vx: 0.0, vy: 0.0, flap: 0.0, shade: 0, tx: 0.0, ty: 0.0, delay: 0.0, color: 0.0, alive: false },
            );
            let (mut sx, mut sy) = (0.0f32, 0.0f32);
            for (j, &(ox, oy)) in pos.iter().enumerate() {
                if j == i {
                    continue;
                }
                let (dx, dy) = (m.x - ox, m.y - oy);
                let d2 = dx * dx + dy * dy;
                if d2 < 9.0 && d2 > 0.01 {
                    sx += dx / d2;
                    sy += dy / d2;
                }
            }
            let mut sep = 30.0;
            let (mut desx, mut desy, maxs) = match mode {
                MothMode::Orbit => {
                    let (dx, dy) = (m.x - dcx, m.y - dcy);
                    let d = (dx * dx + dy * dy).sqrt().max(0.5);
                    let rr = hw * 1.5 + (i % 6) as f32 * 2.5;
                    let dir = if i % 3 == 0 { -1.0 } else { 1.0 };
                    let (tx, ty) = (-dy / d * dir, dx / d * dir);
                    let pull = (rr - d) * 1.6;
                    (tx * 26.0 + dx / d * pull, ty * 26.0 + dy / d * pull, 45.0)
                }
                MothMode::Eat | MothMode::Crawl => {
                    let (dx, dy) = (m.tx - m.x, m.ty - m.y);
                    let d = (dx * dx + dy * dy).sqrt();
                    if d < 2.0 || (m.tx == m.x && m.ty == m.y) {
                        // 挑一个有字的格子当下一口
                        for _ in 0..8 {
                            let x = self.rng.int(self.scr.w);
                            let y = self.rng.int(self.scr.h);
                            m.tx = x as f32 + 0.5;
                            m.ty = y as f32 * 2.0 + 1.0;
                            if self.scr.cells[y * self.scr.w + x].ch != ' ' {
                                break;
                            }
                        }
                    }
                    let d = d.max(0.5);
                    (dx / d * 22.0, dy / d * 22.0, 30.0)
                }
                MothMode::Converge => {
                    let (dx, dy) = (cx - m.x, cy - m.y);
                    let d = (dx * dx + dy * dy).sqrt().max(0.3);
                    let k = 20.0 + pt * pt * 14.0;
                    sep = 30.0 * (1.0 - smooth(0.5, 2.5, pt));
                    (dx / d * k + dy / d * 25.0, dy / d * k - dx / d * 25.0, 40.0 + pt * 40.0)
                }
                MothMode::Burst => (m.vx * 0.9, m.vy * 0.9, 200.0),
                        MothMode::Heart => {
                    let a = i as f32 / n as f32 * TAU;
                    let hx = 16.0 * a.sin().powi(3);
                    let hy = -(13.0 * a.cos() - 5.0 * (2.0 * a).cos() - 2.0 * (3.0 * a).cos() - (4.0 * a).cos());
                    let (tx, ty) = (dcx + hx * heart_k, dcy + hy * heart_k - hh * 0.2);
                    sep = 4.0;
                    ((tx - m.x) * 4.0, (ty - m.y) * 4.0, 70.0)
                }
                MothMode::Fix => {
                    if pt < m.delay {
                        let (dx, dy) = (m.x - dcx, m.y - dcy);
                        let d = (dx * dx + dy * dy).sqrt().max(0.5);
                        (-dy / d * 20.0, dx / d * 20.0, 30.0)
                    } else {
                        sep = 0.0;
                        m.color = 1.0;
                        (0.0, 90.0, 90.0)
                    }
                }
                MothMode::Balloon => {
                    let a = (i as f32 * 2.399).rem_euclid(TAU);
                    let rr = (i % 7) as f32 * 1.6 + 2.0;
                    let (tx, ty) = (dcx + a.cos() * rr * 2.0, dcy - hh - 10.0 + a.sin() * rr);
                    sep = 8.0;
                    ((tx - m.x) * 5.0, (ty - m.y) * 5.0, 90.0)
                }
            };
            if mode != MothMode::Burst {
                let (noise, steer) = match mode {
                    MothMode::Heart | MothMode::Balloon => (1.5, 7.0),
                    _ => (10.0, 4.0),
                };
                desx += self.rng.range(-1.0, 1.0) * noise;
                desy += self.rng.range(-1.0, 1.0) * noise;
                m.vx += ((desx - m.vx) * steer + sx * sep) * dt;
                m.vy += ((desy - m.vy) * steer + sy * sep) * dt;
            } else {
                let drag = 1.0 - 2.0 * dt;
                m.vx *= drag;
                m.vy *= drag;
            }
            let sp = (m.vx * m.vx + m.vy * m.vy).sqrt();
            if sp > maxs {
                m.vx *= maxs / sp;
                m.vy *= maxs / sp;
            }
            m.x += m.vx * dt;
            m.y += m.vy * dt;
            m.flap += dt * (9.0 + (i % 4) as f32);
            match mode {
                MothMode::Eat => {
                    let (x, y) = (m.x.floor() as i32, (m.y / 2.0).floor() as i32);
                    if x >= 0 && y >= 0 && (x as usize) < self.scr.w && (y as usize) < self.scr.h {
                        let li = start + y as usize;
                        if li < self.logs.len() {
                            let line = &mut self.logs[li];
                            if line.eat.len() < self.scr.w + 2 {
                                line.eat.resize(self.scr.w + 2, 0);
                            }
                            for dx in -2i32..=2 {
                                let c = (x + dx).max(0) as usize;
                                if line.eat[c] == 0 {
                                    line.eat[c] = 1;
                                    self.eaten += 1;
                                }
                            }
                            if line.eat[x as usize] == 1 && self.rng.f() < dt * 2.5 {
                                line.eat[x as usize] = 2;
                                self.bitten += 1;
                            }
                        }
                    }
                }
                MothMode::Fix => {
                    if m.y > hp - 1.0 {
                        m.alive = false;
                        fixed_lines += 1;
                    }
                }
                _ => {}
            }
            self.moths[i] = m;
        }
        for _ in 0..fixed_lines {
            self.fixed_n += 1;
            let id = 1000 + self.rng.int(9000);
            self.push(log_line("FIXED", "duck::debug", &format!("bug #{} 已修复", id), Kind::Fixed));
        }
        self.moths.retain(|m| m.alive);
    }

    /// 把屏幕上每个可见字符变成粒子（不含鸭鸭和虫子）
    fn snapshot(&mut self) {
        self.scr.clear();
        self.last_top = None;
        self.draw_logs(9.0, 0, 0.0);
        self.parts.clear();
        self.eaten = 0;
        let w = self.scr.w;
        // 粒子限量：正文全保留，暗色尾巴抽样补足到上限（每个移动的粒子都要往终端写字节）
        let is_dim = |c: &Cell| match c.fg {
            Col::P(i) => (Band::Dim as u8..Band::Duck as u8).contains(&i),
            Col::T(r) => (r[0] as u32 + r[1] as u32 + r[2] as u32) < 260,
        };
        let (mut main, mut dims) = (0usize, 0usize);
        for c in &self.scr.cells {
            if c.w != 2 && c.ch != ' ' {
                if is_dim(c) { dims += 1 } else { main += 1 }
            }
        }
        let keep_dim = (PARTICLE_CAP.saturating_sub(main) as f32 / dims.max(1) as f32).min(1.0);
        for y in 0..self.scr.h {
            for x in 0..w {
                let c = self.scr.cells[y * w + x];
                if c.w == 2 || c.ch == ' ' {
                    continue;
                }
                if is_dim(&c) && self.rng.f() > keep_dim {
                    continue;
                }
                let (px, py) = (x as f32 + 0.5, y as f32 * 2.0 + 1.0);
                let gray = matches!(c.fg, Col::T(r) if r[0] == r[1] && r[1] == r[2]);
                self.parts.push(Particle {
                    col: c.fg,
                    hx: px,
                    hy: py,
                    ch: c.ch,
                    x: px,
                    y: py,
                    vx: 0.0,
                    vy: 0.0,
                    sx: 0.0,
                    sy: 0.0,
                    delay: 0.0,
                    gray,
                    alive: true,
                });
            }
        }
    }

    fn hole_radius(&self) -> f32 {
        (self.scr.w.min(self.scr.h * 2) as f32 * 0.19).clamp(5.0, 25.0)
    }

    fn update_hole(&mut self, dt: f32) {
        let (cx, cy) = self.center();
        let tau = self.pt;
        let ramp = smooth(0.0, 0.9, tau);
        let r_hole = self.hole_radius() * smooth(0.0, 1.8, tau);
        for p in self.parts.iter_mut().filter(|p| p.alive) {
            let (dx, dy) = (p.x - cx, p.y - cy);
            let mut r = (dx * dx + dy * dy).sqrt().max(0.01);
            let mut a = dy.atan2(dx);
            let vr = -(2.0 + 3.0 * tau * tau) * (1.0 + 15.0 / (r + 2.0)) * ramp;
            let om = (1.0 + 0.6 * tau) * (28.0 / (r + 3.0)) * ramp;
            r += vr * dt;
            a += om * dt;
            let (nx, ny) = (cx + r * a.cos(), cy + r * a.sin());
            p.vx = (nx - p.x) / dt.max(1e-4);
            p.vy = (ny - p.y) / dt.max(1e-4);
            p.x = nx;
            p.y = ny;
            if r < r_hole * 0.92 && tau > 0.6 || r < 0.8 {
                p.alive = false;
                self.eaten += 1;
            }
        }
    }

    fn start_bang(&mut self) {
        let (cx, cy) = self.center();
        for p in self.parts.iter_mut() {
            let a = self.rng.range(0.0, TAU);
            let s = self.rng.range(15.0, 150.0);
            p.x = cx;
            p.y = cy;
            p.vx = a.cos() * s;
            p.vy = a.sin() * s;
            p.delay = self.rng.range(0.0, 0.5);
            p.alive = true;
            p.gray = false;
        }
        self.moths.clear();
        for _ in 0..56 {
            let a = self.rng.range(0.0, TAU);
            let s = self.rng.range(40.0, 140.0);
            self.spawn_moth(cx, cy, a.cos() * s, a.sin() * s);
        }
        self.mmode = MothMode::Burst;
    }

    fn update_bang(&mut self, dt: f32) {
        let pt = self.pt;
        for p in self.parts.iter_mut() {
            if pt < 0.9 {
                p.x += p.vx * dt;
                p.y += p.vy * dt;
                let drag = 1.0 - 2.4 * dt;
                p.vx *= drag;
                p.vy *= drag;
                p.sx = p.x;
                p.sy = p.y;
            } else {
                let k = ((pt - 0.9 - p.delay) / 1.7).clamp(0.0, 1.0);
                let e = if k < 0.5 { 4.0 * k * k * k } else { 1.0 - (-2.0 * k + 2.0).powi(3) / 2.0 };
                p.x = p.sx + (p.hx - p.sx) * e;
                p.y = p.sy + (p.hy - p.sy) * e;
            }
        }
    }

    // ═════════════════════════ 绘制 ═════════════════════════
    /// 这一帧该用的帧率：黑洞几千个粒子同时在动，那几秒按 20 帧跑（电影感，终端压力减三分之一）
    fn fps_hint(&self) -> f32 {
        match self.phase {
            Phase::Hole => 20.0,
            Phase::Collapse if self.pt < 0.6 => 20.0,
            _ => 30.0,
        }
    }

    /// 调色板当前时刻（每秒 10 步）；不支持改调色板的终端固定为 0，彩虹静止
    fn pal_t(&self) -> f32 {
        if self.scr.pal { (self.t * 10.0).floor() / 10.0 } else { 0.0 }
    }

    fn draw(&mut self) {
        self.scr.clear();
        self.layer.clear();
        let mut duck_clip = None;
        let drew_logs_before = self.last_top;
        self.last_top = None;
        match self.phase {
            Phase::Boot => self.draw_logs(0.0, 0, 0.0),
            Phase::Sweep => self.draw_logs(self.pt / 1.2, 0, 0.0),
            Phase::Daily | Phase::Nap => {
                self.last_top = drew_logs_before;
                self.draw_logs(9.0, 0, 0.0)
            }
            Phase::Swarm => {
                // 只在 FATAL 砸下来那半秒震屏（震屏 = 整屏重写，很贵）
                let sx = if self.pt > 1.6 && self.pt < 2.2 { jit(self.t, 7) } else { 0 };
                let dim = if self.pt > 2.4 { 1.0 } else { 0.0 };
                if sx == 0 {
                    self.last_top = drew_logs_before;
                }
                self.draw_logs(9.0, sx, dim);
            }
            Phase::Hole => {
                // 黑洞半径按每秒 12 步长大，和吸积盘旋转同一节拍，避免整片光晕每帧重画
                let g = smooth(0.0, 1.8, (self.pt * 12.0).floor() / 12.0);
                let r = self.hole_radius() * g;
                self.draw_hole(r, g, 1.0);
                self.draw_particles(true);
                if self.duck.orb.sin() < 0.0 {
                    let (cx, cy) = self.center();
                    duck_clip = Some((cx, cy, r));
                }
            }
            Phase::Collapse => {
                let pt = self.pt;
                if pt < 0.6 {
                    let k = ((pt * 12.0).floor() / 12.0) / 0.6;
                    self.draw_hole(self.hole_radius() * (1.0 - k * k), 1.0, 1.0 + 2.5 * k);
                }
            }
            Phase::Bang => self.draw_bang(),
            Phase::End(Ending::BugWin) => {
                if self.pt < 0.6 {
                    self.draw_logs(9.0, 0, 0.0);
                } else {
                    self.draw_crash();
                }
            }
            Phase::End(_) => {
                self.last_top = drew_logs_before;
                self.draw_logs(9.0, 0, 0.0)
            }
        }
        if self.duck.visible {
            let g = self.duck.glow * if matches!(self.phase, Phase::Collapse | Phase::Bang) { 1.6 } else { 1.0 };
            self.draw_glow(g);
            self.draw_duck(duck_clip);
        }
        self.draw_moths();
        self.layer.composite(&mut self.scr);
        if self.duck.visible {
            self.draw_duck_fx();
        }
        if let Phase::Bang = self.phase {
            self.draw_flash();
        }
    }

    /// 画日志。彩虹按「第几行日志」而不是「屏幕第几行」上色，颜色跟着文字走，
    /// 这样滚动时可以用终端原生滚屏。self.last_top 有值表示上一帧也是正常日志画面，可以算滚动量。
    fn draw_logs(&mut self, sweep: f32, ox: i32, dim: f32) {
        let (w, h) = (self.scr.w, self.scr.h);
        let start = self.logs.len().saturating_sub(h);
        let span = (w + 2 * h) as f32;
        let t = self.t;
        let pt = self.pal_t();
        let full = self.logs.len() >= h && sweep >= 1.0 && ox == 0;
        if full {
            let top = self.logs[start].no;
            if let Some(prev) = self.last_top {
                let k = top.saturating_sub(prev) as usize;
                if k > 0 && k < h {
                    self.scr.scroll = k;
                }
            }
            self.last_top = Some(top);
        } else {
            self.last_top = None;
        }
        for row in 0..self.logs.len() - start {
            let line = &self.logs[start + row];
            let y = row as i32;
            let ly = (line.no % 10_000) as f32;
            let jitter = if line.kind == Kind::Error { jit(t, row as u32) } else { 0 };
            if line.kind == Kind::Fatal {
                for x in 0..w {
                    self.scr.cells[row * w + x].bg = Col::T([170, 0, 0]);
                }
            }
            let mut lx = 0usize;
            for (ci, ch) in line.text.chars().enumerate() {
                let cw = char_w(ch);
                let x = lx as i32 + ox + jitter;
                let e = line.eat.get(lx).copied().unwrap_or(0).max(if cw == 2 { line.eat.get(lx + 1).copied().unwrap_or(0) } else { 0 });
                lx += cw;
                if e == 2 {
                    continue;
                }
                let edge = (x as f32 + y as f32 * 2.0) / span;
                let tail = ci >= line.tail && line.kind != Kind::Fatal;
                let lx_f = (lx - cw) as f32;
                let (mut fg, bg): (Col, Option<Col>) = match line.kind {
                    Kind::Error => (RED.into(), None),
                    Kind::Fatal => (WHITE.into(), Some(Col::T([170, 0, 0]))),
                    Kind::Fixed => (GREEN.into(), None),
                    _ if edge > sweep => ((if line.kind == Kind::Boot && ci >= line.ok_col { GREEN } else { GRAY }).into(), None),
                    _ if sweep < 1.5 => {
                        // 彩虹扫过的那 1 秒：波峰闪白，用真彩色
                        let c = resolve(rb(Band::Text, lx_f, ly), pt);
                        let glint = (1.0 - (sweep - edge) * 14.0).clamp(0.0, 1.0);
                        (mix(c, WHITE, glint).into(), None)
                    }
                    _ => (rb(if tail || dim > 0.5 { Band::Dim } else { Band::Text }, lx_f, ly), None),
                };
                if tail {
                    if let Col::T(c) = fg {
                        fg = Col::T(scale(c, 0.42));
                    }
                }
                if e == 1 {
                    // 被吃掉的颜色：固定灰，不随彩虹流动（不产生额外重绘）
                    fg = Col::T(if tail { [62, 62, 62] } else { [138, 138, 138] });
                }
                self.scr.put_c(x, y, ch, fg, bg);
            }
        }
    }

    fn draw_glow(&mut self, g: f32) {
        if g < 0.02 {
            return;
        }
        // 光晕：固定暖白色，圆心对齐到整格、强度分 8 档——鸭鸭不换格子时这一大片背景一个字节都不用重写
        let (dx0, dy0) = (self.duck.x.round(), (self.duck.y / 2.0).round() * 2.0);
        let g = (g * 8.0).round() / 8.0;
        if g <= 0.0 {
            return;
        }
        let r = self.duck_half().0 * 2.6;
        let col: Rgb = [255, 226, 180];
        let (x0, x1) = ((dx0 - r) as i32, (dx0 + r) as i32);
        let (y0, y1) = (((dy0 - r) / 2.0) as i32, ((dy0 + r) / 2.0) as i32);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (ex, ey) = (x as f32 + 0.5 - dx0, y as f32 * 2.0 + 1.0 - dy0);
                let d = (ex * ex + ey * ey).sqrt() / r;
                if d < 1.0 {
                    if let Some(bg) = self.scr.bg_at(x, y) {
                        if let Col::T(b) = *bg {
                            let k = ((0.26 * g * (1.0 - d).powi(2)) * 24.0).round() / 24.0;
                            *bg = Col::T(mix(b, col, k));
                        }
                    }
                }
            }
        }
    }

    fn draw_duck(&mut self, clip: Option<(f32, f32, f32)>) {
        let s = self.s();
        let d = &self.duck;
        let bob = (self.t * 2.3).sin() * 1.3;
        let (hw, hh) = (9 * s, 7 * s);
        let ox = (d.x - hw as f32).round() as i32;
        let oy = (d.y - hh as f32 + bob).round() as i32;
        let (gw, gh) = (18 * s + 2, 14 * s + 2);
        let mut buf: Vec<Option<Col>> = vec![None; (gw * gh) as usize];
        for (j, row) in DUCK.iter().enumerate() {
            for (i, ch) in row.chars().enumerate() {
                if ch == '.' {
                    continue;
                }
                let ii = if d.right { i } else { 17 - i };
                for sy in 0..s {
                    for sx in 0..s {
                        let (px, py) = (ii as i32 * s + sx + 1, j as i32 * s + sy + 1);
                        let (wx, wy) = (ox + px, oy + py);
                        let _ = (wx, wy);
                        // 按鸭子身上的局部坐标取调色板色：鸭鸭移动时颜色编号不变，只换位置
                        let (lxf, lyf) = (px as f32 * 0.8 / s as f32 * 2.0, py as f32 * 0.4 / s as f32 * 2.0);
                        let c: Col = match ch {
                            'W' => rb(Band::Dim, lxf, lyf),
                            'B' => [255, 160, 40].into(),
                            'b' => [215, 95, 25].into(),
                            'e' if d.asleep => [60, 45, 70].into(),
                            'e' => [12, 10, 20].into(),
                            _ => rb(Band::Duck, lxf, lyf),
                        };
                        buf[(py * gw + px) as usize] = Some(c);
                    }
                }
            }
        }
        if d.asleep {
            // 闭眼：眼睛往后再拉一像素成一条线
            let ex = if d.right { 12 } else { 5 };
            for sy in 0..s {
                for sx in 0..s {
                    let (px, py) = (ex * s + sx + 1, 3 * s + sy + 1);
                    buf[(py * gw + px) as usize] = Some([60, 45, 70].into());
                }
            }
        }
        // 自动描一圈深色轮廓，压在彩色日志上也看得清
        let mut out = buf.clone();
        for y in 0..gh {
            for x in 0..gw {
                if buf[(y * gw + x) as usize].is_some() {
                    continue;
                }
                let near = [(-1, 0), (1, 0), (0, -1), (0, 1)].iter().any(|(dx, dy)| {
                    let (nx, ny) = (x + dx, y + dy);
                    nx >= 0 && ny >= 0 && nx < gw && ny < gh && buf[(ny * gw + nx) as usize].is_some()
                });
                if near {
                    out[(y * gw + x) as usize] = Some([14, 12, 24].into());
                }
            }
        }
        for y in 0..gh {
            for x in 0..gw {
                if let Some(c) = out[(y * gw + x) as usize] {
                    let (wx, wy) = (ox + x - 1, oy + y - 1);
                    if let Some((cx, cy, r)) = clip {
                        let (ex, ey) = (wx as f32 + 0.5 - cx, wy as f32 + 0.5 - cy);
                        if ex * ex + ey * ey < r * r {
                            continue;
                        }
                    }
                    self.layer.set(wx, wy, c);
                }
            }
        }
    }

    fn draw_duck_fx(&mut self) {
        let (hw, hh) = self.duck_half();
        let (dx, dy) = (self.duck.x, self.duck.y);
        let right = self.duck.right;
        for i in 0..self.duck.zz.len() {
            let (x, y, age) = self.duck.zz[i];
            let c = scale([200, 200, 230], 1.0 - age / 2.6);
            let ch = if age > 1.0 { 'Z' } else { 'z' };
            self.scr.put(x as i32, (y / 2.0) as i32, ch, c, None);
        }
        if let Some((text, _)) = self.duck.bubble.clone() {
            let tw = str_w(&text) as i32 + 4;
            let bx = if right { (dx + hw * 0.5) as i32 } else { (dx - hw * 0.5) as i32 - tw };
            let by = ((dy - hh) / 2.0) as i32 - 4;
            let bgc = [250, 250, 245];
            for row in 0..3 {
                for x in 0..tw {
                    self.scr.put(bx + x, by + row, ' ', BLACK, Some(bgc));
                }
            }
            self.scr.text(bx + 2, by + 1, &text, [20, 20, 30], Some(bgc));
            let tx = if right { bx + 1 } else { bx + tw - 2 };
            self.scr.put(tx, by + 3, '▀', bgc, None);
        }
    }

    /// 飞蛾画进像素层：5×3 像素，扇翅膀；有一部分是纯黑的（白身子）
    fn draw_moths(&mut self) {
        let t = self.t;
        let (cx, cy) = self.center();
        for i in 0..self.moths.len() {
            let m = &self.moths[i];
            let mut wing = match m.shade {
                0 | 1 => [238, 238, 238],
                2 => [175, 175, 175],
                3 => [120, 120, 120],
                _ => [22, 22, 22],
            };
            let mut body = if m.shade == 4 { [235, 235, 235] } else { scale(wing, 0.4) };
            if self.phase == Phase::Swarm {
                let d = ((m.x - cx).powi(2) + (m.y - cy).powi(2)).sqrt();
                let k = (1.0 - smooth(0.0, 30.0, d)) * smooth(1.5, 3.2, self.pt);
                wing = mix(wing, [30, 30, 30], k);
                body = mix(body, [10, 10, 10], k);
            }
            if m.color > 0.0 {
                let c = lolcat(m.x, m.y / 2.0, t);
                wing = mix(wing, c, m.color);
                body = mix(body, scale(c, 0.5), m.color);
            }
            let fr = &MOTH[((m.flap as i32) & 1) as usize];
            let (ox, oy) = (m.x.round() as i32 - 2, m.y.round() as i32 - 1);
            for (j, row) in fr.iter().enumerate() {
                for (k, ch) in row.chars().enumerate() {
                    let c = match ch {
                        '#' => wing,
                        'o' => body,
                        _ => continue,
                    };
                    self.layer.set(ox + k as i32, oy + j as i32, c);
                }
            }
        }
    }

    /// 黑洞本体：阴影 + 光子环 + 倾斜的吸积盘（像浴缸下水口）+ 被引力透镜折上去的盘背面
    fn draw_hole(&mut self, r: f32, fade: f32, flare: f32) {
        if r < 0.3 {
            return;
        }
        const RAMP: &[char] = &[' ', '.', ':', '-', '=', '+', '*', '#', '%', '@'];
        let (cx, cy) = self.center();
        // 吸积盘按每秒 12 步旋转（不必每帧重画整片光晕），并且只扫黑洞外接框
        let t = (self.t * 12.0).floor() / 12.0;
        let reach = r * 4.2;
        let (x0, x1) = ((cx - reach).max(0.0) as usize, ((cx + reach) as usize + 1).min(self.scr.w));
        let (y0, y1) = (((cy - reach) / 2.0).max(0.0) as usize, (((cy + reach) / 2.0) as usize + 1).min(self.scr.h));
        for y in y0..y1 {
            for x in x0..x1 {
                let dx = x as f32 + 0.5 - cx;
                let dy = y as f32 * 2.0 + 1.0 - cy;
                let d = (dx * dx + dy * dy).sqrt();
                if d > r * 4.2 {
                    continue;
                }
                let tilt = 0.24;
                let (u, v) = (dx, dy / tilt);
                let rd = (u * u + v * v).sqrt();
                let ang = v.atan2(u);
                let (din, dout) = (r * 1.5, r * 3.8);
                let mut disk = 0.0;
                if rd > din && rd < dout {
                    let s = (rd - din) / (dout - din);
                    let swirl = 0.6 + 0.4 * (ang * 3.0 - t * 7.0 * (r / rd) + rd * 0.35).sin();
                    let doppler = 1.0 - 0.45 * ang.cos();
                    disk = (1.0 - s).powf(1.4) * swirl * doppler * smooth(0.0, 0.15, s);
                }
                let mut halo = 0.0;
                let (h1, h2) = (r * 1.04, r * 1.8);
                if d > h1 && d < h2 {
                    let s = (d - h1) / (h2 - h1);
                    let vert = (dy / d).abs();
                    let flick = 0.75 + 0.25 * (dy.atan2(dx) * 5.0 + t * 4.0).sin();
                    halo = (1.0 - s).powi(2) * (0.3 + 0.7 * vert) * flick;
                }
                let ring = (-((d - r * 1.02) / (r * 0.07 + 0.35)).powi(2)).exp() * flare;
                let front = dy > 0.0 && disk > 0.0;
                if d < r && !front {
                    continue;
                }
                let i = (disk * 1.05).max(halo).max(ring) * fade;
                if i < 0.04 {
                    continue;
                }
                let c = heat(i);
                let ch = RAMP[((i * 9.99) as usize).min(9)];
                self.scr.put(x as i32, y as i32, ch, c, Some(scale(c, 0.38 * i)));
            }
        }
    }

    fn draw_particles(&mut self, in_hole: bool) {
        let (cx, cy) = self.center();
        let r_hole = self.hole_radius() * smooth(0.0, 1.8, self.pt);
        let pt = self.pal_t();
        for i in 0..self.parts.len() {
            let p = &self.parts[i];
            if !p.alive {
                continue;
            }
            let (dx, dy) = (p.x - cx, p.y - cy);
            let d = (dx * dx + dy * dy).sqrt();
            // 粒子沿用原来那个字的调色板编号：大爆炸归位时和日志颜色严丝合缝，也不用每帧换色
            let mut col = p.col;
            if !in_hole && p.gray {
                col = lolcat(p.hx, p.hy / 2.0, pt).into();
            }
            let mut ch = p.ch;
            if in_hole {
                if d < r_hole {
                    continue;
                }
                let near = 1.0 - smooth(r_hole * 1.2, r_hole * 4.0, d);
                if near > 0.05 {
                    let k = (near * 6.0).round() / 6.0;
                    col = mix(resolve(col, pt), heat(0.6 + 0.4 * k), k * 0.8).into();
                }
                let sp = (p.vx * p.vx + p.vy * p.vy).sqrt();
                if char_w(ch) == 1 && sp > 18.0 && near > 0.15 {
                    let a = (p.vy / 2.0).atan2(p.vx).rem_euclid(std::f32::consts::PI);
                    ch = match (a / std::f32::consts::FRAC_PI_4).round() as i32 {
                        1 => '\\',
                        2 => '|',
                        3 => '/',
                        _ => '-',
                    };
                }
            }
            let (x, y) = (p.x.floor() as i32, (p.y / 2.0).floor() as i32);
            self.scr.put(x, y, ch, col, None);
        }
    }

    fn draw_bang(&mut self) {
        let pt = self.pt;
        let (cx, cy) = self.center();
        if pt < 1.3 {
            let rr = pt * 170.0;
            let fade = 1.0 - pt / 1.3;
            let n = (rr * 5.0) as usize + 8;
            for k in 0..n {
                let a = k as f32 / n as f32 * TAU;
                let (x, y) = (cx + rr * a.cos(), (cy + rr * a.sin()) / 2.0);
                self.scr.put(x as i32, y as i32, if k % 3 == 0 { '*' } else { '·' }, heat(fade), None);
            }
        }
        self.draw_particles(false);
    }

    fn draw_flash(&mut self) {
        let pt = self.pt;
        if pt < 0.5 {
            // 白闪分 4 档递减：看起来一样是一闪，但整屏只重写 4 次而不是 15 次
            let f = (1.0 - ((pt / 0.5) * 4.0).floor() / 4.0).powi(2);
            for c in self.scr.cells.iter_mut() {
                c.bg = Col::T(mix(resolve(c.bg, 0.0), WHITE, f));
                c.fg = Col::T(mix(resolve(c.fg, 0.0), WHITE, f * 0.6));
            }
        }
    }

    /// 恶搞崩溃屏：bug 连蓝屏的蓝都吃掉了；鸭鸭漂过的地方变回彩虹
    fn draw_crash(&mut self) {
        let (w, h) = (self.scr.w, self.scr.h);
        let wipe = if self.pt > 4.0 { self.duck.x as i32 } else { -1000 };
        let flick = self.pt < 0.9 && ((self.pt * 20.0) as i32) % 2 == 0;
        for y in 0..h {
            for x in 0..w {
                let bg = if (x as i32) < wipe {
                    rb(Band::Dim, x as f32 * 0.5, y as f32 * 0.5)
                } else if flick {
                    Col::T([20, 20, 20])
                } else {
                    Col::T([78, 78, 78])
                };
                self.scr.cells[y * w + x] = Cell { ch: ' ', fg: Col::T(WHITE), bg, w: 0 };
            }
        }
        let s = if h >= 40 { 3 } else { 2 };
        let mx = (w as f32 * 0.12) as i32;
        let my = (h as f32 * 0.16) as i32;
        // 大号表情：先画哭脸，再把鸭鸭漂过的部分换成笑脸
        let sad_lines = [
            "你的电脑遇到了太多 bug，需要重新启动。",
            "它们连蓝屏的蓝色都吃掉了。",
            "",
            "正在收集 bug：99% 完成",
            "",
            "如需了解详细信息，请咨询一只鸭子。",
            "终止代码：TOO_MANY_BUGS",
        ];
        let happy_lines = [
            "你的电脑遇到了一只鸭子。",
            "一切都变好了。",
            "",
            "正在收集彩虹：100% 完成",
            "",
            "嘎。",
            "终止代码：QUACK",
        ];
        let face_px_y = my * 2;
        let mut sad = Layer::new(w, h);
        let mut happy = Layer::new(w, h);
        blit_glyph(&mut sad, &COLON, mx, face_px_y, s, WHITE);
        blit_glyph(&mut sad, &LPAREN, mx + 4 * s, face_px_y, s, WHITE);
        blit_glyph(&mut happy, &COLON, mx, face_px_y, s, WHITE);
        blit_glyph(&mut happy, &RPAREN, mx + 4 * s, face_px_y, s, WHITE);
        for (i, px) in self.layer.px.iter_mut().enumerate() {
            let x = (i % w) as i32;
            let src = if x < wipe { &happy } else { &sad };
            if let Some(c) = src.px[i] {
                *px = Some(c);
            }
        }
        let ty = my + 5 * s + 2;
        for (i, (a, b)) in sad_lines.iter().zip(happy_lines.iter()).enumerate() {
            let y = ty + i as i32 + if i >= 1 { 1 } else { 0 };
            let mut x = mx;
            let pct = if i == 3 && x >= wipe && ((self.t * 2.0) as i32) % 2 == 0 { a.replace("99%", "98%") } else { a.to_string() };
            let sad_s: Vec<char> = pct.chars().collect();
            let hap_s: Vec<char> = b.chars().collect();
            let n = sad_s.len().max(hap_s.len());
            for k in 0..n {
                let (ch, happy_side) = if x < wipe { (hap_s.get(k).copied(), true) } else { (sad_s.get(k).copied(), false) };
                let Some(ch) = ch else {
                    x += 1;
                    continue;
                };
                let _ = happy_side;
                let bgc = self.scr.cells[(y.clamp(0, h as i32 - 1) as usize) * w + (x.clamp(0, w as i32 - 1) as usize)].bg;
                self.scr.put_c(x, y, ch, WHITE.into(), Some(bgc));
                x += char_w(ch) as i32;
            }
        }
    }
}

// ───────────────────────── 离线渲染（开发自查） ─────────────────────────
fn frame_html(scr: &Screen, label: &str, out: &mut String, t: f32) {
    let _ = write!(out, "<h3>{}</h3><pre>", label);
    for y in 0..scr.h {
        let mut x = 0;
        while x < scr.w {
            let c = scr.cells[y * scr.w + x];
            let (fg, bg) = (resolve(c.fg, t), resolve(c.bg, t));
            if c.w == 2 {
                x += 1;
                continue;
            }
            let ch = match c.ch {
                '<' => "&lt;".to_string(),
                '>' => "&gt;".to_string(),
                '&' => "&amp;".to_string(),
                _ => c.ch.to_string(),
            };
            let wide = if c.w == 1 { "display:inline-block;width:2ch;" } else { "" };
            let _ = write!(
                out,
                "<span style=\"{}color:#{:02x}{:02x}{:02x};background:#{:02x}{:02x}{:02x}\">{}</span>",
                wide, fg[0], fg[1], fg[2], bg[0], bg[1], bg[2], ch
            );
            x += if c.w == 1 { 2 } else { 1 };
        }
        out.push('\n');
    }
    out.push_str("</pre>");
}

fn frame_json(scr: &Screen, label: &str, out: &mut String, t: f32) {
    let _ = write!(out, "{{\"label\":\"{}\",\"w\":{},\"h\":{},\"c\":[", label, scr.w, scr.h);
    for c in &scr.cells {
        let (fg, bg) = (resolve(c.fg, t), resolve(c.bg, t));
        let ch = match c.ch {
            '"' => "\\\"".to_string(),
            '\\' => "\\\\".to_string(),
            x => x.to_string(),
        };
        let _ = write!(
            out,
            "[\"{}\",\"{:02x}{:02x}{:02x}\",\"{:02x}{:02x}{:02x}\",{}],",
            ch, fg[0], fg[1], fg[2], bg[0], bg[1], bg[2], c.w
        );
    }
    out.pop();
    out.push_str("]},");
}

fn dump(args: &[String]) {
    let get = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let path = get("--dump").unwrap();
    let (w, h) = get("--size")
        .and_then(|s| {
            let (a, b) = s.split_once('x')?;
            Some((a.parse().ok()?, b.parse().ok()?))
        })
        .unwrap_or((200, 56));
    let seed = get("--seed").and_then(|s| s.parse().ok()).unwrap_or(7);
    let mut times: Vec<f32> = get("--at").unwrap_or("5".into()).split(',').filter_map(|s| s.parse().ok()).collect();
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut app = App::new(w, h, seed);
    app.forced = get("--ending").and_then(|s| s.parse::<usize>().ok()).map(|i| ENDINGS[i % 4]);
    let mut html = String::from(
        "<!doctype html><meta charset=utf-8><style>body{background:#111;color:#ccc;font-family:sans-serif}pre{font-family:Consolas,'Cascadia Mono',monospace;font-size:9px;line-height:1;background:#000;display:inline-block;margin:0 0 8px}</style>",
    );
    let dt = 1.0 / 60.0;
    if path == "bench" {
        // 按真实帧率模拟：统计往终端写的字节（每秒平均 / 最忙的一秒）和自身计算耗时
        let fps: f32 = get("--fps").and_then(|s| s.parse().ok()).unwrap_or(30.0);
        let secs: f32 = get("--secs").and_then(|s| s.parse().ok()).unwrap_or(150.0);
        let mut out = String::new();
        let mut costs: Vec<f64> = Vec::new();
        let mut per_sec: Vec<usize> = vec![0; secs as usize + 2];
        let mut sec_phase: Vec<String> = vec![String::new(); secs as usize + 2];
        let mut by_phase: Vec<(String, usize, usize, f64, f32)> = Vec::new();
        let mut total = 0usize;
        let mut sim = 0.0f32;
        let mut max_logs = 0;
        let mut max_parts = 0;
        let mut last_pal = f32::NAN;
        let mut pal_bytes = 0usize;
        let mut frames = 0usize;
        while sim < secs {
            let dt = 1.0 / app.fps_hint().min(fps);
            frames += 1;
            let t0 = Instant::now();
            app.update(dt);
            app.draw();
            out.clear();
            if app.pal_t() != last_pal {
                last_pal = app.pal_t();
                palette_osc(&mut out, last_pal);
                pal_bytes += out.len();
            }
            app.scr.flush(&mut out);
            let ms = t0.elapsed().as_secs_f64() * 1000.0;
            sim += dt;
            costs.push(ms);
            total += out.len();
            per_sec[sim as usize] += out.len();
            sec_phase[sim as usize] = format!("{:?} pt={:.1}", app.phase, app.pt);
            max_logs = max_logs.max(app.logs.len());
            max_parts = max_parts.max(app.parts.len());
            let name = match app.phase {
                Phase::End(_) => "Ending".to_string(),
                p => format!("{:?}", p),
            };
            match by_phase.iter_mut().find(|p| p.0 == name) {
                Some(p) => {
                    p.1 += out.len();
                    p.2 += 1;
                    p.3 += ms;
                    p.4 += dt;
                }
                None => by_phase.push((name, out.len(), 1, ms, dt)),
            }
        }
        costs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let avg = costs.iter().sum::<f64>() / costs.len() as f64;
        let p99 = costs[costs.len() * 99 / 100];
        println!("{}x{} @ {}fps，模拟 {} 秒", app.scr.w, app.scr.h, fps, secs);
        println!("  终端输出：平均 {} KB/s，最忙的一秒 {} KB/s", total / secs as usize / 1024, per_sec.iter().max().unwrap() / 1024);
        println!("  自身计算：平均 {:.2} ms/帧，p99 {:.2} ms，最大 {:.2} ms，≈ 单核 {:.1}%", avg, p99, costs[costs.len() - 1], avg * frames as f64 / secs as f64 / 10.0);
        let mut idx: Vec<usize> = (0..per_sec.len()).collect();
        idx.sort_by_key(|&i| std::cmp::Reverse(per_sec[i]));
        for &i in idx.iter().take(4) {
            println!("  忙的一秒：第 {} 秒 {} KB（{}）", i, per_sec[i] / 1024, sec_phase[i]);
        }
        println!("  其中调色板：平均 {} KB/s", pal_bytes / secs as usize / 1024);
        println!("  上限检查：日志最多 {} 行，粒子最多 {} 个", max_logs, max_parts);
        for (n, b, f, ms, secs) in by_phase {
            println!("  {:<10} {:>6} KB/s  {:>5.2} ms/帧", n, (b as f32 / secs / 1024.0) as usize, ms / f as f64);
        }
        return;
    }
    let json = path.ends_with(".json");
    let mut js = String::from("[");
    for at in times {
        while app.t < at {
            app.update(dt);
            app.draw();
        }
        let label = format!("t={:.1}s  phase={:?}  pt={:.1}  moths={}", app.t, app.phase, app.pt, app.moths.len());
        if json {
            frame_json(&app.scr, &label, &mut js, app.pal_t());
        } else {
            frame_html(&app.scr, &label, &mut html, app.pal_t());
        }
    }
    if json {
        js.pop();
        js.push(']');
        std::fs::write(&path, js).unwrap();
    } else {
        std::fs::write(&path, html).unwrap();
    }
}

// ───────────────────────── 启动与主循环 ─────────────────────────
fn relaunch_fullscreen() -> bool {
    let Ok(exe) = std::env::current_exe() else { return false };
    let mut cands = vec![std::path::PathBuf::from("wt.exe")];
    if let Ok(la) = std::env::var("LOCALAPPDATA") {
        cands.push(std::path::Path::new(&la).join("Microsoft\\WindowsApps\\wt.exe"));
    }
    for wt in cands {
        if std::process::Command::new(&wt).args(["-w", "new", "-F"]).arg(&exe).arg("--inner").spawn().is_ok() {
            return true;
        }
    }
    false
}

fn restore(con: &win::Console) {
    let mut o = std::io::stdout();
    let _ = write!(o, "\x1b]104\x07\x1b[0m\x1b[?7h\x1b[?25h\x1b[?1049l\x1b]0;\x07");
    let _ = o.flush();
    con.restore();
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--dump") {
        dump(&args);
        return Ok(());
    }
    let inner = args.iter().any(|a| a == "--inner" || a == "--here");
    if !inner && relaunch_fullscreen() {
        return Ok(());
    }

    let con = win::Console::open();
    let mut stdout = std::io::stdout();
    write!(stdout, "\x1b[?1049h\x1b[?25l\x1b[?7l\x1b[48;2;0;0;0m\x1b[2J")?;
    stdout.flush()?;

    let (w, h) = con.size();
    let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos() as u64;
    let mut app = App::new(w, h, seed);
    // Windows Terminal 支持改调色板和同步刷新；老式 cmd 窗口退回静态彩虹 + 20 帧
    let modern = std::env::var_os("WT_SESSION").is_some();
    app.scr.pal = modern;
    let mut out = String::with_capacity(1 << 20);
    let fast = Duration::from_micros(33_333); // 屏保标准 30 帧
    let slow = Duration::from_millis(50); // 终端跟不上时降到 20 帧
    let mut frame = if modern { fast } else { slow };
    let mut write_ema = 0.0f32; // 写终端耗时的滑动平均（毫秒），反映终端消化得过来没有
    let mut last_pal = f32::NAN;
    let mut last = Instant::now();

    loop {
        if con.key_pressed() {
            break;
        }
        let (w, h) = con.size();
        if (w, h) != (app.scr.w, app.scr.h) {
            app.resize(w, h);
            last_pal = f32::NAN;
        }
        let now = Instant::now();
        let dt = (now - last).as_secs_f32().min(0.1);
        last = now;

        app.update(dt);
        app.draw();
        out.clear();
        if app.title_dirty {
            let _ = write!(out, "\x1b]0;{}\x07", app.title);
            app.title_dirty = false;
        }
        if app.scr.pal && app.pal_t() != last_pal {
            last_pal = app.pal_t();
            palette_osc(&mut out, last_pal);
        }
        app.scr.flush(&mut out);
        let t0 = Instant::now();
        if out.len() > 16 {
            let mut lock = stdout.lock();
            lock.write_all(out.as_bytes())?;
            lock.flush()?;
        }
        let wms = t0.elapsed().as_secs_f32() * 1000.0;
        write_ema = write_ema * 0.9 + wms * 0.1;
        if modern {
            if frame == fast && write_ema > 18.0 {
                frame = slow;
            } else if frame == slow && write_ema < 6.0 {
                frame = fast;
            }
        }
        let budget = frame.max(Duration::from_secs_f32(1.0 / app.fps_hint()));
        let spent = now.elapsed();
        if spent < budget {
            std::thread::sleep(budget - spent);
        }
    }

    restore(&con);
    Ok(())
}

/// 每秒变 15 次的 -1/0/1 抖动（按时间片和行号哈希，不必每帧都换，省终端带宽）
fn jit(t: f32, salt: u32) -> i32 {
    let k = ((t * 15.0) as u32).wrapping_mul(2654435761) ^ salt.wrapping_mul(40503);
    (k >> 7) as i32 % 3 - 1
}
