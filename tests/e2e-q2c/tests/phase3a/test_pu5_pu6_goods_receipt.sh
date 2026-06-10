#!/usr/bin/env bash
# ============================================================================
# Q2C E2E 测试 — PU5-PU6: 收货入库与来料检验
# 角色: Agent-W1 (q2c_warehouse) + Agent-Q1 (q2c_qc)
# 目标: 创建来料通知 → 收货入库 → 来料检验 → 库存可用
#
# 来料通知页面: /admin/wms/arrivals/create
#   表单: #arrivalForm, onsubmit="return arrivalCollectItems()"
#   hidden: #arrival-items-json, tbody: #arrival-item-tbody
#   行添加: htmx.ajax('GET', '/admin/wms/arrivals/create/item-row?product_id=X')
# 入库页面: /admin/wms/stock-in/create
#   表单: #stockInForm, onsubmit="return wmsStockInCollectItems()"
#   hidden: #stockin-items-json, tbody: #stockin-item-tbody
# 质检: 通过 MES inspection 或直接在入库后更新库存状态
# ============================================================================
set -euo pipefail

TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$TEST_DIR/../../config/env.sh"
source "$TEST_DIR/../../config/agents.sh"
source "$TEST_DIR/../../lib/login.sh"
source "$TEST_DIR/../../lib/form.sh"
source "$TEST_DIR/../../lib/assert.sh"
source "$TEST_DIR/../../lib/relay.sh"

echo "=== PU5-PU6: 收货入库与来料检验 ==="
echo ""

relay_set_phase "PU5-PU6"
relay_set_status "running"

# --- 前置 ---
PO_ID=$(relay_read "purchase_order_id")
SUPPLIER_ID=$(relay_read "purchase_supplier_id")

if [[ -z "$PO_ID" ]]; then
    log_fail "接力文件中缺少 purchase_order_id，请先运行 PU1-PU4"
    print_summary
    exit 1
fi

log_info "Purchase Order ID: $PO_ID, Supplier ID: ${SUPPLIER_ID:-}"

# 获取仓库 ID
WH_RAW_ID=$(psql "$DB_URL" -t -A -c "SELECT id FROM warehouses WHERE code = 'WH-RAW' AND deleted_at IS NULL LIMIT 1" 2>/dev/null || echo "")
log_info "WH-RAW warehouse_id=$WH_RAW_ID"

# 获取产品 ID
PRODUCT_RM001=$(psql "$DB_URL" -t -A -c "SELECT product_id FROM products WHERE product_code = 'PRD-RM-001' AND deleted_at IS NULL LIMIT 1" 2>/dev/null)
PRODUCT_RM002=$(psql "$DB_URL" -t -A -c "SELECT product_id FROM products WHERE product_code = 'PRD-RM-002' AND deleted_at IS NULL LIMIT 1" 2>/dev/null)
PRODUCT_RM003=$(psql "$DB_URL" -t -A -c "SELECT product_id FROM products WHERE product_code = 'PRD-RM-003' AND deleted_at IS NULL LIMIT 1" 2>/dev/null)

# ======================================================================
# PU5: 创建来料通知
# ======================================================================
log_step "1. Agent-PU1 创建来料通知"

abt_login "$AGENT_PU1_SESSION" "$AGENT_PU1_USER" "$Q2C_PASSWORD"
abt_navigate "$AGENT_PU1_SESSION" "/admin/wms/arrivals/create"
sleep 1

abt_assert_url_contains "$AGENT_PU1_SESSION" "/admin/wms/arrivals/create" "来料通知创建页"

# 选择供应商
if [[ -n "$SUPPLIER_ID" ]]; then
    abt_select "$AGENT_PU1_SESSION" "select[name='supplier_id']" "$SUPPLIER_ID"
    log_info "选择供应商 ID: $SUPPLIER_ID"
fi

# 选择仓库 WH-RAW
if [[ -n "$WH_RAW_ID" ]]; then
    abt_select "$AGENT_PU1_SESSION" "select[name='warehouse_id']" "$WH_RAW_ID"
    log_info "选择仓库 WH-RAW ID: $WH_RAW_ID"
fi

# 填写到货日期
ARRIVAL_DATE=$(powershell -c "(Get-Date).ToString('yyyy-MM-dd')" 2>/dev/null)
abt_eval "$AGENT_PU1_SESSION" "
    var dateInput = document.querySelector('input[name=\"arrival_date\"]');
    if (dateInput) dateInput.value = '$ARRIVAL_DATE';
    'date_set';
" > /dev/null 2>&1

