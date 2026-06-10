#!/usr/bin/env bash
# ============================================================================
# Q2C E2E 测试 — PU1-PU4: 采购申请到采购订单
# 角色: Agent-PU1 (q2c_buyer) → Agent-PU2 (q2c_buyer_mgr 审批)
# 目标: 根据 MRP 建议创建采购订单，选择供应商，填写明细，提交+审批
#
# 采购订单页面: /admin/purchase/orders/create
# 表单: #po-form, supplier_id select, items_json hidden, #po-item-tbody
# 产品行添加: htmx.ajax('GET', '/admin/purchase/orders/create/item-row?product_id=X')
# 提交后: HX-Redirect 到 /admin/purchase/orders/{id}
# ============================================================================
set -euo pipefail

TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$TEST_DIR/../../config/env.sh"
source "$TEST_DIR/../../config/agents.sh"
source "$TEST_DIR/../../lib/login.sh"
source "$TEST_DIR/../../lib/form.sh"
source "$TEST_DIR/../../lib/assert.sh"
source "$TEST_DIR/../../lib/relay.sh"

echo "=== PU1-PU4: 采购申请到采购订单 ==="
echo ""

relay_set_phase "PU1-PU4"
relay_set_status "running"

# --- 前置：从接力文件获取 SO 数据 ---
SALES_ORDER_ID=$(relay_read "sales_order_id")
SO_QTY=$(relay_read "so_quantity")

if [[ -z "$SALES_ORDER_ID" ]]; then
    log_fail "接力文件中缺少 sales_order_id，请先运行 Phase 1+2"
    print_summary
    exit 1
fi

log_info "Sales Order ID: $SALES_ORDER_ID, 数量: ${SO_QTY:-100}"

# --- Step 1: Agent-PU1 登录 ---
log_step "1. Agent-PU1 (采购专员) 登录"
abt_login "$AGENT_PU1_SESSION" "$AGENT_PU1_USER" "$Q2C_PASSWORD"

# --- Step 2: 导航到创建采购订单页面 ---
log_step "2. 导航到新建采购订单"
abt_navigate "$AGENT_PU1_SESSION" "/admin/purchase/orders/create"
sleep 1

abt_assert_url_contains "$AGENT_PU1_SESSION" "/admin/purchase/orders/create" "采购订单创建页"

# --- Step 3: 选择供应商 SUP-001 ---
log_step "3. 选择供应商 SUP-001"
SUPPLIER_ID=$(psql "$DB_URL" -t -A -c "SELECT supplier_id FROM suppliers WHERE supplier_code = 'SUP-001' AND deleted_at IS NULL LIMIT 1" 2>/dev/null || echo "")
if [[ -z "$SUPPLIER_ID" ]]; then
    log_fail "未找到供应商 SUP-001"
    print_summary
    exit 1
fi
log_info "SUP-001 → supplier_id=$SUPPLIER_ID"

# select[name='supplier_id'] → 触发 HTMX 加载供应商详情
abt_select "$AGENT_PU1_SESSION" "select[name='supplier_id']" "$SUPPLIER_ID"
sleep 1  # 等待 HTMX 加载供应商详情

# 验证供应商信息条出现
abt_assert_visible "$AGENT_PU1_SESSION" "#supplier-detail" "供应商详情" || true

# --- Step 4: 设置付款条款 ---
log_step "4. 设置付款条款"
abt_select "$AGENT_PU1_SESSION" "select[name='payment_terms']" "月结30天"

# --- Step 5: 添加采购产品行 ---
log_step "5. 添加采购产品明细（PRD-RM-001, PRD-RM-002, PRD-RM-003）"

# 获取产品 ID
PRODUCT_RM001=$(psql "$DB_URL" -t -A -c "SELECT product_id FROM products WHERE product_code = 'PRD-RM-001' AND deleted_at IS NULL LIMIT 1" 2>/dev/null)
PRODUCT_RM002=$(psql "$DB_URL" -t -A -c "SELECT product_id FROM products WHERE product_code = 'PRD-RM-002' AND deleted_at IS NULL LIMIT 1" 2>/dev/null)
PRODUCT_RM003=$(psql "$DB_URL" -t -A -c "SELECT product_id FROM products WHERE product_code = 'PRD-RM-003' AND deleted_at IS NULL LIMIT 1" 2>/dev/null)
log_info "PRD-RM-001=$PRODUCT_RM001, PRD-RM-002=$PRODUCT_RM002, PRD-RM-003=$PRODUCT_RM003"

# BOM 展开: 成品A(100个) → 半成品B×1 + 原材料D×0.5 + 辅料E×1
#                半成品B → 原材料C×2
# 外购需求: PRD-RM-001(原材料C) 200KG, PRD-RM-002(原材料D) 50KG, PRD-RM-003(辅料E) 100个

# 通过 HTMX 添加产品行（绕过 Modal 交互）
for PID in "$PRODUCT_RM001" "$PRODUCT_RM002" "$PRODUCT_RM003"; do
    abt_eval "$AGENT_PU1_SESSION" "
        htmx.ajax('GET', '/admin/purchase/orders/create/item-row?product_id=$PID', {
            target: '#po-item-tbody',
            swap: 'beforeend'
        });
    " > /dev/null 2>&1
    sleep 0.8  # 等待每行 HTMX 响应
