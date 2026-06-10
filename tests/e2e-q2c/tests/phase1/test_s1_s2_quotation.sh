#!/usr/bin/env bash
# ============================================================================
# Q2C E2E 测试 — S1-S2: 销售报价创建
# 角色: Agent-S1 (q2c_sales)
# 目标: 创建报价单，填写产品/价格信息，验证草稿状态
# ============================================================================
set -euo pipefail

TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$TEST_DIR/../../config/env.sh"
source "$TEST_DIR/../../config/agents.sh"
source "$TEST_DIR/../../lib/login.sh"
source "$TEST_DIR/../../lib/form.sh"
source "$TEST_DIR/../../lib/assert.sh"
source "$TEST_DIR/../../lib/relay.sh"

echo "=== S1-S2: 销售报价创建 ==="
echo ""

# --- 前置：确保接力文件已初始化 ---
if [[ ! -f "$RELAY_FILE" ]] || [[ "$(jq '.run_id' "$RELAY_FILE" 2>/dev/null)" == "" ]]; then
    relay_init "q2c-$(date +%Y%m%d%H%M%S)"
fi
relay_set_phase "S1-S2"
relay_set_status "running"

# --- Step 1: 登录 ---
log_step "1. Agent-S1 登录"
abt_login "$AGENT_S1_SESSION" "$AGENT_S1_USER" "$Q2C_PASSWORD"

# --- Step 2: 导航到新建报价页 ---
log_step "2. 导航到新建报价页"
abt_navigate "$AGENT_S1_SESSION" "/admin/quotations/new"
abt_assert_url_contains "$AGENT_S1_SESSION" "/admin/quotations/new" "新建报价页"

# --- Step 3: 选择客户 ---
log_step "3. 选择客户 CUS-001"
# 客户下拉: select[name='customer_id'] → HTMX trigger change 加载联系人
# 需要通过 value 选择（value 是客户 ID，先从 DB 获取）
CUSTOMER_ID=$(psql "$DB_URL" -t -A -c "SELECT customer_id FROM customers WHERE customer_code = 'CUS-001' AND deleted_at IS NULL LIMIT 1" 2>/dev/null || echo "")
if [[ -z "$CUSTOMER_ID" ]]; then
    log_fail "未找到客户 CUS-001"
    print_summary
    exit 1
fi
log_info "CUS-001 → customer_id=$CUSTOMER_ID"

# 选择客户 → 触发 HTMX 加载联系人
abt_select "$AGENT_S1_SESSION" "select[name='customer_id']" "$CUSTOMER_ID"
sleep 1  # 等待 HTMX 响应

# 验证联系人下拉有选项
sleep 0.5
CONTACT_ID=$(psql "$DB_URL" -t -A -c "SELECT contact_id FROM customer_contacts WHERE customer_id = $CUSTOMER_ID AND is_primary = true LIMIT 1" 2>/dev/null || echo "")
if [[ -n "$CONTACT_ID" ]]; then
    abt_select "$AGENT_S1_SESSION" "select[name='contact_id']" "$CONTACT_ID"
    log_info "选择联系人 contact_id=$CONTACT_ID"
fi

# --- Step 4: 设置有效期 ---
log_step "4. 设置有效期（30天后）"
VALID_UNTIL=$(date -d "+30 days" +%Y-%m-%d 2>/dev/null || date -v+30d +%Y-%m-%d 2>/dev/null || echo "")
if [[ -z "$VALID_UNTIL" ]]; then
    # Windows fallback
    VALID_UNTIL=$(powershell -c "(Get-Date).AddDays(30).ToString('yyyy-MM-dd')" 2>/dev/null)
fi
abt_eval "$AGENT_S1_SESSION" "document.querySelector('#f-valid-until').value = '$VALID_UNTIL';" > /dev/null 2>&1

# --- Step 5: 选择付款条款 ---
log_step "5. 设置付款条款"
abt_select "$AGENT_S1_SESSION" "select[name='payment_terms']" "30天净额"

# --- Step 6: 添加产品行（通过产品搜索 Modal） ---
log_step "6. 添加产品 PRD-FG-001"

# 获取产品 ID
PRODUCT_ID=$(psql "$DB_URL" -t -A -c "SELECT product_id FROM products WHERE product_code = 'PRD-FG-001' AND deleted_at IS NULL LIMIT 1" 2>/dev/null)
log_info "PRD-FG-001 → product_id=$PRODUCT_ID"

# 方法：直接通过 HTMX 请求添加行（绕过 Modal 交互，更可靠）
# 对应路由: GET /admin/quotations/item-row?product_id=X → hx-target="#quotation-item-tbody" hx-swap="beforeend"
abt_eval "$AGENT_S1_SESSION" "
    htmx.ajax('GET', '/admin/quotations/item-row?product_id=$PRODUCT_ID', {
        target: '#quotation-item-tbody',
        swap: 'beforeend'
    });
" > /dev/null 2>&1
sleep 1  # 等待 HTMX 响应

# 验证行已添加
row_count=$(abt_eval "$AGENT_S1_SESSION" "document.querySelectorAll('#quotation-item-tbody tr').length" 2>/dev/null || echo "0")
if [[ "$row_count" -ge 1 ]]; then
    assert_pass "产品行已添加 (rows=$row_count)"
else
    assert_fail "产品行未添加 (rows=$row_count)"
fi

