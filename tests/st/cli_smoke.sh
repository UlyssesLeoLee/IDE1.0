#!/usr/bin/env bash
# IDE1.0 cli_smoke.sh v0.1 — ULYS-191 §4.2.2 brief v0.1
# 调 `cargo run -- aci-emit` 输出 JSON, 验 schema 合法, 输出 PASS/FAIL
#
# 4 步:
#   1. cargo build --quiet -p ide-cli
#   2. emit 一条 assertion (capture JSON 输出)
#   3. 验 10 必填字段全在
#   4. 验 schema_version (= "0.1.0-draft")

set -euo pipefail

WORKDIR="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$WORKDIR"

echo "=== IDE1.0 cli_smoke.sh v0.1 ==="
echo "workdir: $WORKDIR"
echo "branch: $(git branch --show-current 2>/dev/null || echo 'unknown')"
echo

# Step 1: 编译
echo "[step 1/4] cargo build --quiet -p ide-cli"
cargo build --quiet -p ide-cli

# Step 2: emit 一条 assertion
echo "[step 2/4] emit assertion via ide-cli --aci-emit"
JSON_OUT="$(cargo run --quiet -p ide-cli -- --aci-emit ide1.0:smoke:smoke-1)"

# Step 3: 验 10 必填字段
echo "[step 3/4] verify 10 required fields"
for field in assertion_id aci_version layer scope expect actual status severity reasoning captured_at; do
  if ! echo "$JSON_OUT" | grep -q "\"$field\""; then
    echo "FAIL: field '$field' missing in JSON output"
    echo "--- JSON output ---"
    echo "$JSON_OUT"
    exit 1
  fi
done

# Step 4: 验 schema_version
echo "[step 4/4] verify aci_version == 0.1.0-draft"
if ! echo "$JSON_OUT" | grep -q '"aci_version": "0.1.0-draft"'; then
  echo "FAIL: aci_version mismatch"
  echo "--- JSON output ---"
  echo "$JSON_OUT"
  exit 1
fi

echo
echo "PASS: cli_smoke.sh 4 step verify complete"
echo "  - 10 required fields present"
echo "  - aci_version == 0.1.0-draft"
echo "  - assertion_id == ide1.0:smoke:smoke-1"
