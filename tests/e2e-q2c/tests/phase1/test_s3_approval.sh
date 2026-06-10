#!/usr/bin/env bash
# ============================================================================
# Q2C E2E 测试 — S3: 报价审批（提交→接受）
# 角色: Agent-S1 (提交) → Agent-S2 (接受/拒绝)
# 目标: 验证报价从"草稿/已发送"到"已接受"的状态流转
# ============================================================================
set -euo pipefail

TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$TEST_DIR/../../config/env.sh"
source "$TEST_DIR/../../config/agents.sh"
source "$TEST_DIR/../../lib/login.sh"
source "$TEST_DIR/../../lib/form.sh"
source "$TEST_DIR/../../lib/assert.sh"
source "$TEST_DIR/../../lib/relay.sh"

echo "=== S3: 报价审批 ==="
echo ""

relay_set_phase "S3"

# --- 前置：获取报价 ID ---
QUOTATION_ID=$(relay_read "quotation_id")
QUOTATION_URL=$(relay_read "quotation_url")

if [[ -z "$QUOTATION_ID" ]]; then
    log_fail "接力文件中缺少 quotation_id，请先运行 test_s1_s2_quotation.sh"
    print_summary
    exit 1
fi

log_info "报价 ID: $QUOTATION_ID, URL: $QUOTATION_URL"

# --- Step 1: 检查当前报价状态 ---
log_step "1. 检查报价当前状态"

# 先用 DB 查当前状态
Q_STATUS=$(psql "$DB_URL" -t -A -c "SELECT status FROM quotations WHERE id = $QUOTATION_ID" 2>/dev/null || echo "")
log_info "数据库报价状态: $Q_STATUS"

# 如果是草稿(Draft/status=1)，先由 Agent-S1 提交
if [[ "$Q_STATUS" == "1" ]]; then
    log_step "1a. Agent-S1 提交报价（从草稿→已发送）"

    abt_login "$AGENT_S1_SESSION" "$AGENT_S1_USER" "$Q2C_PASSWORD"
    abt_navigate "$AGENT_S1_SESSION" "/admin/quotations/$QUOTATION_ID"
    sleep 1

    # 点击"提交报价"按钮（详情页上的 hx-post 按钮）
    abt_click_by_text "$AGENT_S1_SESSION" "提交报价"
    sleep 2

    # 验证状态变化
    Q_STATUS=$(psql "$DB_URL" -t -A -c "SELECT status FROM quotations WHERE id = $QUOTATION_ID" 2>/dev/null || echo "")
    log_info "提交后状态: $Q_STATUS"

    if [[ "$Q_STATUS" == "2" ]]; then
        assert_pass "报价已从草稿→已发送"
    else
        assert_fail "提交后状态不是'已发送' (status=$Q_STATUS)"
    fi
fi

# --- Step 2: Agent-S2（销售经理）审批接受 ---
log_step "2. Agent-S2 登录并审批报价"

abt_login "$AGENT_S2_SESSION" "$AGENT_S2_USER" "$Q2C_PASSWORD"
abt_navigate "$AGENT_S2_SESSION" "/admin/quotations/$QUOTATION_ID"
sleep 1

# 验证详情页加载
abt_assert_url_contains "$AGENT_S2_SESSION" "/admin/quotations/$QUOTATION_ID" "报价详情页"

# 验证页面显示"已发送"状态
abt_assert_page_contains "$AGENT_S2_SESSION" "已发送" "报价状态显示'已发送'" || \
abt_assert_page_contains "$AGENT_S2_SESSION" "Sent" "报价状态显示'Sent'"

# 点击"接受"按钮
log_info "Agent-S2 点击'接受'按钮..."
abt_click_by_text "$AGENT_S2_SESSION" "接受"
sleep 2

# --- Step 3: 验证审批结果 ---
log_step "3. 验证审批结果"

# 数据库验证
Q_STATUS_AFTER=$(psql "$DB_URL" -t -A -c "SELECT status FROM quotations WHERE id = $QUOTATION_ID" 2>/dev/null || echo "")
log_info "审批后状态: $Q_STATUS_AFTER"

# status=3 对应 Accepted
if [[ "$Q_STATUS_AFTER" == "3" ]]; then
    assert_pass "报价已接受 (status=Accepted)"
else
    assert_fail "报价状态不是'已接受' (status=$Q_STATUS_AFTER)"
fi

# 页面验证
abt_assert_page_contains "$AGENT_S2_SESSION" "已接受" "页面显示'已接受'" || \
abt_assert_page_contains "$AGENT_S2_SESSION" "Accepted" "页面显示'Accepted'"

# 验证"转销售订单"按钮出现（已接受状态才有）
abt_assert_visible "$AGENT_S2_SESSION" "a[href*='from_quotation']" "转销售订单按钮" || \
abt_assert_page_contains "$AGENT_S2_SESSION" "转销售订单" "转销售订单链接"

# --- Step 4: 写入接力 ---
relay_write "quotation_status" "accepted"
relay_snapshot "SNAP-S3"
relay_set_status "completed"

echo ""
echo "=== S3 完成 ==="
print_summary
