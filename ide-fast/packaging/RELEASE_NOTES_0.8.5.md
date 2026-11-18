# IDE1.0 v0.8.5 — 启动与内核全面加速 · IDE1.0 统一命名 · 数学化性能框架升级

## 1. 启动、操作与文本内核性能优化
- **极速无阻塞冷启动**：启动路径 `apply_settings` 零磁盘写盘，延迟初始化非首屏字体、图标与系统字体枚举；新增 `theme_tc` 主题缓存与无落盘切换路径 `set_theme_plugin_no_save`。
- **文档与文本核加速**：
  - `editcore` 引入单遍 SIMD `normalize_to_lf` / `normalize_to_lf_owned`、`detect_charset` `0x1B` 提前短路、小选区上下文按需构建。
  - `Doc` 撤销栈采用 `VecDeque<Undo>` O(1) 环形淘汰，`rows_for_mode` 小选区栈路径，单行 `insert-text` / `Doc::splice` 局部行比较。
  - `TypeConfig::highlight_line` 将整行注释检查前移、新增纯 ASCII 零堆分配快路径及 SIMD `str::find`。
  - Minimap `line_w_capped` 零堆分配扫描（前 256 字符 + 超长缩进行 2048 字节探测）。

## 2. `IDE1.0` 品牌与环境变量全面对齐
- 发行包名、Windows 版本资源（`IDE1.0.exe`）、打印页眉、标签跳转提示、文档与 E2E 矩阵全面由 `ide-fast` 对齐至 `IDE1.0`。
- 环境变量优先支持 `IDE10_TRACE`、`IDE10_SETTINGS`、`IDE10_BACKDROP`、`IDE10_MULTI`、`IDE10_NO_NETWORK`、`IDE10_OFFLINE`（保留 `IDE_FAST_*` 向下兼容）；单实例 IPC 握手升级为 `IDE1.0-OPEN`（兼容旧握手）；更新暂存前缀改为 `IDE1.0-update`。

## 3. 对抗式审核全面加固（10 项全量清零）
- **P0 修复**：`Doc` 新增 `line_aligned` 不变量，确保插入/删行后正确回退严格 LCS 增量行对比，杜绝等长快路径误判。
- **P1 修复**：
  - 移除终端 `\\.\PHYSICALDRIVE0` 提权探测，新增 `output_rev` 与内容哈希缓存键解决同长度输出替换布局缓存陈旧问题。
  - 修复单行多命中搜索/替换 $O(H \cdot L)$ 退化，采用 ASCII/UTF-8 增量游标 $O(L + H)$。
  - `theme_luminance` 新增 2s TTL 缓存，并在切换主题与窗口重新聚焦时调用 `invalidate_cache()`。
  - `e2e-run-all.ps1` 严格校验 `$LASTEXITCODE`、失败统计及最低用例阈值，根治 `ide-shell-web` 静默报 `0 passed`。
  - 数学性能基准框架升级：精确 Student's $t$ 95% 置信区间（下界非负截断）、Tukey 1.5×IQR 离群值识别与去极值均值、$N < 5$ 分位点置空保护、RAII 临时目录、1MB 单行搜索 SLA (`<= 15 ms`)、4 尺度 `[5k, 20k, 80k, 200k]` OLS 对数-对数拟合与 `-Compare` 回归门禁。

## 4. 自动化测试与数学性能实测指标（本机实测全通）
- **全量 Rust 自动化测试**（[tests/e2e-run-all.ps1](tests/e2e-run-all.ps1) `-SkipPlaywright`）：**1478 passed, 0 failed** (`100 %` 通过率，15/15 核心特性 `100 %` 覆盖)
  - `editcore, nema-codec, ide-agent-router`：744 passed
  - `IDE1.0 (ide-fast)`：633 passed
  - `ide-shell-web`：101 passed
- **冷启动与关键操作 P50（Release x64，Warmup=1，N=5+）**：
  - `startup_app_build_ms`：**0.192 ms** | `startup_trace_first_frame_ms`：**184.100 ms** (`CV=0.051`) | `startup_wall_first_frame_ms`：**217.434 ms** (`CV=0.039`) | 工作集：**80.570 MB**
  - 打开 5MB / 30MB / 200k 行：**16.739 ms** / **111.158 ms** / **51.811 ms**
  - 200k 行全文无命中查找 / 正则查找：**4.042 ms** / **4.522 ms**
  - 1MB 单行全文多命中查找：**0.194 ms** (`SLA <= 15 ms`)
  - 200k 文档 10k 编辑 / 全撤回：**10.594 ms** / **0.705 ms**
  - 1k 行 Rust 语法高亮：**0.838 ms** (`CV=0.009`) | 200k 文档稳态单帧：**0.288 ms** (`3,211 frames/s`)
- **4 尺度 OLS 渐近复杂度实测 (`5k / 20k / 80k / 200k`)**：
  - 单步编辑 + `changed()`：$\alpha = 0.0280 \le 0.25$ (**O(1) PASS**)
  - 稳态单帧渲染：$\alpha = -0.1687 \le 0.25$ (**O(1) PASS**)
  - 全文扫描查找：$\alpha = 0.8122 \le 1.25$ ($R^2 = 0.9858$, **O(N) PASS**)
  - 文档打开解析：$\alpha = 1.1841 \le 1.25$ ($R^2 = 0.9999$, **O(N) PASS**)

## 5. 发布安装包清单（Windows x64）
- `IDE1.0_Setup_0.8.5_x64.exe`（图形安装向导）
- `IDE1.0_0.8.5_x64.msi`（Windows MSI 安装包）
- `IDE1.0-0.8.5-windows-x64.exe`（便携单文件绿色版）
- `SHA256SUMS.txt`（SHA-256 校验清单）