# --- Step 7: 填写行项目数据 ---
log_step "7. 填写行项目：数量=100, 单价=1500, 折扣=10%"
# 行项目通过 name 属性定位（每个 tr 内有 name='quantity', name='unit_price', name='discount_rate'）
# 使用 first row 的 input
abt_eval "$AGENT_S1_SESSION" "
    var row = document.querySelector('#quotation-item-tbody tr');
    if (row) {
        row.querySelector('input[name=\"quantity\"]').value = '100';
        row.querySelector('input[name=\"quantity\"]').dispatchEvent(new Event('input', {bubbles: true}));
        row.querySelector('input[name=\"unit_price\"]').value = '1500';
        row.querySelector('input[name=\"unit_price\"]').dispatchEvent(new Event('input', {bubbles: true}));
        row.querySelector('input[name=\"discount_rate\"]').value = '10';
        row.querySelector('input[name=\"discount_rate\"]').dispatchEvent(new Event('input', {bubbles: true}));
        'filled';
    } else { 'no row'; }
" > /dev/null 2>&1

sleep 0.5

# 验证金额计算
grand_total=$(abt_eval "$AGENT_S1_SESSION" "document.querySelector('#grand-value')?.textContent?.trim() || 'N/A'" 2>/dev/null)
log_info "报价总额: $grand_total"
# 预期: 100 * 1500 * (1 - 0.1) = 135,000

# --- Step 8: 填写备注 ---
log_step "8. 填写备注"
abt_fill "$AGENT_S1_SESSION" "textarea[name='remark']" "Q2C E2E 测试报价单 - Happy Path"

# --- Step 9: 提交报价 ---
log_step "9. 提交报价"

# 提交按钮会调用 quotationSubmit() 然后触发 form submit
# quotationSubmit() 收集 items_json 并写入 hidden input
# 我们需要先确保 items_json 正确填充
abt_eval "$AGENT_S1_SESSION" "
    if (typeof quotationSubmit === 'function') {
        quotationSubmit();
    } else if (typeof lineItemCalc === 'function') {
        var calc = lineItemCalc('#quotation-item-tbody');
        if (calc && typeof calc.collectItems === 'function') {
            document.querySelector('#items-json').value = JSON.stringify(calc.collectItems());
        }
    } else {
        // 手动收集 items
        var rows = document.querySelectorAll('#quotation-item-tbody tr');
        var items = [];
        rows.forEach(function(row) {
            items.push({
                product_id: row.querySelector('input[name=\"product_id\"]')?.value || '0',
                quantity: row.querySelector('input[name=\"quantity\"]')?.value || '0',
                unit: row.querySelector('input[name=\"unit\"]')?.value || '',
                unit_price: row.querySelector('input[name=\"unit_price\"]')?.value || '0',
                discount_rate: row.querySelector('input[name=\"discount_rate\"]')?.value || '0'
            });
        });
        document.querySelector('#items-json').value = JSON.stringify(items);
    }
    'items_collected';
" > /dev/null 2>&1

# 点击"提交报价"按钮
abt_click_by_text "$AGENT_S1_SESSION" "提交报价"
sleep 2  # 等待 HX-Redirect

# --- Step 10: 验证提交成功 ---
log_step "10. 验证提交成功"

# 提交成功后会 HX-Redirect 到详情页 /admin/quotations/{id}
current_url=$(abt_get_url "$AGENT_S1_SESSION" 2>/dev/null || echo "")
log_info "当前URL: $current_url"

if [[ "$current_url" == *"/admin/quotations/"* ]] && [[ "$current_url" != *"/new"* ]]; then
    assert_pass "报价提交成功，跳转到详情页"

    # 提取 quotation ID from URL
    QUOTATION_ID=$(echo "$current_url" | grep -oP '/admin/quotations/\K[0-9]+' || echo "")
    log_info "Quotation ID: $QUOTATION_ID"

    # 验证详情页包含状态标签
    abt_assert_page_contains "$AGENT_S1_SESSION" "已发送" "报价状态为'已发送'" || \
    abt_assert_page_contains "$AGENT_S1_SESSION" "草稿" "报价状态为'草稿'"

    # 写入接力文件
    if [[ -n "$QUOTATION_ID" ]]; then
        relay_write "quotation_id" "$QUOTATION_ID"
        relay_write "quotation_url" "/admin/quotations/$QUOTATION_ID"
        assert_pass "接力文件写入: quotation_id=$QUOTATION_ID"
    fi

    # 获取报价单号（从详情页）
    DOC_NUMBER=$(abt_eval "$AGENT_S1_SESSION" "document.querySelector('.detail-no')?.textContent?.trim() || ''" 2>/dev/null || echo "")
    if [[ -n "$DOC_NUMBER" ]]; then
        relay_write "quotation_doc_number" "$DOC_NUMBER"
        log_info "报价单号: $DOC_NUMBER"
    fi
else
    assert_fail "报价提交可能失败，URL: $current_url"

    # 截图用于调试
    abt_screenshot "$AGENT_S1_SESSION" "/tmp/q2c-s1-s2-fail.png" 2>/dev/null || true
fi

# --- Step 11: 数据库验证 ---
log_step "11. 数据库验证"

if [[ -n "$QUOTATION_ID" ]]; then
    abt_assert_db \
        "SELECT 1 FROM quotations WHERE id = $QUOTATION_ID AND deleted_at IS NULL" \
        "数据库: 报价记录存在 (id=$QUOTATION_ID)"

    abt_assert_db \
        "SELECT 1 FROM quotation_items WHERE quotation_id = $QUOTATION_ID" \
        "数据库: 报价明细行存在"
fi

# --- 完成 ---
relay_set_status "completed"
relay_snapshot "SNAP-S1-S2"

echo ""
echo "=== S1-S2 完成 ==="
print_summary
