# just bug · 开发者文档

给想自己编译、改画面、加插曲的人。玩家看 [README](../README.md) 就够了。

## 一分钟认识代码

- 零第三方依赖的 Rust（edition 2024），只调 Windows 的 `kernel32`。exe 只依赖系统自带 DLL。
- `src/main.rs`：主线（日常 → 打盹 → 黑洞 → 大爆炸 → 结局）、渲染、自动降档和命令行入口。文件很长，改插曲时不用通读。
- `src/scenes/`：11 个插曲，一个文件一个。接口和约定写在 `src/scenes/mod.rs` 开头。
- `tools/`：离线审图用的 Python 脚本（需要 Pillow）。

## 构建

本项目用 rustup 的 **GNU 工具链**，不需要装 MSVC：

```powershell
rustup toolchain install stable-x86_64-pc-windows-gnu
powershell -NoProfile -File build.ps1
```

产物在 `target\release\just-bug.exe`。

坑：
- Git Bash 自带一个 `link` 命令，会挡住 MSVC 的链接器。用 PowerShell 跑 `build.ps1`，别在 Git Bash 里构建。
- 带 `windows-link` 的依赖在 GNU 工具链下需要汇编器。这也是项目坚持零依赖的原因之一。
- release 配置 `panic = "abort"`。程序装了 panic hook：出事时先还原终端，再把信息写到 `%TEMP%\just-bug-panic.log`。

## 开发时千万别做的事

**不要在开发终端里直接运行不带 `--dump` 的 exe。** 它会立刻拉起全屏屏保。下面所有自查都走离线模拟，不碰真终端。

## 命令行参数

正常使用不带任何参数。以下参数供开发自查：

| 参数 | 含义 |
|---|---|
| `--dump <文件.json>` | 离线模拟，把指定时刻的画面导出成 JSON，不打开终端 |
| `--dump bench` | 离线模拟并统计每秒写往终端的字节数和每帧计算耗时 |
| `--size WxH` | 模拟的终端尺寸（字符），默认 `200x56` |
| `--at t1,t2,…` | 导出哪些时刻（秒）。配合 `--inter` 时表示插曲开始后第几秒 |
| `--inter N` | 每轮都固定插第 N 号插曲（0–10） |
| `--ending N` | 固定结局：0 心、1 Bug 赢了、2 调试成功、3 气球 |
| `--seed N` | 随机种子，默认 7 |
| `--secs N` | bench 模拟时长（秒），默认 150 |
| `--fps N` | bench 模拟帧率，默认 60 |
| `--lite` | 用省流量档（低带宽画法） |
| `--inner` / `--here` | 内部使用：已在全屏终端里，不再重新拉起窗口 |

## 看画面：导出与拼图

审图只看一张拼图，不用一帧帧翻：

```powershell
.\target\release\just-bug.exe --dump preview\s.json --inter 7 --at 0.5,3,6,9,12,15,18,20 --size 240x63
python tools\sheet.py preview\s.json preview\s.png 0.4 2
```

`sheet.py` 的参数是 `输入.json 输出.png [缩放=0.4] [列数=2]`。只想要单帧 PNG，用 `tools\render_frames.py 输入.json 输出目录 缩放`。

每个插曲至少看三档尺寸：`240x63`（大屏）、`80x24`（标准）、`40x12`（极小窗口兜底）。

## 性能与质量线

```powershell
.\target\release\just-bug.exe --dump bench --inter 7 --size 240x63 --secs 300
.\target\release\just-bug.exe --dump bench --inter 7 --size 240x63 --secs 300 --lite --fps 30
```

看输出里 `Inter:` 那一行。合并前必须满足：

| 项目 | 要求 |
|---|---|
| 插曲时长 | 15–22 秒；开场从日志画面过渡进来，最后约 0.5 秒回到日志画面 |
| 尺寸 | 40x12 到 320x90 都不能 panic，画面不能乱 |
| 满效果（240x63） | ≤ 2.5 MB/s，且 ≤ 2 ms/帧 |
| 省流量（240x63） | ≤ 0.6 MB/s |
| 整体（240x63，含主线） | 平均 ≤ 2 MB/s |

