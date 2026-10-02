#!/usr/bin/env bash
# ============================================================================
# 全体回帰テスト (Regression Test) Runner — IDE1.0-dev / MVP-1
# ----------------------------------------------------------------------------
# DD-11 §3-§4 (UT/IT/ST) + §7 (CI/CD Pipeline) に従い、UT → IT → ST の
# 順で全レイヤを実行し、統合回帰レポートを生成する。
#
# Usage:
#   bash scripts/test/regression.sh              # 全レイヤ実行
#   bash scripts/test/regression.sh --skip-ut    # UT スキップ
#   bash scripts/test/regression.sh --skip-it    # IT スキップ
#   bash scripts/test/regression.sh --skip-st    # ST スキップ
#   bash scripts/test/regression.sh --ut-only    # UT のみ
#   bash scripts/test/regression.sh --it-only    # IT のみ
#   bash scripts/test/regression.sh --st-only    # ST のみ
#
# レポート出力:
#   target/test-reports/regression/REGRESSION_<timestamp>.md
#
# Exit:
#   0 = 全レイヤ成功 / 1 = いずれか失敗
# ============================================================================
set -u

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LOG_DIR="${ROOT_DIR}/target/test-reports/regression"
mkdir -p "${LOG_DIR}"

# 引数パース
SKIP_UT=0; SKIP_IT=0; SKIP_ST=0
ONLY_UT=0; ONLY_IT=0; ONLY_ST=0
for arg in "$@"; do
    case "${arg}" in
        --skip-ut) SKIP_UT=1 ;;
        --skip-it) SKIP_IT=1 ;;
        --skip-st) SKIP_ST=1 ;;
        --ut-only) ONLY_UT=1 ;;
        --it-only) ONLY_IT=1 ;;
        --st-only) ONLY_ST=1 ;;
        -h|--help)
            grep '^#' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *)
            echo "Unknown argument: ${arg}" >&2
            exit 2
            ;;
    esac
done

if [ "${ONLY_UT}" = "1" ]; then SKIP_IT=1; SKIP_ST=1; fi
if [ "${ONLY_IT}" = "1" ]; then SKIP_UT=1; SKIP_ST=1; fi
if [ "${ONLY_ST}" = "1" ]; then SKIP_UT=1; SKIP_IT=1; fi

TS=$(date -u +%Y%m%dT%H%M%SZ)
REPORT="${LOG_DIR}/REGRESSION_${TS}.md"

# サマリ用カウンタ
UT_RESULT="SKIPPED"; UT_PASS=0; UT_FAIL=0; UT_ELAPSED=0
IT_RESULT="SKIPPED"; IT_PASS=0; IT_FAIL=0; IT_ELAPSED=0
ST_RESULT="SKIPPED"; ST_PASS=0; ST_FAIL=0; ST_ELAPSED=0
OVERALL_EXIT=0

# ----- 各レイヤ実行 -----------------------------------------------------
START_TS=$(date +%s)

if [ "${SKIP_UT}" != "1" ]; then
    echo "===== [1/3] UT (Unit Tests) ====="
    ut_start=$(date +%s)
    if bash "${ROOT_DIR}/scripts/test/ut.sh" 2>&1 | tee "${LOG_DIR}/ut_${TS}.log"; then
        UT_RESULT="PASS"
        UT_PASS=$(grep -oE "tests_passed *: *[0-9]+" "${LOG_DIR}/ut_${TS}.log" | awk '{print $3}' | head -1)
        UT_PASS=${UT_PASS:-0}
        UT_ELAPSED=$(( $(date +%s) - ut_start ))
    else
        UT_RESULT="FAIL"
        UT_FAIL=1
        UT_ELAPSED=$(( $(date +%s) - ut_start ))
        OVERALL_EXIT=1
    fi
fi

if [ "${SKIP_IT}" != "1" ]; then
    echo
    echo "===== [2/3] IT (Integration Tests) ====="
    it_start=$(date +%s)
    if bash "${ROOT_DIR}/scripts/test/it.sh" 2>&1 | tee "${LOG_DIR}/it_${TS}.log"; then
        IT_RESULT="PASS"
        IT_PASS=$(grep -oE "tests_passed *: *[0-9]+" "${LOG_DIR}/it_${TS}.log" | awk '{print $3}' | head -1)
        IT_PASS=${IT_PASS:-0}
        IT_ELAPSED=$(( $(date +%s) - it_start ))
    else
        IT_RESULT="FAIL"
        IT_FAIL=1
        IT_ELAPSED=$(( $(date +%s) - it_start ))
        OVERALL_EXIT=1
    fi
