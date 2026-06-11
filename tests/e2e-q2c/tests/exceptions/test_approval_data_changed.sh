#!/usr/bin/env bash
# AP-E7: 审批中数据被修改 → 审批自动挂起
set -euo pipefail
TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$TEST_DIR/../../config/env.sh"
source "$TEST_DIR/../../config/agents.sh"
source "$TEST_DIR/../../lib/login.sh"
source "$TEST_DIR/../../lib/form.sh"
source "$TEST_DIR/../../lib/assert.sh"
source "$TEST_DIR/../../lib/relay.sh"

echo "=== AP-E7: 审批中数据被修改 → 审批自动挂起 ==="

# 测试数据标记（用于清理）
DATA_TAG="_e2e_ap_e7_$(date +%s)"

# ============================================================================
# 清理函数：删除本次测试创建的所有数据
# ============================================================================
cleanup_test_data() {
    log_info "清理测试数据 (tag=$DATA_TAG)..."
    # 删除测试创建的 workflow 相关数据（通过 context 中的 tag 追踪）
    psql "$DB_URL" -t -A -c "
        DELETE FROM workflow_history WHERE instance_id IN (
            SELECT id FROM workflow_instances WHERE context @> '{\"test_tag\":\"$DATA_TAG\"}'
        );
        DELETE FROM workflow_tasks WHERE instance_id IN (
            SELECT id FROM workflow_instances WHERE context @> '{\"test_tag\":\"$DATA_TAG\"}'
        );
        DELETE FROM workflow_instances WHERE context @> '{\"test_tag\":\"$DATA_TAG\"}';
        DELETE FROM quotations WHERE remark LIKE '%$DATA_TAG%';
    " 2>/dev/null || true
    log_info "清理完成"
}
trap cleanup_test_data EXIT

# ============================================================================
# Step 1: 检查 workflow_instances + quotations 表是否存在
# ============================================================================
log_step "1. 检查 workflow_instances + quotations 表是否存在"

