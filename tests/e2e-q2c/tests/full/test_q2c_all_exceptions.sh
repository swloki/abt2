#!/usr/bin/env bash
# ============================================================================
# Q2C E2E — 异常+逆向+通知 全链路集成测试
# 按类别顺序执行所有异常、逆向操作和通知验证脚本
# 汇总 PASS/FAIL/SKIP 统计
# ============================================================================
set -euo pipefail

BASE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
source "$BASE_DIR/config/env.sh"
source "$BASE_DIR/config/agents.sh"
source "$BASE_DIR/lib/login.sh"
source "$BASE_DIR/lib/form.sh"
source "$BASE_DIR/lib/assert.sh"
source "$BASE_DIR/lib/relay.sh"

EXCEPTIONS_DIR="$BASE_DIR/tests/exceptions"
REVERSE_DIR="$BASE_DIR/tests/reverse"
NOTIFICATIONS_DIR="$BASE_DIR/tests/notifications"

echo "============================================"
echo "  Q2C 异常+逆向+通知 全链路集成测试"
echo "============================================"
echo ""

# --- 分类统计 ---
declare -A CATEGORY_PASS
declare -A CATEGORY_FAIL
declare -A CATEGORY_SKIP
TOTAL_PASS=0
TOTAL_FAIL=0
TOTAL_SKIP=0
TOTAL_RUN=0

# --- 执行单个测试脚本 ---
run_test() {
    local script_path="$1"
    local category="$2"
    local script_name
    script_name=$(basename "$script_path")

    echo ""
    echo "--- 运行: $script_name ---"

    if [[ ! -f "$script_path" ]]; then
        log_fail "脚本不存在: $script_path"
        ((CATEGORY_FAIL[$category]++))
        ((TOTAL_FAIL++))
        ((TOTAL_RUN++))
        return 1
    fi

    # 执行脚本，捕获退出码
    if bash "$script_path" 2>&1; then
        ((CATEGORY_PASS[$category]++))
        ((TOTAL_PASS++))
        log_pass "$script_name: PASSED"
    else
        exit_code=$?
        if [[ $exit_code -eq 0 ]]; then
            ((CATEGORY_PASS[$category]++))
            ((TOTAL_PASS++))
            log_pass "$script_name: PASSED"
        else
            ((CATEGORY_FAIL[$category]++))
            ((TOTAL_FAIL++))
            log_fail "$script_name: FAILED (exit=$exit_code)"
        fi
    fi
    ((TOTAL_RUN++))
    return 0
}

# --- 执行分类中所有脚本 ---
run_category() {
    local category="$1"
    local dir="$2"
    shift 2
    local scripts=("$@")

    echo ""
    echo "============================================"
    echo "  类别: $category"
    echo "============================================"

    CATEGORY_PASS[$category]=0
    CATEGORY_FAIL[$category]=0
    CATEGORY_SKIP[$category]=0

    if [[ ${#scripts[@]} -eq 0 ]]; then
        # 运行目录下所有脚本
        if [[ -d "$dir" ]]; then
            for script in "$dir"/test_*.sh; do
                if [[ -f "$script" ]]; then
                    run_test "$script" "$category"
                fi
            done
        else
            log_warn "目录不存在: $dir"
        fi
    else
        for script in "${scripts[@]}"; do
            local full_path="$dir/$script"
            run_test "$full_path" "$category"
        done
    fi
}

# ============================================================================
# 执行所有测试类别
# ============================================================================

# --- 1. 审批异常 (AP-E1~E8) ---
run_category "审批异常" "$EXCEPTIONS_DIR" \
    test_approval_reject_resubmit.sh \
    test_approval_timeout.sh \
    test_approval_delegate.sh \
    test_approval_withdraw.sh \
    test_approval_countersign_reject.sh \
    test_approval_concurrent.sh \
    test_approval_data_changed.sh \
    test_approval_history.sh

# --- 2. 销售异常 (SE-1~6) ---
run_category "销售异常" "$EXCEPTIONS_DIR" \
    test_sales_credit_freeze.sh \
    test_sales_order_change.sh \
    test_sales_order_cancel.sh \
    test_sales_quotation_expire.sh \
    test_sales_contract_change.sh \
    test_sales_partial_delivery.sh

# --- 3. 采购异常 (PE-1~6) ---
run_category "采购异常" "$EXCEPTIONS_DIR" \
    test_purchase_over_delivery.sh \
    test_purchase_short_delivery.sh \
    test_purchase_reject.sh \
    test_purchase_order_change.sh \
    test_purchase_over_budget.sh \
    test_purchase_single_source.sh

# --- 4. 生产+质量异常 (ME-1~4, QE-1~3) ---
run_category "生产异常" "$EXCEPTIONS_DIR" \
    test_production_over_issue.sh \
    test_production_return_material.sh \
    test_production_rework.sh \
    test_production_scrap.sh

run_category "质量异常" "$EXCEPTIONS_DIR" \
    test_quality_reject_mrb.sh \
    test_quality_concession.sh \
    test_quality_batch_scrap.sh

# --- 5. 边界条件 (BND-1~3) ---
run_category "边界条件" "$EXCEPTIONS_DIR" \
    test_boundary_insufficient_stock.sh \
    test_boundary_credit_exceeded.sh \
    test_boundary_bom_missing.sh

# --- 6. 逆向操作 (REV-1~4) ---
run_category "逆向操作" "$REVERSE_DIR" \
    test_sales_return.sh \
    test_purchase_return.sh \
    test_invoice_reversal.sh \
    test_payment_reversal.sh

# --- 7. 通知验证 (NT-ALL) ---
run_category "通知验证" "$NOTIFICATIONS_DIR" \
    test_notifications.sh

# ============================================================================
# 汇总报告
# ============================================================================

echo ""
echo "============================================"
echo "  Q2C 异常+逆向+通知 测试汇总"
echo "============================================"
echo ""

# 分类汇总
for cat in "审批异常" "销售异常" "采购异常" "生产异常" "质量异常" "边界条件" "逆向操作" "通知验证"; do
    p=${CATEGORY_PASS[$cat]:-0}
    f=${CATEGORY_FAIL[$cat]:-0}
    s=${CATEGORY_SKIP[$cat]:-0}
    total=$((p + f + s))
    if [[ $total -gt 0 ]]; then
        printf "  %-12s  PASS: %2d  FAIL: %2d  SKIP: %2d  (%d scripts)\n" "$cat" "$p" "$f" "$s" "$total"
    fi
done

echo ""
echo "-------------------------------------------"
printf "  %-12s  PASS: %2d  FAIL: %2d  SKIP: %2d  (%d total)\n" "TOTAL" "$TOTAL_PASS" "$TOTAL_FAIL" "$TOTAL_SKIP" "$TOTAL_RUN"
echo "-------------------------------------------"
echo ""

if [[ $TOTAL_FAIL -eq 0 ]]; then
    echo -e "${GREEN}RESULT: ALL PASSED (with $TOTAL_SKIP skips)${NC}"
    EXIT_CODE=0
else
    echo -e "${RED}RESULT: $TOTAL_FAIL FAILURES${NC}"
    EXIT_CODE=1
fi

echo "============================================"
exit $EXIT_CODE
