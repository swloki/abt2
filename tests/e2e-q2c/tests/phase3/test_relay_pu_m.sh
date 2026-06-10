#!/usr/bin/env bash
# ============================================================================
# Q2C E2E 测试 — U7: 采购+生产并行链路验证
# 整合 U1-U6 为完整链路: 采购(先) → 生产(后，依赖采购入库)
# 执行顺序:
#   1. PU1-PU4: 创建采购订单
#   2. PU5-PU6: 收货入库 + 来料检验
#   3. M1: 生产工单下达
#   4. M2: 生产领料（依赖采购入库）
#   5. M3-M4: 报工 + 质检
#   6. M5: 成品入库
#   7. 数据完整性验证
# ============================================================================
set -euo pipefail

TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_DIR="$TEST_DIR/../.."
PHASE3A="$BASE_DIR/tests/phase3a"
PHASE3B="$BASE_DIR/tests/phase3b"

source "$BASE_DIR/config/env.sh"
source "$BASE_DIR/config/agents.sh"
source "$BASE_DIR/lib/login.sh"
source "$BASE_DIR/lib/form.sh"
source "$BASE_DIR/lib/assert.sh"
source "$BASE_DIR/lib/relay.sh"

echo "============================================"
echo "  Phase 3: 采购+生产并行链路验证"
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

# 初始化接力文件（如果还没有的话）
if [[ ! -f "$RELAY_FILE" ]] || [[ "$(jq '.run_id' "$RELAY_FILE" 2>/dev/null)" == "" ]]; then
    relay_init "q2c-p3-$(date +%Y%m%d%H%M%S)"
fi
relay_set_status "running"

# --- 定义测试步骤 ---
TOTAL_STEPS=6
PASS_COUNT=0
FAIL_COUNT=0

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

# Step 1: 采购订单创建 (PU1-PU4)
run_step 1 "PU1-PU4 采购订单" "$PHASE3A/test_pu1_pu4_purchase_order.sh" || {
    log_fail "链路在 PU1-PU4 断裂"
    goto_summary
}

# Step 2: 收货入库 + 来料检验 (PU5-PU6)
run_step 2 "PU5-PU6 收货入库" "$PHASE3A/test_pu5_pu6_goods_receipt.sh" || {
    log_fail "链路在 PU5-PU6 断裂"
    goto_summary
}

# Step 3: 生产工单下达 (M1)
run_step 3 "M1 工单下达" "$PHASE3B/test_m1_work_order_release.sh" || {
    log_fail "链路在 M1 断裂"
    goto_summary
}

# Step 4: 生产领料 (M2) — 同步点：依赖采购入库
run_step 4 "M2 生产领料" "$PHASE3B/test_m2_material_requisition.sh" || {
    log_fail "链路在 M2 断裂"
    goto_summary
}

# Step 5: 报工 + 质检 (M3-M4)
run_step 5 "M3-M4 报工与质检" "$PHASE3B/test_m3_m4_work_report_qc.sh" || {
    log_fail "链路在 M3-M4 断裂"
    goto_summary
}

# Step 6: 成品入库 (M5)
run_step 6 "M5 成品入库" "$PHASE3B/test_m5_finished_goods_receipt.sh" || {
    log_fail "链路在 M5 断裂"
    goto_summary
}

# --- Step 7: 数据完整性验证 ---
echo ""
echo "============================================"
echo "  数据完整性验证"
echo "============================================"

log_step "7a. 检查接力文件关键 artifacts"
relay_dump

KEYS=(
    "purchase_order_id"
    "purchase_receipt_done"
    "work_order_id"
    "material_requisition_done"
    "finished_goods_receipt_done"
    "finished_goods_qty"
)
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

# --- 最终库存验证 ---
log_step "7b. 最终库存验证"

PRODUCT_FG_ID=$(psql "$DB_URL" -t -A -c "SELECT product_id FROM products WHERE product_code = 'PRD-FG-001' AND deleted_at IS NULL LIMIT 1" 2>/dev/null)
WH_FG_ID=$(psql "$DB_URL" -t -A -c "SELECT id FROM warehouses WHERE code = 'WH-FG' AND deleted_at IS NULL LIMIT 1" 2>/dev/null || echo "")

# 成品库存
FG_QTY=$(psql "$DB_URL" -t -A -c "SELECT COALESCE(SUM(quantity),0) FROM stock_ledger WHERE product_id=$PRODUCT_FG_ID AND warehouse_id=$WH_FG_ID AND deleted_at IS NULL" 2>/dev/null || echo "0")
log_info "WH-FG 成品库存: PRD-FG-001 = $FG_QTY"

# SO 需求数量
SO_QTY=$(relay_read "so_quantity")
log_info "SO 需求: ${SO_QTY:-100}"

# 原材料剩余库存
WH_RAW_ID=$(psql "$DB_URL" -t -A -c "SELECT id FROM warehouses WHERE code = 'WH-RAW' AND deleted_at IS NULL LIMIT 1" 2>/dev/null || echo "")
echo "  原材料库存:"
for CODE in "PRD-RM-001" "PRD-RM-002" "PRD-RM-003"; do
    QTY=$(psql "$DB_URL" -t -A -c "
        SELECT COALESCE(SUM(quantity),0) FROM stock_ledger
        WHERE product_id = (SELECT product_id FROM products WHERE product_code = '$CODE')
          AND warehouse_id = $WH_RAW_ID AND deleted_at IS NULL" 2>/dev/null || echo "0")
    log_info "  $CODE (WH-RAW): $QTY"
done

# --- 汇总 ---
goto_summary() {
    :
}

echo ""
echo "============================================"
echo "  Phase 3 采购+生产链路完成"
echo "============================================"
echo "  通过: $PASS_COUNT/$TOTAL_STEPS"
echo "  失败: $FAIL_COUNT"
echo "============================================"

relay_set_status "$([[ $FAIL_COUNT -eq 0 ]] && echo 'completed' || echo 'failed')"
relay_snapshot "SNAP-PHASE3"

if [[ $FAIL_COUNT -eq 0 ]]; then
    echo ""
    echo -e "${GREEN}✅ Phase 3 PASSED${NC}"
    echo ""
    echo "接力数据摘要:"
    echo "  采购订单 ID:  $(relay_read purchase_order_id)"
    echo "  采购订单号:   $(relay_read purchase_order_doc_number)"
    echo "  工单 ID:      $(relay_read work_order_id)"
    echo "  工单号:       $(relay_read work_order_doc_number)"
    echo "  成品入库数量: $(relay_read finished_goods_qty)"
    echo "  成品仓:       $(relay_read finished_goods_warehouse)"
    echo ""
    echo "库存:"
    echo "  WH-FG: PRD-FG-001 = $FG_QTY"
    echo "  WH-RAW 剩余原材料如上"
    exit 0
else
    echo ""
    echo -e "${RED}❌ Phase 3 FAILED ($FAIL_COUNT failures)${NC}"
    exit 1
fi
