# Sakura 差分比对 (tests/sakura-diff)

以**真实的 Sakura Editor** 为准, 逐条比对编辑命令的行为。

## 做法

1. 每条用例给一份种子文本 + 起点 (光标 `cur` 或选区 `sel`)。
2. Sakura 侧: 写成键盘宏 (`S_GoFileTop(); S_MoveCursor(...); <命令>; <收尾>; S_FileSave(); S_ExitAll();`),
   用 `sakura.exe -M=<宏> <文件>` 执行, 读回结果文件。
3. 我们这边: 在 mock 环境里开编辑器, 摆好同样的起点, 用真实按键 / 命令执行, 再做同样的收尾。
4. 收尾 (`tail`) 决定比什么:
   - `marker` (默认, 光标类用例): 在光标处插入 `|`, 比较文本 = 同时比较了内容和光标位置
   - `selcopy` (默认, 选区类用例): 复制当前选区并贴到文末, 比较的是选区内容
   - `paste`: 把剪贴板贴到文末 (比较复制 / 剪切类命令写进剪贴板的内容)

## 运行

```bash
QA_BIN=E:/IDE1.0-wt/fix-target/debug/ide-shell-web.exe node run.mjs            # 全部
QA_BIN=E:/IDE1.0-wt/fix-target/debug/ide-shell-web.exe node run.mjs "UNDO"     # 只跑 id 含 UNDO 的
QA_BIN=E:/IDE1.0-wt/fix-target/debug/ide-shell-web.exe node run.mjs --md report.md
```

需要本机装有 Sakura Editor (`SAKURA_EXE`, 缺省找常见安装路径); 找不到时整组跳过。

## 已知差异

目前没有 (全部用例与 Sakura 一致)。

## 没有作为比对依据的情形 (用例已删除, 行为未经 Sakura 验证)

这些情形在 Sakura 的宏里表现不稳定或与手动操作不一致, 无法当作基准:

| 情形 | 原因 |
|---|---|
| 连续两次以上 `S_BookmarkNext` | 宏里第二次起跳到文件首, 与手动操作不一致 (疑为宏连续执行的时序); 单次 Next / 两个书签时的 Prev 有用例覆盖 |
| 单个书签时从文件末尾 `S_BookmarkPrev` | 结果落在书签的下一行, 无法解释; 两个书签的 Prev 有用例覆盖 |
| 有选区时 `S_DeleteLine` / `S_CutLine` | 宏里文本不变, 像是命令没执行; 无选区的情形有用例覆盖 |
| 矩形选择中 `S_Char` 输入 | 宏里不删矩形内容只插入, 与上游源码 (先删再插) 相反; 按源码实现 |

## 这套方法比对不了的

- 增量搜索、移动履歴 (跳转历史): Sakura 宏里没有对应命令
- 鼠标操作、IME、对话框里的交互
