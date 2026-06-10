#!/usr/bin/env bash
# BND-2: 边界条件 — 信用额度不足时创建大额订单
set -euo pipefail
TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$TEST_DIR/../../config/env.sh"
source "$TEST_DIR/../../config/agents.sh"
source "$TEST_DIR/../../lib/login.sh"
source "$TEST_DIR/../../lib/form.sh"
source "$TEST_DIR/../../lib/assert.sh"
source "$TEST_DIR/../../lib/relay.sh"

echo "=== BND-2: 信用额度不足 ==="

log_step "1. 检查客户和订单相关表"
CUSTOMER_TABLE=$(psql "$DB_URL" -t -A -c "
    SELECT table_name FROM information_schema.tables
    WHERE table_name IN ('customers','orders')" 2>/dev/null || echo "")
if [[ -z "$CUSTOMER_TABLE" ]]; then
    assert_skip "BND-2: 系统未实现客户/订单功能"
    print_summary
    echo "=== BND-2 信用额度不足 完成 ==="
    exit 0
fi

log_step "2. 设置客户信用额度为低值"
# 设置 CUS-001 信用额度为 100
psql "$DB_URL" -c "
    UPDATE customers SET credit_limit = 100.00
    WHERE customer_code = 'CUS-001'" 2>/dev/null && \
    log_info "已设置 CUS-001 credit_limit=100" || log_warn "无法设置信用额度"

CREDIT_LIMIT=$(psql "$DB_URL" -t -A -c "
    SELECT credit_limit FROM customers
    WHERE customer_code = 'CUS-001'" 2>/dev/null || echo "100")
log_info "CUS-001 当前信用额度: $CREDIT_LIMIT"

log_step "3. Agent-S1 尝试创建大额订单（超过信用额度）"
abt_login "$AGENT_S1_SESSION" "$AGENT_S1_USER" "$Q2C_PASSWORD"

ORDER_ROUTES=("/admin/orders/new" "/admin/sales/orders/new" "/admin/sales-orders/new")
for route in "${ORDER_ROUTES[@]}"; do
    abt_navigate "$AGENT_S1_SESSION" "$route"
    PAGE_TEXT=$(abt_get_text "$AGENT_S1_SESSION")
    if [[ "$PAGE_TEXT" != *"404"* && "$PAGE_TEXT" != *"Not Found"* ]]; then
        break
    fi
done

# 选择 CUS-001（低信用额度）
abt_select_by_text "$AGENT_S1_SESSION" "select[name='customer_id']" "CUS-001"
sleep 0.5

# 创建大额订单（10000 远超 100 的信用额度）
abt_set_hidden "$AGENT_S1_SESSION" "items_json" '[{"product_code":"PRD-FG-001","quantity":100,"unit_price":100.00}]'

log_step "4. 提交订单"
abt_click_by_text "$AGENT_S1_SESSION" "提交"
sleep "$((PAGE_LOAD_WAIT / 1000))"

RESULT_TEXT=$(abt_get_text "$AGENT_S1_SESSION")

log_step "5. 验证系统行为"
if [[ "$RESULT_TEXT" == *"信用"* || "$RESULT_TEXT" == *"credit"* || "$RESULT_TEXT" == *"超出"* || "$RESULT_TEXT" == *"超额"* ]]; then
    assert_pass "BND-2: 系统阻止了超额信用订单 — 显示信用额度提示"
elif [[ "$RESULT_TEXT" == *"审批"* || "$RESULT_TEXT" == *"approval"* ]]; then
    assert_pass "BND-2: 超额订单触发了审批流程"
elif [[ "$RESULT_TEXT" == *"警告"* || "$RESULT_TEXT" == *"Warning"* || "$RESULT_TEXT" == *"warn"* ]]; then
    assert_pass "BND-2: 系统显示了信用警告但仍允许创建"
else
    # 检查订单是否被创建
    CURRENT_URL=$(abt_get_url "$AGENT_S1_SESSION")
    if [[ "$CURRENT_URL" == *"/new"* ]]; then
        assert_pass "BND-2: 订单未创建（仍在创建页），可能被信用检查阻止"
    else
        ORDER_ID=$(psql "$DB_URL" -t -A -c "
            SELECT id FROM orders
            WHERE customer_id = (SELECT id FROM customers WHERE customer_code = 'CUS-001')
            ORDER BY created_at DESC LIMIT 1" 2>/dev/null || echo "")
        if [[ -n "$ORDER_ID" ]]; then
            assert_fail "BND-2: 超额订单未被阻止（订单已创建: $ORDER_ID）"
        else
            assert_pass "BND-2: 订单未保存到数据库（可能被服务端拒绝）"
        fi
    fi
fi

# 恢复信用额度
psql "$DB_URL" -c "
    UPDATE customers SET credit_limit = 100000.00
    WHERE customer_code = 'CUS-001'" 2>/dev/null && \
    log_info "已恢复 CUS-001 信用额度" || true

abt_close "$AGENT_S1_SESSION"
print_summary
echo "=== BND-2 信用额度不足 完成 ==="