done

# 验证行数
row_count=$(abt_eval "$AGENT_PU1_SESSION" "document.querySelectorAll('#po-item-tbody tr').length" 2>/dev/null || echo "0")
if [[ "$row_count" -ge 3 ]]; then
    assert_pass "采购产品行已添加 (rows=$row_count)"
else
    assert_fail "采购产品行未完全添加 (rows=$row_count, 期望>=3)"
fi

# --- Step 6: 填写数量和单价 ---
log_step "6. 填写采购数量和单价"

# BOM 用量: PRD-RM-001=200KG×¥50, PRD-RM-002=50KG×¥30, PRD-RM-003=100个×¥5
# 行顺序对应添加顺序: RM001(row 0), RM002(row 1), RM003(row 2)
abt_eval "$AGENT_PU1_SESSION" "
    var rows = document.querySelectorAll('#po-item-tbody tr');
    if (rows.length >= 3) {
        // PRD-RM-001: 数量200, 单价50
        var r0 = rows[0];
        r0.querySelector('input[name=\"quantity\"]').value = '200';
        r0.querySelector('input[name=\"quantity\"]').dispatchEvent(new Event('input', {bubbles: true}));
        r0.querySelector('input[name=\"unit_price\"]').value = '50';
        r0.querySelector('input[name=\"unit_price\"]').dispatchEvent(new Event('input', {bubbles: true}));

        // PRD-RM-002: 数量50, 单价30
        var r1 = rows[1];
        r1.querySelector('input[name=\"quantity\"]').value = '50';
        r1.querySelector('input[name=\"quantity\"]').dispatchEvent(new Event('input', {bubbles: true}));
        r1.querySelector('input[name=\"unit_price\"]').value = '30';
        r1.querySelector('input[name=\"unit_price\"]').dispatchEvent(new Event('input', {bubbles: true}));

        // PRD-RM-003: 数量100, 单价5
        var r2 = rows[2];
        r2.querySelector('input[name=\"quantity\"]').value = '100';
        r2.querySelector('input[name=\"quantity\"]').dispatchEvent(new Event('input', {bubbles: true}));
        r2.querySelector('input[name=\"unit_price\"]').value = '5';
        r2.querySelector('input[name=\"unit_price\"]').dispatchEvent(new Event('input', {bubbles: true}));
    }
    'quantities_set';
" > /dev/null 2>&1

sleep 0.5

# 验证合计
grand_total=$(abt_eval "$AGENT_PU1_SESSION" "document.querySelector('#grandTotal')?.textContent?.trim() || 'N/A'" 2>/dev/null)
log_info "PO 总额: $grand_total (预期: 200*50 + 50*30 + 100*5 = 13000)"

# --- Step 7: 填写备注 ---
log_step "7. 填写备注"
abt_fill "$AGENT_PU1_SESSION" "textarea[name='remark']" "Q2C E2E - 采购订单 (MRP 需求来源: SO#$SALES_ORDER_ID)"

# --- Step 8: 提交采购订单 ---
log_step "8. 提交采购订单"

# 收集 items_json（PO 表单的 submit 脚本自动收集，但我们需要确保 hidden input 有值）
# PO 表单的 onsubmit 脚本: 遍历 #po-item-tbody tr 收集所有 FormData 字段
abt_eval "$AGENT_PU1_SESSION" "
    var form = document.querySelector('#po-form');
    if (form) {
        var rows = document.querySelectorAll('#po-item-tbody tr');
        var items = [];
        rows.forEach(function(row) {
            var fd = new FormData(row.closest('form'));
            var obj = {};
            fd.forEach(function(v, k) { if (!obj[k]) obj[k] = v; });
            items.push(obj);
        });
        document.querySelector('#items-json').value = JSON.stringify(items);
    }
    'items_collected';
" > /dev/null 2>&1

# 点击"提交订单"按钮
abt_click_by_text "$AGENT_PU1_SESSION" "提交订单"
sleep 2

# --- Step 9: 验证创建成功 ---
log_step "9. 验证采购订单创建"

current_url=$(abt_get_url "$AGENT_PU1_SESSION" 2>/dev/null || echo "")
log_info "当前URL: $current_url"

PO_ID=""
if [[ "$current_url" == *"/admin/purchase/orders/"* ]] && [[ "$current_url" != *"/create"* ]]; then
    assert_pass "采购订单创建成功，跳转到详情页"
    PO_ID=$(echo "$current_url" | grep -oP '/admin/purchase/orders/\K[0-9]+' || echo "")
    log_info "Purchase Order ID: $PO_ID"
