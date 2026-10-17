# SPA-02 — Sakura 缺口清点与推进计划

本文接续 SPA-01（Grep / 列块规格），做**全量缺口清点**，并记录按模块推进的进度。

清点基线：`sakura-editor/sakura` @ `628de79`（已 clone 到 `E:/Temp/sakura-ref/sakura`）。

---

## 1. 上游真实规模（实测，非估计）

| 项目 | 数量 |
|---|---|
| 文件总数 | 2133 |
| C/C++ 行数 | 131507 |
| 官方帮助文档 | 391 篇（其中功能类 367，元信息 24） |
| 用户可绑定命令（`Funccode_x.hsrc` 的 `F_` 枚举） | 474 |

核心子系统行数（Top 10）：

```
22403  sakura_core/view/      13850  sakura_core/env/        8590  sakura_core/types/
16860  sakura_core/dlg/       12287  sakura_core/window/      8499  sakura_core/typeprop/
14270  sakura_core/cmd/       10478  sakura_core/prop/        8356  sakura_core/macro/
                                  9252  sakura_core/util/       8111  sakura_core/doc/
                                     8939  sakura_core/charset/  5826  sakura_core/outline/
```

### 1.1 必须一并 Rust 化的外部依赖

Sakura 依赖 8 个外部 C/C++ 库。「仅有用 rust 实现这一点区别」意味着这些都要有 Rust 等价物：

| 依赖 | 用途 | 本项目对应 | 状态 |
|---|---|---|---|
| bregonig | 正则引擎（BreezoLib/Oniguruma） | `search.rs` 内置引擎 | 进行中 |
| Onigmo | 正则引擎（Ruby 风格） | 同上 | 进行中 |
| cmigemo + cmigemo-dict | 日文罗马音（**40MB 级字典**） | `sakura-rs/migemo.rs` | 进行中 |
| ctags | 标签索引 | `sakura-rs/tag_jump.rs` | 部分 |
| darkmodelib | 暗色模式 API | 前端 CSS token | 已绕过 |
| miniz-cpp | zip（安装包/插件包） | 未处理 | 未处理 |
| LocaleEmulator | 日文编码兼容 | `encoding_rs` 部分覆盖 | 部分 |
| diffutils | 差异比较 | 未处理 | 未处理 |

---

## 2. 功能文档分布（367 条功能类）

| 类别 | 条数 |
|---|---|
| 其他（细分项，尚未归类） | 132 |
| 设置项（类型別 / 共通設定） | 37 |
| **光标移动 / 跳转** | 37 |
| **文件操作** | 34 |
| 选择 | 20 |
| 矩形 / 列块 | 20 |
| 外观 / 显示 | 16 |
| 文字 / 大小写 / 全半角 | 15 |
| 编码 / 转换 | 14 |
| 查找替换 | 13 |
| 宏 / 插件 | 11 |
| 窗口 | 6 |
| 标签页 | 5 |
| 编辑操作 | 4 |
| 代码操作 | 3 |

缺口最大的三类：**光标移动（37）、文件操作（34）、设置项（37）**。

---

## 3. 推进批次

### 3.1 批次一（已合并）— Grep + 列块 + 光标移动

commit `affa0f1` + `0de97ab`，见 SPA-01。

光标移动（0de97ab）实现了 14 条命令，对标 HLP000220~HLP000263：

| 规格 | Sakura 命令 | 状态 |
|---|---|---|
| M-01 | 単語の左端に移動 | ✅ |
| M-02 | 単語の右端に移動 | ✅ |
| M-03 | 行頭に移動 | ✅ |
| M-04 | 行末に移動 | ✅ |
| M-05 | カーソル上移動(２行ごと) | ✅ |
| M-06 | カーソル下移動(２行ごと) | ✅ |
| M-07 | １ページアップ / ダウン | ✅ |
| M-08 | 半ページアップ / ダウン | ✅ |
| M-09 | ファイルの先頭 / 最後に移動 | ✅ |
| M-10 | 次の段落 / 前の段落に移動 | ✅ |
| M-11 | **(選択) 全部 — 有选区时收缩选区** | ✅ |
| M-12 | (矩形選択) 全部 — 列块模式下的移动 | ✅ |

**关键行为规则**：Sakura 的移动命令在有选区时是**收缩选区到起点/终点**，而非丢弃选区。
原来的 `moveCaret*` 缺这个分支，这是与 Sakura 行为不一致的根因，对应 HLP000234~HLP000243
一整组「(選択)」前缀命令。

未做（移动类剩余 61 条规格）：
- 移動履歴 前へ/次へ（MOVE 记录栈，Sakura 的多次跳转回退）
- オートスクロール（M-296，拖动光标到边缘自动滚屏）
- 前の変更行へ移動（diff 感知跳转）
- ダイレクトタグジャンプ / 閉じてタグジャンプ（tag_jump.rs 已有基础，前端未接）

### 3.2 批次二（进行中）— 三个并行 worktree

按**互不重叠的文件边界**拆三个并行任务，各自独立 worktree，最后 rebase 到 dev 并合并：

| worktree | 负责文件 | 目标 |
|---|---|---|
| `E:/Temp/wt-grep-rust` | `ide-shell-web/src/search.rs`、`sakura-rs/src/grep.rs` | 后端从 shell out 到 `rg` 改为内置 Rust（glob + 搜索 + 递归枚举） |
| `E:/Temp/wt-migemo-rust` | `sakura-rs/src/migemo.rs` | 补全 cmigemo 水准：拗音、浊音组合、促音、长音、汉字音读 |
| `E:/Temp/wt-types-rust` | `sakura-rs/src/types.rs` | 关键字集合合并、正则语法、类型规则匹配（通配）、颜色分类、标识符抽取 |

拆分依据：三者文件无交集，因此并行不会冲突；合并时互不覆盖。

---

## 4. 验收红线

1. 每条标记 ✅ 的规格必须有对应 UAT 或 Rust 单测，不接受「调用不报错」。
2. CI 门禁含 `RUSTFLAGS: -D warnings` + `cargo clippy -- -D warnings`，本地新增 warning 就是未来的红。
3. sakura-rs 已有 6 个 dead_code warning（type_config.rs），属既有状态，不在清理范围内，但不得新增。
4. 改前端不得打红 `minimal-layout.spec.ts`（该 spec 把「抽屉覆盖式展开」锁成了契约——
   这与「展开必须让位」的新需求冲突，需在对应批次里一并修正契约）。
