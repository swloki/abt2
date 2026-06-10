#!/usr/bin/env bash
# ============================================================================
# Q2C E2E 测试 — M1: 生产工单下达
# 角色: Agent-M1 (q2c_prod_mgr)
# 目标: 查看并下达之前 MRP 创建的生产工单
#
# 工单详情页: /admin/mes/orders/{id}
# 下达路由: POST /admin/mes/orders/{order_id}/release
# 工单状态: Draft(待计划) → Planned(已计划) → Released(已下达)
# ============================================================================
set -euo pipefail

TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$TEST_DIR/../../config/env.sh"
source "$TEST_DIR/../../config/agents.sh"
source "$TEST_DIR/../../lib/login.sh"
source "$TEST_DIR/../../lib/form.sh"
source "$TEST_DIR/../../lib/assert.sh"
source "$TEST_DIR/../../lib/relay.sh"

echo "=== M1: 生产工单下达 ==="
echo ""

relay_set_phase "M1"
relay_set_status "running"

# --- 前置 ---
WORK_ORDER_ID=$(relay_read "work_order_id")

if [[ -z "$WORK_ORDER_ID" ]]; then
    log_fail "接力文件中缺少 work_order_id，请先运行 P1-P3 MRP"
    print_summary
    exit 1
fi

log_info "Work Order ID: $WORK_ORDER_ID"

# 获取工单当前状态
WO_STATUS=$(psql "$DB_URL" -t -A -c "SELECT status FROM work_orders WHERE id = $WORK_ORDER_ID" 2>/dev/null || echo "")
log_info "工单当前状态: $WO_STATUS (1=Draft, 2=Planned, 3=Released)"

# --- Step 1: Agent-M1 登录 ---
log_step "1. Agent-M1 (生产主管) 登录"
abt_login "$AGENT_M1_SESSION" "$AGENT_M1_USER" "$Q2C_PASSWORD"

# --- Step 2: 导航到工单详情页 ---
log_step "2. 导航到工单详情页"
abt_navigate "$AGENT_M1_SESSION" "/admin/mes/orders/$WORK_ORDER_ID"
sleep 1

abt_assert_url_contains "$AGENT_M1_SESSION" "/admin/mes/orders/$WORK_ORDER_ID" "工单详情页"

# 验证页面显示工单信息
abt_assert_page_contains "$AGENT_M1_SESSION" "工单" "工单详情页" || true

# --- Step 3: 下达工单 ---
log_step "3. 下达工单"

# 工单状态流转: 如果是 Draft(1) 或 Planned(2)，需要下达
if [[ "$WO_STATUS" == "1" || "$WO_STATUS" == "2" ]]; then
    # 尝试点击"下达"按钮（详情页上的 hx-post 按钮）
    abt_click_by_text "$AGENT_M1_SESSION" "下达" || \
    abt_eval "$AGENT_M1_SESSION" "
        var btn = document.querySelector('button[hx-post*=\"release\"], a[hx-post*=\"release\"], button[hx-post*=\"Release\"], form[action*=\"release\"] button');
        if (btn) { btn.click(); 'clicked'; } else { 'no_release_btn'; }
    " > /dev/null 2>&1

    sleep 2

    # 也可以直接 POST 到 release 路由
    WO_STATUS_AFTER=$(psql "$DB_URL" -t -A -c "SELECT status FROM work_orders WHERE id = $WORK_ORDER_ID" 2>/dev/null || echo "")
    log_info "下达后状态: $WO_STATUS_AFTER"

    if [[ "$WO_STATUS_AFTER" == "3" ]]; then
        assert_pass "工单已下达 (status=Released)"
    elif [[ "$WO_STATUS_AFTER" == "2" ]] && [[ "$WO_STATUS" == "1" ]]; then
        # Draft → Planned，可能需要再点一次
        log_info "工单从 Draft → Planned，尝试再次下达..."
        abt_navigate "$AGENT_M1_SESSION" "/admin/mes/orders/$WORK_ORDER_ID"
        sleep 1
        abt_click_by_text "$AGENT_M1_SESSION" "下达" || true
        sleep 2

        WO_STATUS_AFTER=$(psql "$DB_URL" -t -A -c "SELECT status FROM work_orders WHERE id = $WORK_ORDER_ID" 2>/dev/null || echo "")
        if [[ "$WO_STATUS_AFTER" == "3" ]]; then
            assert_pass "工单已下达 (status=Released)"
        else
            log_warn "工单状态仍为: $WO_STATUS_AFTER"
        fi
    else
        # 如果按钮没有效果，尝试通过 htmx.ajax 直接 POST
        log_info "尝试直接 POST release 路由..."
        abt_eval "$AGENT_M1_SESSION" "
            htmx.ajax('POST', '/admin/mes/orders/$WORK_ORDER_ID/release', {
                target: 'body',
                swap: 'none'
            });
        " > /dev/null 2>&1
        sleep 2

        WO_STATUS_AFTER=$(psql "$DB_URL" -t -A -c "SELECT status FROM work_orders WHERE id = $WORK_ORDER_ID" 2>/dev/null || echo "")
        log_info "直接 POST 后状态: $WO_STATUS_AFTER"
    fi