这些都是离线数字，不含真实终端的渲染开销。改动大的画面，最后还要在 Windows Terminal 里真机看一遍。

### 画面为什么省带宽

自动降档之外，所有档位都有这些优化，新代码要顺着它们写：

- **彩虹靠调色板流动**：日志彩虹写的是 256 色索引，每秒只用 OSC 4 改 10 次调色板，终端自己重新上色。所以彩虹优先用 `rb(Band::…)`，逐帧变化的真彩色只在 `app.hi` 为真时用。
- 日志滚动用终端原生滚屏（`CSI S`），只补新的一行。
- 逐格差量刷新：只有变了的格子才写出去，并用同步输出（`?2026`）包起来。
- 长时间挂机：时间每 50 分钟回绕，所有容器都有上限。退出时用 OSC 104 还原调色板。

### 自动降档

程序每帧看「计算 + 写终端」用掉多少时间预算，持续吃掉大半就降一档，长时间很闲再升回去。同一档被降两次就不再升，避免每轮黑洞来回切。老式控制台直接从第 2 档起步。

| 档位 | 画法 |
|---|---|
| 0 | 满效果 60 帧（默认） |
| 1 | 满效果 30 帧 |
| 2 | 省流量 30 帧：光晕固定色、黑洞降步数、飞字限量、鸭鸭用调色板 |
| 3 | 省流量 20 帧 |

## 加一个新插曲

1. 读 `src/scenes/mod.rs` 开头的约定，再看 `src/scenes/runner.rs` 当参考写法。
2. 新建 `src/scenes/<名字>.rs`，实现 `Interlude` trait：
   - `name()`：简短中文名。
   - `update(app, dt)`：每帧逻辑，返回 `true` 表示结束。结束后自动回到「打盹」主线。
   - `draw(app)`：画字符层和像素层。
   - `draw_over(app)`（可选）：盖在像素层上面的文字。
3. 在 `mod.rs` 里加 `mod`、把 `COUNT` 加一，并在 `make()` 里接上新编号。
4. 用上面的命令导出三档尺寸的拼图，跑满效果和省流量两次 bench，达标再合并。
5. 在 README 的插曲一览里补一行。

几条硬规矩：

- **只借玩法**。名字、角色、美术、配色、音效、标志性 UI 一律不用原作的，画面只用终端字符加我们自己的像素鸭和飞蛾。
- **结尾必须有鸭鸭 × Bug 的梗**。
- 插曲期间主线不再驱动飞蛾，鸭鸭只做位置缓动；要瞬移就同时设 `x` 和 `tx`。
- 坐标：字符格用 `app.scr`；像素层 `app.layer` 是半格像素，`py = 行 × 2`。
- 可以 push 日志、`set_title`、`quack` 冒泡，让插曲和主线的「终端里的故事」接得上。

## 压力测试

发布前跑一遍完整矩阵：5 个尺寸（40x12、80x24、120x36、240x63、320x90）× 2 个 seed，满效果每次 900 秒，240x63 另跑一次省流量档：

```powershell
.\target\release\just-bug.exe --dump bench --size 320x90 --seed 2 --secs 900
```

要求零 panic，240x63 整体平均 ≤ 2 MB/s。上一次的完整记录见 [T3_ACCEPTANCE.md](T3_ACCEPTANCE.md)。

## 发布清单

1. `build.ps1` 构建 release，把 `target\release\just-bug.exe` 复制到仓库根目录。
2. 记下 exe 大小和 SHA256，写进发布说明。
3. 压力测试矩阵通过。
4. README 的插曲表、`docs/` 下的图和当前版本一致。
5. 在 Windows Terminal 里真机看一轮：调色板流动、自动降档、插曲观感。
6. exe 没有签名，发布说明里保留 SmartScreen 提示。
7. 公开仓库前，确认 git 提交邮箱不是私人邮箱。
