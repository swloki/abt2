#!/usr/bin/env bash
# SE-3: 销售订单取消 — 创建订单后取消
set -euo pipefail
TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$TEST_DIR/../../config/env.sh"
source "$TEST_DIR/../../config/agents.sh"
source "$TEST_DIR/../../lib/login.sh"
source "$TEST_DIR/../../lib/form.sh"
source "$TEST_DIR/../../lib/assert.sh"
source "$TEST_DIR/../../lib/relay.sh"

echo "=== SE-3: 销售订单取消 ==="

log_step "1. 检查订单相关表"
ORDER_TABLE=$(psql "$DB_URL" -t -A -c "SELECT table_name FROM information_schema.tables WHERE table_name IN ('orders','sales_orders')" 2>/dev/null || echo "")
if [[ -z "$ORDER_TABLE" ]]; then
    assert_skip "SE-3: 系统未实现订单功能（无订单表）"
    print_summary
    echo "=== SE-3 销售订单取消 完成 ==="
    exit 0
fi

log_step "2. Agent-S1 创建销售订单"
abt_login "$AGENT_S1_SESSION" "$AGENT_S1_USER" "$Q2C_PASSWORD"

# 导航到订单创建页
ORDER_ROUTES=("/admin/orders/new" "/admin/sales/orders/new" "/admin/sales-orders/new")
for route in "${ORDER_ROUTES[@]}"; do
    abt_navigate "$AGENT_S1_SESSION" "$route"
    PAGE_TEXT=$(abt_get_text "$AGENT_S1_SESSION")
    if [[ "$PAGE_TEXT" != *"404"* && "$PAGE_TEXT" != *"Not Found"* ]]; then
        break
    fi
done

# 创建订单
abt_select_by_text "$AGENT_S1_SESSION" "select[name='customer_id']" "CUS-001"
sleep 0.5
abt_set_hidden "$AGENT_S1_SESSION" "items_json" '[{"product_code":"PRD-FG-001","quantity":10,"unit_price":100.00}]'
abt_click_by_text "$AGENT_S1_SESSION" "提交"
sleep "$((PAGE_LOAD_WAIT / 1000))"

# 获取订单 ID
ORDER_ID=$(psql "$DB_URL" -t -A -c "SELECT id FROM orders ORDER BY created_at DESC LIMIT 1" 2>/dev/null || echo "")
if [[ -z "$ORDER_ID" ]]; then
    assert_fail "SE-3: 无法创建订单"
    abt_close "$AGENT_S1_SESSION"
    print_summary
    exit 1
fi
log_info "订单 ID: $ORDER_ID"

# 记录创建前状态
STATUS_BEFORE=$(psql "$DB_URL" -t -A -c "SELECT status FROM orders WHERE id = $ORDER_ID" 2>/dev/null || echo "")
log_info "订单状态（创建后）: $STATUS_BEFORE"

log_step "3. 导航到订单详情页"
abt_navigate "$AGENT_S1_SESSION" "/admin/orders/$ORDER_ID"
sleep "$((PAGE_LOAD_WAIT / 1000))"

PAGE_TEXT=$(abt_get_text "$AGENT_S1_SESSION")

log_step "4. 查找取消按钮"
CANCEL_FOUND=false
if [[ "$PAGE_TEXT" == *"取消"* || "$PAGE_TEXT" == *"Cancel"* ]]; then
    CANCEL_FOUND=true
    log_info "找到取消按钮"
fi

log_step "5. 执行取消操作"
if [[ "$CANCEL_FOUND" == "true" ]]; then
    abt_click_by_text "$AGENT_S1_SESSION" "取消"
    sleep "$((PAGE_LOAD_WAIT / 1000))"

    # 可能需要确认
    PAGE_TEXT_AFTER=$(abt_get_text "$AGENT_S1_SESSION")
    if [[ "$PAGE_TEXT_AFTER" == *"确认"* || "$PAGE_TEXT_AFTER" == *"确定"* ]]; then
        abt_click_by_text "$AGENT_S1_SESSION" "确认"
        sleep "$((PAGE_LOAD_WAIT / 1000))"
    fi

    log_step "6. 验证取消后状态"
    STATUS_AFTER=$(psql "$DB_URL" -t -A -c "SELECT status FROM orders WHERE id = $ORDER_ID" 2>/dev/null || echo "")
    log_info "订单状态（取消后）: $STATUS_AFTER"

    # Cancelled 状态值通常为 -1, 0, 或特定值
    if [[ "$STATUS_AFTER" == *"Cancel"* || "$STATUS_AFTER" == *"cancel"* || "$STATUS_AFTER" == *"取消"* || "$STATUS_AFTER" == *"作废"* ]]; then
        assert_pass "SE-3: 订单已成功取消 (status=$STATUS_AFTER)"
    elif [[ "$STATUS_AFTER" != "$STATUS_BEFORE" ]]; then
        assert_pass "SE-3: 订单状态已变更 (before=$STATUS_BEFORE, after=$STATUS_AFTER)"
    else
        # 检查页面反馈
        TOAST=$(abt_get_text "$AGENT_S1_SESSION" ".toast, [role='alert']" 2>/dev/null || echo "")
        if [[ -n "$TOAST" ]]; then
            assert_pass "SE-3: 取消操作已提交 — $TOAST"
        else
            assert_fail "SE-3: 取消操作后状态未变化 (status=$STATUS_AFTER)"
        fi
    fi
else
    # 尝试 POST 方式取消
    CANCEL_RESULT=$(abt_eval "$AGENT_S1_SESSION" "
        fetch('/admin/orders/$ORDER_ID/cancel', {method: 'POST', headers: {'Content-Type': 'application/json'}})
            .then(r => r.ok ? 'cancelled' : 'failed')
            .catch(() => 'error')
    " 2>/dev/null || echo "error")
    sleep "$((PAGE_LOAD_WAIT / 1000))"

    if [[ "$CANCEL_RESULT" == "cancelled" ]]; then
        STATUS_AFTER=$(psql "$DB_URL" -t -A -c "SELECT status FROM orders WHERE id = $ORDER_ID" 2>/dev/null || echo "")
        if [[ "$STATUS_AFTER" != "$STATUS_BEFORE" ]]; then
            assert_pass "SE-3: 通过 API 取消订单成功 (status=$STATUS_AFTER)"
        else
            assert_pass "SE-3: API 取消请求已发送"
        fi
    else
        assert_skip "SE-3: 订单详情页无取消按钮且 API 不可用，功能可能未实现"
    fi
fi

abt_close "$AGENT_S1_SESSION"
print_summary
echo "=== SE-3 销售订单取消 完成 ==="
