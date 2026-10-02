#!/usr/bin/env bash
# ============================================================================
# IT (Integration Test) Runner — IDE1.0-dev / MVP-1
# ----------------------------------------------------------------------------
# DD-11 §3 (IT 観点) に従い、各 crate の `tests/` ディレクトリにある
# Integration Test を実行する。クレート公開 API に対するブラックボックステスト。
#
# Usage:
#   bash scripts/test/it.sh
#
# Coverage in this run:
#   - kernel-eventbus    : event_bus_api       (IT-EB-001..008)
#   - kernel-session     : session_lifecycle   (IT-SM-001..010)
#   - kernel-cmd-bus     : command_bus_api     (IT-CB-001..008)
#   - kernel-core        : microkernel_e2e     (IT-CORE-001..006)
#
# Environment:
#   IT_FAIL_FAST       "1" で最初の失敗で停止 (default: 0)
#
# Exit:
#   0 = 全 IT 成功 / 1 = いずれか失敗
# ============================================================================
set -u

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LOG_DIR="${ROOT_DIR}/target/test-reports/it"
mkdir -p "${LOG_DIR}"

# 各 crate と、その crate 配下の tests/ ディレクトリにある IT ファイル名
# (拡張子なし)。cargo test --test <file> で個別実行する。
declare -A IT_FILES=(
    [kernel-eventbus]="event_bus_api"
    [kernel-session]="session_lifecycle"
    [kernel-cmd-bus]="command_bus_api"
    [kernel-core]="microkernel_e2e"
)

EXIT_CODE=0
TOTAL_PASS=0
TOTAL_FAIL=0
START_TS=$(date +%s)

echo "================================================================"
echo " IT (Integration Test) Run — IDE1.0-dev"
echo " workspace : ${ROOT_DIR}"
echo " log dir   : ${LOG_DIR}"
echo " started   : $(date -Iseconds)"
echo "================================================================"

for crate in "${!IT_FILES[@]}"; do
    for test_file in "${IT_FILES[$crate]}"; do
        echo
        echo "--- [${crate}] cargo test --test ${test_file} ---"
        crate_start=$(date +%s)
        if ! cargo test --test "${test_file}" -p "${crate}" --color never \
            > "${LOG_DIR}/${crate}__${test_file}.log" 2>&1; then
            crate_end=$(date +%s)
            echo "  FAIL (${crate}/${test_file}) — ${crate_end}-${crate_start} sec"
            echo "  tail of log:"
            tail -n 30 "${LOG_DIR}/${crate}__${test_file}.log" | sed 's/^/    /'
            TOTAL_FAIL=$((TOTAL_FAIL + 1))
            EXIT_CODE=1
            if [ "${IT_FAIL_FAST:-0}" = "1" ]; then
                echo "  IT_FAIL_FAST=1 → aborting on first failure."
                break 2
            fi
        else
            crate_end=$(date +%s)
            summary=$(grep -E "^test result:" "${LOG_DIR}/${crate}__${test_file}.log" | tail -n 1)
            passed=$(echo "${summary}" | grep -oE "[0-9]+ passed" | awk '{print $1}')
            passed=${passed:-0}
            echo "  PASS (${crate}/${test_file}) — ${crate_end}-${crate_start} sec — ${passed} tests"
            TOTAL_PASS=$((TOTAL_PASS + passed))
        fi
    done
done

END_TS=$(date +%s)
ELAPSED=$((END_TS - START_TS))

echo
echo "================================================================"
echo " IT Summary"
echo "   test_files_total : $(printf '%s\n' "${IT_FILES[@]}" | wc -l)"
echo "   tests_passed     : ${TOTAL_PASS}"
echo "   files_failed     : ${TOTAL_FAIL}"
echo "   elapsed_sec      : ${ELAPSED}"
echo "   exit_code        : ${EXIT_CODE}"
echo "   log_dir          : ${LOG_DIR}"
echo "================================================================"

exit "${EXIT_CODE}"