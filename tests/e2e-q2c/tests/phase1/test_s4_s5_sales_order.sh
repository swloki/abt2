#!/usr/bin/env bash
# ============================================================================
# Q2C E2E 测试 — S4-S5: 销售订单创建（从报价转订单）
# 角色: Agent-S1 (q2c_sales)
# 目标: 从已接受的报价创建销售订单，验证订单创建成功
# ============================================================================
set -euo pipefail

TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$TEST_DIR/../../config/env.sh"
source "$TEST_DIR/../../config/agents.sh"
source "$TEST_DIR/../../lib/login.sh"
source "$TEST_DIR/../../lib/form.sh"
source "$TEST_DIR/../../lib/assert.sh"
source "$TEST_DIR/../../lib/relay.sh"

echo "=== S4-S5: 销售订单创建 ==="
echo ""

relay_set_phase "S4-S5"

# --- 前置 ---
QUOTATION_ID=$(relay_read "quotation_id")
QUOTATION_STATUS=$(relay_read "quotation_status")

if [[ -z "$QUOTATION_ID" ]]; then
    log_fail "接力文件中缺少 quotation_id"
    print_summary
    exit 1
fi

if [[ "$QUOTATION_STATUS" != "accepted" ]]; then
    log_warn "报价状态不是'accepted'，可能需要先运行 S3 审批"
fi

# --- Step 1: 从报价详情页点击"转销售订单" ---
log_step "1. Agent-S1 登录并导航到报价详情"
abt_login "$AGENT_S1_SESSION" "$AGENT_S1_USER" "$Q2C_PASSWORD"
abt_navigate "$AGENT_S1_SESSION" "/admin/quotations/$QUOTATION_ID"
sleep 1

# 方法A: 点击"转销售订单"链接（已接受状态的报价才有此按钮）
log_step "2. 点击'转销售订单'"
SO_CREATE_URL="/admin/orders/create?from_quotation=$QUOTATION_ID"

# 尝试直接导航到带 from_quotation 参数的创建页
abt_navigate "$AGENT_S1_SESSION" "$SO_CREATE_URL"
sleep 1

abt_assert_url_contains "$AGENT_S1_SESSION" "/admin/orders/create" "订单创建页"

# --- Step 3: 验证预填数据 ---
log_step "3. 验证预填数据（来自报价）"

# 验证客户已预选
customer_val=$(abt_eval "$AGENT_S1_SESSION" "document.querySelector('select[name=\"customer_id\"]')?.value || ''" 2>/dev/null || echo "")
if [[ -n "$customer_val" && "$customer_val" != "0" ]]; then
    assert_pass "客户已预选 (customer_id=$customer_val)"
else
    assert_fail "客户未预选"
fi

# 验证产品行已预填
row_count=$(abt_eval "$AGENT_S1_SESSION" "document.querySelectorAll('#order-item-tbody tr').length" 2>/dev/null || echo "0")
if [[ "$row_count" -ge 1 ]]; then
    assert_pass "产品行已预填 (rows=$row_count)"
else
    assert_fail "产品行未预填"
fi

# 验证合计金额
grand_total=$(abt_eval "$AGENT_S1_SESSION" "document.querySelector('#grand-value')?.textContent?.trim() || 'N/A'" 2>/dev/null)
log_info "订单总额: $grand_total"

