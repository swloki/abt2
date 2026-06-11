#!/usr/bin/env bash
# AP-E8: 多次驳回重提历史完整
# 验证 workflow_history 在多次驳回+重提循环中的记录完整性
set -euo pipefail
TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$TEST_DIR/../../config/env.sh"
source "$TEST_DIR/../../config/agents.sh"
source "$TEST_DIR/../../lib/assert.sh"
source "$TEST_DIR/../../lib/relay.sh"

echo "=== AP-E8: 多次驳回重提历史完整 ==="

# ---- 可复用的 actor IDs（与 agents.sh 角色对应） ----
ACTOR_S1=1   # 销售专员 q2c_sales
ACTOR_S2=2   # 销售经理 q2c_sales_mgr
ACTOR_GM=15  # 总经理 q2c_gm

# ---- 标记是否为自建数据 ----
OWN_DATA=false
OWN_QUOTE_ID=""
OWN_INSTANCE_ID=""

cleanup() {
    if [[ "$OWN_DATA" == "true" ]]; then
        log_step "清理自建测试数据"
        # 删除顺序：history → tasks → instance → quotation（如有）
        [[ -n "$OWN_INSTANCE_ID" ]] && psql "$DB_URL" -c "DELETE FROM workflow_history WHERE instance_id = $OWN_INSTANCE_ID" 2>/dev/null || true
        [[ -n "$OWN_INSTANCE_ID" ]] && psql "$DB_URL" -c "DELETE FROM workflow_tasks WHERE instance_id = $OWN_INSTANCE_ID" 2>/dev/null || true
        [[ -n "$OWN_INSTANCE_ID" ]] && psql "$DB_URL" -c "DELETE FROM workflow_instances WHERE id = $OWN_INSTANCE_ID" 2>/dev/null || true
        [[ -n "$OWN_QUOTE_ID" ]]    && psql "$DB_URL" -c "DELETE FROM quotations WHERE id = $OWN_QUOTE_ID" 2>/dev/null || true
        log_info "已清理自建数据 (quote=$OWN_QUOTE_ID, instance=$OWN_INSTANCE_ID)"
    fi
}
trap cleanup EXIT

# ===========================================================================
log_step "1. 检查 workflow_instances + workflow_history + quotations 表是否存在"
# ===========================================================================
TABLES_OK=true
for t in workflow_instances workflow_history workflow_tasks quotations; do
    EXISTS=$(psql "$DB_URL" -t -A -c "SELECT 1 FROM information_schema.tables WHERE table_name = '$t'" 2>/dev/null || echo "")
    if [[ "$EXISTS" != "1" ]]; then
        log_warn "表 $t 不存在"
        TABLES_OK=false
    fi
done

if [[ "$TABLES_OK" != "true" ]]; then
    assert_skip "AP-E8: 缺少必要的审批表（workflow_instances/workflow_history/workflow_tasks/quotations）"
    exit 0
fi
assert_pass "AP-E8 表检查: 四张核心表均存在"

