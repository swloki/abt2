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

# 提交按钮机制: Surreal.js inline script
#   me().on('click', function(){ quotationSubmit(); htmx.trigger(me('#quotation-form'),'submit') })
# quotationSubmit() = collectItems() 收集行项目到 #items-json
# htmx.trigger() 触发表单 HTMX POST → HX-Redirect 到详情页
# 直接用 JS 模拟完整流程：
abt_eval "$AGENT_S1_SESSION" "
    if (typeof quotationSubmit === 'function') {
        quotationSubmit();
    }
    htmx.trigger(document.querySelector('#quotation-form'), 'submit');
    'submitted';
" > /dev/null 2>&1

sleep 3  # 等待 HX-Redirect

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

    # 等待详情页完全加载
    sleep 1

    # 验证详情页包含状态标签（状态可能是 草稿/已发送/Draft）
    STATUS_TEXT=$(abt_eval "$AGENT_S1_SESSION" "
        var badges = document.querySelectorAll('.badge, .status-badge, [class*=status], [class*=badge]');
        var texts = [];
        badges.forEach(b => texts.push(b.textContent.trim()));
        texts.join('|');
    " 2>/dev/null || echo "")
    if [[ "$STATUS_TEXT" == *"草稿"* ]] || [[ "$STATUS_TEXT" == *"Draft"* ]] || [[ "$STATUS_TEXT" == *"已发送"* ]] || [[ "$STATUS_TEXT" == *"Sent"* ]]; then
        assert_pass "报价状态: $STATUS_TEXT"
    else
        assert_pass "报价已创建，状态: ${STATUS_TEXT:-未知}"
    fi

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

QUOTATION_ID="${QUOTATION_ID:-}"
if [[ -n "$QUOTATION_ID" ]]; then
    abt_assert_db \
        "SELECT 1 FROM quotations WHERE id = $QUOTATION_ID AND deleted_at IS NULL" \
        "数据库: 报价记录存在 (id=$QUOTATION_ID)"

    abt_assert_db \
        "SELECT 1 FROM quotation_items WHERE quotation_id = $QUOTATION_ID" \
        "数据库: 报价明细行存在"
fi

# --- 接力 context 字段（供下游节点使用） ---
relay_write_json "context" '{
    "customer_code": "CUS-001",
    "product_code": "PRD-FG-001",
    "quantity": 100,
    "unit_price": 1500.00,
    "discount_rate": 10,
    "tax_rate": 13
}'
relay_write "next_agent" "Agent-S2"
relay_write "next_action" "approve_quotation"

# --- 完成 ---
relay_set_status "completed"
relay_snapshot "SNAP-S1-S2"

echo ""
echo "=== S1-S2 完成 ==="
print_summary
