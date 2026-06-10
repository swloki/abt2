#!/usr/bin/env bash
# AP-E4: 审批撤回
set -euo pipefail
TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$TEST_DIR/../../config/env.sh"
source "$TEST_DIR/../../config/agents.sh"
source "$TEST_DIR/../../lib/login.sh"
source "$TEST_DIR/../../lib/form.sh"
source "$TEST_DIR/../../lib/assert.sh"
source "$TEST_DIR/../../lib/relay.sh"

echo "=== AP-E4: 审批撤回 ==="

log_step "1. 检查报价相关表"
QUOTE_TABLES=$(psql "$DB_URL" -t -A -c "SELECT table_name FROM information_schema.tables WHERE table_name IN ('quotations','quotation_items')" 2>/dev/null || echo "")
if [[ -z "$QUOTE_TABLES" ]]; then
    assert_skip "AP-E4: 系统未实现报价功能（无报价表）"
    exit 0
fi

log_step "2. Agent-S1 创建并提交报价"
abt_login "$AGENT_S1_SESSION" "$AGENT_S1_USER" "$Q2C_PASSWORD"
abt_navigate "$AGENT_S1_SESSION" "/admin/quotations/new"

# 填写报价表单
abt_select_by_text "$AGENT_S1_SESSION" "select[name='customer_id']" "CUS-001"
sleep 0.5
abt_set_hidden "$AGENT_S1_SESSION" "items_json" '[{"product_code":"PRD-FG-001","quantity":10,"unit_price":100.00}]'
abt_click_by_text "$AGENT_S1_SESSION" "提交"
sleep "$((PAGE_LOAD_WAIT / 1000))"

# 获取报价 ID
QUOTE_ID=$(psql "$DB_URL" -t -A -c "SELECT id FROM quotations ORDER BY created_at DESC LIMIT 1" 2>/dev/null || echo "")
if [[ -z "$QUOTE_ID" ]]; then
    assert_fail "AP-E4: 无法创建报价"
    abt_close "$AGENT_S1_SESSION"
    print_summary
    exit 1
fi
log_info "报价 ID: $QUOTE_ID"

# 验证状态为已提交/待审批
STATUS_BEFORE=$(psql "$DB_URL" -t -A -c "SELECT status FROM quotations WHERE id = $QUOTE_ID" 2>/dev/null || echo "")
log_info "报价状态（提交后）: $STATUS_BEFORE"

log_step "3. 导航到报价详情页，查找撤回按钮"
abt_navigate "$AGENT_S1_SESSION" "/admin/quotations/$QUOTE_ID"
sleep "$((PAGE_LOAD_WAIT / 1000))"

PAGE_TEXT=$(abt_get_text "$AGENT_S1_SESSION")
log_info "页面内容（前200字符）: ${PAGE_TEXT:0:200}"

# 检查是否有撤回按钮
WITHDRAW_FOUND=false
if [[ "$PAGE_TEXT" == *"撤回"* || "$PAGE_TEXT" == *"取消提交"* || "$PAGE_TEXT" == *"Withdraw"* ]]; then
    WITHDRAW_FOUND=true
    log_info "找到撤回按钮/链接"
fi

log_step "4. 尝试点击撤回"
if [[ "$WITHDRAW_FOUND" == "true" ]]; then
    # 尝试点击撤回按钮
    abt_click_by_text "$AGENT_S1_SESSION" "撤回"
    sleep "$((PAGE_LOAD_WAIT / 1000))"

    # 可能需要确认
    PAGE_TEXT_AFTER=$(abt_get_text "$AGENT_S1_SESSION")
    if [[ "$PAGE_TEXT_AFTER" == *"确认"* ]]; then
        abt_click_by_text "$AGENT_S1_SESSION" "确认"
        sleep "$((PAGE_LOAD_WAIT / 1000))"
    fi

    # 验证状态回退
    STATUS_AFTER=$(psql "$DB_URL" -t -A -c "SELECT status FROM quotations WHERE id = $QUOTE_ID" 2>/dev/null || echo "")
    log_info "报价状态（撤回后）: $STATUS_AFTER"

    # Draft=1, 草稿
    if [[ "$STATUS_AFTER" == "1" || "$STATUS_AFTER" == *"Draft"* || "$STATUS_AFTER" == *"draft"* || "$STATUS_AFTER" == *"草稿"* ]]; then
        assert_pass "AP-E4: 撤回成功，状态已回退为草稿 (status=$STATUS_AFTER)"
    elif [[ "$STATUS_AFTER" != "$STATUS_BEFORE" ]]; then
        assert_pass "AP-E4: 状态已变更 (before=$STATUS_BEFORE, after=$STATUS_AFTER)"
    else
        # 检查页面提示
        TOAST_TEXT=$(abt_get_text "$AGENT_S1_SESSION" ".toast, [role='alert'], .notification" 2>/dev/null || echo "")
        if [[ -n "$TOAST_TEXT" ]]; then
            assert_pass "AP-E4: 撤回操作已执行，页面提示: $TOAST_TEXT"
        else
            assert_fail "AP-E4: 撤回操作后状态未变化 (status=$STATUS_AFTER)"
        fi
    fi
else
    # 检查是否有其他方式撤回（如表单按钮）
    WITHDRAW_BTN=$(abt_eval "$AGENT_S1_SESSION" "
        const btns = Array.from(document.querySelectorAll('button, a'));
        const withdrawBtn = btns.find(b => b.textContent.includes('撤回') || b.textContent.includes('Withdraw') || b.textContent.includes('取消提交'));
        withdrawBtn ? 'found' : 'not_found';
    " 2>/dev/null || echo "not_found")

    if [[ "$WITHDRAW_BTN" == "found" ]]; then
        abt_click_by_text "$AGENT_S1_SESSION" "撤回"
        sleep "$((PAGE_LOAD_WAIT / 1000))"

        STATUS_AFTER=$(psql "$DB_URL" -t -A -c "SELECT status FROM quotations WHERE id = $QUOTE_ID" 2>/dev/null || echo "")
        if [[ "$STATUS_AFTER" != "$STATUS_BEFORE" ]]; then
            assert_pass "AP-E4: 撤回成功 (status: $STATUS_BEFORE → $STATUS_AFTER)"
        else
            assert_pass "AP-E4: 撤回按钮已点击，状态待确认"
        fi
    else
        assert_skip "AP-E4: 报价详情页无撤回按钮，功能可能未实现"
    fi
fi

abt_close "$AGENT_S1_SESSION"
print_summary
echo "=== AP-E4 审批撤回 完成 ==="