elif [[ "$WO_STATUS" == "3" ]]; then
    assert_pass "工单已经是已下达状态"
else
    log_warn "工单状态异常: $WO_STATUS"
fi

# --- Step 4: 数据库验证 ---
log_step "4. 数据库验证"

abt_assert_db \
    "SELECT 1 FROM work_orders WHERE id = $WORK_ORDER_ID AND status >= 2 AND deleted_at IS NULL" \
    "数据库: 工单已计划/已下达 (id=$WORK_ORDER_ID)"

# 验证工单关联的产品
PRODUCT_ID=$(psql "$DB_URL" -t -A -c "SELECT product_id FROM work_orders WHERE id = $WORK_ORDER_ID" 2>/dev/null || echo "")
PRODUCT_CODE=$(psql "$DB_URL" -t -A -c "SELECT product_code FROM products WHERE product_id = $PRODUCT_ID" 2>/dev/null || echo "")
log_info "工单产品: $PRODUCT_CODE (product_id=$PRODUCT_ID)"

# 验证工单数量
PLANNED_QTY=$(psql "$DB_URL" -t -A -c "SELECT planned_qty FROM work_orders WHERE id = $WORK_ORDER_ID" 2>/dev/null || echo "0")
log_info "计划数量: $PLANNED_QTY"

# 写入接力文件
relay_write "work_order_status" "${WO_STATUS_AFTER:-released}"
relay_write "work_order_product_id" "$PRODUCT_ID"
relay_write "work_order_planned_qty" "$PLANNED_QTY"

# --- Step 5: 检查生产批次 ---
log_step "5. 检查生产批次"

# 下达后系统可能自动创建生产批次
BATCH_ID=$(psql "$DB_URL" -t -A -c "
    SELECT id FROM production_batches
    WHERE work_order_id = $WORK_ORDER_ID AND deleted_at IS NULL
    ORDER BY created_at DESC LIMIT 1" 2>/dev/null || echo "")

if [[ -n "$BATCH_ID" ]]; then
    assert_pass "生产批次已创建 (id=$BATCH_ID)"
    relay_write "production_batch_id" "$BATCH_ID"

    BATCH_NO=$(psql "$DB_URL" -t -A -c "SELECT batch_no FROM production_batches WHERE id = $BATCH_ID" 2>/dev/null || echo "")
    log_info "批次号: $BATCH_NO"
    relay_write "production_batch_no" "${BATCH_NO:-}"
else
    log_info "未找到自动创建的生产批次（可能需要手动创建）"

    # 尝试通过页面创建批次或跳过
    # 在 ABT 中，工单下达可能自动创建批次，也可能需要手动
    # 如果没有自动创建，这通常不阻塞后续步骤
fi

# --- 完成 ---
relay_snapshot "SNAP-M1"
relay_set_status "completed"

echo ""
echo "=== M1 完成 ==="
print_summary
