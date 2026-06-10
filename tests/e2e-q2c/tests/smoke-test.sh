#!/usr/bin/env bash
# ============================================================================
# Q2C E2E 测试 — 集成冒烟测试
# 验证整个基础设施层协同工作：
#   1. 环境初始化 (SQL fixtures)
#   2. Agent 登录 (agent-browser)
#   3. 页面导航 + 断言
#   4. 接力数据传递
#   5. 环境清理
# ============================================================================

set -euo pipefail

TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$TEST_DIR/../.." && pwd)"

# 加载所有配置和工具库
source "$TEST_DIR/../config/env.sh"
source "$TEST_DIR/../config/agents.sh"
source "$TEST_DIR/../lib/login.sh"
source "$TEST_DIR/../lib/form.sh"
source "$TEST_DIR/../lib/assert.sh"
source "$TEST_DIR/../lib/relay.sh"

echo "============================================"
echo "  Q2C E2E Smoke Test"
echo "============================================"
echo ""

# ============================================================
# 1. 环境初始化
# ============================================================
log_step "1. Environment Setup"
bash "$TEST_DIR/../scripts/setup.sh"

if [[ $? -ne 0 ]]; then
    log_fail "Environment setup failed"
    print_summary
    exit 1
fi
assert_pass "Environment setup completed"

# ============================================================
# 2. Agent 登录测试
# ============================================================
log_step "2. Agent Login Tests"

# 2.1 测试 q2c_sales 登录
log_info "Testing q2c_sales login..."
if abt_login "$AGENT_S1_SESSION" "$AGENT_S1_USER" "$Q2C_PASSWORD"; then
    assert_pass "q2c_sales login OK"
else
    assert_fail "q2c_sales login FAILED"
fi

# 2.2 测试 q2c_warehouse 登录
log_info "Testing q2c_warehouse login..."
if abt_login "$AGENT_W1_SESSION" "$AGENT_W1_USER" "$Q2C_PASSWORD"; then
    assert_pass "q2c_warehouse login OK"
else
    assert_fail "q2c_warehouse login FAILED"
fi

# ============================================================
# 3. 页面导航 + 断言测试
# ============================================================
log_step "3. Navigation & Assertion Tests"

# 3.1 q2c_sales → 报价列表
abt_navigate "$AGENT_S1_SESSION" "/admin/quotations"
abt_assert_url_contains "$AGENT_S1_SESSION" "/admin/quotations" "Sales quotation page"

# 3.2 q2c_sales → 订单列表
abt_navigate "$AGENT_S1_SESSION" "/admin/orders"
abt_assert_url_contains "$AGENT_S1_SESSION" "/admin/orders" "Sales order page"

# 3.3 q2c_warehouse → 库存页面
abt_navigate "$AGENT_W1_SESSION" "/admin/wms/stock"
abt_assert_url_contains "$AGENT_W1_SESSION" "/admin/wms/stock" "Inventory page"

# ============================================================
# 4. 接力数据传递测试
# ============================================================
log_step "4. Relay Data Transfer Tests"

# 4.1 初始化接力文件
relay_init "smoke-$(date +%Y%m%d%H%M%S)"
assert_pass "Relay initialized"

# 4.2 写入 → 读取
relay_write "test_quotation_id" "QT-2026-001"
relay_write "test_sales_order_id" "SO-2026-001"
relay_write "test_total_amount" "169500.00"

q_val=$(relay_read "test_quotation_id")
if [[ "$q_val" == "QT-2026-001" ]]; then
    assert_pass "Relay write/read OK: quotation_id=$q_val"
else
    assert_fail "Relay read mismatch: expected QT-2026-001, got $q_val"
fi

so_val=$(relay_read "test_sales_order_id")
if [[ "$so_val" == "SO-2026-001" ]]; then
    assert_pass "Relay write/read OK: sales_order_id=$so_val"
else
    assert_fail "Relay read mismatch: expected SO-2026-001, got $so_val"
fi

# 4.3 读取不存在的 key
missing=$(relay_read "nonexistent_key")
if [[ -z "$missing" ]]; then
    assert_pass "Relay returns empty for missing key"
else
    assert_fail "Relay should return empty for missing key, got: $missing"
fi

# 4.4 阶段更新
relay_set_phase "smoke-test"
phase=$(jq -r '.phase' "$RELAY_FILE" 2>/dev/null)
if [[ "$phase" == "smoke-test" ]]; then
    assert_pass "Relay phase update OK"
else
    assert_fail "Relay phase update failed: got $phase"
fi

# ============================================================
# 5. 数据库验证测试
# ============================================================
log_step "5. Database Verification Tests"

# 5.1 验证测试用户
abt_assert_db \
    "SELECT 1 FROM users WHERE username = 'q2c_sales' AND is_active = true" \
    "q2c_sales user exists and active"

# 5.2 验证物料
abt_assert_db \
    "SELECT 1 FROM products WHERE product_code = 'PRD-FG-001' AND status = 1 AND deleted_at IS NULL" \
    "Product PRD-FG-001 exists and active"

# 5.3 验证客户
abt_assert_db \
    "SELECT 1 FROM customers WHERE customer_code = 'CUS-001' AND credit_limit = 500000 AND deleted_at IS NULL" \
    "Customer CUS-001 exists with credit limit 500000"

# 5.4 验证库存
abt_assert_db \
    "SELECT 1 FROM stock_ledger sl
     JOIN products p ON sl.product_id = p.product_id
     JOIN warehouses w ON sl.warehouse_id = w.id
     WHERE p.product_code = 'PRD-RM-001' AND w.code = 'WH-RAW' AND sl.quantity = 500" \
    "Initial inventory: PRD-RM-001 = 500 KG in WH-RAW"

# 5.5 验证成品仓为空
abt_assert_db_empty \
    "SELECT 1 FROM stock_ledger sl
     JOIN warehouses w ON sl.warehouse_id = w.id
     WHERE w.code = 'WH-FG'" \
    "WH-FG inventory is empty (as expected)"

# ============================================================
# 6. 清理
# ============================================================
log_step "6. Cleanup"

# 清理浏览器会话
abt_close "$AGENT_S1_SESSION" 2>/dev/null || true
abt_close "$AGENT_W1_SESSION" 2>/dev/null || true

# 清理接力文件
relay_clean
assert_pass "Relay cleaned"

# 清理测试数据
bash "$TEST_DIR/../scripts/teardown.sh"
assert_pass "Environment teardown completed"

# ============================================================
# 7. 汇总
# ============================================================
echo ""
echo "============================================"
echo "  Q2C E2E Smoke Test Complete"
echo "============================================"
print_summary
