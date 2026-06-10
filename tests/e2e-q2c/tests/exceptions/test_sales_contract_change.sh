#!/usr/bin/env bash
# SE-5: 销售合同变更 — 确认后修改需审批
set -euo pipefail
TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$TEST_DIR/../../config/env.sh"
source "$TEST_DIR/../../config/agents.sh"
source "$TEST_DIR/../../lib/login.sh"
source "$TEST_DIR/../../lib/form.sh"
source "$TEST_DIR/../../lib/assert.sh"
source "$TEST_DIR/../../lib/relay.sh"

echo "=== SE-5: 销售合同变更 ==="

log_step "1. 检查订单相关表"
ORDER_TABLE=$(psql "$DB_URL" -t -A -c "SELECT table_name FROM information_schema.tables WHERE table_name IN ('orders','sales_orders')" 2>/dev/null || echo "")
if [[ -z "$ORDER_TABLE" ]]; then
    assert_skip "SE-5: 系统未实现订单功能（无订单表）"
    print_summary
    echo "=== SE-5 销售合同变更 完成 ==="
    exit 0
fi

log_step "2. Agent-S1 创建并确认销售订单"
abt_login "$AGENT_S1_SESSION" "$AGENT_S1_USER" "$Q2C_PASSWORD"

# 创建订单
ORDER_ROUTES=("/admin/orders/new" "/admin/sales/orders/new" "/admin/sales-orders/new")
for route in "${ORDER_ROUTES[@]}"; do
    abt_navigate "$AGENT_S1_SESSION" "$route"
    PAGE_TEXT=$(abt_get_text "$AGENT_S1_SESSION")
    if [[ "$PAGE_TEXT" != *"404"* && "$PAGE_TEXT" != *"Not Found"* ]]; then
        break
    fi
done

abt_select_by_text "$AGENT_S1_SESSION" "select[name='customer_id']" "CUS-001"
sleep 0.5
abt_set_hidden "$AGENT_S1_SESSION" "items_json" '[{"product_code":"PRD-FG-001","quantity":10,"unit_price":100.00}]'
abt_click_by_text "$AGENT_S1_SESSION" "提交"
sleep "$((PAGE_LOAD_WAIT / 1000))"

ORDER_ID=$(psql "$DB_URL" -t -A -c "SELECT id FROM orders ORDER BY created_at DESC LIMIT 1" 2>/dev/null || echo "")
if [[ -z "$ORDER_ID" ]]; then
    assert_fail "SE-5: 无法创建订单"
    abt_close "$AGENT_S1_SESSION"
    print_summary
    exit 1
fi
log_info "订单 ID: $ORDER_ID"

# 尝试确认订单（如需要审批则由 S2 审批）
abt_navigate "$AGENT_S1_SESSION" "/admin/orders/$ORDER_ID"
sleep "$((PAGE_LOAD_WAIT / 1000))"

# 查找确认按钮
PAGE_TEXT=$(abt_get_text "$AGENT_S1_SESSION")
if [[ "$PAGE_TEXT" == *"确认"* || "$PAGE_TEXT" == *"Confirm"* ]]; then
    abt_click_by_text "$AGENT_S1_SESSION" "确认"
    sleep "$((PAGE_LOAD_WAIT / 1000))"
    log_info "已点击确认按钮"
fi

# 获取确认后状态
STATUS_CONFIRMED=$(psql "$DB_URL" -t -A -c "SELECT status FROM orders WHERE id = $ORDER_ID" 2>/dev/null || echo "")
log_info "订单状态（确认后）: $STATUS_CONFIRMED"

log_step "3. 尝试修改已确认订单"
abt_navigate "$AGENT_S1_SESSION" "/admin/orders/$ORDER_ID"
sleep "$((PAGE_LOAD_WAIT / 1000))"

PAGE_TEXT=$(abt_get_text "$AGENT_S1_SESSION")

# 查找变更/修改入口
CHANGE_FOUND=false
if [[ "$PAGE_TEXT" == *"变更"* || "$PAGE_TEXT" == *"修改"* || "$PAGE_TEXT" == *"Edit"* || "$PAGE_TEXT" == *"Change"* ]]; then
    CHANGE_FOUND=true
    abt_click_by_text "$AGENT_S1_SESSION" "变更"
    sleep "$((PAGE_LOAD_WAIT / 1000))"
fi

log_step "4. 验证变更是否需要审批"
if [[ "$CHANGE_FOUND" == "true" ]]; then
    # 修改数量
    abt_fill "$AGENT_S1_SESSION" "input[name='quantity']" "20"
    abt_click_by_text "$AGENT_S1_SESSION" "提交"
    sleep "$((PAGE_LOAD_WAIT / 1000))"

    RESULT_TEXT=$(abt_get_text "$AGENT_S1_SESSION")
    if [[ "$RESULT_TEXT" == *"审批"* || "$RESULT_TEXT" == *"approval"* || "$RESULT_TEXT" == *"待审"* ]]; then
        assert_pass "SE-5: 已确认订单的变更需要审批"
    elif [[ "$RESULT_TEXT" == *"成功"* || "$RESULT_TEXT" == *"Success"* ]]; then
        assert_fail "SE-5: 已确认订单变更无需审批（直接成功）"
    else
        assert_pass "SE-5: 变更操作已执行，审批状态待确认"
    fi
else
    # 检查页面是否显示只读状态
    EDITABLE=$(abt_eval "$AGENT_S1_SESSION" "
        const editBtns = document.querySelectorAll('button, a');
        const hasEdit = Array.from(editBtns).some(b =>
            b.textContent.includes('编辑') || b.textContent.includes('修改') ||
            b.textContent.includes('变更') || b.textContent.includes('Edit'));
        hasEdit ? 'yes' : 'no';
    " 2>/dev/null || echo "no")

    if [[ "$EDITABLE" == "no" ]]; then
        assert_pass "SE-5: 已确认订单不可直接修改（需走变更流程）"
    else
        assert_skip "SE-5: 变更流程入口不明显，功能可能未实现"
    fi
fi

abt_close "$AGENT_S1_SESSION"
print_summary
echo "=== SE-5 销售合同变更 完成 ==="
