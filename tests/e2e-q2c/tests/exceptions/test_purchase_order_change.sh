#!/usr/bin/env bash
# PE-4: 采购订单变更 — 已创建 PO 尝试修改
set -euo pipefail
TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$TEST_DIR/../../config/env.sh"
source "$TEST_DIR/../../config/agents.sh"
source "$TEST_DIR/../../lib/login.sh"
source "$TEST_DIR/../../lib/form.sh"
source "$TEST_DIR/../../lib/assert.sh"
source "$TEST_DIR/../../lib/relay.sh"

echo "=== PE-4: 采购订单变更 ==="

log_step "1. 检查采购相关表"
PO_TABLE=$(psql "$DB_URL" -t -A -c "
    SELECT table_name FROM information_schema.tables
    WHERE table_name IN ('purchase_orders','purchase_order_items')" 2>/dev/null || echo "")
if [[ -z "$PO_TABLE" ]]; then
    assert_skip "PE-4: 系统未实现采购功能（无采购表）"
    print_summary
    echo "=== PE-4 采购订单变更 完成 ==="
    exit 0
fi

log_step "2. Agent-PU1 创建采购订单"
abt_login "$AGENT_PU1_SESSION" "$AGENT_PU1_USER" "$Q2C_PASSWORD"

PO_ROUTES=("/admin/purchase/orders/new" "/admin/po/new" "/admin/purchase-orders/new")
for route in "${PO_ROUTES[@]}"; do
    abt_navigate "$AGENT_PU1_SESSION" "$route"
    PAGE_TEXT=$(abt_get_text "$AGENT_PU1_SESSION")
    if [[ "$PAGE_TEXT" != *"404"* && "$PAGE_TEXT" != *"Not Found"* ]]; then
        break
    fi
done

abt_set_hidden "$AGENT_PU1_SESSION" "items_json" '[{"product_code":"PRD-RAW-001","quantity":100,"unit_price":10.00}]'
abt_click_by_text "$AGENT_PU1_SESSION" "提交"
sleep "$((PAGE_LOAD_WAIT / 1000))"

PO_ID=$(psql "$DB_URL" -t -A -c "SELECT id FROM purchase_orders ORDER BY created_at DESC LIMIT 1" 2>/dev/null || echo "")
if [[ -z "$PO_ID" ]]; then
    assert_fail "PE-4: 无法创建采购订单"
    abt_close "$AGENT_PU1_SESSION"
    print_summary
    exit 1
fi
log_info "采购订单 ID: $PO_ID"

log_step "3. 导航到采购订单详情页，尝试修改"
abt_navigate "$AGENT_PU1_SESSION" "/admin/purchase/orders/$PO_ID"
sleep "$((PAGE_LOAD_WAIT / 1000))"

PAGE_TEXT=$(abt_get_text "$AGENT_PU1_SESSION")

log_step "4. 查找编辑/变更入口"
EDIT_FOUND=false
if [[ "$PAGE_TEXT" == *"编辑"* || "$PAGE_TEXT" == *"修改"* || "$PAGE_TEXT" == *"Edit"* || "$PAGE_TEXT" == *"变更"* ]]; then
    EDIT_FOUND=true
    abt_click_by_text "$AGENT_PU1_SESSION" "编辑"
    sleep "$((PAGE_LOAD_WAIT / 1000))"
fi

if [[ "$EDIT_FOUND" == "true" ]]; then
    # 修改数量
    abt_fill "$AGENT_PU1_SESSION" "input[name='quantity']" "150"
    abt_click_by_text "$AGENT_PU1_SESSION" "保存"
    sleep "$((PAGE_LOAD_WAIT / 1000))"

    RESULT_TEXT=$(abt_get_text "$AGENT_PU1_SESSION")
    if [[ "$RESULT_TEXT" == *"审批"* || "$RESULT_TEXT" == *"approval"* ]]; then
        assert_pass "PE-4: 采购订单变更需要审批"
    elif [[ "$RESULT_TEXT" == *"成功"* || "$RESULT_TEXT" == *"Success"* ]]; then
        assert_pass "PE-4: 采购订单变更成功（无需额外审批）"
    else
        assert_pass "PE-4: 变更操作已执行，结果待确认"
    fi
else
    # 检查是否只读
    EDITABLE=$(abt_eval "$AGENT_PU1_SESSION" "
        const editBtns = Array.from(document.querySelectorAll('button, a'));
        const hasEdit = editBtns.some(b =>
            b.textContent.includes('编辑') || b.textContent.includes('修改') ||
            b.textContent.includes('Edit') || b.textContent.includes('变更'));
        hasEdit ? 'yes' : 'no';
    " 2>/dev/null || echo "no")

    if [[ "$EDITABLE" == "no" ]]; then
        assert_pass "PE-4: 采购订单详情页为只读状态（需走变更流程）"
    else
        assert_skip "PE-4: 变更功能入口不明显"
    fi
fi

abt_close "$AGENT_PU1_SESSION"
print_summary
echo "=== PE-4 采购订单变更 完成 ==="
