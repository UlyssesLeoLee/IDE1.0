#!/bin/bash
# cargo test editcore, 重试 (Windows 上刚生成的 exe 偶发被占用, LNK1104)
export PATH="/e/DevCache/cargo/bin:$PATH" CARGO_HOME=E:/DevCache/cargo RUSTUP_HOME=E:/DevCache/rustup CARGO_TARGET_DIR=E:/IDE1.0-wt/test-target2
cd /e/IDE1.0
for i in 1 2 3; do
  out=$(cargo test -p editcore --lib "$@" 2>&1)
  if echo "$out" | grep -q "LNK1104"; then sleep 8; continue; fi
  echo "$out" | grep -E "^error|test result|panicked|left:|right:|^test .* FAILED" | cut -c1-200
  exit 0
done
echo "link failed 3x"