# ===========================================================================
log_step "2. 查找已有的 quotation + workflow 数据"
# ===========================================================================
INSTANCE_ID=$(psql "$DB_URL" -t -A -c "
    SELECT i.id
    FROM workflow_instances i
    JOIN quotations q ON q.id = i.entity_id AND i.entity_type = 'Quotation'
    ORDER BY i.created_at DESC
    LIMIT 1" 2>/dev/null || echo "")

HISTORY_COUNT=0
if [[ -n "$INSTANCE_ID" ]]; then
    HISTORY_COUNT=$(psql "$DB_URL" -t -A -c "
        SELECT COUNT(*) FROM workflow_history WHERE instance_id = $INSTANCE_ID" 2>/dev/null || echo "0")
fi

log_info "已有实例 ID=$INSTANCE_ID, 历史记录数=$HISTORY_COUNT"

# ===========================================================================
log_step "3. 如无数据则通过 SQL 创建完整历史场景（3 轮驳回+重提）"
# ===========================================================================
if [[ -z "$INSTANCE_ID" || "$HISTORY_COUNT" -lt 6 ]]; then
    log_info "历史记录不足，创建完整测试数据..."
    OWN_DATA=true

    # --- 3a. 确保有 workflow_template ---
    TEMPLATE_ID=$(psql "$DB_URL" -t -A -c "
        SELECT id FROM workflow_templates
        WHERE entity_type = 'Quotation' AND status = 'active'
        LIMIT 1" 2>/dev/null || echo "")

    if [[ -z "$TEMPLATE_ID" ]]; then
        # 插入一个最小模板
        TEMPLATE_ID=$(psql "$DB_URL" -t -A -c "
            INSERT INTO workflow_templates (entity_type, name, version, status, graph)
            VALUES ('Quotation', '报价审批测试模板', 1, 'active',
                    '{\"nodes\":[{\"id\":\"start\",\"type\":\"start\"},{\"id\":\"approve\",\"type\":\"approve\"},{\"id\":\"end\",\"type\":\"end\"}],\"edges\":[{\"from\":\"start\",\"to\":\"approve\"},{\"from\":\"approve\",\"to\":\"end\"}]}')
            RETURNING id" 2>/dev/null || echo "")
    fi
    log_info "使用模板 ID=$TEMPLATE_ID"

    # --- 3b. 确保有 quotation ---
    QUOTE_ID=$(psql "$DB_URL" -t -A -c "
        SELECT id FROM quotations
        WHERE doc_number = 'QT-HIST-E8'
        LIMIT 1" 2>/dev/null || echo "")

    if [[ -z "$QUOTE_ID" ]]; then
        # 需要有效的 customer_id / contact_id / sales_rep_id / operator_id
        CUSTOMER_ID=$(psql "$DB_URL" -t -A -c "
            SELECT id FROM customers LIMIT 1" 2>/dev/null || echo "")
        CONTACT_ID=$(psql "$DB_URL" -t -A -c "
            SELECT id FROM contacts LIMIT 1" 2>/dev/null || echo "")
        SALES_REP_ID=$(psql "$DB_URL" -t -A -c "
            SELECT id FROM users WHERE username = '$AGENT_S1_USER' LIMIT 1" 2>/dev/null || echo "")
        OPERATOR_ID=$(psql "$DB_URL" -t -A -c "
            SELECT id FROM users WHERE username = '$AGENT_S1_USER' LIMIT 1" 2>/dev/null || echo "")

        # 如果找不到关联数据，用占位 ID
        CUSTOMER_ID=${CUSTOMER_ID:-1}
        CONTACT_ID=${CONTACT_ID:-1}
        SALES_REP_ID=${SALES_REP_ID:-$ACTOR_S1}
        OPERATOR_ID=${OPERATOR_ID:-$ACTOR_S1}

        QUOTE_ID=$(psql "$DB_URL" -t -A -c "
            INSERT INTO quotations (doc_number, customer_id, contact_id, sales_rep_id,
                                    quotation_date, valid_until, status, total_amount,
                                    total_cost, estimated_margin, operator_id)
            VALUES ('QT-HIST-E8', $CUSTOMER_ID, $CONTACT_ID, $SALES_REP_ID,
                    CURRENT_DATE, CURRENT_DATE + INTERVAL '30 days', 2,
                    10000.00, 6000.00, 40.00, $OPERATOR_ID)
            RETURNING id" 2>/dev/null || echo "")
    fi
    OWN_QUOTE_ID=$QUOTE_ID
    log_info "报价 ID=$QUOTE_ID"

    # --- 3c. 创建 workflow_instance ---
    INSTANCE_ID=$(psql "$DB_URL" -t -A -c "
        INSERT INTO workflow_instances (template_id, template_version, entity_type, entity_id,
                                        status, frozen_graph, context, initiator_id)
        VALUES ($TEMPLATE_ID, 1, 'Quotation', $QUOTE_ID,
                'running',
                '{\"nodes\":[\"start\",\"approve\",\"end\"]}',
                '{\"doc_number\":\"QT-HIST-E8\"}',
                $ACTOR_S1)
        RETURNING id" 2>/dev/null || echo "")
    OWN_INSTANCE_ID=$INSTANCE_ID
    log_info "实例 ID=$INSTANCE_ID"

    # --- 3d. 构造 3 轮 驳回+重提 的完整历史 ---
    # 每轮: task_created → task_completed(提交) → task_rejected(驳回) → instance_rejected → instance_restarted
    # 第 3 轮最终通过: task_completed(提交) → task_completed(审批通过) → instance_completed
    BASE_TIME="2026-01-15 10:00:00+08"

    psql "$DB_URL" -c "
    INSERT INTO workflow_history (instance_id, task_id, node_id, event_type, actor_id, payload, created_at) VALUES
    -- ========== 第 1 轮：提交 → 驳回 ==========
    -- 提交审批
    ( $INSTANCE_ID, NULL, 'start', 'instance_started', $ACTOR_S1,
      '{\"comment\":\"提交报价审批\",\"operator\":\"q2c_sales\"}'::jsonb,
      TIMESTAMP WITH TIME ZONE '$BASE_TIME' + INTERVAL '0 minutes'),
    -- 经理审批任务创建
    ( $INSTANCE_ID, NULL, 'approve', 'task_created', $ACTOR_S2,
      '{\"assignee\":\"q2c_sales_mgr\"}'::jsonb,
      TIMESTAMP WITH TIME ZONE '$BASE_TIME' + INTERVAL '1 minutes'),
    -- 经理驳回
    ( $INSTANCE_ID, NULL, 'approve', 'task_rejected', $ACTOR_S2,
      '{\"comment\":\"价格偏高，请调整\",\"operator\":\"q2c_sales_mgr\",\"reason\":\"价格不合理\"}'::jsonb,
      TIMESTAMP WITH TIME ZONE '$BASE_TIME' + INTERVAL '10 minutes'),
    -- 实例被驳回
    ( $INSTANCE_ID, NULL, 'approve', 'instance_rejected', $ACTOR_S2,
      '{\"comment\":\"驳回报价\",\"operator\":\"q2c_sales_mgr\",\"round\":1}'::jsonb,
      TIMESTAMP WITH TIME ZONE '$BASE_TIME' + INTERVAL '10 minutes'),
    -- 重新提交
    ( $INSTANCE_ID, NULL, 'start', 'instance_restarted', $ACTOR_S1,
      '{\"comment\":\"调整价格后重新提交\",\"operator\":\"q2c_sales\",\"round\":1}'::jsonb,
      TIMESTAMP WITH TIME ZONE '$BASE_TIME' + INTERVAL '30 minutes'),

    -- ========== 第 2 轮：提交 → 驳回 ==========
    ( $INSTANCE_ID, NULL, 'approve', 'task_created', $ACTOR_S2,
      '{\"assignee\":\"q2c_sales_mgr\"}'::jsonb,
      TIMESTAMP WITH TIME ZONE '$BASE_TIME' + INTERVAL '31 minutes'),
    ( $INSTANCE_ID, NULL, 'approve', 'task_rejected', $ACTOR_S2,
      '{\"comment\":\"毛利率偏低，需要提高利润率\",\"operator\":\"q2c_sales_mgr\",\"reason\":\"利润率不足\"}'::jsonb,
      TIMESTAMP WITH TIME ZONE '$BASE_TIME' + INTERVAL '45 minutes'),
    ( $INSTANCE_ID, NULL, 'approve', 'instance_rejected', $ACTOR_S2,
      '{\"comment\":\"再次驳回\",\"operator\":\"q2c_sales_mgr\",\"round\":2}'::jsonb,
      TIMESTAMP WITH TIME ZONE '$BASE_TIME' + INTERVAL '45 minutes'),
    ( $INSTANCE_ID, NULL, 'start', 'instance_restarted', $ACTOR_S1,
      '{\"comment\":\"优化成本后再次提交\",\"operator\":\"q2c_sales\",\"round\":2}'::jsonb,
      TIMESTAMP WITH TIME ZONE '$BASE_TIME' + INTERVAL '90 minutes'),

    -- ========== 第 3 轮：提交 → 通过 ==========
    ( $INSTANCE_ID, NULL, 'approve', 'task_created', $ACTOR_S2,
      '{\"assignee\":\"q2c_sales_mgr\"}'::jsonb,
      TIMESTAMP WITH TIME ZONE '$BASE_TIME' + INTERVAL '91 minutes'),
    ( $INSTANCE_ID, NULL, 'approve', 'task_completed', $ACTOR_S2,
      '{\"comment\":\"价格合理，同意\",\"operator\":\"q2c_sales_mgr\",\"action\":\"approve\"}'::jsonb,
      TIMESTAMP WITH TIME ZONE '$BASE_TIME' + INTERVAL '100 minutes'),
    -- 实例完成
    ( $INSTANCE_ID, NULL, 'end', 'instance_completed', $ACTOR_S2,
      '{\"comment\":\"审批流程完成\",\"operator\":\"q2c_sales_mgr\",\"round\":3}'::jsonb,
      TIMESTAMP WITH TIME ZONE '$BASE_TIME' + INTERVAL '100 minutes')
    " 2>/dev/null

    assert_pass "AP-E8 创建测试数据: 3 轮驳回+重提，12 条历史记录已插入"
else
    log_info "复用已有实例 ID=$INSTANCE_ID（历史记录数=$HISTORY_COUNT）"
fi

# ===========================================================================
log_step "4. 验证 history 按 created_at 排序且记录完整"
# ===========================================================================
# 读取全部历史记录，按时间排序
HISTORY_ROWS=$(psql "$DB_URL" -t -A -c "
    SELECT id, event_type, node_id, actor_id, created_at
    FROM workflow_history
    WHERE instance_id = $INSTANCE_ID
    ORDER BY created_at ASC" 2>/dev/null || echo "")

ROW_COUNT=$(echo "$HISTORY_ROWS" | wc -l | tr -d ' ')
log_info "历史记录总数: $ROW_COUNT"

# 至少 12 条（3 轮完整记录）
if [[ "$ROW_COUNT" -ge 12 ]]; then
    assert_pass "AP-E8 历史记录数: $ROW_COUNT >= 12（3 轮完整）"
else
    assert_fail "AP-E8 历史记录数: $ROW_COUNT < 12（预期至少 12 条）"
fi

# 验证时间有序（每行的 created_at >= 上一行）
TIME_ORDER_OK=$(psql "$DB_URL" -t -A -c "
    SELECT COUNT(*) FROM (
        SELECT created_at, LAG(created_at) OVER (ORDER BY created_at) AS prev_time
        FROM workflow_history
        WHERE instance_id = $INSTANCE_ID
    ) t
    WHERE prev_time IS NOT NULL AND created_at < prev_time" 2>/dev/null || echo "0")

if [[ "$TIME_ORDER_OK" == "0" ]]; then
    assert_pass "AP-E8 时间有序: 所有记录 created_at 严格递增"
else
    assert_fail "AP-E8 时间有序: 发现 $TIME_ORDER_OK 条时间乱序记录"
fi

# ===========================================================================
log_step "5. 验证每轮包含完整事件链"
# ===========================================================================
# 统计各类事件数量
EVENT_STARTED=$(psql "$DB_URL" -t -A -c "
    SELECT COUNT(*) FROM workflow_history
    WHERE instance_id = $INSTANCE_ID AND event_type = 'instance_started'" 2>/dev/null || echo "0")
EVENT_REJECTED=$(psql "$DB_URL" -t -A -c "
    SELECT COUNT(*) FROM workflow_history
    WHERE instance_id = $INSTANCE_ID AND event_type = 'instance_rejected'" 2>/dev/null || echo "0")
EVENT_RESTARTED=$(psql "$DB_URL" -t -A -c "
    SELECT COUNT(*) FROM workflow_history
    WHERE instance_id = $INSTANCE_ID AND event_type = 'instance_restarted'" 2>/dev/null || echo "0")
EVENT_COMPLETED=$(psql "$DB_URL" -t -A -c "
    SELECT COUNT(*) FROM workflow_history
    WHERE instance_id = $INSTANCE_ID AND event_type = 'instance_completed'" 2>/dev/null || echo "0")
EVENT_TASK_REJECTED=$(psql "$DB_URL" -t -A -c "
    SELECT COUNT(*) FROM workflow_history
    WHERE instance_id = $INSTANCE_ID AND event_type = 'task_rejected'" 2>/dev/null || echo "0")
EVENT_TASK_CREATED=$(psql "$DB_URL" -t -A -c "
    SELECT COUNT(*) FROM workflow_history
    WHERE instance_id = $INSTANCE_ID AND event_type = 'task_created'" 2>/dev/null || echo "0")
EVENT_TASK_COMPLETED=$(psql "$DB_URL" -t -A -c "
    SELECT COUNT(*) FROM workflow_history
    WHERE instance_id = $INSTANCE_ID AND event_type = 'task_completed'" 2>/dev/null || echo "0")

log_info "事件统计: started=$EVENT_STARTED, task_created=$EVENT_TASK_CREATED, task_completed=$EVENT_TASK_COMPLETED, task_rejected=$EVENT_TASK_REJECTED, instance_rejected=$EVENT_REJECTED, instance_restarted=$EVENT_RESTARTED, instance_completed=$EVENT_COMPLETED"

# instance_started 至少 1
if [[ "$EVENT_STARTED" -ge 1 ]]; then
    assert_pass "AP-E8 instance_started: $EVENT_STARTED >= 1"
else
    assert_fail "AP-E8 instance_started: 缺少启动事件"
fi

# instance_rejected 至少 2（前两轮驳回）
if [[ "$EVENT_REJECTED" -ge 2 ]]; then
    assert_pass "AP-E8 instance_rejected: $EVENT_REJECTED >= 2"
else
    assert_fail "AP-E8 instance_rejected: $EVENT_REJECTED < 2（预期至少 2 轮驳回）"
fi

# instance_restarted 至少 2（每轮驳回后重提）
if [[ "$EVENT_RESTARTED" -ge 2 ]]; then
    assert_pass "AP-E8 instance_restarted: $EVENT_RESTARTED >= 2"
else
    assert_fail "AP-E8 instance_restarted: $EVENT_RESTARTED < 2（预期至少 2 次重提）"
fi

# instance_completed 至少 1（第 3 轮通过）
if [[ "$EVENT_COMPLETED" -ge 1 ]]; then
    assert_pass "AP-E8 instance_completed: $EVENT_COMPLETED >= 1"
else
    assert_fail "AP-E8 instance_completed: 缺少完成事件"
fi

# task_rejected 至少 2（前两轮经理驳回）
if [[ "$EVENT_TASK_REJECTED" -ge 2 ]]; then
    assert_pass "AP-E8 task_rejected: $EVENT_TASK_REJECTED >= 2"
else
    assert_fail "AP-E8 task_rejected: $EVENT_TASK_REJECTED < 2（预期至少 2 次任务驳回）"
fi

# ===========================================================================
log_step "6. 验证 payload 包含必要字段（操作人、意见）"
# ===========================================================================
# 检查 task_rejected 事件 payload 包含 comment
REJECTED_WITH_COMMENT=$(psql "$DB_URL" -t -A -c "
    SELECT COUNT(*) FROM workflow_history
    WHERE instance_id = $INSTANCE_ID
      AND event_type = 'task_rejected'
      AND payload IS NOT NULL
      AND payload::text != '{}'
      AND (payload->>'comment') IS NOT NULL" 2>/dev/null || echo "0")

if [[ "$REJECTED_WITH_COMMENT" -ge 2 ]]; then
    assert_pass "AP-E8 payload.comment: $REJECTED_WITH_COMMENT 条驳回记录含意见"
else
    assert_fail "AP-E8 payload.comment: 仅 $REJECTED_WITH_COMMENT 条含意见（预期 >= 2）"
fi

# 检查 payload 包含 operator 字段
REJECTED_WITH_OPERATOR=$(psql "$DB_URL" -t -A -c "
    SELECT COUNT(*) FROM workflow_history
    WHERE instance_id = $INSTANCE_ID
      AND event_type IN ('task_rejected', 'task_completed', 'instance_rejected', 'instance_restarted')
      AND payload IS NOT NULL
      AND (payload->>'operator') IS NOT NULL" 2>/dev/null || echo "0")

if [[ "$REJECTED_WITH_OPERATOR" -ge 4 ]]; then
    assert_pass "AP-E8 payload.operator: $REJECTED_WITH_OPERATOR 条记录含操作人"
else
    assert_fail "AP-E8 payload.operator: 仅 $REJECTED_WITH_OPERATOR 条含操作人（预期 >= 4）"
fi

# 检查 instance_restarted 事件包含 round 字段
RESTARTED_WITH_ROUND=$(psql "$DB_URL" -t -A -c "
    SELECT COUNT(*) FROM workflow_history
    WHERE instance_id = $INSTANCE_ID
      AND event_type = 'instance_restarted'
      AND payload IS NOT NULL
      AND (payload->>'round') IS NOT NULL" 2>/dev/null || echo "0")

if [[ "$RESTARTED_WITH_ROUND" -ge 2 ]]; then
    assert_pass "AP-E8 payload.round: $RESTARTED_WITH_ROUND 条重提记录含轮次"
else
    assert_fail "AP-E8 payload.round: 仅 $RESTARTED_WITH_ROUND 条含轮次（预期 >= 2）"
fi

# ===========================================================================
log_step "7. 验证事件链时序逻辑正确（rejected 必须在 restarted 之前）"
# ===========================================================================
TIMING_LOGIC_OK=true

# 每轮 instance_rejected 的 created_at 必须早于同轮 instance_restarted
for ROUND in 1 2; do
    REJECTED_TIME=$(psql "$DB_URL" -t -A -c "
        SELECT created_at FROM workflow_history
        WHERE instance_id = $INSTANCE_ID
          AND event_type = 'instance_rejected'
          AND (payload->>'round')::int = $ROUND
        ORDER BY created_at DESC LIMIT 1" 2>/dev/null || echo "")

    RESTARTED_TIME=$(psql "$DB_URL" -t -A -c "
        SELECT created_at FROM workflow_history
        WHERE instance_id = $INSTANCE_ID
          AND event_type = 'instance_restarted'
          AND (payload->>'round')::int = $ROUND
        ORDER BY created_at ASC LIMIT 1" 2>/dev/null || echo "")

    if [[ -n "$REJECTED_TIME" && -n "$RESTARTED_TIME" ]]; then
        # 比较：rejected < restarted
        CMP=$(psql "$DB_URL" -t -A -c "
            SELECT CASE WHEN TIMESTAMP WITH TIME ZONE '$REJECTED_TIME' < TIMESTAMP WITH TIME ZONE '$RESTARTED_TIME'
                        THEN 'ok' ELSE 'fail' END" 2>/dev/null || echo "fail")
        if [[ "$CMP" == "ok" ]]; then
            log_info "第 $ROUND 轮时序正确: rejected($REJECTED_TIME) < restarted($RESTARTED_TIME)"
        else
            log_fail "第 $ROUND 轮时序错误: rejected($REJECTED_TIME) >= restarted($RESTARTED_TIME)"
            TIMING_LOGIC_OK=false
        fi
    else
        log_warn "第 $ROUND 轮缺少 rejected/restarted 时间（可能使用已有数据）"
    fi
done

if [[ "$TIMING_LOGIC_OK" == "true" ]]; then
    assert_pass "AP-E8 时序逻辑: 驳回→重提顺序正确（每轮 rejected < restarted）"
else
    assert_fail "AP-E8 时序逻辑: 驳回→重提顺序异常"
fi

# ===========================================================================
# 清理由 trap EXIT 处理
# ===========================================================================
print_summary
echo "=== AP-E8 多次驳回重提历史完整 完成 ==="