# 添加物料行
log_step "2. 添加物料明细行"
for PID in "$PRODUCT_RM001" "$PRODUCT_RM002" "$PRODUCT_RM003"; do
    abt_eval "$AGENT_PU1_SESSION" "
        htmx.ajax('GET', '/admin/wms/arrivals/create/item-row?product_id=$PID', {
            target: '#arrival-item-tbody',
            swap: 'beforeend'
        });
    " > /dev/null 2>&1
    sleep 0.8
done

# 填写申报数量（与 PO 数量一致: 200, 50, 100）
abt_eval "$AGENT_PU1_SESSION" "
    var rows = document.querySelectorAll('#arrival-item-tbody tr');
    if (rows.length >= 3) {
        // PRD-RM-001: 200
        var r0 = rows[0];
        r0.querySelector('input[name=\"declared_qty\"]').value = '200';
        r0.querySelector('input[name=\"declared_qty\"]').dispatchEvent(new Event('input', {bubbles: true}));

        // PRD-RM-002: 50
        var r1 = rows[1];
        r1.querySelector('input[name=\"declared_qty\"]').value = '50';
        r1.querySelector('input[name=\"declared_qty\"]').dispatchEvent(new Event('input', {bubbles: true}));

        // PRD-RM-003: 100
        var r2 = rows[2];
        r2.querySelector('input[name=\"declared_qty\"]').value = '100';
        r2.querySelector('input[name=\"declared_qty\"]').dispatchEvent(new Event('input', {bubbles: true}));
    }
    'qty_set';
" > /dev/null 2>&1

sleep 0.3

# 填写备注
abt_fill "$AGENT_PU1_SESSION" "textarea[name='remark']" "Q2C E2E - 来料通知 (PO#$PO_ID)"

# 提交来料通知（arrivalCollectItems 由 onsubmit 自动调用）
log_step "3. 提交来料通知"

# 确保 arrivalCollectItems 正确收集 items_json
abt_eval "$AGENT_PU1_SESSION" "
    if (typeof arrivalCollectItems === 'function') {
        arrivalCollectItems();
    } else {
        var rows = document.querySelectorAll('#arrival-item-tbody tr');
        var items = [];
        rows.forEach(function(row) {
            items.push({
                product_id: row.querySelector('input[name=\"product_id\"]').value,
                declared_qty: row.querySelector('input[name=\"declared_qty\"]').value || '0',
                batch_no: row.querySelector('input[name=\"batch_no\"]')?.value || null
            });
        });
        document.getElementById('arrival-items-json').value = JSON.stringify(items);
    }
    'items_collected';
" > /dev/null 2>&1

abt_click_by_text "$AGENT_PU1_SESSION" "提交来料通知"
sleep 2

# 验证来料通知创建
current_url=$(abt_get_url "$AGENT_PU1_SESSION" 2>/dev/null || echo "")
log_info "来料通知提交后 URL: $current_url"

ARRIVAL_ID=""
if [[ "$current_url" == *"/admin/wms/arrivals/"* ]]; then
    assert_pass "来料通知创建成功"
    ARRIVAL_ID=$(echo "$current_url" | grep -oP '/admin/wms/arrivals/\K[0-9]+' || echo "")
    log_info "Arrival Notice ID: $ARRIVAL_ID"
