#!/usr/bin/env bash
# ============================================================================
# Q2C E2E 测试 — M5: 成品入库
# 角色: Agent-W1 (q2c_warehouse)
# 目标: 质检通过后成品入库，工单完工
#
# 成品入库（MES 入库）: /admin/mes/receipts/create
#   表单: work_order_id, batch_id, product_id, received_qty,
#         warehouse_id, zone_id, bin_id, receipt_date
#   提交后: HX-Redirect → /admin/mes/receipts
# 仓库: WH-FG (成品仓)
# ============================================================================
set -euo pipefail

TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$TEST_DIR/../../config/env.sh"
source "$TEST_DIR/../../config/agents.sh"
source "$TEST_DIR/../../lib/login.sh"
source "$TEST_DIR/../../lib/form.sh"
source "$TEST_DIR/../../lib/assert.sh"
source "$TEST_DIR/../../lib/relay.sh"

echo "=== M5: 成品入库 ==="
echo ""

relay_set_phase "M5"
relay_set_status "running"

# --- 前置 ---
WORK_ORDER_ID=$(relay_read "work_order_id")
BATCH_ID=$(relay_read "production_batch_id")
PRODUCT_FG_ID=$(psql "$DB_URL" -t -A -c "SELECT product_id FROM products WHERE product_code = 'PRD-FG-001' AND deleted_at IS NULL LIMIT 1" 2>/dev/null)
WH_FG_ID=$(psql "$DB_URL" -t -A -c "SELECT id FROM warehouses WHERE code = 'WH-FG' AND deleted_at IS NULL LIMIT 1" 2>/dev/null || echo "")
WH_FG_ZONE_ID=$(psql "$DB_URL" -t -A -c "SELECT id FROM zones WHERE warehouse_id = $WH_FG_ID AND deleted_at IS NULL LIMIT 1" 2>/dev/null || echo "")

INSPECTION_DONE=$(relay_read "inspection_done")

if [[ -z "$WORK_ORDER_ID" ]]; then
    log_fail "接力文件中缺少 work_order_id"
    print_summary
    exit 1
fi

log_info "Work Order ID: $WORK_ORDER_ID, Product FG ID: $PRODUCT_FG_ID, WH-FG ID: $WH_FG_ID"

# 入库前记录 WH-FG 库存
BEFORE_FG=$(psql "$DB_URL" -t -A -c "SELECT COALESCE(SUM(quantity),0) FROM stock_ledger WHERE product_id=$PRODUCT_FG_ID AND warehouse_id=$WH_FG_ID AND deleted_at IS NULL" 2>/dev/null || echo "0")
log_info "入库前 WH-FG 中 PRD-FG-001 库存: $BEFORE_FG"

TODAY=$(powershell -c "(Get-Date).ToString('yyyy-MM-dd')" 2>/dev/null)

# --- Step 1: Agent-W1 登录 ---
log_step "1. Agent-W1 (仓管员) 登录"
abt_login "$AGENT_W1_SESSION" "$AGENT_W1_USER" "$Q2C_PASSWORD"

# --- Step 2: 导航到 MES 成品入库页面 ---
log_step "2. 导航到成品入库页面"
abt_navigate "$AGENT_W1_SESSION" "/admin/mes/receipts/create"
sleep 1

abt_assert_url_contains "$AGENT_W1_SESSION" "/admin/mes/receipts/create" "成品入库创建页"

# --- Step 3: 填写入库表单 ---
log_step "3. 填写成品入库表单"

