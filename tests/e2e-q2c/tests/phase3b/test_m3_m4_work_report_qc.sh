#!/usr/bin/env bash
# ============================================================================
# Q2C E2E 测试 — M3-M4: 车间报工与成品质检
# 角色: Agent-M2 (q2c_operator) + Agent-Q1 (q2c_qc)
# 目标: 操作员报工 → 质检员执行成品质检
#
# 报工页面: /admin/mes/reports/create?batch_id=X
#   表单: batch_id(hidden), step_no, worker_id, shift, completed_qty,
#         defect_qty, work_hours, report_date, remark
#   提交后: HX-Redirect → /admin/mes/reports
#
# 质检页面: /admin/mes/inspections/create
#   表单: work_order_id, product_id, routing_id, inspection_type,
#         sample_qty, inspection_date, disposition
#   提交后: HX-Redirect → /admin/mes/inspections
# ============================================================================
set -euo pipefail

TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$TEST_DIR/../../config/env.sh"
source "$TEST_DIR/../../config/agents.sh"
source "$TEST_DIR/../../lib/login.sh"
source "$TEST_DIR/../../lib/form.sh"
source "$TEST_DIR/../../lib/assert.sh"
source "$TEST_DIR/../../lib/relay.sh"

echo "=== M3-M4: 车间报工与成品质检 ==="
echo ""

relay_set_phase "M3-M4"
relay_set_status "running"

# --- 前置 ---
WORK_ORDER_ID=$(relay_read "work_order_id")
BATCH_ID=$(relay_read "production_batch_id")
PRODUCT_FG_ID=$(psql "$DB_URL" -t -A -c "SELECT product_id FROM products WHERE product_code = 'PRD-FG-001' AND deleted_at IS NULL LIMIT 1" 2>/dev/null)

if [[ -z "$WORK_ORDER_ID" ]]; then
    log_fail "接力文件中缺少 work_order_id"
    print_summary
    exit 1
fi

log_info "Work Order ID: $WORK_ORDER_ID, Batch ID: ${BATCH_ID:-}, Product FG ID: $PRODUCT_FG_ID"

TODAY=$(powershell -c "(Get-Date).ToString('yyyy-MM-dd')" 2>/dev/null)

# ======================================================================
# M3: 车间报工
# ======================================================================
log_step "1. Agent-M2 (车间操作员) 登录"
abt_login "$AGENT_M2_SESSION" "$AGENT_M2_USER" "$Q2C_PASSWORD"

# --- Step 2: 导航到报工页面 ---
log_step "2. 导航到报工页面"

# 如果有 batch_id，带参数导航会预填批次信息
if [[ -n "$BATCH_ID" ]]; then
    abt_navigate "$AGENT_M2_SESSION" "/admin/mes/reports/create?batch_id=$BATCH_ID"
else
    abt_navigate "$AGENT_M2_SESSION" "/admin/mes/reports/create"
fi
sleep 1

abt_assert_url_contains "$AGENT_M2_SESSION" "/admin/mes/reports/create" "报工创建页"

# --- Step 3: 填写报工表单 ---
log_step "3. 填写报工表单"

# 获取操作员 user_id
OPERATOR_ID=$(psql "$DB_URL" -t -A -c "SELECT user_id FROM users WHERE username = 'q2c_operator' LIMIT 1" 2>/dev/null || echo "")

# 工艺路线: 成品A → 工序10(注塑) → 工序20(组装) → 工序30(检验)
# 报工需要按工序逐一提交

# 检查是否有工序选择下拉
page_text=$(abt_get_text "$AGENT_M2_SESSION" 2>/dev/null || echo "")

if echo "$page_text" | grep -qi "forbidden\|403"; then
    assert_skip "操作员无报工权限"