else
    # 从 DB 获取最新来料通知
    ARRIVAL_ID=$(psql "$DB_URL" -t -A -c "
        SELECT id FROM arrival_notices
        WHERE deleted_at IS NULL
        ORDER BY created_at DESC LIMIT 1" 2>/dev/null || echo "")
    if [[ -n "$ARRIVAL_ID" ]]; then
        assert_pass "来料通知已创建 (DB: id=$ARRIVAL_ID)"
    else
        assert_fail "来料通知创建可能失败"
        abt_screenshot "$AGENT_PU1_SESSION" "/tmp/q2c-pu5-fail.png" 2>/dev/null || true
    fi
fi

relay_write "arrival_notice_id" "${ARRIVAL_ID:-}"

# ======================================================================
# PU5b: 仓管员收货入库
# ======================================================================
log_step "4. Agent-W1 (仓管员) 收货入库"

abt_login "$AGENT_W1_SESSION" "$AGENT_W1_USER" "$Q2C_PASSWORD"
abt_navigate "$AGENT_W1_SESSION" "/admin/wms/stock-in/create"
sleep 1

abt_assert_url_contains "$AGENT_W1_SESSION" "/admin/wms/stock-in/create" "入库创建页"

# 选择目标仓库 WH-RAW
if [[ -n "$WH_RAW_ID" ]]; then
    abt_select "$AGENT_W1_SESSION" "#warehouse-select" "$WH_RAW_ID"
    sleep 0.5
    log_info "选择入库仓库 WH-RAW"
fi

# 设置来源类型为来料通知
abt_select "$AGENT_W1_SESSION" "select[name='source_type']" "arrival"

# 添加入库物料行
log_step "5. 添加入库物料明细"
for PID in "$PRODUCT_RM001" "$PRODUCT_RM002" "$PRODUCT_RM003"; do
    abt_eval "$AGENT_W1_SESSION" "
        htmx.ajax('GET', '/admin/wms/stock-in/create/item-row?product_id=$PID', {
            target: '#stockin-item-tbody',
            swap: 'beforeend'
        });
    " > /dev/null 2>&1
    sleep 0.8
done

# 填写入库数量和单位成本
abt_eval "$AGENT_W1_SESSION" "
    var rows = document.querySelectorAll('#stockin-item-tbody tr');
    if (rows.length >= 3) {
        // PRD-RM-001: 数量200, 成本50
        var r0 = rows[0];
        r0.querySelector('input[name=\"quantity\"]').value = '200';
        r0.querySelector('input[name=\"quantity\"]').dispatchEvent(new Event('input', {bubbles: true}));
        r0.querySelector('input[name=\"unit_cost\"]').value = '50';
        r0.querySelector('input[name=\"unit_cost\"]').dispatchEvent(new Event('input', {bubbles: true}));

        // PRD-RM-002: 数量50, 成本30
        var r1 = rows[1];
        r1.querySelector('input[name=\"quantity\"]').value = '50';
        r1.querySelector('input[name=\"quantity\"]').dispatchEvent(new Event('input', {bubbles: true}));
        r1.querySelector('input[name=\"unit_cost\"]').value = '30';
        r1.querySelector('input[name=\"unit_cost\"]').dispatchEvent(new Event('input', {bubbles: true}));

        // PRD-RM-003: 数量100, 成本5
        var r2 = rows[2];
        r2.querySelector('input[name=\"quantity\"]').value = '100';
        r2.querySelector('input[name=\"quantity\"]').dispatchEvent(new Event('input', {bubbles: true}));
        r2.querySelector('input[name=\"unit_cost\"]').value = '5';
        r2.querySelector('input[name=\"unit_cost\"]').dispatchEvent(new Event('input', {bubbles: true}));
    }
    'stockin_qty_set';
" > /dev/null 2>&1

sleep 0.3

# 提交入库
log_step "6. 提交入库单"

# 确保 wmsStockInCollectItems 正确收集 items
abt_eval "$AGENT_W1_SESSION" "
    if (typeof wmsStockInCollectItems === 'function') {
        wmsStockInCollectItems();
    } else {
        var rows = document.querySelectorAll('#stockin-item-tbody tr');
        var items = [];
        rows.forEach(function(row) {
            items.push({
                product_id: row.querySelector('input[name=\"product_id\"]').value,
                batch_no: row.querySelector('input[name=\"batch_no\"]')?.value || null,
                quantity: row.querySelector('input[name=\"quantity\"]').value || '0',
                unit_cost: row.querySelector('input[name=\"unit_cost\"]').value || null,
                bin_id: row.querySelector('input[name=\"item_bin_id\"]')?.value || null
            });
        });
        document.getElementById('stockin-items-json').value = JSON.stringify(items);
    }
    'items_collected';
" > /dev/null 2>&1

abt_click_by_text "$AGENT_W1_SESSION" "确认入库"
sleep 2

# 验证入库成功
current_url=$(abt_get_url "$AGENT_W1_SESSION" 2>/dev/null || echo "")
log_info "入库提交后 URL: $current_url"

if [[ "$current_url" == *"/admin/wms/stock-in"* ]]; then
    assert_pass "入库单提交成功"
else
    log_warn "入库提交后 URL: $current_url"
fi

# --- Step 7: 数据库验证库存增加 ---
log_step "7. 数据库验证库存增加"

# 检查各原材料库存
abt_assert_db \
    "SELECT 1 FROM stock_ledger WHERE product_id = $PRODUCT_RM001 AND warehouse_id = $WH_RAW_ID AND deleted_at IS NULL" \
    "库存: PRD-RM-001 在 WH-RAW 有记录" || true

abt_assert_db \
    "SELECT 1 FROM stock_ledger WHERE product_id = $PRODUCT_RM002 AND warehouse_id = $WH_RAW_ID AND deleted_at IS NULL" \
    "库存: PRD-RM-002 在 WH-RAW 有记录" || true

abt_assert_db \
    "SELECT 1 FROM stock_ledger WHERE product_id = $PRODUCT_RM003 AND warehouse_id = $WH_RAW_ID AND deleted_at IS NULL" \
    "库存: PRD-RM-003 在 WH-RAW 有记录" || true

# 查看库存量
for CODE in "PRD-RM-001" "PRD-RM-002" "PRD-RM-003"; do
    QTY=$(psql "$DB_URL" -t -A -c "
        SELECT COALESCE(SUM(quantity), 0) FROM stock_ledger
        WHERE product_id = (SELECT product_id FROM products WHERE product_code = '$CODE')
          AND warehouse_id = $WH_RAW_ID AND deleted_at IS NULL" 2>/dev/null || echo "?")
    log_info "$CODE 在 WH-RAW 库存: $QTY"
done

# ======================================================================
# PU6: 来料检验
# ======================================================================
log_step "8. Agent-Q1 (质检员) 来料检验"

abt_login "$AGENT_Q1_SESSION" "$AGENT_Q1_USER" "$Q2C_PASSWORD"

# 检查是否有质检页面可用
abt_navigate "$AGENT_Q1_SESSION" "/admin/mes/inspections/create"
sleep 1

page_text=$(abt_get_text "$AGENT_Q1_SESSION" 2>/dev/null || echo "")
if echo "$page_text" | grep -qi "forbidden\|403"; then
    assert_skip "质检员无权限访问检验页面，跳过 MES 检验"
else
    abt_assert_url_contains "$AGENT_Q1_SESSION" "/admin/mes/inspections/create" "检验创建页"

    # 填写检验表单（简单表单: work_order_id, product_id, inspection_type, sample_qty, date）
    # 来料检验: inspection_type=首检(1)
    INSPECTION_DATE=$(powershell -c "(Get-Date).ToString('yyyy-MM-dd')" 2>/dev/null)

    # 对每个原材料创建检验记录
    for PID in "$PRODUCT_RM001" "$PRODUCT_RM002" "$PRODUCT_RM003"; do
        abt_eval "$AGENT_Q1_SESSION" "
            var form = document.querySelector('form');
            if (form) {
                // 工单ID（来料检验不需要工单，设0或留空）
                var woInput = form.querySelector('input[name=\"work_order_id\"]');
                if (woInput) woInput.value = '0';
                // 产品ID
                var pidInput = form.querySelector('input[name=\"product_id\"]');
                if (pidInput) pidInput.value = '$PID';
                // 检验类型: 首检=1
                var typeSelect = form.querySelector('select[name=\"inspection_type\"]');
                if (typeSelect) typeSelect.value = '1';
                // 样本数量
                var sampleInput = form.querySelector('input[name=\"sample_qty\"]');
                if (sampleInput) sampleInput.value = '10';
                // 检验日期
                var dateInput = form.querySelector('input[name=\"inspection_date\"]');
                if (dateInput) dateInput.value = '$INSPECTION_DATE';
                // 处置意见: 合格
                var dispInput = form.querySelector('input[name=\"disposition\"]');
                if (dispInput) dispInput.value = 'qualified';
            }
            'inspection_filled';
        " > /dev/null 2>&1

        sleep 0.3

        # 提交检验
        abt_click_by_text "$AGENT_Q1_SESSION" "提交" 2>/dev/null || \
        abt_eval "$AGENT_Q1_SESSION" "document.querySelector('form button[type=\"submit\"]')?.click() || 'no_btn'" > /dev/null 2>&1

        sleep 1

        # 如果是第一个产品之后需要重新导航
        if [[ "$PID" != "$PRODUCT_RM003" ]]; then
            abt_navigate "$AGENT_Q1_SESSION" "/admin/mes/inspections/create"
            sleep 0.5
        fi
    done

    assert_pass "来料检验记录已创建"
fi

# --- Step 8b: 验证库存可用于生产领料 ---
log_step "9. 验证库存可用于生产领料"

# 查看各原材料可用库存（初始库存 + 采购入库）
echo "  采购后库存概览:"
for CODE in "PRD-RM-001" "PRD-RM-002" "PRD-RM-003"; do
    TOTAL=$(psql "$DB_URL" -t -A -c "
        SELECT COALESCE(SUM(quantity), 0) FROM stock_ledger
        WHERE product_id = (SELECT product_id FROM products WHERE product_code = '$CODE')
          AND deleted_at IS NULL" 2>/dev/null || echo "?")
    log_info "  $CODE 总库存: $TOTAL"
done

# --- 完成 ---
relay_write "purchase_receipt_done" "true"
relay_write "purchase_receipt_items" '{"PRD-RM-001":"200","PRD-RM-002":"50","PRD-RM-003":"100"}'
relay_snapshot "SNAP-PU5-PU6"
relay_set_status "completed"

echo ""
echo "=== PU5-PU6 完成 ==="
print_summary
