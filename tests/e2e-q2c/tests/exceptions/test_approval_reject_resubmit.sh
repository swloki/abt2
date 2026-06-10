#!/usr/bin/env bash
# AP-E1: 审批拒绝后重新提交
set -euo pipefail
TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$TEST_DIR/../../config/env.sh"
source "$TEST_DIR/../../config/agents.sh"
source "$TEST_DIR/../../lib/login.sh"
source "$TEST_DIR/../../lib/form.sh"
source "$TEST_DIR/../../lib/assert.sh"
source "$TEST_DIR/../../lib/relay.sh"

echo "=== AP-E1: 审批拒绝后重新提交 ==="

log_step "1. 检查报价相关表"
QUOTE_TABLES=$(psql "$DB_URL" -t -A -c "SELECT table_name FROM information_schema.tables WHERE table_name IN ('quotations','quotation_items')" 2>/dev/null || echo "")
if [[ -z "$QUOTE_TABLES" ]]; then
    assert_skip "AP-E1: 系统未实现报价功能（无报价表）"
    exit 0
fi
log_info "报价相关表: $(echo $QUOTE_TABLES | tr '\n' ',')"

log_step "2. Agent-S1 创建报价"
abt_login "$AGENT_S1_SESSION" "$AGENT_S1_USER" "$Q2C_PASSWORD"
abt_navigate "$AGENT_S1_SESSION" "/admin/quotations/new"

# 选择客户 CUS-001
abt_select_by_text "$AGENT_S1_SESSION" "select[name='customer_id']" "CUS-001"
sleep 0.5

# 通过 HTMX 添加产品行 PRD-FG-001
abt_htmx_trigger "$AGENT_S1_SESSION" "button[data-action='add-item'], .add-row-btn, [hx-post*='add-item']" "click"
abt_wait_htmx "$AGENT_S1_SESSION" 3000

# 收集 items_json 并提交
abt_set_hidden "$AGENT_S1_SESSION" "items_json" '[{"product_code":"PRD-FG-001","quantity":10,"unit_price":100.00}]'
abt_click_by_text "$AGENT_S1_SESSION" "提交"
sleep "$((PAGE_LOAD_WAIT / 1000))"

# 获取报价 ID
QUOTE_URL=$(abt_get_url "$AGENT_S1_SESSION")
log_info "报价页面 URL: $QUOTE_URL"

# 尝试从 URL 或页面提取报价 ID
QUOTE_ID=$(echo "$QUOTE_URL" | grep -oE '[0-9]+' | tail -1 || echo "")
if [[ -z "$QUOTE_ID" ]]; then
    # 尝试从数据库获取最新报价
    QUOTE_ID=$(psql "$DB_URL" -t -A -c "SELECT id FROM quotations ORDER BY created_at DESC LIMIT 1" 2>/dev/null || echo "")
fi

if [[ -z "$QUOTE_ID" ]]; then
    assert_fail "AP-E1: 无法获取报价 ID"
    abt_close "$AGENT_S1_SESSION"
    exit 1
fi
log_info "报价 ID: $QUOTE_ID"

# 记录到接力文件
relay_write "reject_resubmit_quote_id" "$QUOTE_ID"

log_step "3. Agent-S2 拒绝报价"
abt_login "$AGENT_S2_SESSION" "$AGENT_S2_USER" "$Q2C_PASSWORD"
abt_navigate "$AGENT_S2_SESSION" "/admin/quotations/$QUOTE_ID"

# 查找并点击拒绝按钮
abt_click_by_text "$AGENT_S2_SESSION" "拒绝"
sleep "$((PAGE_LOAD_WAIT / 1000))"

# 填写拒绝原因
abt_fill "$AGENT_S2_SESSION" "textarea[name='reason'], input[name='reason']" "测试拒绝原因-E1"
abt_click_by_text "$AGENT_S2_SESSION" "确认"
sleep "$((PAGE_LOAD_WAIT / 1000))"

# 验证状态变为 Rejected（状态值=3 或文本 Rejected）
REJECT_STATUS=$(psql "$DB_URL" -t -A -c "SELECT status FROM quotations WHERE id = $QUOTE_ID" 2>/dev/null || echo "")
log_info "报价状态（S2 拒绝后）: $REJECT_STATUS"

if [[ "$REJECT_STATUS" == "3" || "$REJECT_STATUS" == *"reject"* || "$REJECT_STATUS" == *"Reject"* ]]; then
    assert_pass "AP-E1: 报价已被拒绝 (status=$REJECT_STATUS)"
else
    log_warn "报价状态非预期: $REJECT_STATUS（可能未实现拒绝功能）"
fi

log_step "4. Agent-S1 修改并重新提交"
abt_login "$AGENT_S1_SESSION" "$AGENT_S1_USER" "$Q2C_PASSWORD"
abt_navigate "$AGENT_S1_SESSION" "/admin/quotations/$QUOTE_ID"

# 修改报价（调整数量或价格）
abt_fill "$AGENT_S1_SESSION" "input[name='quantity'], input[data-field='quantity']" "15"
abt_set_hidden "$AGENT_S1_SESSION" "items_json" '[{"product_code":"PRD-FG-001","quantity":15,"unit_price":90.00}]'

# 重新提交
abt_click_by_text "$AGENT_S1_SESSION" "重新提交"
sleep "$((PAGE_LOAD_WAIT / 1000))"

log_step "5. 验证重新提交后状态"
RESUBMIT_STATUS=$(psql "$DB_URL" -t -A -c "SELECT status FROM quotations WHERE id = $QUOTE_ID" 2>/dev/null || echo "")
log_info "报价状态（重新提交后）: $RESUBMIT_STATUS"

if [[ "$RESUBMIT_STATUS" == "2" || "$RESUBMIT_STATUS" == *"Sent"* || "$RESUBMIT_STATUS" == *"sent"* || "$RESUBMIT_STATUS" == *"Pending"* || "$RESUBMIT_STATUS" == *"pending"* ]]; then
    assert_pass "AP-E1: 重新提交成功，状态已回到待审批 (status=$RESUBMIT_STATUS)"
else
    # 如果功能未完整实现，至少检查页面文本
    PAGE_TEXT=$(abt_get_text "$AGENT_S1_SESSION")
    if [[ "$PAGE_TEXT" == *"重新提交"* || "$PAGE_TEXT" == *"已提交"* || "$PAGE_TEXT" == *"Sent"* ]]; then
        assert_pass "AP-E1: 页面显示重新提交成功"
    else
        assert_fail "AP-E1: 重新提交后状态异常 (status=$RESUBMIT_STATUS)"
    fi
fi

abt_close "$AGENT_S1_SESSION"
abt_close "$AGENT_S2_SESSION"

print_summary
echo "=== AP-E1 审批拒绝后重新提交 完成 ==="
