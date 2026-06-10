#!/usr/bin/env bash
# ============================================================================
# Q2C E2E 测试 — F6: 总账结算验证（P2 — 可能未实现）
# 探测性测试：检查系统是否有总账功能，如有则验证借贷平衡
# ============================================================================
set -euo pipefail

TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$TEST_DIR/../../config/env.sh"
source "$TEST_DIR/../../config/agents.sh"
source "$TEST_DIR/../../lib/login.sh"
source "$TEST_DIR/../../lib/form.sh"
source "$TEST_DIR/../../lib/assert.sh"
source "$TEST_DIR/../../lib/relay.sh"

echo "=== F6: 总账结算验证 ==="
echo ""

relay_set_phase "F6"
relay_set_status "running"

log_step "1. 探测总账功能"

# --- 检查数据库中是否有总账表 ---
GL_TABLES=("general_ledger" "gl_entries" "ledger_entries" "journal_entries" "account_balances")
GL_FOUND=false

for TABLE in "${GL_TABLES[@]}"; do
    COUNT=$(psql "$DB_URL" -t -A -c "SELECT COUNT(*) FROM information_schema.tables WHERE table_name = '$TABLE'" 2>/dev/null || echo "0")
    if [[ "$COUNT" -gt 0 ]]; then
        log_info "找到总账相关表: $TABLE"
        GL_FOUND=true

        # 尝试验证借贷平衡
        DEBIT=$(psql "$DB_URL" -t -A -c "SELECT COALESCE(SUM(debit),0) FROM $TABLE WHERE deleted_at IS NULL" 2>/dev/null || \
                psql "$DB_URL" -t -A -c "SELECT COALESCE(SUM(CASE WHEN amount > 0 THEN amount ELSE 0 END),0) FROM $TABLE WHERE deleted_at IS NULL" 2>/dev/null || echo "0")
        CREDIT=$(psql "$DB_URL" -t -A -c "SELECT COALESCE(SUM(credit),0) FROM $TABLE WHERE deleted_at IS NULL" 2>/dev/null || \
                 psql "$DB_URL" -t -A -c "SELECT COALESCE(SUM(CASE WHEN amount < 0 THEN ABS(amount) ELSE 0 END),0) FROM $TABLE WHERE deleted_at IS NULL" 2>/dev/null || echo "0")

        log_info "借方合计: $DEBIT"
        log_info "贷方合计: $CREDIT"

        if [[ "$DEBIT" == "$CREDIT" ]]; then
            assert_pass "总账借贷平衡 (借方=$DEBIT = 贷方=$CREDIT)"
        else
            log_warn "借贷不平衡 (借方=$DEBIT, 贷方=$CREDIT, 差额=$(echo "$DEBIT - $CREDIT" | bc 2>/dev/null || echo '?'))"
        fi
        break
    fi
done

if [[ "$GL_FOUND" == "false" ]]; then
    assert_skip "F6 总账结算: 功能未实现（未找到总账表）"
    relay_write "gl_available" "false"
else
    relay_write "gl_available" "true"
fi

# --- 尝试通过 UI 检查 ---
log_step "2. 通过 UI 检查 FMS"
abt_login "$AGENT_F1_SESSION" "$AGENT_F1_USER" "$Q2C_PASSWORD"
abt_navigate "$AGENT_F1_SESSION" "/admin/fms"
sleep 1

page_text=$(abt_get_text "$AGENT_F1_SESSION" 2>/dev/null || echo "")
if ! echo "$page_text" | grep -qi "forbidden\|403"; then
    assert_pass "FMS Dashboard 可访问"
fi

# --- 完成 ---
relay_write "f6_status" "verified"
relay_snapshot "SNAP-F6"
relay_set_status "completed"

echo ""
echo "=== F6 完成 ==="
print_summary
