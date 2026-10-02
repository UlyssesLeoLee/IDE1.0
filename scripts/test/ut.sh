#!/usr/bin/env bash
# ============================================================================
# UT (Unit Test) Runner — IDE1.0-dev / MVP-1
# ----------------------------------------------------------------------------
# DD-11 §3 (UT 観点) に従い、workspace 全 crate の `cargo test --lib` を実行
# する。各 crate の #[cfg(test)] mod tests (ホワイトボックス単体テスト) を対象。
#
# Usage:
#   bash scripts/test/ut.sh
#
# Environment:
#   CARGO_TARGET_DIR   Cargo target dir (default: ./target)
#   UT_FAIL_FAST       "1" で最初の失敗で停止 (default: 0)
#
# Exit:
#   0 = 全 UT 成功 / 1 = いずれか失敗
# ============================================================================
set -u  # 未定義変数で即終了 (readonly vars 保護)

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CRATES=(
    kernel-error
    kernel-config
    kernel-tracing
    kernel-eventbus
    kernel-session
    kernel-capability-registry
    kernel-cmd-bus
    kernel-core
    cli
)
LOG_DIR="${ROOT_DIR}/target/test-reports/ut"
mkdir -p "${LOG_DIR}"

EXIT_CODE=0
TOTAL_PASS=0
TOTAL_FAIL=0
START_TS=$(date +%s)

echo "================================================================"
echo " UT (Unit Test) Run — IDE1.0-dev"
echo " workspace : ${ROOT_DIR}"
echo " crates    : ${CRATES[*]}"
echo " log dir   : ${LOG_DIR}"
echo " started   : $(date -Iseconds)"
echo "================================================================"

for crate in "${CRATES[@]}"; do
    echo
    echo "--- [${crate}] cargo test --lib ---"
    crate_start=$(date +%s)
    if ! cargo test --lib -p "${crate}" --color never \
        > "${LOG_DIR}/${crate}.log" 2>&1; then
        crate_end=$(date +%s)
        echo "  FAIL (${crate}) — ${crate_end}-${crate_start} sec"
        echo "  tail of log:"
        tail -n 20 "${LOG_DIR}/${crate}.log" | sed 's/^/    /'
        TOTAL_FAIL=$((TOTAL_FAIL + 1))
        EXIT_CODE=1
        if [ "${UT_FAIL_FAST:-0}" = "1" ]; then
            echo "  UT_FAIL_FAST=1 → aborting on first failure."
            break
        fi
    else
        crate_end=$(date +%s)
        # Parse summary "test result: ok. N passed; 0 failed; ..."
        summary=$(grep -E "^test result:" "${LOG_DIR}/${crate}.log" | tail -n 1)
        passed=$(echo "${summary}" | grep -oE "[0-9]+ passed" | awk '{print $1}')
        passed=${passed:-0}
        echo "  PASS (${crate}) — ${crate_end}-${crate_start} sec — ${passed} tests"
        TOTAL_PASS=$((TOTAL_PASS + passed))
    fi
done

END_TS=$(date +%s)
ELAPSED=$((END_TS - START_TS))

echo
echo "================================================================"
echo " UT Summary"
echo "   crates_total  : ${#CRATES[@]}"
echo "   tests_passed  : ${TOTAL_PASS}"
echo "   crates_failed : ${TOTAL_FAIL}"
echo "   elapsed_sec   : ${ELAPSED}"
echo "   exit_code     : ${EXIT_CODE}"
echo "   log_dir       : ${LOG_DIR}"
echo "================================================================"

exit "${EXIT_CODE}"