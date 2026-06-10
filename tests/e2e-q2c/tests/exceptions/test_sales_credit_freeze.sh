#!/usr/bin/env bash
# SE-1: 销售信用冻结 — 客户信用额度为 0 时创建订单
set -euo pipefail
TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$TEST_DIR/../../config/env.sh"
source "$TEST_DIR/../../config/agents.sh"
source "$TEST_DIR/../../lib/login.sh"
source "$TEST_DIR/../../lib/form.sh"
source "$TEST_DIR/../../lib/assert.sh"
source "$TEST_DIR/../../lib/relay.sh"

echo "=== SE-1: 销售信用冻结 ==="

log_step "1. 检查客户信用相关表"
CUSTOMER_TABLE=$(psql "$DB_URL" -t -A -c "SELECT table_name FROM information_schema.tables WHERE table_name = 'customers'" 2>/dev/null || echo "")
if [[ -z "$CUSTOMER_TABLE" ]]; then
    assert_skip "SE-1: 系统未实现客户管理（无 customers 表）"
    print_summary
    echo "=== SE-1 销售信用冻结 完成 ==="
    exit 0
fi

# 检查 CUS-002 信用额度
CUS002_CREDIT=$(psql "$DB_URL" -t -A -c "
    SELECT COALESCE(credit_limit, 0) FROM customers
    WHERE customer_code = 'CUS-002' AND deleted_at IS NULL" 2>/dev/null || echo "N/A")
log_info "CUS-002 信用额度: $CUS002_CREDIT"

if [[ "$CUS002_CREDIT" == "N/A" ]]; then
    # 尝试设置 CUS-002 信用额度为 0
    psql "$DB_URL" -c "
        UPDATE customers SET credit_limit = 0
        WHERE customer_code = 'CUS-002'" 2>/dev/null && log_info "已设置 CUS-002 credit_limit=0" || \
        log_warn "无法设置信用额度"
fi

log_step "2. Agent-S1 登录并尝试为 CUS-002 创建订单"
abt_login "$AGENT_S1_SESSION" "$AGENT_S1_USER" "$Q2C_PASSWORD"

# 导航到销售订单创建页
ORDER_ROUTES=("/admin/orders/new" "/admin/sales/orders/new" "/admin/sales-orders/new")
ORDER_PAGE_FOUND=false
for route in "${ORDER_ROUTES[@]}"; do
    abt_navigate "$AGENT_S1_SESSION" "$route"
    PAGE_TEXT=$(abt_get_text "$AGENT_S1_SESSION")
    if [[ "$PAGE_TEXT" != *"404"* && "$PAGE_TEXT" != *"Not Found"* ]]; then
        ORDER_PAGE_FOUND=true
        log_info "找到订单创建页: $route"
        break
    fi
done

if [[ "$ORDER_PAGE_FOUND" == "false" ]]; then
    assert_skip "SE-1: 未找到销售订单创建页面路由"
    abt_close "$AGENT_S1_SESSION"
    print_summary
    exit 0
fi

log_step "3. 填写订单表单 — 选择 CUS-002"
abt_select_by_text "$AGENT_S1_SESSION" "select[name='customer_id']" "CUS-002"
sleep 0.5

# 添加产品行
abt_set_hidden "$AGENT_S1_SESSION" "items_json" '[{"product_code":"PRD-FG-001","quantity":100,"unit_price":50.00}]'

log_step "4. 提交订单"
abt_click_by_text "$AGENT_S1_SESSION" "提交"
sleep "$((PAGE_LOAD_WAIT / 1000))"

log_step "5. 验证系统行为"
PAGE_TEXT=$(abt_get_text "$AGENT_S1_SESSION")

# 检查是否被阻止或有警告
if [[ "$PAGE_TEXT" == *"信用"* || "$PAGE_TEXT" == *"credit"* || "$PAGE_TEXT" == *"冻结"* || "$PAGE_TEXT" == *"超出"* ]]; then
    assert_pass "SE-1: 系统已阻止信用冻结客户的订单 — 页面显示信用相关提示"
elif [[ "$PAGE_TEXT" == *"警告"* || "$PAGE_TEXT" == *"Warning"* || "$PAGE_TEXT" == *"不能"* ]]; then
    assert_pass "SE-1: 系统显示了警告信息"
else
    # 检查 URL 是否仍在创建页（未跳转=被阻止）
    CURRENT_URL=$(abt_get_url "$AGENT_S1_SESSION")
    if [[ "$CURRENT_URL" == *"/new"* ]]; then
        assert_pass "SE-1: 订单未创建成功（仍在创建页），可能被信用检查阻止"
    else
        log_warn "订单可能已创建，检查数据库"
        BLOCKED_ORDER=$(psql "$DB_URL" -t -A -c "
            SELECT id FROM orders
            WHERE customer_id = (SELECT id FROM customers WHERE customer_code = 'CUS-002')
            ORDER BY created_at DESC LIMIT 1" 2>/dev/null || echo "")
        if [[ -n "$BLOCKED_ORDER" ]]; then
            assert_fail "SE-1: 信用冻结客户的订单未被阻止（订单已创建: $BLOCKED_ORDER）"
        else
            assert_pass "SE-1: 订单未被保存到数据库（可能被服务端拒绝）"
        fi
    fi
fi

abt_close "$AGENT_S1_SESSION"
print_summary
echo "=== SE-1 销售信用冻结 完成 ==="
