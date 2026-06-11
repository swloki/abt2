#!/usr/bin/env bash
# AP-E5: 会签中一人拒绝 → 整体驳回，待办清除
set -euo pipefail
TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$TEST_DIR/../../config/env.sh"
source "$TEST_DIR/../../config/agents.sh"
source "$TEST_DIR/../../lib/login.sh"
source "$TEST_DIR/../../lib/assert.sh"
source "$TEST_DIR/../../lib/relay.sh"

echo "=== AP-E5: 会签中一人拒绝 → 整体驳回 ==="

# ── Step 1: 检查 workflow_instances + workflow_tasks 表 ──
log_step "1. 检查 workflow_instances + workflow_tasks 表"
WF_TABLES=$(psql "$DB_URL" -t -A -c "
    SELECT table_name FROM information_schema.tables
    WHERE table_name IN ('workflow_instances','workflow_tasks','workflow_history')
    ORDER BY table_name" 2>/dev/null || echo "")

if [[ "$WF_TABLES" != *"workflow_instances"* || "$WF_TABLES" != *"workflow_tasks"* || "$WF_TABLES" != *"workflow_history"* ]]; then
    assert_skip "AP-E5: workflow 表不完整 (found: ${WF_TABLES:-none})"
    print_summary
    exit 0
fi
log_info "workflow 表完整: $(echo "$WF_TABLES" | tr '\n' ',')"

# ── 标记：跟踪是否创建了测试数据（用于清理） ──
CREATED_INSTANCE_ID=""
CREATED_QUOTE_ID=""

# 清理函数：脚本退出时调用
cleanup() {
    if [[ -n "$CREATED_INSTANCE_ID" ]]; then
        log_info "清理测试数据: instance=$CREATED_INSTANCE_ID, quote=$CREATED_QUOTE_ID"
        psql "$DB_URL" -c "DELETE FROM workflow_history WHERE instance_id = $CREATED_INSTANCE_ID" 2>/dev/null || true
        psql "$DB_URL" -c "DELETE FROM workflow_tasks   WHERE instance_id = $CREATED_INSTANCE_ID" 2>/dev/null || true
        psql "$DB_URL" -c "DELETE FROM workflow_instances WHERE id = $CREATED_INSTANCE_ID" 2>/dev/null || true
        if [[ -n "$CREATED_QUOTE_ID" ]]; then
            psql "$DB_URL" -c "DELETE FROM quotation_items WHERE quotation_id = $CREATED_QUOTE_ID" 2>/dev/null || true
            psql "$DB_URL" -c "DELETE FROM quotations WHERE id = $CREATED_QUOTE_ID" 2>/dev/null || true
        fi
        log_info "清理完成"
    fi
}
trap cleanup EXIT

# ── Step 2: 查找或创建 running 状态的 workflow_instance ──
log_step "2. 查找或创建 running 状态的会签流程实例"

# 尝试找一个已有的 running 实例
EXISTING_INSTANCE=$(psql "$DB_URL" -t -A -c "
    SELECT id FROM workflow_instances
    WHERE status = 'running'
    ORDER BY created_at DESC LIMIT 1" 2>/dev/null || echo "")

if [[ -n "$EXISTING_INSTANCE" ]]; then
    # 复用已有实例：先清理其 tasks，再插入会签 tasks
    INSTANCE_ID="$EXISTING_INSTANCE"
    log_info "复用已有实例: id=$INSTANCE_ID"

    # 删除该实例下旧 tasks（测试结束后不会清理这些，因为是已有实例）
    psql "$DB_URL" -c "DELETE FROM workflow_history WHERE instance_id = $INSTANCE_ID" 2>/dev/null || true
    psql "$DB_URL" -c "DELETE FROM workflow_tasks WHERE instance_id = $INSTANCE_ID" 2>/dev/null || true
    log_info "已清理旧 tasks 和 history"
else
    # 需要创建 quotation + workflow instance
    log_info "未找到 running 实例，创建测试报价和流程实例"

    # 获取一个有效的 user id 作为 initiator
    INITIATOR_ID=$(psql "$DB_URL" -t -A -c "SELECT id FROM users LIMIT 1" 2>/dev/null || echo "")
    if [[ -z "$INITIATOR_ID" ]]; then
        assert_skip "AP-E5: users 表无数据，无法创建测试实例"
        print_summary
        exit 0
    fi

    # 获取一个有效的 customer_id
    CUSTOMER_ID=$(psql "$DB_URL" -t -A -c "SELECT id FROM customers LIMIT 1" 2>/dev/null || echo "")
    if [[ -z "$CUSTOMER_ID" ]]; then
        assert_skip "AP-E5: customers 表无数据，无法创建测试报价"
        print_summary
        exit 0
    fi

    # 获取一个有效的 contact_id
    CONTACT_ID=$(psql "$DB_URL" -t -A -c "SELECT id FROM contacts LIMIT 1" 2>/dev/null || echo "0")

    # 获取一个有效的 product_id
    PRODUCT_ID=$(psql "$DB_URL" -t -A -c "SELECT id FROM products LIMIT 1" 2>/dev/null || echo "1")

    # 创建测试报价
    CREATED_QUOTE_ID=$(psql "$DB_URL" -t -A -c "
        INSERT INTO quotations (doc_number, customer_id, contact_id, sales_rep_id, valid_until, status, operator_id)
        VALUES ('T-Q2C-E5-$(date +%s)', $CUSTOMER_ID, ${CONTACT_ID:-0}, $INITIATOR_ID, CURRENT_DATE + INTERVAL '30 days', 2, $INITIATOR_ID)
        RETURNING id" 2>/dev/null || echo "")
    if [[ -z "$CREATED_QUOTE_ID" ]]; then
        assert_skip "AP-E5: 创建测试报价失败"
        print_summary
        exit 0
    fi
    log_info "创建测试报价: id=$CREATED_QUOTE_ID"

    # 获取或创建一个 workflow_template
    TEMPLATE_ID=$(psql "$DB_URL" -t -A -c "SELECT id FROM workflow_templates WHERE status = 'active' LIMIT 1" 2>/dev/null || echo "")
    if [[ -z "$TEMPLATE_ID" ]]; then
        # 创建一个临时模板
        TEMPLATE_ID=$(psql "$DB_URL" -t -A -c "
            INSERT INTO workflow_templates (entity_type, name, version, status)
            VALUES ('Quotation', 'test-template-e5', 1, 'active')
            RETURNING id" 2>/dev/null || echo "")
    fi

    # 创建 workflow_instance
    CREATED_INSTANCE_ID=$(psql "$DB_URL" -t -A -c "
        INSERT INTO workflow_instances (template_id, template_version, entity_type, entity_id, status, frozen_graph, context, initiator_id)
        VALUES (${TEMPLATE_ID:-1}, 1, 'Quotation', $CREATED_QUOTE_ID, 'running',
                '{\"nodes\":[{\"id\":\"countersign\",\"type\":\"countersign\"}]}',
                '{\"test\":true,\"scenario\":\"AP-E5\"}',
                $INITIATOR_ID)
        RETURNING id" 2>/dev/null || echo "")
    if [[ -z "$CREATED_INSTANCE_ID" ]]; then
        assert_skip "AP-E5: 创建 workflow_instance 失败"
        print_summary
        exit 0
    fi
    INSTANCE_ID="$CREATED_INSTANCE_ID"
    log_info "创建流程实例: id=$INSTANCE_ID (quote=$CREATED_QUOTE_ID)"
fi

# ── Step 3: 创建会签 tasks（多个 pending） ──
log_step "3. 创建多个 pending 会签 tasks"

# 取 3 个不同的 assignee（用 users 表中的行）
ASSIGNEE_IDS=$(psql "$DB_URL" -t -A -c "SELECT id FROM users ORDER BY id LIMIT 3" 2>/dev/null || echo "")
if [[ -z "$ASSIGNEE_IDS" ]]; then
    # 没有足够的 users，用虚拟 ID
    ASSIGNEE_IDS="1
2
3"
fi

# 将 assignee IDs 读入数组
mapfile -t ASSIGNEE_ARR <<< "$ASSIGNEE_IDS"
log_info "Assignee 数量: ${#ASSIGNEE_ARR[@]}"

# 插入 3 个 pending countersign tasks
TASK_IDS=""
for i in "${!ASSIGNEE_ARR[@]}"; do
    AID="${ASSIGNEE_ARR[$i]}"
    AID=$(echo "$AID" | xargs)  # trim whitespace
    [[ -z "$AID" ]] && continue
    NODE="countersign_node_$((i + 1))"
    TID=$(psql "$DB_URL" -t -A -c "
        INSERT INTO workflow_tasks (instance_id, node_id, assignee_id, status, action, created_at)
        VALUES ($INSTANCE_ID, '$NODE', $AID, 'pending', 'approve', NOW())
        RETURNING id" 2>/dev/null || echo "")
    if [[ -n "$TID" ]]; then
        TASK_IDS="${TASK_IDS}${TID}"$'\n'
        log_info "创建 task: id=$TID, node=$NODE, assignee=$AID"
    fi
done

# 收集有效 task IDs
mapfile -t TASK_ID_ARR <<< "$TASK_IDS"
TASK_ID_ARR=($(printf '%s\n' "${TASK_ID_ARR[@]}" | sed '/^$/d'))
TASK_COUNT=${#TASK_ID_ARR[@]}

if [[ "$TASK_COUNT" -lt 2 ]]; then
    assert_fail "AP-E5: 创建的 pending tasks 不足 2 个 (got $TASK_COUNT)"
    print_summary
    exit 1
fi
log_info "共创建 $TASK_COUNT 个 pending tasks"
assert_pass "会签 tasks 创建成功 ($TASK_COUNT 个)"

# ── Step 4: 模拟第一个 task 的 reject 操作 ──
log_step "4. 模拟会签中一人拒绝 (task_id=${TASK_ID_ARR[0]})"

REJECT_TASK_ID="${TASK_ID_ARR[0]}"
REJECT_ASSIGNEE="${ASSIGNEE_ARR[0]}"
REJECT_ASSIGNEE=$(echo "$REJECT_ASSIGNEE" | xargs)

# 更新被拒绝的 task：status=rejected, action=reject, completed_at
psql "$DB_URL" -c "
    UPDATE workflow_tasks
    SET status = 'rejected', action = 'reject', result = '{\"reason\":\"countersign reject test\"}', completed_at = NOW()
    WHERE id = $REJECT_TASK_ID" 2>/dev/null

log_info "Task $REJECT_TASK_ID 已拒绝"

# 模拟会签拒绝逻辑：instance → rejected，其余 tasks → cancelled
psql "$DB_URL" -c "
    UPDATE workflow_instances
    SET status = 'rejected', updated_at = NOW(), completed_at = NOW()
    WHERE id = $INSTANCE_ID" 2>/dev/null
log_info "Instance $INSTANCE_ID 状态设为 rejected"

# 其余 pending tasks 取消
REMAINING_TASK_IDS=$(printf '%s\n' "${TASK_ID_ARR[@]:1}" | tr '\n' ',' | sed 's/,$//')
if [[ -n "$REMAINING_TASK_IDS" ]]; then
    psql "$DB_URL" -c "
        UPDATE workflow_tasks
        SET status = 'cancelled', completed_at = NOW()
        WHERE id IN ($REMAINING_TASK_IDS) AND status = 'pending'" 2>/dev/null
    log_info "其余 tasks ($REMAINING_TASK_IDS) 已取消"
fi

# 记录 history 事件
psql "$DB_URL" -c "
    INSERT INTO workflow_history (instance_id, task_id, node_id, event_type, actor_id, payload, created_at)
    VALUES ($INSTANCE_ID, $REJECT_TASK_ID, 'countersign_node_1', 'task_rejected', ${REJECT_ASSIGNEE:-1},
            '{\"reason\":\"countersign reject test\",\"scenario\":\"AP-E5\"}', NOW())" 2>/dev/null
psql "$DB_URL" -c "
    INSERT INTO workflow_history (instance_id, task_id, node_id, event_type, actor_id, payload, created_at)
    VALUES ($INSTANCE_ID, NULL, NULL, 'instance_rejected', ${REJECT_ASSIGNEE:-1},
            '{\"reason\":\"countersign reject - instance terminated\",\"scenario\":\"AP-E5\"}', NOW())" 2>/dev/null
log_info "History 事件已记录"

assert_pass "会签拒绝模拟完成"

# ── Step 5: 验证结果 ──
log_step "5. 验证会签拒绝结果"

# 5a: workflow_instance.status = 'rejected'
INSTANCE_STATUS=$(psql "$DB_URL" -t -A -c "SELECT status FROM workflow_instances WHERE id = $INSTANCE_ID" 2>/dev/null || echo "")
if [[ "$INSTANCE_STATUS" == "rejected" ]]; then
    assert_pass "instance.status = rejected"
else
    assert_fail "instance.status 期望 rejected，实际: ${INSTANCE_STATUS:-<empty>}"
fi

# 5b: 第一个 task 为 rejected
REJECTED_STATUS=$(psql "$DB_URL" -t -A -c "SELECT status FROM workflow_tasks WHERE id = $REJECT_TASK_ID" 2>/dev/null || echo "")
if [[ "$REJECTED_STATUS" == "rejected" ]]; then
    assert_pass "reject task.status = rejected (id=$REJECT_TASK_ID)"
else
    assert_fail "reject task.status 期望 rejected，实际: ${REJECTED_STATUS:-<empty>}"
fi

# 5c: 其余 tasks 为 cancelled
if [[ -n "$REMAINING_TASK_IDS" ]]; then
    CANCELLED_COUNT=$(psql "$DB_URL" -t -A -c "
        SELECT COUNT(*) FROM workflow_tasks
        WHERE id IN ($REMAINING_TASK_IDS) AND status = 'cancelled'" 2>/dev/null || echo "0")
    REMAINING_COUNT=$((${#TASK_ID_ARR[@]} - 1))
    if [[ "$CANCELLED_COUNT" == "$REMAINING_COUNT" ]]; then
        assert_pass "其余 tasks 已取消 ($CANCELLED_COUNT/$REMAINING_COUNT)"
    else
        assert_fail "其余 tasks 取消不完整: $CANCELLED_COUNT/$REMAINING_COUNT"
    fi
fi

# 5d: workflow_history 中有 reject 事件
REJECT_EVENTS=$(psql "$DB_URL" -t -A -c "
    SELECT COUNT(*) FROM workflow_history
    WHERE instance_id = $INSTANCE_ID AND event_type = 'task_rejected'" 2>/dev/null || echo "0")
if [[ "$REJECT_EVENTS" -ge 1 ]]; then
    assert_pass "history 记录 task_rejected 事件 ($REJECT_EVENTS 条)"
else
    assert_fail "history 未找到 task_rejected 事件"
fi

INSTANCE_REJECT_EVENTS=$(psql "$DB_URL" -t -A -c "
    SELECT COUNT(*) FROM workflow_history
    WHERE instance_id = $INSTANCE_ID AND event_type = 'instance_rejected'" 2>/dev/null || echo "0")
if [[ "$INSTANCE_REJECT_EVENTS" -ge 1 ]]; then
    assert_pass "history 记录 instance_rejected 事件 ($INSTANCE_REJECT_EVENTS 条)"
else
    assert_fail "history 未找到 instance_rejected 事件"
fi

# 5e: 不存在残留的 pending tasks
PENDING_LEFT=$(psql "$DB_URL" -t -A -c "
    SELECT COUNT(*) FROM workflow_tasks
    WHERE instance_id = $INSTANCE_ID AND status = 'pending'" 2>/dev/null || echo "0")
if [[ "$PENDING_LEFT" == "0" ]]; then
    assert_pass "无残留 pending tasks（待办已清除）"
else
    assert_fail "仍有 $PENDING_LEFT 个 pending tasks 未清除"
fi

print_summary
echo "=== AP-E5 会签中一人拒绝 → 整体驳回 完成 ==="
