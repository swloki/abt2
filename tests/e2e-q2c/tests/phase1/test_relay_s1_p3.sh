#!/usr/bin/env bash
# ============================================================================
# Q2C E2E 测试 — Phase 1+2 完整接力链路
# 按顺序执行 S1→S2→S3→S4→S5→P1→P2→P3，验证数据在节点间正确传递
# 如果任一节点失败，记录失败点并停止
# ============================================================================
set -euo pipefail

TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_DIR="$TEST_DIR/../.."

source "$BASE_DIR/config/env.sh"
source "$BASE_DIR/config/agents.sh"
source "$BASE_DIR/lib/login.sh"
source "$BASE_DIR/lib/form.sh"
source "$BASE_DIR/lib/assert.sh"
source "$BASE_DIR/lib/relay.sh"

echo "============================================"
echo "  Phase 1+2: S1→P3 完整接力链路"
echo "============================================"
echo ""

# --- 环境初始化 ---
log_step "0. 环境初始化"
bash "$BASE_DIR/scripts/setup.sh"

if [[ $? -ne 0 ]]; then
    log_fail "环境初始化失败"
    exit 1
fi
assert_pass "环境初始化完成"

# 初始化接力文件
relay_init "q2c-s1-p3-$(date +%Y%m%d%H%M%S)"
relay_set_status "running"

# --- 定义测试步骤 ---
TOTAL_STEPS=5
PASS_COUNT=0
FAIL_COUNT=0

# goto_summary: 跳到汇总输出（在失败时调用）
goto_summary() {
    :
}

run_step() {
    local step_num="$1"
    local step_name="$2"
    local script_path="$3"

    echo ""
    echo "============================================"
    echo "  Step $step_num/$TOTAL_STEPS: $step_name"
    echo "============================================"

    relay_set_phase "$step_name"

    if [[ ! -f "$script_path" ]]; then
        log_fail "脚本不存在: $script_path"
        ((FAIL_COUNT++))
        return 1
    fi

    if bash "$script_path"; then
        log_pass "Step $step_num: $step_name PASSED"
        ((PASS_COUNT++))
        return 0
    else
        log_fail "Step $step_num: $step_name FAILED"
        ((FAIL_COUNT++))
        return 1
    fi
}

# --- 执行链路 ---

# Step 1: S1-S2 报价创建
run_step 1 "S1-S2 销售报价创建" "$TEST_DIR/test_s1_s2_quotation.sh" || {
    log_fail "链路在 S1-S2 断裂"
    goto_summary
}

# Step 2: S3 报价审批
run_step 2 "S3 报价审批" "$TEST_DIR/test_s3_approval.sh" || {
    log_fail "链路在 S3 断裂"
    goto_summary
}

# Step 3: S4-S5 销售订单
run_step 3 "S4-S5 销售订单创建" "$TEST_DIR/test_s4_s5_sales_order.sh" || {
    log_fail "链路在 S4-S5 断裂"
    goto_summary
}

# Step 4: P1-P3 生产计划（MRP）
run_step 4 "P1-P3 生产计划" "$BASE_DIR/tests/phase2/test_p1_p3_mrp.sh" || {
    log_fail "链路在 P1-P3 断裂"
    goto_summary
}

# Step 5: 接力数据完整性验证
echo ""
echo "============================================"
echo "  Step 5/5: 接力数据完整性验证"
echo "============================================"

log_step "5a. 检查接力文件关键 artifacts"
relay_dump

KEYS=("quotation_id" "quotation_status" "sales_order_id" "work_order_id")
ALL_PRESENT=true
for key in "${KEYS[@]}"; do
    val=$(relay_read "$key")
    if [[ -z "$val" ]]; then
        log_fail "接力文件缺少: $key"
        ALL_PRESENT=false
        ((FAIL_COUNT++))
    else
        log_pass "接力文件包含: $key=$val"
        ((PASS_COUNT++))
    fi
done

if [[ "$ALL_PRESENT" == "true" ]]; then
    assert_pass "所有接力关键数据完整"
fi

# --- 汇总 ---

echo ""
echo "============================================"
echo "  Phase 1+2 接力链路完成"
echo "============================================"
echo "  通过: $PASS_COUNT/$TOTAL_STEPS"
echo "  失败: $FAIL_COUNT"
echo "============================================"

relay_set_status "$([[ $FAIL_COUNT -eq 0 ]] && echo 'completed' || echo 'failed')"
relay_snapshot "SNAP-S1-P3"

if [[ $FAIL_COUNT -eq 0 ]]; then
    echo ""
    echo -e "${GREEN}✅ Phase 1+2 PASSED${NC}"
    echo ""
    echo "接力数据摘要:"
    echo "  报价 ID:    $(relay_read quotation_id)"
    echo "  报价状态:   $(relay_read quotation_status)"
    echo "  订单 ID:    $(relay_read sales_order_id)"
    echo "  订单号:     $(relay_read sales_order_doc_number)"
    echo "  工单 ID:    $(relay_read work_order_id)"
    echo "  工单号:     $(relay_read work_order_doc_number)"
    exit 0
else
    echo ""
    echo -e "${RED}❌ Phase 1+2 FAILED ($FAIL_COUNT failures)${NC}"
    exit 1
fi