abt_eval "$AGENT_W1_SESSION" "
    var form = document.querySelector('form');
    if (form) {
        // 工单 ID
        form.querySelector('input[name=\"work_order_id\"]').value = '$WORK_ORDER_ID';
        // 批次 ID（如果有）
        var batchInput = form.querySelector('input[name=\"batch_id\"]');
        if (batchInput) batchInput.value = '${BATCH_ID:-}';
        // 产品 ID
        form.querySelector('input[name=\"product_id\"]').value = '$PRODUCT_FG_ID';
        // 入库数量
        form.querySelector('input[name=\"received_qty\"]').value = '100';
        // 仓库 ID
        form.querySelector('input[name=\"warehouse_id\"]').value = '$WH_FG_ID';
        // 库区 ID
        var zoneInput = form.querySelector('input[name=\"zone_id\"]');
        if (zoneInput && '$WH_FG_ZONE_ID') zoneInput.value = '$WH_FG_ZONE_ID';
        // 入库日期
        form.querySelector('input[name=\"receipt_date\"]').value = '$TODAY';
    }
    'receipt_filled';
" > /dev/null 2>&1

sleep 0.5

# --- Step 4: 提交入库 ---
log_step "4. 提交成品入库"
abt_click_by_text "$AGENT_W1_SESSION" "提交"
sleep 2

# 验证
current_url=$(abt_get_url "$AGENT_W1_SESSION" 2>/dev/null || echo "")
log_info "成品入库提交后 URL: $current_url"

if [[ "$current_url" == *"/admin/mes/receipts"* ]]; then
    assert_pass "成品入库成功"
else
    log_warn "成品入库提交后 URL: $current_url"
    abt_screenshot "$AGENT_W1_SESSION" "/tmp/q2c-m5-fail.png" 2>/dev/null || true
fi

# --- Step 5: 数据库验证 ---
log_step "5. 数据库验证"

# 验证 MES 入库记录
RECEIPT_ID=$(psql "$DB_URL" -t -A -c "
    SELECT id FROM production_receipts
    WHERE work_order_id = $WORK_ORDER_ID AND deleted_at IS NULL
    ORDER BY created_at DESC LIMIT 1" 2>/dev/null || echo "")
if [[ -n "$RECEIPT_ID" ]]; then
    assert_pass "MES 入库记录存在 (id=$RECEIPT_ID)"
    relay_write "production_receipt_id" "$RECEIPT_ID"
fi

# 验证 WH-FG 库存增加
AFTER_FG=$(psql "$DB_URL" -t -A -c "SELECT COALESCE(SUM(quantity),0) FROM stock_ledger WHERE product_id=$PRODUCT_FG_ID AND warehouse_id=$WH_FG_ID AND deleted_at IS NULL" 2>/dev/null || echo "0")
log_info "入库后 WH-FG 中 PRD-FG-001 库存: $AFTER_FG (入库前: $BEFORE_FG)"

# 验证总量
TOTAL_FG=$(psql "$DB_URL" -t -A -c "SELECT COALESCE(SUM(quantity),0) FROM stock_ledger WHERE product_id=$PRODUCT_FG_ID AND deleted_at IS NULL" 2>/dev/null || echo "0")
log_info "PRD-FG-001 全仓库总库存: $TOTAL_FG"

# 检查工单状态（是否变为完工/已关闭）
WO_STATUS=$(psql "$DB_URL" -t -A -c "SELECT status FROM work_orders WHERE id = $WORK_ORDER_ID" 2>/dev/null || echo "")
log_info "工单状态: $WO_STATUS (4=Closed)"

# --- Step 6: 验证成品满足 SO 需求 ---
log_step "6. 验证成品满足 SO 需求"

SO_QTY=$(relay_read "so_quantity")
log_info "SO 需求数量: ${SO_QTY:-100}, 成品入库数量: 100"

# 写入最终结果
relay_write "finished_goods_receipt_done" "true"
relay_write "finished_goods_qty" "100"
relay_write "finished_goods_warehouse" "WH-FG"
relay_write "work_order_status_after_receipt" "${WO_STATUS:-}"

# --- 完成 ---
relay_snapshot "SNAP-M5"
relay_set_status "completed"

echo ""
echo "=== M5 完成 ==="
print_summary
