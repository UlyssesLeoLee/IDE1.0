# ide-fast 人工 / 半自动窗口验证

`gui-shot.ps1` 把正在运行的 ide-fast 窗口截成 PNG, 并可先发送按键或点击 / 右键 (坐标相对窗口左上角):

    $env:IDE_FAST_SETTINGS='off'; Start-Process <ide-fast.exe> -ArgumentList a.rs,b.md
    powershell -File ide-fast/tests/gui-shot.ps1 -Out s.png -ClickX 321 -ClickY 44          # 点「视图」菜单
    powershell -File ide-fast/tests/gui-shot.ps1 -Out s.png -ClickX 140 -ClickY 68 -Right 1 # 右键标签 (分组)

0.2.0 发布后的核对清单 (已逐项在真窗口里点过): 视图 → 分割窗口、点右栏后键入只进右栏、右键标签 → 绿组出现色点、插件菜单列出主题 / 脚本插件、选「浅色」两栏同时换色、文件 → 打印生成 `ide-fast-print/<文件名>.html`。
