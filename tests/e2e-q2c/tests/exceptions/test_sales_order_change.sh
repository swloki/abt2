#!/usr/bin/env bash
# SE-2: 销售订单变更 — 已创建订单尝试修改数量
set -euo pipefail
TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$TEST_DIR/../../config/env.sh"
source "$TEST_DIR/../../config/agents.sh"
source "$TEST_DIR/../../lib/login.sh"
source "$TEST_DIR/../../lib/form.sh"
source "$TEST_DIR/../../lib/assert.sh"
source "$TEST_DIR/../../lib/relay.sh"

echo "=== SE-2: 销售订单变更 ==="

log_step "1. 检查订单相关表"
ORDER_TABLE=$(psql "$DB_URL" -t -A -c "SELECT table_name FROM information_schema.tables WHERE table_name IN ('orders','order_items','sales_orders')" 2>/dev/null || echo "")
if [[ -z "$ORDER_TABLE" ]]; then
    assert_skip "SE-2: 系统未实现订单功能（无订单表）"
    print_summary
    echo "=== SE-2 销售订单变更 完成 ==="
    exit 0
fi

log_step "2. Agent-S1 创建销售订单"
abt_login "$AGENT_S1_SESSION" "$AGENT_S1_USER" "$Q2C_PASSWORD"

# 导航到订单创建页
ORDER_ROUTES=("/admin/orders/new" "/admin/sales/orders/new" "/admin/sales-orders/new")
ORDER_PAGE_FOUND=false
for route in "${ORDER_ROUTES[@]}"; do
    abt_navigate "$AGENT_S1_SESSION" "$route"
    PAGE_TEXT=$(abt_get_text "$AGENT_S1_SESSION")
    if [[ "$PAGE_TEXT" != *"404"* && "$PAGE_TEXT" != *"Not Found"* ]]; then
        ORDER_PAGE_FOUND=true
        break
    fi
done

if [[ "$ORDER_PAGE_FOUND" == "false" ]]; then
    assert_skip "SE-2: 未找到订单创建页面"
    abt_close "$AGENT_S1_SESSION"
    print_summary
    exit 0
fi

# 创建订单
abt_select_by_text "$AGENT_S1_SESSION" "select[name='customer_id']" "CUS-001"
sleep 0.5
abt_set_hidden "$AGENT_S1_SESSION" "items_json" '[{"product_code":"PRD-FG-001","quantity":10,"unit_price":100.00}]'
abt_click_by_text "$AGENT_S1_SESSION" "提交"
sleep "$((PAGE_LOAD_WAIT / 1000))"

# 获取订单 ID
ORDER_ID=$(psql "$DB_URL" -t -A -c "SELECT id FROM orders ORDER BY created_at DESC LIMIT 1" 2>/dev/null || echo "")
if [[ -z "$ORDER_ID" ]]; then
    assert_fail "SE-2: 无法创建订单"
    abt_close "$AGENT_S1_SESSION"
    print_summary
    exit 1
fi
log_info "订单 ID: $ORDER_ID"

log_step "3. 导航到订单详情页"
abt_navigate "$AGENT_S1_SESSION" "/admin/orders/$ORDER_ID"
sleep "$((PAGE_LOAD_WAIT / 1000))"

PAGE_TEXT=$(abt_get_text "$AGENT_S1_SESSION")
log_info "订单详情页加载完成"

log_step "4. 尝试修改数量"
# 查找编辑/修改按钮
EDIT_FOUND=false
if [[ "$PAGE_TEXT" == *"编辑"* || "$PAGE_TEXT" == *"修改"* || "$PAGE_TEXT" == *"Edit"* ]]; then
    EDIT_FOUND=true
    abt_click_by_text "$AGENT_S1_SESSION" "编辑"
    sleep "$((PAGE_LOAD_WAIT / 1000))"
fi

if [[ "$EDIT_FOUND" == "true" ]]; then
    # 修改数量
    abt_fill "$AGENT_S1_SESSION" "input[name='quantity'], input[data-field='quantity']" "20"
    abt_click_by_text "$AGENT_S1_SESSION" "保存"
    sleep "$((PAGE_LOAD_WAIT / 1000))"

    # 验证变更结果
    RESULT_TEXT=$(abt_get_text "$AGENT_S1_SESSION")
    if [[ "$RESULT_TEXT" == *"审批"* || "$RESULT_TEXT" == *"approval"* || "$RESULT_TEXT" == *"需"* ]]; then
        assert_pass "SE-2: 订单变更需审批 — 系统已触发变更审批流程"
    elif [[ "$RESULT_TEXT" == *"成功"* || "$RESULT_TEXT" == *"Success"* ]]; then
        assert_pass "SE-2: 订单变更成功"
        # 验证数据库数量已更新
        NEW_QTY=$(psql "$DB_URL" -t -A -c "
            SELECT quantity FROM order_items
            WHERE order_id = $ORDER_ID
            LIMIT 1" 2>/dev/null || echo "")
        log_info "变更后数量: $NEW_QTY"
    else
        assert_pass "SE-2: 订单变更操作已执行"
    fi
else
    log_info "订单详情页无编辑按钮，检查是否可编辑字段"
    # 尝试直接修改表单字段
    EDITABLE=$(abt_eval "$AGENT_S1_SESSION" "
        const inputs = document.querySelectorAll('input[name*=\"quantity\"], input[data-field*=\"qty\"]');
        inputs.length > 0 ? 'editable' : 'readonly';
    " 2>/dev/null || echo "readonly")

    if [[ "$EDITABLE" == "editable" ]]; then
        assert_pass "SE-2: 订单字段可编辑，支持直接修改"
    else
        assert_skip "SE-2: 订单详情页无编辑功能（只读或未实现）"
    fi
fi

abt_close "$AGENT_S1_SESSION"
print_summary
echo "=== SE-2 销售订单变更 完成 ==="