fi

if [ "${SKIP_ST}" != "1" ]; then
    echo
    echo "===== [3/3] ST (System Tests) ====="
    st_start=$(date +%s)
    if bash "${ROOT_DIR}/scripts/test/st.sh" 2>&1 | tee "${LOG_DIR}/st_${TS}.log"; then
        ST_RESULT="PASS"
        ST_PASS=$(grep -oE "tests_passed *: *[0-9]+" "${LOG_DIR}/st_${TS}.log" | awk '{print $3}' | head -1)
        ST_PASS=${ST_PASS:-0}
        ST_ELAPSED=$(( $(date +%s) - st_start ))
    else
        ST_RESULT="FAIL"
        ST_FAIL=1
        ST_ELAPSED=$(( $(date +%s) - st_start ))
        OVERALL_EXIT=1
    fi
fi

END_TS=$(date +%s)
TOTAL_ELAPSED=$((END_TS - START_TS))

# ----- Markdown レポート生成 --------------------------------------------
TOTAL_PASS=$((UT_PASS + IT_PASS + ST_PASS))

cat > "${REPORT}" <<EOF
# 回帰テスト レポート — IDE1.0-dev / MVP-1

**実行日時**: $(date -Iseconds)
**workspace**: ${ROOT_DIR}
**Git HEAD**: $(git -C "${ROOT_DIR}" rev-parse --short HEAD 2>/dev/null || echo "n/a")
**Git branch**: $(git -C "${ROOT_DIR}" branch --show-current 2>/dev/null || echo "n/a")
**実行環境**: $(uname -a 2>/dev/null || echo "windows")

---

## サマリ

| テストレイヤ | 結果    | 成功 | 失敗 | 所要時間(秒) |
|--------------|---------|------|------|--------------|
| UT           | ${UT_RESULT} | ${UT_PASS}    | ${UT_FAIL}    | ${UT_ELAPSED}        |
| IT           | ${IT_RESULT} | ${IT_PASS}    | ${IT_FAIL}    | ${IT_ELAPSED}        |
| ST           | ${ST_RESULT} | ${ST_PASS}    | ${ST_FAIL}    | ${ST_ELAPSED}        |
| **合計**     | -       | **${TOTAL_PASS}** | **$((UT_FAIL + IT_FAIL + ST_FAIL))** | **${TOTAL_ELAPSED}**        |

**総合判定**: $([ "${OVERALL_EXIT}" = "0" ] && echo "✅ PASS — 全体回帰 OK" || echo "❌ FAIL — いずれか失敗")

---

## カバレッジ観点

### UT (Unit Tests)
- 各 crate 内部の \`#[cfg(test)] mod tests\` (ホワイトボックス)
- 9 crate × モジュール網羅
- log: \`${LOG_DIR}/ut_${TS}.log\`

### IT (Integration Tests)
- 公開 API に対するブラックボックス検証
- IT-EB-001..008 (kernel-eventbus)
- IT-SM-001..010 (kernel-session)
- IT-CB-001..008 (kernel-cmd-bus)
- IT-CORE-001..006 (kernel-core)
- log: \`${LOG_DIR}/it_${TS}.log\`

### ST (System Tests)
- ビルド済み \`kernel-cli\` バイナリを spawn して E2E 検証
- ST-CLI-001..014 (cli)
- log: \`${LOG_DIR}/st_${TS}.log\`

---

## 実行コマンド

\`\`\`bash
# 全レイヤ
bash scripts/test/regression.sh

# 個別レイヤ
bash scripts/test/ut.sh
bash scripts/test/it.sh
bash scripts/test/st.sh
\`\`\`

---

## 既知の制限 / TBD

- 共有 Cargo target dir (E:/DevCache/cargo/target) を使うため、
  他の worktree とビルドロック競合する場合はリトライが必要。
- ネットワーク疎通が必要なテスト (Plugin / Remote AI) は MVP-1 範囲外。
- Coverage (line/branch) は TBD-11-05 (CI 初期化後に測定)。
EOF

echo
echo "================================================================"
echo " 全体回帰レポート: ${REPORT}"
echo "================================================================"

exit "${OVERALL_EXIT}"