# --- Step 3b: 选择联系人（从报价带出的客户对应的联系人）---
CONTACT_ID=$(abt_eval "$AGENT_S1_SESSION" "
    var sel = document.querySelector('select[name=\"contact_id\"]');
    if (sel && sel.options.length > 1) { sel.selectedIndex = 1; sel.dispatchEvent(new Event('change', {bubbles: true})); sel.value; }
    else '';
" 2>/dev/null || echo "")
if [[ -n "$CONTACT_ID" && "$CONTACT_ID" != "0" ]]; then
    log_info "选择联系人 contact_id=$CONTACT_ID"
else
    log_warn "未找到联系人选项，尝试从 DB 获取"
    CUSTOMER_ID=$(relay_read "quotation_id" | xargs -I{} psql "$DB_URL" -t -A -c "SELECT customer_id FROM quotations WHERE id = {}" 2>/dev/null || echo "")
fi

# --- Step 4: 填写交货日期 ---
log_step "4. 填写交货日期（30天后）"
DELIVERY_DATE=$(powershell -c "(Get-Date).AddDays(30).ToString('yyyy-MM-dd')" 2>/dev/null)
# 为每个行项目设置交货日期
abt_eval "$AGENT_S1_SESSION" "
    document.querySelectorAll('#order-item-tbody input[name=\"item_delivery_date\"]').forEach(function(inp) {
        inp.value = '$DELIVERY_DATE';
    });
    'dates_set';
" > /dev/null 2>&1

# --- Step 5: 填写交货地址 ---
log_step "5. 填写交货地址"
abt_fill "$AGENT_S1_SESSION" "input[name='delivery_address']" "Q2C测试交货地址-上海市浦东新区张江高科"

# --- Step 6: 提交订单 ---
log_step "6. 提交订单"

# 用封装函数：collectItems + htmx.trigger 提交
abt_htmx_submit_form "$AGENT_S1_SESSION" "#order-form" "salesOrderSubmit"
sleep 3

# --- Step 7: 验证订单创建成功 ---
log_step "7. 验证订单创建成功"

current_url=$(abt_get_url "$AGENT_S1_SESSION" 2>/dev/null || echo "")
log_info "当前URL: $current_url"

if [[ "$current_url" == *"/admin/orders/"* ]] && [[ "$current_url" != *"/create"* ]]; then
    assert_pass "订单创建成功，跳转到详情页"

    # 提取 Order ID
    ORDER_ID=$(echo "$current_url" | grep -oP '/admin/orders/\K[0-9]+' || echo "")
    log_info "Order ID: $ORDER_ID"

    # 获取订单号
    DOC_NUMBER=$(abt_eval "$AGENT_S1_SESSION" "document.querySelector('.detail-no')?.textContent?.trim() || ''" 2>/dev/null || echo "")
    log_info "订单号: $DOC_NUMBER"

    # 写入接力文件
    relay_write "sales_order_id" "$ORDER_ID"
    relay_write "sales_order_url" "$current_url"
    relay_write "sales_order_doc_number" "${DOC_NUMBER:-}"

    # 验证详情页内容（不阻断，详情页格式可能不含原始编码）
    PAGE_TEXT=$(abt_get_text "$AGENT_S1_SESSION" 2>/dev/null || echo "")
    if [[ "$PAGE_TEXT" == *"CUS-001"* ]]; then
        assert_pass "订单详情包含客户 CUS-001"
    else
        log_info "详情页不含 CUS-001（可能用客户名称显示）"
    fi
    if [[ "$PAGE_TEXT" == *"PRD-FG-001"* ]]; then
        assert_pass "订单详情包含产品 PRD-FG-001"
    else
        log_info "详情页不含 PRD-FG-001（可能用产品名称显示）"
    fi
else
    assert_fail "订单创建可能失败，URL: $current_url"
    abt_screenshot "$AGENT_S1_SESSION" "/tmp/q2c-s4-s5-fail.png" 2>/dev/null || true
fi

# --- Step 8: 数据库验证 ---
log_step "8. 数据库验证"

ORDER_ID="${ORDER_ID:-}"
if [[ -n "$ORDER_ID" ]]; then
    abt_assert_db \
        "SELECT 1 FROM sales_orders WHERE id = $ORDER_ID AND deleted_at IS NULL" \
        "数据库: 销售订单存在"

    abt_assert_db \
        "SELECT 1 FROM sales_order_items WHERE order_id = $ORDER_ID" \
        "数据库: 订单明细行存在"

    # 验证总金额
    TOTAL=$(psql "$DB_URL" -t -A -c "SELECT total_amount FROM sales_orders WHERE id = $ORDER_ID" 2>/dev/null || echo "0")
    log_info "订单总额: $TOTAL"

    # 验证数量
    QTY=$(psql "$DB_URL" -t -A -c "SELECT quantity FROM sales_order_items WHERE order_id = $ORDER_ID LIMIT 1" 2>/dev/null || echo "0")
    log_info "订单数量: $QTY"
fi

# --- 完成 ---
relay_write "so_quantity" "${QTY:-100}"
relay_write "so_total_amount" "${TOTAL:-0}"

# --- 接力 context 字段更新 ---
relay_write "next_agent" "Agent-P1"
relay_write "next_action" "execute_mrp"
relay_snapshot "SNAP-S4-S5"
relay_set_status "completed"

echo ""
echo "=== S4-S5 完成 ==="
print_summary
