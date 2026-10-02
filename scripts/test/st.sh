#!/usr/bin/env bash
# ============================================================================
# ST (System Test) Runner — IDE1.0-dev / MVP-1
# ----------------------------------------------------------------------------
# DD-11 §4 (ST 観点) に従い、ビルド済み `kernel-cli` バイナリを使った
# End-to-End CLI 動作確認テストを実行する。
#
# Strategy:
#   1. cargo build -p cli で `kernel-cli` バイナリをビルド
#   2. ビルド成果物を spawn する System Test を cargo test 経由で実行
#   3. 結果を集計
#
# Usage:
#   bash scripts/test/st.sh
#
# Environment:
#   ST_SKIP_BUILD    "1" で build をスキップ (default: 0)
#
# Exit:
#   0 = 全 ST 成功 / 1 = いずれか失敗
# ============================================================================
set -u

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LOG_DIR="${ROOT_DIR}/target/test-reports/st"
mkdir -p "${LOG_DIR}"

EXIT_CODE=0
TOTAL_PASS=0
TOTAL_FAIL=0
START_TS=$(date +%s)

echo "================================================================"
echo " ST (System Test) Run — IDE1.0-dev"
echo " workspace : ${ROOT_DIR}"
echo " log dir   : ${LOG_DIR}"
echo " started   : $(date -Iseconds)"
echo "================================================================"

if [ "${ST_SKIP_BUILD:-0}" != "1" ]; then
    echo
    echo "--- [build] cargo build -p cli ---"
    if ! cargo build -p cli --color never > "${LOG_DIR}/build.log" 2>&1; then
        echo "  FAIL (build) — see ${LOG_DIR}/build.log"
        tail -n 20 "${LOG_DIR}/build.log" | sed 's/^/    /'
        exit 1
    fi
    echo "  PASS (build)"
fi

echo
echo "--- [system tests] cargo test -p cli --test cli_system ---"
st_start=$(date +%s)
if ! cargo test -p cli --test cli_system --color never \
    > "${LOG_DIR}/cli_system.log" 2>&1; then
    st_end=$(date +%s)
    echo "  FAIL (cli_system) — ${st_end}-${st_start} sec"
    echo "  tail of log:"
    tail -n 40 "${LOG_DIR}/cli_system.log" | sed 's/^/    /'
    TOTAL_FAIL=$((TOTAL_FAIL + 1))
    EXIT_CODE=1
else
    st_end=$(date +%s)
    summary=$(grep -E "^test result:" "${LOG_DIR}/cli_system.log" | tail -n 1)
    passed=$(echo "${summary}" | grep -oE "[0-9]+ passed" | awk '{print $1}')
    failed=$(echo "${summary}" | grep -oE "[0-9]+ failed" | awk '{print $1}')
    passed=${passed:-0}
    failed=${failed:-0}
    TOTAL_PASS=$((TOTAL_PASS + passed))
    TOTAL_FAIL=$((TOTAL_FAIL + failed))
    if [ "${failed}" -gt 0 ]; then
        EXIT_CODE=1
    fi
    echo "  PASS — ${st_end}-${st_start} sec — ${passed} passed, ${failed} failed"
fi

END_TS=$(date +%s)
ELAPSED=$((END_TS - START_TS))

echo
echo "================================================================"
echo " ST Summary"
echo "   tests_passed : ${TOTAL_PASS}"
echo "   tests_failed : ${TOTAL_FAIL}"
echo "   elapsed_sec  : ${ELAPSED}"
echo "   exit_code    : ${EXIT_CODE}"
echo "   log_dir      : ${LOG_DIR}"
echo "================================================================"

exit "${EXIT_CODE}"