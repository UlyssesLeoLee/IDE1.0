#!/bin/bash
# 全量回归: Rust 单测 -> 编译 web 版 -> mock -> UAT -> Sakura 差分。一条命令跑完, 结果汇总在最后。
#
# 用法 (在仓库根目录):  bash tests/run-all.sh [--skip-rust] [--skip-sakura]
# 环境变量 (都有默认值):
#   TARGET_DIR   cargo 产物目录 (默认 $REPO/../IDE1.0-wt/regress-target, 避免和日常构建互相锁住)
#   OUT_DIR      各阶段日志目录 (默认 $TMPDIR/ide-regress)
#   UAT_ADDR     UAT 用的 web 壳地址 (默认 127.0.0.1:7599)
# 退出码: 0 = 全部通过; 非 0 = 有失败的阶段 (日志在 OUT_DIR)。
#
# 注意: 跑的过程中不要再改 crates/ide-shell-web/src/editor.html —— 测试会检查二进制内嵌的页面和源文件一致, 不一致就直接失败。

REPO="$(cd "$(dirname "$0")/.." && pwd)"
TARGET_DIR="${TARGET_DIR:-$REPO/../IDE1.0-wt/regress-target}"
OUT_DIR="${OUT_DIR:-${TMPDIR:-/tmp}/ide-regress}"
UAT_ADDR="${UAT_ADDR:-127.0.0.1:7599}"
SKIP_RUST=0; SKIP_SAKURA=0
for a in "$@"; do case "$a" in --skip-rust) SKIP_RUST=1;; --skip-sakura) SKIP_SAKURA=1;; esac; done
mkdir -p "$OUT_DIR"
FAIL=0
summary() { echo "[$1] $2"; }

cd "$REPO" || exit 2
if [ "$SKIP_RUST" = 0 ]; then
  CARGO_TARGET_DIR="$TARGET_DIR" cargo test --workspace --exclude ide-shell-desktop > "$OUT_DIR/rust.out" 2>&1 || FAIL=1
  summary rust "$(grep -E '^test result' "$OUT_DIR/rust.out" | awk '{p+=$4; f+=$6} END {print p" passed, "f" failed"}')"
fi
CARGO_TARGET_DIR="$TARGET_DIR" cargo build -p ide-shell-web > "$OUT_DIR/build.out" 2>&1 || { summary build FAILED; exit 1; }
BIN="$TARGET_DIR/debug/ide-shell-web.exe"; [ -f "$BIN" ] || BIN="$TARGET_DIR/debug/ide-shell-web"

( cd tests/mock && QA_BIN="$BIN" node --test --test-concurrency=1 "specs/web/*.test.mjs" > "$OUT_DIR/mock.out" 2>&1 ) || FAIL=1
summary mock "$(grep -E '^# (pass|fail)' "$OUT_DIR/mock.out" | tr '\n' ' ')"

( cd tests/uat && IDE_SHELL_WEB_BIN="$BIN" IDE_SHELL_WEB_ADDR="$UAT_ADDR" npx playwright test --timeout=30000 > "$OUT_DIR/uat.out" 2>&1 ) || FAIL=1
summary uat "$(grep -E '^\s+[0-9]+ (passed|failed)' "$OUT_DIR/uat.out" | tr '\n' ' ')"

if [ "$SKIP_SAKURA" = 0 ]; then
  ( cd tests/sakura-diff && QA_BIN="$BIN" node run.mjs > "$OUT_DIR/sakura.out" 2>&1 ) || FAIL=1
  summary sakura "$(grep -c '^OK' "$OUT_DIR/sakura.out") OK, $(grep -c '^FAIL\|^DIFF' "$OUT_DIR/sakura.out") 差异 (已知的 Sakura 宏侧抖动见 tests/sakura-diff/README)"
fi
echo "日志: $OUT_DIR"
exit $FAIL
