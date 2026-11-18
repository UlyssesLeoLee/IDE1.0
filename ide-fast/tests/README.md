# IDE1.0 人工 / 半自动窗口验证

`gui-shot.ps1` 把正在运行的 IDE1.0 窗口截成 PNG, 并可先发送按键或点击 / 右键 (坐标相对窗口左上角):

    $env:IDE_FAST_SETTINGS='off'; Start-Process <IDE1.0.exe> -ArgumentList a.rs,b.md
    powershell -File ide-fast/tests/gui-shot.ps1 -Out s.png -ClickX 321 -ClickY 44          # 点「视图」菜单
    powershell -File ide-fast/tests/gui-shot.ps1 -Out s.png -ClickX 140 -ClickY 68 -Right 1 # 右键标签 (分组)

0.2.0 发布后的核对清单 (已逐项在真窗口里点过): 视图 → 分割窗口、点右栏后键入只进右栏、右键标签 → 绿组出现色点、插件菜单列出主题 / 脚本插件、选「浅色」两栏同时换色、文件 → 打印生成 `IDE1.0-print/<文件名>.html`。

## 性能基准与数学化评估框架 (回归工具)

- `tests/perf-math-bench.ps1`: 数学化统计分布 ($P_{50}/P_{90}/P_{95}/P_{99}/\text{IQR}/\mu/\sigma/\text{CV}/\text{CI}_{95}$) + OLS 对数-对数复杂度拟合 ($\alpha, R^2$) + 强制 SLA 门禁。
- `ide-fast/tests/perf.ps1`: 对 `IDE1.0.exe` 连跑 N 次冷启动 (追踪点中位数 / p90 / p95 / CV、空闲内存 / 线程 / CPU), 再跑引擎无头基准。
基线与机器信息见 `docs/PERF.md`。改动影响启动 / 打开 / 绘制路径时, 先 `-Json before.json` 存一份, 改完用 `-Compare before.json` 对比。

    powershell -File tests/perf-math-bench.ps1
    powershell -File ide-fast/tests/perf.ps1 -Exe <IDE1.0.exe> -Json before.json
    powershell -File ide-fast/tests/perf.ps1 -Exe <IDE1.0.exe> -Compare before.json
