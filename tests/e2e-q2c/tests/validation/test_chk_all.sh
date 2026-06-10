#!/usr/bin/env bash
# ============================================================================
# Q2C E2E 测试 — CHK-01~CHK-12: 全链路数据一致性校验
# 每项 CHK 是一个 SQL 脚本，查询数据库并输出结果
# 0 行返回 = PASS，任何行返回 = FAIL
# ============================================================================
set -euo pipefail

TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SQL_DIR="$TEST_DIR/sql"

source "$TEST_DIR/../../config/env.sh"
source "$TEST_DIR/../../config/agents.sh"
source "$TEST_DIR/../../lib/relay.sh"

echo "============================================"
echo "  CHK-01~12: 全链路数据一致性校验"
echo "============================================"
echo ""

TOTAL_CHK=12
PASS_COUNT=0
FAIL_COUNT=0
SKIP_COUNT=0

run_chk() {
    local num="$1"
    local name="$2"
    local sql_file="$SQL_DIR/chk_$(printf '%02d' $num)_*.sql"

    # 展开通配符
    local resolved
    resolved=$(ls $sql_file 2>/dev/null | head -1)

    echo ""
    echo "--- CHK-$(printf '%02d' $num): $name ---"

    if [[ -z "$resolved" ]]; then
        echo -e "  ${YELLOW}SKIP${NC} — SQL 文件不存在"
        ((SKIP_COUNT++)) || true
        return
    fi

    # 执行 SQL
    local result
    result=$(psql "$DB_URL" -t -A -f "$resolved" 2>&1) || true

    if [[ -z "$result" ]]; then
        echo -e "  ${GREEN}PASS${NC} — 0 行返回（无差异）"
        ((PASS_COUNT++)) || true
    else
        local line_count
        line_count=$(echo "$result" | wc -l)
        if [[ "$line_count" -eq 0 ]]; then
            echo -e "  ${GREEN}PASS${NC} — 无差异"
            ((PASS_COUNT++)) || true
        else
            echo -e "  ${RED}FAIL${NC} — ${line_count} 行差异:"
            echo "$result" | head -10 | while IFS= read -r line; do
                echo "    $line"
            done
            ((FAIL_COUNT++)) || true
        fi
    fi
}

# --- 执行 12 项校验 ---

run_chk  1 "SO 与发货一致性"
run_chk  2 "SO 与 AR 金额一致"
run_chk  3 "PO 与收货一致性"
run_chk  4 "PO 与 AP 金额一致"
run_chk  5 "工单用料与 BOM 一致"
run_chk  6 "工单成本归集完整"
run_chk  7 "库存余额正确（无负库存）"
run_chk  8 "库存预留一致性"
run_chk  9 "总账借贷平衡"
run_chk 10 "AR 核销完整性"
run_chk 11 "AP 核销完整性"
run_chk 12 "审计日志完整性"

# --- 汇总 ---
echo ""
echo "============================================"
echo "  CHK 校验汇总"
echo "============================================"
echo "  PASS:  $PASS_COUNT/$TOTAL_CHK"
echo "  FAIL:  $FAIL_COUNT"
echo "  SKIP:  $SKIP_COUNT"
echo "============================================"

# 写入接力
relay_write "chk_pass" "$PASS_COUNT"
relay_write "chk_fail" "$FAIL_COUNT"
relay_write "chk_skip" "$SKIP_COUNT"
relay_snapshot "SNAP-CHK"

if [[ $FAIL_COUNT -eq 0 ]]; then
    echo ""
    echo -e "${GREEN}✅ CHK ALL PASSED ($PASS_COUNT passed, $SKIP_COUNT skipped)${NC}"
    exit 0
else
    echo ""
    echo -e "${RED}❌ CHK FAILED ($FAIL_COUNT failures)${NC}"
    exit 1
fi
