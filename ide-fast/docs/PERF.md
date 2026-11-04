# ide-fast 性能基线

工具: `tests/perf.ps1` (启动 + 空闲资源, 对任意 exe) 和 `src/app/perf_bench.rs` (引擎无头基准, `cargo test -p ide-fast --release perf_bench -- --ignored --nocapture --test-threads=1`)。
改动影响启动 / 打开 / 绘制路径时: 改前 `perf.ps1 -Json before.json`, 改后 `-Compare before.json`, 提交说明写出对比 (见 CODING-STANDARDS §8)。

## 机器

13th Gen Intel Core i7-13700F, 32 GB, Windows 11 (10.0.26300), NVIDIA 独显 (OpenGL: nvoglv64), 系统里装有搜狗拼音输入法。
exe: `release` 配置 (lto=thin, codegen-units=1), GUI 子系统。2026-11-03 在提交 7619835 之后测得, N=10 冷启动 (`IDE_FAST_SETTINGS=off`, `--new`)。

## 启动 (进程入口 `main` 起算, 毫秒)

| 追踪点 | 中位数 | p90 |
|---|---|---|
| main enter | 0 | 0 |
| settings loaded | 0.3 | 1.6 |
| before run_native | 0.5 | 1.8 |
| event loop builder (winit 事件循环已建) | 0.6 | 2.0 |
| window builder (窗口 + GL 开始创建) | 10.0 | 11.8 |
| window + GL context ready | 238.4 | 388.3 |
| fonts installed | 248.1 | 399.3 |
| App::new done | 248.4 | 399.6 |
| first frame | 251.5 | 402.2 |
| 外部墙钟: Start-Process -> 看到首帧 | 294.3 | 447.5 |

归因: 我们自己的代码 (设置读取 → 进 `run_native` → 字体 → `App::new`) 合计约 **15 ms**; 其余全是 `window builder` → `window + GL context ready` 的 **约 228 ms (p90 约 378 ms)**,
即 winit 创建窗口 + glutin 选像素格式 / 建 OpenGL 上下文 (含显卡驱动 `nvoglv64.dll` 加载与初始化, 以及输入法 DLL 被注入窗口)。
eframe 没有在这两步之间暴露钩子, 无法再细分窗口创建和 GL 初始化 (需要 ETW / 分析器)。进程在 `main` 之前约 40 ms (墙钟 294 - 首帧 251) 是系统加载器 + Rust 运行时。
未复现「窗口就绪前 500-800 ms」: 本机冷启动该段约 240 ms; 更高的数字多半来自更慢的显卡驱动 / 杀毒软件扫描首次运行的 exe。

## 空闲资源 (首帧后 3 s)

| 项 | 中位数 |
|---|---|
| 工作集 | 134.6 MB |
| 私有内存 | 132 MB |
| 线程数 | 17 |
| CPU (随后 2 s, 占一个核的百分比) | 0 % |

## 引擎 (无头, release 测试构建, 毫秒)

| 项 | 毫秒 |
|---|---|
| 打开 5 MB 文本 (约 7.6 万行) | 20.3 |
| 打开 30 MB 文本 (约 45 万行) | 128.4 |
| 打开 20 万行 | 59.1 |
| 打开 1 MB 单行 | 3.2 |
| 一帧: 空文档 (整个界面) | 0.10 |
| 一帧: 20 万行文档, 静止 | 0.25 |
| 一帧: 20 万行文档, 滚动 (光标每帧跳 97 行, 含排版 + 三角化) | 0.57 |
| 一帧: 1 MB 单行文档, 首帧 (单次) | 3.2 |
| 一帧: 1 MB 单行文档, 稳态 | **85.9** (已知热点: 每帧都要处理一行 100 万字符) |
| 全部查找, 20 万行, 命中很多 (上限 5001) | 17.7 |
| 全部查找, 20 万行, 无命中 | 21.2 |
| 全部查找, 20 万行, 正则 | 20.3 |
| 在 20 万行文档里做 10k 次独立编辑 | 64.9 |
| 撤销全部 (撤销栈上限 5000 步, 实际 5000 步) | 0.92 |
| 1000 行文档里 10k 次编辑 + 全部撤销 | 54.7 |

说明: 滚动一帧用「光标每帧跳到新行 + reveal」驱动 (headless 没有鼠标滚轮); 帧时间包含 `ui_main` + 三角化, 不含 GPU 上传 / 驱动。
撤销栈上限是 5000 步 (doc.rs), 所以「撤销 10k 次编辑」最多撤得回 5000 步。

## 输入法的 libpng 警告

窗口创建时 stderr 里出现 10 行 `libpng warning: iCCP: known incorrect sRGB profile` **不是我们的代码**:
- ide-fast.exe 里没有 `libpng` / `known incorrect sRGB` 字符串; 窗口图标走纯 Rust 的 `png` crate (appicon.rs), 它不产生这种警告。
- 扫描进程加载的 77 个模块, 含这句话的只有 `C:\WINDOWS\system32\SogouPY.ime` 和 `SogouTSF.ime` (搜狗拼音输入法, 窗口一建立就被注入并加载自己的皮肤 PNG)。
- 发行版是 GUI 子系统, 没有控制台, 这些输出看不到; 只有重定向 stderr 时才会出现。
要让它消失只能对窗口关闭输入法 (`with_ime_allowed(false)`), 那样中日文输入就废了, 所以不改。

## 优化日志

每步 N=10 冷启动 (`perf.ps1`), 与基线 exe 交替跑两轮取一致的结果 (机器噪声约 ±15 ms)。

### 步骤 1: 字体后台读取 + 窗口先隐藏、首帧后显示 (perf(ide-fast): ...)

- `fonts::prefetch`: 设置读出后立刻在线程里读 CJK fallback / 编辑器字体文件 (几十 MB), 与窗口 + OpenGL 创建 (约 230 ms) 并行; 创建回调里只 join + `set_fonts`。
  `window + GL ready -> fonts installed` 从 约 10 ms 降到 0.2 ms。冷磁盘缓存时收益更大 (读文件本来在关键路径上)。
- 窗口以 `with_visible(false)` 创建, `ui_main` 第一帧画完后 `ViewportCommand::Visible(true)`: 用户看不到未绘制的空白窗口。新追踪点 `window shown`。
- 结果 (两轮): 首帧中位数 258.0 / 258.2 -> 236.8 / 245.0 ms (约 -5% ~ -8%); 外部墙钟 299 / 313.6 -> 285.2 / 284.5 ms。
- 内存 / 线程 / 空闲 CPU 不变 (134.8 MB, 17 线程, 0 %)。

### 试了没采用: 发行配置 lto="fat" + strip + opt-level=3

- 完整重编 5 分 52 秒 (thin 约 3 分钟量级), exe 16.5 MB -> 15.8 MB (-4.2%); 启动首帧 256 -> 258.5 ms (噪声内), 空闲内存 / 线程 / CPU 无变化。
  启动 95% 在驱动 / winit / GL 里, 我们的代码在其中不起作用 -> 不值得更慢的构建, 已还原。
- `panic=abort` 没试: 后台线程 (grep / AI / curl / 字体预读) 现在 panic 只会让那个线程结束, 改 abort 会让整个编辑器直接崩溃并丢掉未保存内容, 属于功能降级。保持 unwind。