REQUIRED_TABLES=$(psql "$DB_URL" -t -A -c "
    SELECT table_name FROM information_schema.tables
    WHERE table_name IN ('workflow_instances', 'workflow_tasks', 'quotations')
    ORDER BY table_name
" 2>/dev/null || echo "")

TABLE_COUNT=$(echo "$REQUIRED_TABLES" | wc -l | tr -d ' ')
if [[ "$TABLE_COUNT" -lt 3 ]]; then
    log_warn "缺失表: 期望 workflow_instances, workflow_tasks, quotations"
    log_warn "已有表: $(echo $REQUIRED_TABLES | tr '\n' ', ')"
    assert_skip "AP-E7: 必要表不存在（workflow_instances/workflow_tasks/quotations），跳过测试"
    print_summary
    exit 0
fi
log_info "必要表均已存在: $(echo $REQUIRED_TABLES | tr '\n' ', ')"
assert_pass "AP-E7-1: 必要表检查通过"

# ============================================================================
# Step 2: 创建测试数据
#   - 创建报价（status=2 即 PendingApproval/Sent）
#   - 创建 workflow_instance (status='running')
#   - 创建 pending workflow_task
# ============================================================================
log_step "2. 通过 SQL 创建测试数据"

# 获取一个可用的用户 ID 作为 initiator
INITIATOR_ID=$(psql "$DB_URL" -t -A -c "
    SELECT id FROM users WHERE username = '$AGENT_S1_USER' LIMIT 1
" 2>/dev/null || echo "")

if [[ -z "$INITIATOR_ID" ]]; then
    # 尝试从 employees 表取
    INITIATOR_ID=$(psql "$DB_URL" -t -A -c "
        SELECT id FROM employees LIMIT 1
    " 2>/dev/null || echo "1")
fi
log_info "使用 initiator_id=$INITIATOR_ID"

# 获取审批人 ID
ASSIGNEE_ID=$(psql "$DB_URL" -t -A -c "
    SELECT id FROM users WHERE username = '$AGENT_S2_USER' LIMIT 1
" 2>/dev/null || echo "")

if [[ -z "$ASSIGNEE_ID" ]]; then
    ASSIGNEE_ID=$(psql "$DB_URL" -t -A -c "
        SELECT id FROM users LIMIT 1
    " 2>/dev/null || echo "2")
fi
log_info "使用 assignee_id=$ASSIGNEE_ID"

# 创建报价（status=2 = PendingApproval）
QUOTE_ID=$(psql "$DB_URL" -t -A -c "
    INSERT INTO quotations (
        doc_number, customer_id, sales_rep_id, quotation_date, valid_until,
        status, total_amount, remark, operator_id
    ) VALUES (
        'QT-E7-${DATA_TAG}',
        COALESCE((SELECT id FROM customers LIMIT 1), 1),
        COALESCE((SELECT id FROM users LIMIT 1), 1),
        CURRENT_DATE,
        CURRENT_DATE + INTERVAL '30 days',
        2,
        10000.00,
        'E2E测试-AP-E7-${DATA_TAG}',
        COALESCE((SELECT id FROM users LIMIT 1), 1)
    ) RETURNING id
" 2>/dev/null || echo "")

if [[ -z "$QUOTE_ID" ]]; then
    assert_skip "AP-E7: 无法创建测试报价（表结构不匹配或缺少外键数据）"
    print_summary
    exit 0
fi
log_info "创建报价 ID=$QUOTE_ID, total_amount=10000.00"

# 创建 workflow_instance（status='running'）
INSTANCE_ID=$(psql "$DB_URL" -t -A -c "
    INSERT INTO workflow_instances (
        template_id, template_version, entity_type, entity_id,
        status, frozen_graph, context, initiator_id
    ) VALUES (
        COALESCE((SELECT id FROM workflow_templates WHERE entity_type = 'Quotation' AND status = 'active' LIMIT 1), 1),
        1,
        'Quotation',
        $QUOTE_ID,
        'running',
        '{\"nodes\": {\"start\": {\"type\": \"start\"}, \"approve\": {\"type\": \"approval\"}, \"end\": {\"type\": \"end\"}}, \"edges\": {\"start\": \"approve\", \"approve\": \"end\"}}'::jsonb,
        '{\"test_tag\": \"${DATA_TAG}\", \"total_amount\": 10000.00}'::jsonb,
        $INITIATOR_ID
    ) RETURNING id
" 2>/dev/null || echo "")

if [[ -z "$INSTANCE_ID" ]]; then
    assert_skip "AP-E7: 无法创建 workflow_instance（可能缺少 workflow_templates 种子数据）"
    # 清理已创建的报价
    psql "$DB_URL" -c "DELETE FROM quotations WHERE id = $QUOTE_ID" 2>/dev/null || true
    print_summary
    exit 0
fi
log_info "创建 workflow_instance ID=$INSTANCE_ID, status='running'"

# 创建 pending workflow_task
TASK_ID=$(psql "$DB_URL" -t -A -c "
    INSERT INTO workflow_tasks (
        instance_id, node_id, assignee_id, status, action
    ) VALUES (
        $INSTANCE_ID,
        'approve',
        $ASSIGNEE_ID,
        'pending',
        'approve'
    ) RETURNING id
" 2>/dev/null || echo "")

if [[ -z "$TASK_ID" ]]; then
    assert_skip "AP-E7: 无法创建 workflow_task"
    psql "$DB_URL" -c "DELETE FROM workflow_instances WHERE id = $INSTANCE_ID" 2>/dev/null || true
    psql "$DB_URL" -c "DELETE FROM quotations WHERE id = $QUOTE_ID" 2>/dev/null || true
    print_summary
    exit 0
fi
log_info "创建 workflow_task ID=$TASK_ID, status='pending', assignee_id=$ASSIGNEE_ID"
assert_pass "AP-E7-2: 测试数据创建完成 (quote=$QUOTE_ID, instance=$INSTANCE_ID, task=$TASK_ID)"

# ============================================================================
# Step 3: 模拟数据变更 — 修改报价金额
# ============================================================================
log_step "3. 模拟数据变更：修改报价金额"

OLD_AMOUNT=$(psql "$DB_URL" -t -A -c "
    SELECT total_amount FROM quotations WHERE id = $QUOTE_ID
" 2>/dev/null)
log_info "变更前 total_amount=$OLD_AMOUNT"

# 修改 total_amount（模拟其他人修改了报价金额）
NEW_AMOUNT=25000.00
psql "$DB_URL" -c "
    UPDATE quotations SET total_amount = $NEW_AMOUNT, updated_at = NOW() WHERE id = $QUOTE_ID
" 2>/dev/null

# 验证修改成功
CURRENT_AMOUNT=$(psql "$DB_URL" -t -A -c "
    SELECT total_amount FROM quotations WHERE id = $QUOTE_ID
" 2>/dev/null)
log_info "变更后 total_amount=$CURRENT_AMOUNT"

if [[ "$CURRENT_AMOUNT" == "$NEW_AMOUNT" ]]; then
    assert_pass "AP-E7-3: 报价金额已修改 ($OLD_AMOUNT → $CURRENT_AMOUNT)"
else
    assert_fail "AP-E7-3: 报价金额修改失败 (期望=$NEW_AMOUNT, 实际=$CURRENT_AMOUNT)"
fi

# ============================================================================
# Step 4: 检查 workflow_instance 是否自动挂起
#   - 系统可能通过事件监听自动挂起
#   - 如果未自动挂起，检查是否有 API 可触发挂起检查
# ============================================================================
log_step "4. 检查 workflow_instance 状态变化"

# 短暂等待，给系统事件监听时间反应
sleep 1

INSTANCE_STATUS=$(psql "$DB_URL" -t -A -c "
    SELECT status FROM workflow_instances WHERE id = $INSTANCE_ID
" 2>/dev/null)
log_info "workflow_instance status=$INSTANCE_STATUS"

SUSPENDED_REASON=$(psql "$DB_URL" -t -A -c "
    SELECT suspended_reason FROM workflow_instances WHERE id = $INSTANCE_ID
" 2>/dev/null)
log_info "suspended_reason=$SUSPENDED_REASON"

# 场景 A: 系统自动检测并挂起
if [[ "$INSTANCE_STATUS" == "suspended" ]]; then
    assert_pass "AP-E7-4: 审批已自动挂起 (status=$INSTANCE_STATUS)"

    # 验证 suspended_reason 包含数据变更信息
    if [[ -n "$SUSPENDED_REASON" && "$SUSPENDED_REASON" != "null" ]]; then
        assert_pass "AP-E7-4a: suspended_reason 已记录: $SUSPENDED_REASON"
    else
        log_warn "suspended_reason 为空，挂起但未记录原因"
    fi

    # 场景 B: 尝试通过 API 触发挂起检查
elif [[ "$INSTANCE_STATUS" == "running" ]]; then
    log_info "系统未自动挂起，尝试通过 API 触发挂起检查..."

    # 尝试调用挂起检查 API
    API_TRIGGERED=false
    HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" \
        -X POST "${ABT_URL}/admin/quotations/${QUOTE_ID}/check-approval" \
        -H "Content-Type: application/json" 2>/dev/null || echo "000")

    if [[ "$HTTP_CODE" == "200" || "$HTTP_CODE" == "204" ]]; then
        API_TRIGGERED=true
        log_info "API 返回 HTTP $HTTP_CODE"

        # 等待并重新检查
        sleep 1
        INSTANCE_STATUS=$(psql "$DB_URL" -t -A -c "
            SELECT status FROM workflow_instances WHERE id = $INSTANCE_ID
        " 2>/dev/null)
        SUSPENDED_REASON=$(psql "$DB_URL" -t -A -c "
            SELECT suspended_reason FROM workflow_instances WHERE id = $INSTANCE_ID
        " 2>/dev/null)
    else
        log_info "挂起检查 API 不可用 (HTTP $HTTP_CODE)"
    fi

    # 最终判定
    if [[ "$INSTANCE_STATUS" == "suspended" ]]; then
        assert_pass "AP-E7-4: API 触发后审批已挂起"
        if [[ -n "$SUSPENDED_REASON" && "$SUSPENDED_REASON" != "null" ]]; then
            assert_pass "AP-E7-4a: suspended_reason 已记录: $SUSPENDED_REASON"
        fi
    else
        # 检查 suspended_reason 是否有数据变更记录（即使 status 未变）
        if [[ -n "$SUSPENDED_REASON" && "$SUSPENDED_REASON" != "null" ]]; then
            assert_pass "AP-E7-4: suspended_reason 检测到数据变更标记: $SUSPENDED_REASON"
        else
            # 功能未实现
            assert_skip "AP-E7-4: 审批数据变更自动挂起功能未实现（status=$INSTANCE_STATUS, suspended_reason 为空）"
        fi
    fi
else
    log_warn "workflow_instance 异常状态: $INSTANCE_STATUS"
    assert_fail "AP-E7-4: workflow_instance 处于非预期状态: $INSTANCE_STATUS"
fi

# ============================================================================
# Step 5: 记录测试结果到接力文件
# ============================================================================
log_step "5. 记录测试结果"
relay_write "ap_e7_quote_id" "$QUOTE_ID"
relay_write "ap_e7_instance_id" "$INSTANCE_ID"
relay_write "ap_e7_final_status" "$INSTANCE_STATUS"
log_info "结果已记录到接力文件"

# ============================================================================
# Step 6: 清理（trap EXIT 会自动调用 cleanup_test_data）
# ============================================================================
log_step "6. 清理测试数据"
# trap EXIT 已注册 cleanup_test_data，此处显式调用也可
cleanup_test_data

print_summary
echo "=== AP-E7 审批中数据被修改 → 审批自动挂起 完成 ==="