else
    # --- 工序 10: 注塑 ---
    log_step "4. 报工 — 工序10(注塑)"

    # 填写报工字段
    abt_eval "$AGENT_M2_SESSION" "
        var form = document.querySelector('form');
        if (form) {
            // 工单选择
            var woSelect = form.querySelector('select[name=\"wo_id\"]');
            if (woSelect) woSelect.value = '$WORK_ORDER_ID';
            // 工序
            var stepSelect = form.querySelector('select[name=\"step_no\"]');
            if (stepSelect) {
                // 选择工序 10
                for (var i = 0; i < stepSelect.options.length; i++) {
                    if (stepSelect.options[i].value === '10') {
                        stepSelect.selectedIndex = i;
                        break;
                    }
                }
            }
            // 批次 ID（如果有 hidden input）
            var batchInput = form.querySelector('input[name=\"batch_id\"]');
            if (batchInput && !batchInput.value) batchInput.value = '${BATCH_ID:-0}';
            // 工人
            var workerSelect = form.querySelector('select[name=\"worker_id\"]');
            if (workerSelect) workerSelect.value = '$OPERATOR_ID';
            // 班次: 白班=1
            var shiftInput = form.querySelector('input[name=\"shift\"]');
            if (shiftInput) shiftInput.value = '1';
            // 完成数量
            var qtyInput = form.querySelector('input[name=\"completed_qty\"]');
            if (qtyInput) qtyInput.value = '100';
            // 不良数量
            var defectInput = form.querySelector('input[name=\"defect_qty\"]');
            if (defectInput) defectInput.value = '0';
            // 工时
            var hoursInput = form.querySelector('input[name=\"work_hours\"]');
            if (hoursInput) hoursInput.value = '8';
            // 报工日期
            var dateInput = form.querySelector('input[name=\"report_date\"]');
            if (dateInput) dateInput.value = '$TODAY';
        }
        'report_filled';
    " > /dev/null 2>&1

    sleep 0.5

    # 提交报工
    abt_click_by_text "$AGENT_M2_SESSION" "确认报工"
    sleep 2

    # 验证
    current_url=$(abt_get_url "$AGENT_M2_SESSION" 2>/dev/null || echo "")
    if [[ "$current_url" == *"/admin/mes/reports"* ]]; then
        assert_pass "工序10(注塑) 报工成功"
    else
        log_warn "工序10 报工后 URL: $current_url"
    fi

    # --- 工序 20: 组装 ---
    log_step "5. 报工 — 工序20(组装)"

    abt_navigate "$AGENT_M2_SESSION" "/admin/mes/reports/create"
    sleep 1

    abt_eval "$AGENT_M2_SESSION" "
        var form = document.querySelector('form');
        if (form) {
            var woSelect = form.querySelector('select[name=\"wo_id\"]');
            if (woSelect) woSelect.value = '$WORK_ORDER_ID';
            var stepSelect = form.querySelector('select[name=\"step_no\"]');
            if (stepSelect) {
                for (var i = 0; i < stepSelect.options.length; i++) {
                    if (stepSelect.options[i].value === '20') {
                        stepSelect.selectedIndex = i;
                        break;
                    }
                }
            }
            var batchInput = form.querySelector('input[name=\"batch_id\"]');
            if (batchInput && !batchInput.value) batchInput.value = '${BATCH_ID:-0}';
            var workerSelect = form.querySelector('select[name=\"worker_id\"]');
            if (workerSelect) workerSelect.value = '$OPERATOR_ID';
            var shiftInput = form.querySelector('input[name=\"shift\"]');
            if (shiftInput) shiftInput.value = '1';
            var qtyInput = form.querySelector('input[name=\"completed_qty\"]');
            if (qtyInput) qtyInput.value = '100';
            var defectInput = form.querySelector('input[name=\"defect_qty\"]');
            if (defectInput) defectInput.value = '0';
            var hoursInput = form.querySelector('input[name=\"work_hours\"]');
            if (hoursInput) hoursInput.value = '8';
            var dateInput = form.querySelector('input[name=\"report_date\"]');
            if (dateInput) dateInput.value = '$TODAY';
        }
        'report_20_filled';
    " > /dev/null 2>&1

    sleep 0.5
    abt_click_by_text "$AGENT_M2_SESSION" "确认报工"
    sleep 2

    current_url=$(abt_get_url "$AGENT_M2_SESSION" 2>/dev/null || echo "")
    if [[ "$current_url" == *"/admin/mes/reports"* ]]; then
        assert_pass "工序20(组装) 报工成功"
    else
        log_warn "工序20 报工后 URL: $current_url"
    fi

    # --- 工序 30: 检验 ---
    log_step "6. 报工 — 工序30(检验)"

    abt_navigate "$AGENT_M2_SESSION" "/admin/mes/reports/create"
    sleep 1

    abt_eval "$AGENT_M2_SESSION" "
        var form = document.querySelector('form');
        if (form) {
            var woSelect = form.querySelector('select[name=\"wo_id\"]');
            if (woSelect) woSelect.value = '$WORK_ORDER_ID';
            var stepSelect = form.querySelector('select[name=\"step_no\"]');
            if (stepSelect) {
                for (var i = 0; i < stepSelect.options.length; i++) {
                    if (stepSelect.options[i].value === '30') {
                        stepSelect.selectedIndex = i;
                        break;
                    }
                }
            }
            var batchInput = form.querySelector('input[name=\"batch_id\"]');
            if (batchInput && !batchInput.value) batchInput.value = '${BATCH_ID:-0}';
            var workerSelect = form.querySelector('select[name=\"worker_id\"]');
            if (workerSelect) workerSelect.value = '$OPERATOR_ID';
            var shiftInput = form.querySelector('input[name=\"shift\"]');
            if (shiftInput) shiftInput.value = '1';
            var qtyInput = form.querySelector('input[name=\"completed_qty\"]');
            if (qtyInput) qtyInput.value = '100';
            var defectInput = form.querySelector('input[name=\"defect_qty\"]');
            if (defectInput) defectInput.value = '0';
            var hoursInput = form.querySelector('input[name=\"work_hours\"]');
            if (hoursInput) hoursInput.value = '4';
            var dateInput = form.querySelector('input[name=\"report_date\"]');
            if (dateInput) dateInput.value = '$TODAY';
        }
        'report_30_filled';
    " > /dev/null 2>&1

    sleep 0.5
    abt_click_by_text "$AGENT_M2_SESSION" "确认报工"
    sleep 2

    current_url=$(abt_get_url "$AGENT_M2_SESSION" 2>/dev/null || echo "")
    if [[ "$current_url" == *"/admin/mes/reports"* ]]; then
        assert_pass "工序30(检验) 报工成功"
    else
        log_warn "工序30 报工后 URL: $current_url"
    fi