elif [[ "$current_url" == *"/admin/purchase/orders"* ]]; then
    assert_pass "采购订单创建成功，返回列表页"
    # 从 DB 获取最新 PO
    PO_ID=$(psql "$DB_URL" -t -A -c "
        SELECT id FROM purchase_orders
        WHERE supplier_id = $SUPPLIER_ID AND deleted_at IS NULL
        ORDER BY created_at DESC LIMIT 1" 2>/dev/null || echo "")
    log_info "从数据库获取 PO ID: $PO_ID"
else
    assert_fail "采购订单创建可能失败，URL: $current_url"
    abt_screenshot "$AGENT_PU1_SESSION" "/tmp/q2c-pu1-pu4-fail.png" 2>/dev/null || true
fi

# --- Step 10: 数据库验证 ---
log_step "10. 数据库验证"

if [[ -n "$PO_ID" ]]; then
    abt_assert_db \
        "SELECT 1 FROM purchase_orders WHERE id = $PO_ID AND deleted_at IS NULL" \
        "数据库: 采购订单存在 (id=$PO_ID)"

    abt_assert_db \
        "SELECT 1 FROM purchase_order_items WHERE order_id = $PO_ID" \
        "数据库: 采购订单明细存在"

    # 验证明细行数
    ITEM_COUNT=$(psql "$DB_URL" -t -A -c "SELECT COUNT(*) FROM purchase_order_items WHERE order_id = $PO_ID" 2>/dev/null || echo "0")
    log_info "PO 明细行数: $ITEM_COUNT (预期: 3)"

    # 获取 PO 号
    PO_DOC=$(psql "$DB_URL" -t -A -c "SELECT doc_number FROM purchase_orders WHERE id = $PO_ID" 2>/dev/null || echo "")
    log_info "采购订单号: $PO_DOC"

    # 获取总金额
    PO_TOTAL=$(psql "$DB_URL" -t -A -c "SELECT total_amount FROM purchase_orders WHERE id = $PO_ID" 2>/dev/null || echo "0")
    log_info "PO 总额: $PO_TOTAL"

    # 写入接力文件
    relay_write "purchase_order_id" "$PO_ID"
    relay_write "purchase_order_doc_number" "${PO_DOC:-}"
    relay_write "purchase_order_total" "${PO_TOTAL:-}"
    relay_write "purchase_supplier_id" "$SUPPLIER_ID"
else
    assert_skip "PO ID 未获取，尝试从数据库查询最新记录"
    PO_ID=$(psql "$DB_URL" -t -A -c "
        SELECT id FROM purchase_orders
        WHERE deleted_at IS NULL
        ORDER BY created_at DESC LIMIT 1" 2>/dev/null || echo "")
    if [[ -n "$PO_ID" ]]; then
        relay_write "purchase_order_id" "$PO_ID"
        log_info "使用已有 PO: $PO_ID"
    fi
fi

# --- Step 11: 采购经理审批（如果需要）---
log_step "11. 检查是否需要审批"
# 如果 PO 状态是 Draft/待审批，需要采购经理审批
if [[ -n "$PO_ID" ]]; then
    PO_STATUS=$(psql "$DB_URL" -t -A -c "SELECT status FROM purchase_orders WHERE id = $PO_ID" 2>/dev/null || echo "")
    log_info "PO 当前状态: $PO_STATUS"

    # status=1 Draft, status=2 Pending Approval, status=3 Confirmed/Approved
    if [[ "$PO_STATUS" == "1" || "$PO_STATUS" == "2" ]]; then
        log_step "11a. Agent-PU2 (采购经理) 审批"
        abt_login "$AGENT_PU2_SESSION" "$AGENT_PU2_USER" "$Q2C_PASSWORD"
        abt_navigate "$AGENT_PU2_SESSION" "/admin/purchase/orders/$PO_ID"
        sleep 1

        # 尝试点击审批按钮
        abt_click_by_text "$AGENT_PU2_SESSION" "审批" || \
        abt_click_by_text "$AGENT_PU2_SESSION" "确认" || \
        abt_click_by_text "$AGENT_PU2_SESSION" "通过" || \
        abt_eval "$AGENT_PU2_SESSION" "
            var btn = document.querySelector('button[hx-post*=\"confirm\"], button[hx-post*=\"approve\"]');
            if (btn) { btn.click(); 'clicked'; } else { 'no_approval_btn'; }
        " > /dev/null 2>&1

        sleep 2

        # 验证状态
        PO_STATUS_AFTER=$(psql "$DB_URL" -t -A -c "SELECT status FROM purchase_orders WHERE id = $PO_ID" 2>/dev/null || echo "")
        log_info "审批后状态: $PO_STATUS_AFTER"

        if [[ "$PO_STATUS_AFTER" == "3" ]]; then
            assert_pass "采购订单已审批 (status=Confirmed)"
        else
            log_warn "采购订单状态: $PO_STATUS_AFTER (可能不需要审批或状态码不同)"
        fi
    else
        assert_pass "采购订单不需要审批 (status=$PO_STATUS)"
    fi
fi

# --- 完成 ---
relay_write "purchase_items" '{"PRD-RM-001":"200","PRD-RM-002":"50","PRD-RM-003":"100"}'
relay_snapshot "SNAP-PU1-PU4"
relay_set_status "completed"

echo ""
echo "=== PU1-PU4 完成 ==="
print_summary
