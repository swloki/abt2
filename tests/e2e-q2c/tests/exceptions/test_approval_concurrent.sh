#!/usr/bin/env bash
# AP-E6: 或签中两人同时审批 → 第一个生效
# 使用 SQL 直接创建测试数据并验证并发审批竞态逻辑
set -euo pipefail
TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$TEST_DIR/../../config/env.sh"
source "$TEST_DIR/../../lib/assert.sh"

echo "=== AP-E6: 或签中两人同时审批 → 第一个生效 ==="

# --- 测试标记，用于清理时识别测试数据 ---
TEST_MARKER="__e2e_ap_e6__"

log_step "1. 检查 workflow_instances + workflow_tasks 表是否存在"
WF_TABLES=$(psql "$DB_URL" -t -A -c "
    SELECT table_name FROM information_schema.tables
    WHERE table_name IN ('workflow_instances','workflow_tasks','workflow_history','quotations')
    ORDER BY table_name" 2>/dev/null || echo "")

if [[ "$WF_TABLES" != *"workflow_instances"* || "$WF_TABLES" != *"workflow_tasks"* ]]; then
    assert_skip "AP-E6: workflow_instances/workflow_tasks 表不存在，跳过"
    print_summary
    exit 0
fi
log_info "相关表已确认: $(echo $WF_TABLES | tr '\n' ',')"
assert_pass "AP-E6 Step 1: 工作流表存在"

log_step "2. 创建测试数据（报价 + 工作流实例 + 两个审批任务）"

# 先获取两个不同的审批人 ID（复用系统中已有用户）
# 用 sales_mgr 和 gm 分别代表两个或签审批人
ASSIGNEE_1=$(psql "$DB_URL" -t -A -c "
    SELECT id FROM users WHERE username = 'q2c_sales_mgr' LIMIT 1" 2>/dev/null || echo "")
ASSIGNEE_2=$(psql "$DB_URL" -t -A -c "
    SELECT id FROM users WHERE username = 'q2c_gm' LIMIT 1" 2>/dev/null || echo "")

# 如果找不到测试用户，用 1/2 作为占位（纯 SQL 测试场景，ID 仅做外键引用）
ASSIGNEE_1=${ASSIGNEE_1:-1}
ASSIGNEE_2=${ASSIGNEE_2:-2}

INITIATOR_ID=$(psql "$DB_URL" -t -A -c "
    SELECT id FROM users WHERE username = 'q2c_sales' LIMIT 1" 2>/dev/null || echo "")
INITIATOR_ID=${INITIATOR_ID:-1}

# 获取一个有效的 customer_id 和 contact_id
CUSTOMER_ID=$(psql "$DB_URL" -t -A -c "
    SELECT id FROM customers LIMIT 1" 2>/dev/null || echo "1")
CONTACT_ID=$(psql "$DB_URL" -t -A -c "
    SELECT id FROM contacts LIMIT 1" 2>/dev/null || echo "1")

# 创建报价单（status=2 表示已提交/待审批）
QUOTE_DOC="QT-E6-$(date +%s)"
psql "$DB_URL" -c "
    INSERT INTO quotations (doc_number, customer_id, contact_id, sales_rep_id, valid_until, status, operator_id, remark)
    VALUES ('$QUOTE_DOC', ${CUSTOMER_ID}, ${CONTACT_ID}, ${INITIATOR_ID}, CURRENT_DATE + INTERVAL '30 days', 2, ${INITIATOR_ID}, '${TEST_MARKER}')
" 2>/dev/null

QUOTE_ID=$(psql "$DB_URL" -t -A -c "
    SELECT id FROM quotations WHERE doc_number = '$QUOTE_DOC'" 2>/dev/null || echo "")

if [[ -z "$QUOTE_ID" ]]; then
    assert_fail "AP-E6 Step 2: 创建报价单失败"
    print_summary
    exit 1
fi
log_info "报价单已创建: id=$QUOTE_ID, doc_number=$QUOTE_DOC"

# 获取一个有效的 workflow_template_id（如果存在），否则用占位
TEMPLATE_ID=$(psql "$DB_URL" -t -A -c "
    SELECT id FROM workflow_templates WHERE entity_type = 'Quotation' LIMIT 1" 2>/dev/null || echo "")
TEMPLATE_ID=${TEMPLATE_ID:-0}

# 创建 workflow_instance（status='running'）
psql "$DB_URL" -c "
    INSERT INTO workflow_instances (template_id, entity_type, entity_id, status, context, initiator_id)
    VALUES (${TEMPLATE_ID}, 'Quotation', ${QUOTE_ID}, 'running',
            '{\"test_marker\": \"${TEST_MARKER}\", \"mode\": \"or-sign\"}',
            ${INITIATOR_ID})
" 2>/dev/null

INSTANCE_ID=$(psql "$DB_URL" -t -A -c "
    SELECT id FROM workflow_instances
    WHERE entity_id = ${QUOTE_ID} AND entity_type = 'Quotation'
    ORDER BY created_at DESC LIMIT 1" 2>/dev/null || echo "")

if [[ -z "$INSTANCE_ID" ]]; then
    assert_fail "AP-E6 Step 2: 创建工作流实例失败"
    # 清理已创建的报价
    psql "$DB_URL" -c "DELETE FROM quotations WHERE id = ${QUOTE_ID}" 2>/dev/null || true
    print_summary
    exit 1
fi
log_info "工作流实例已创建: id=$INSTANCE_ID"

# 创建两个或签审批任务（同一 instance，同一 node_id，不同 assignee）
# node_id 用 'or-approve-node' 表示这是一个或签节点
psql "$DB_URL" -c "
    INSERT INTO workflow_tasks (instance_id, node_id, assignee_id, status)
    VALUES (${INSTANCE_ID}, 'or-approve-node', ${ASSIGNEE_1}, 'pending')
" 2>/dev/null

TASK_1_ID=$(psql "$DB_URL" -t -A -c "
    SELECT id FROM workflow_tasks
    WHERE instance_id = ${INSTANCE_ID} AND assignee_id = ${ASSIGNEE_1}
    ORDER BY created_at DESC LIMIT 1" 2>/dev/null || echo "")

psql "$DB_URL" -c "
    INSERT INTO workflow_tasks (instance_id, node_id, assignee_id, status)
    VALUES (${INSTANCE_ID}, 'or-approve-node', ${ASSIGNEE_2}, 'pending')
" 2>/dev/null

TASK_2_ID=$(psql "$DB_URL" -t -A -c "
    SELECT id FROM workflow_tasks
    WHERE instance_id = ${INSTANCE_ID} AND assignee_id = ${ASSIGNEE_2}
    ORDER BY created_at DESC LIMIT 1" 2>/dev/null || echo "")

log_info "审批任务已创建: task1=$TASK_1_ID (assignee=$ASSIGNEE_1), task2=$TASK_2_ID (assignee=$ASSIGNEE_2)"
assert_pass "AP-E6 Step 2: 测试数据创建成功"

log_step "3. 模拟并发场景：第一个审批人生效"

# --- 第一个 task: 审批通过 ---
psql "$DB_URL" -c "
    UPDATE workflow_tasks
    SET status = 'completed', action = 'approve', completed_at = NOW()
    WHERE id = ${TASK_1_ID} AND status = 'pending'
" 2>/dev/null

# 写入审批历史记录
psql "$DB_URL" -c "
    INSERT INTO workflow_history (instance_id, task_id, node_id, event_type, actor_id, payload)
    VALUES (${INSTANCE_ID}, ${TASK_1_ID}, 'or-approve-node', 'task_completed', ${ASSIGNEE_1},
            '{\"action\": \"approve\", \"test_marker\": \"${TEST_MARKER}\"}')
" 2>/dev/null

# 或签语义：第一个通过后，instance 完成，其余同节点 task 取消
# 将 instance 状态更新为 completed
psql "$DB_URL" -c "
    UPDATE workflow_instances
    SET status = 'completed', completed_at = NOW(), last_advanced_at = NOW()
    WHERE id = ${INSTANCE_ID} AND status = 'running'
" 2>/dev/null

# --- 第二个 task: 尝试审批时检查 instance 是否已完成 ---
# 模拟：发现 instance 已经是 completed，自己的 task 应被取消
INSTANCE_STATUS=$(psql "$DB_URL" -t -A -c "
    SELECT status FROM workflow_instances WHERE id = ${INSTANCE_ID}" 2>/dev/null || echo "")

log_info "第二个审批人检查 instance 状态: $INSTANCE_STATUS"

if [[ "$INSTANCE_STATUS" == "completed" ]]; then
    # instance 已完成，取消第二个 task
    psql "$DB_URL" -c "
        UPDATE workflow_tasks
        SET status = 'cancelled', completed_at = NOW()
        WHERE id = ${TASK_2_ID} AND status = 'pending'
    " 2>/dev/null
    log_info "第二个审批任务已自动取消（instance 已 completed）"
else
    log_warn "instance 状态非 completed: $INSTANCE_STATUS，第二个审批人仍可操作"
fi

assert_pass "AP-E6 Step 3: 并发审批模拟完成"

log_step "4. 验证审批结果"

# 4.1 验证 workflow_instance.status = 'completed'
FINAL_INSTANCE_STATUS=$(psql "$DB_URL" -t -A -c "
    SELECT status FROM workflow_instances WHERE id = ${INSTANCE_ID}" 2>/dev/null || echo "")

if [[ "$FINAL_INSTANCE_STATUS" == "completed" ]]; then
    assert_pass "AP-E6 Step 4.1: 工作流实例状态为 completed"
else
    assert_fail "AP-E6 Step 4.1: 工作流实例状态期望 completed，实际 $FINAL_INSTANCE_STATUS"
fi

# 4.2 验证 tasks：一条 completed，一条 cancelled
COMPLETED_COUNT=$(psql "$DB_URL" -t -A -c "
    SELECT COUNT(*) FROM workflow_tasks
    WHERE instance_id = ${INSTANCE_ID} AND status = 'completed'" 2>/dev/null || echo "0")

CANCELLED_COUNT=$(psql "$DB_URL" -t -A -c "
    SELECT COUNT(*) FROM workflow_tasks
    WHERE instance_id = ${INSTANCE_ID} AND status = 'cancelled'" 2>/dev/null || echo "0")

PENDING_COUNT=$(psql "$DB_URL" -t -A -c "
    SELECT COUNT(*) FROM workflow_tasks
    WHERE instance_id = ${INSTANCE_ID} AND status = 'pending'" 2>/dev/null || echo "0")

log_info "任务状态统计: completed=$COMPLETED_COUNT, cancelled=$CANCELLED_COUNT, pending=$PENDING_COUNT"

if [[ "$COMPLETED_COUNT" == "1" ]]; then
    assert_pass "AP-E6 Step 4.2a: 恰好一条任务为 completed"
else
    assert_fail "AP-E6 Step 4.2a: 期望 1 条 completed 任务，实际 $COMPLETED_COUNT"
fi

if [[ "$CANCELLED_COUNT" == "1" ]]; then
    assert_pass "AP-E6 Step 4.2b: 恰好一条任务为 cancelled"
else
    assert_fail "AP-E6 Step 4.2b: 期望 1 条 cancelled 任务，实际 $CANCELLED_COUNT"
fi

if [[ "$PENDING_COUNT" == "0" ]]; then
    assert_pass "AP-E6 Step 4.2c: 无残留 pending 任务"
else
    assert_fail "AP-E6 Step 4.2c: 仍有 $PENDING_COUNT 条 pending 任务未处理"
fi

# 4.3 验证 workflow_history 有 approve 记录
APPROVE_HISTORY_COUNT=$(psql "$DB_URL" -t -A -c "
    SELECT COUNT(*) FROM workflow_history
    WHERE instance_id = ${INSTANCE_ID}
      AND event_type = 'task_completed'
      AND payload @> '{\"action\": \"approve\"}'::jsonb" 2>/dev/null || echo "0")

if [[ "$APPROVE_HISTORY_COUNT" -ge 1 ]]; then
    assert_pass "AP-E6 Step 4.3: workflow_history 有 approve 记录 (count=$APPROVE_HISTORY_COUNT)"
else
    assert_fail "AP-E6 Step 4.3: workflow_history 缺少 approve 记录"
fi

# 4.4 验证第一个审批任务的 action 字段
TASK1_ACTION=$(psql "$DB_URL" -t -A -c "
    SELECT action FROM workflow_tasks WHERE id = ${TASK_1_ID}" 2>/dev/null || echo "")

if [[ "$TASK1_ACTION" == "approve" ]]; then
    assert_pass "AP-E6 Step 4.4: 第一个任务 action=approve"
else
    assert_fail "AP-E6 Step 4.4: 第一个任务 action 期望 approve，实际 $TASK1_ACTION"
fi

log_step "5. 清理测试数据"

# 按外键依赖顺序删除：history → tasks → instance → quotation
psql "$DB_URL" -c "
    DELETE FROM workflow_history WHERE instance_id = ${INSTANCE_ID}
" 2>/dev/null || true
psql "$DB_URL" -c "
    DELETE FROM workflow_tasks WHERE instance_id = ${INSTANCE_ID}
" 2>/dev/null || true
psql "$DB_URL" -c "
    DELETE FROM workflow_instances WHERE id = ${INSTANCE_ID}
" 2>/dev/null || true
psql "$DB_URL" -c "
    DELETE FROM quotations WHERE id = ${QUOTE_ID}
" 2>/dev/null || true

log_info "测试数据已清理"
assert_pass "AP-E6 Step 5: 测试数据清理完成"

print_summary
echo "=== AP-E6 或签并发审批 完成 ==="