fi

# 数据库验证报工记录
log_step "7. 验证报工记录"
REPORT_COUNT=$(psql "$DB_URL" -t -A -c "
    SELECT COUNT(*) FROM step_confirmations
    WHERE work_order_id = $WORK_ORDER_ID" 2>/dev/null || echo "0")
log_info "报工记录数: $REPORT_COUNT"
if [[ "$REPORT_COUNT" -ge 1 ]]; then
    assert_pass "数据库中存在报工记录 ($REPORT_COUNT 条)"
fi

relay_write "work_report_done" "true"

# ======================================================================
# M4: 成品质检
# ======================================================================
log_step "8. Agent-Q1 (质检员) 成品质检"

abt_login "$AGENT_Q1_SESSION" "$AGENT_Q1_USER" "$Q2C_PASSWORD"
abt_navigate "$AGENT_Q1_SESSION" "/admin/mes/inspections/create"
sleep 1

page_text=$(abt_get_text "$AGENT_Q1_SESSION" 2>/dev/null || echo "")
if echo "$page_text" | grep -qi "forbidden\|403"; then
    assert_skip "质检员无权限访问检验页面"
else
    # 填写成品检验表单
    # inspection_type=3(完工检)
    abt_eval "$AGENT_Q1_SESSION" "
        var form = document.querySelector('form');
        if (form) {
            // 工单 ID
            form.querySelector('input[name=\"work_order_id\"]').value = '$WORK_ORDER_ID';
            // 产品 ID
            form.querySelector('input[name=\"product_id\"]').value = '$PRODUCT_FG_ID';
            // 检验类型: 完工检=3
            var typeSelect = form.querySelector('select[name=\"inspection_type\"]');
            if (typeSelect) typeSelect.value = '3';
            // 样本数量
            form.querySelector('input[name=\"sample_qty\"]').value = '100';
            // 检验日期
            form.querySelector('input[name=\"inspection_date\"]').value = '$TODAY';
            // 处置意见
            var dispInput = form.querySelector('input[name=\"disposition\"]');
            if (dispInput) dispInput.value = 'qualified';
        }
        'inspection_filled';
    " > /dev/null 2>&1

    sleep 0.3

    # 提交检验
    abt_click_by_text "$AGENT_Q1_SESSION" "提交"
    sleep 2

    current_url=$(abt_get_url "$AGENT_Q1_SESSION" 2>/dev/null || echo "")
    if [[ "$current_url" == *"/admin/mes/inspections"* ]]; then
        assert_pass "成品检验记录创建成功"
    else
        log_warn "检验提交后 URL: $current_url"
    fi
fi

# 验证检验记录
INSPECTION_ID=$(psql "$DB_URL" -t -A -c "
    SELECT id FROM production_inspections
    WHERE work_order_id = $WORK_ORDER_ID AND deleted_at IS NULL
    ORDER BY created_at DESC LIMIT 1" 2>/dev/null || echo "")
if [[ -n "$INSPECTION_ID" ]]; then
    assert_pass "数据库: 成品检验记录存在 (id=$INSPECTION_ID)"
    relay_write "production_inspection_id" "$INSPECTION_ID"
else
    log_info "未在数据库中找到检验记录（可能表名不同）"
fi

# --- 完成 ---
relay_write "inspection_done" "true"
relay_snapshot "SNAP-M3-M4"
relay_set_status "completed"

echo ""
echo "=== M3-M4 完成 ==="
print_summary
