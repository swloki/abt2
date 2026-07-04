/* shipping-create.js — 发货申请交互逻辑（新建页 + 草稿编辑页共用）
 *
 * 服务端渲染明细行（新建页 get_order_items HTMX 端点 / 草稿编辑页 SSR）+ 客户/订单/hint
 * 联动 + 收集提交。对齐 wms-stock-in-create.js 范式：行校验/汇总/收集走 form onsubmit。
 */

// 客户下拉变化：切订单按钮可用态 + 清空已选订单/明细。
// 订单搜索结果改由 #shipping-order-results 的「orderPickerOpened from:body」事件在打开 picker 时刷新，
// 故此处不再联动 hidden customer_id、不再 htmx.trigger。
function wmsShippingOnCustomerChange() {
  var sel = document.getElementById('shipping-customer-select');
  var hasCustomer = !!(sel && sel.value);
  var btn = document.getElementById('order-picker-btn');
  if (btn) btn.disabled = !hasCustomer;
  var orderId = document.getElementById('shipping-order-id-input');
  if (orderId) orderId.value = '';
  var tbody = document.getElementById('shipping-items-tbody');
  if (tbody) tbody.innerHTML = '';
  var emptyHint = document.getElementById('shipping-empty-hint');
  if (emptyHint) emptyHint.style.display = '';
  var hint = document.getElementById('order-selected-hint');
  if (hint) hint.textContent = hasCustomer ? '未选择销售订单' : '请先选择客户';
  wmsShippingCalcSummary();
}

// 单行发货数量校验：不超过待发余量
function wmsShippingValidateRow(input) {
  var pending = parseFloat(input.dataset.pending);
  var qty = parseFloat(input.value);
  if (isNaN(pending)) return true;
  if (!isNaN(qty) && qty > pending) {
    input.style.borderColor = 'var(--danger, #f53f3f)';
    return false;
  }
  input.style.borderColor = '';
  return true;
}

function wmsShippingValidateAll() {
  var ok = true;
  document.querySelectorAll('.shipping-item-row input[name="requested_qty"][data-pending]').forEach(function (el) {
    if (!wmsShippingValidateRow(el)) ok = false;
  });
  return ok;
}

// 汇总：刷新「共 N 项 · M 件」
function wmsShippingCalcSummary() {
  var rows = document.querySelectorAll('.shipping-item-row');
  var totalQty = 0;
  rows.forEach(function (row) {
    var q = row.querySelector('input[name="requested_qty"]');
    if (q && q.value) totalQty += parseFloat(q.value) || 0;
  });
  var ic = document.getElementById('shipping-item-count');
  if (ic) ic.textContent = rows.length > 0
    ? ('共 ' + rows.length + ' 项 · ' + totalQty + ' 件')
    : '共 0 项';
}

// 收集明细行 → items_json；提交前校验
function wmsShippingCollectItems() {
  if (!wmsShippingValidateAll()) {
    alert('存在发货数量超过待发余量的明细，请检查标红的行');
    return false;
  }
  var rows = document.querySelectorAll('.shipping-item-row');
  var items = [];
  var missingWh = false;
  rows.forEach(function (row) {
    var oit = row.querySelector('input[name="order_item_id"]');
    var wh = row.querySelector('select[name="warehouse_id"]');
    var qty = row.querySelector('input[name="requested_qty"]');
    if (!oit || !qty) return;
    if (!qty.value || parseFloat(qty.value) <= 0) return; // 跳过未填数量的行
    if (!wh || !wh.value) { missingWh = true; return; }
    items.push({
      order_item_id: parseInt(oit.value) || 0,
      warehouse_id: parseInt(wh.value) || 0,
      requested_qty: qty.value
    });
  });
  if (missingWh) {
    alert('每条发货明细都需要选择发货仓库');
    return false;
  }
  var hidden = document.getElementById('shipping-items-json');
  if (hidden) hidden.value = JSON.stringify(items);
  if (items.length === 0) {
    alert('请至少添加一条发货明细并填写发货数量');
    return false;
  }
  return true;
}

// get_order_items 端点 HX-Trigger-After-Settle 事件：
// 服务端渲染的明细行已 swap 进 tbody，这里更新已选订单提示 + 隐藏空态 + 行编号 + 汇总
document.body.addEventListener('orderItemsLoaded', function (e) {
  var d = (e && e.detail) || {};
  var hiddenOrderId = document.getElementById('shipping-order-id-input');
  if (hiddenOrderId && d.order_id) hiddenOrderId.value = String(d.order_id);
  var hint = document.getElementById('order-selected-hint');
  if (hint && d.doc_number) {
    hint.textContent = '已选 ' + d.doc_number + ' · 共 ' + (d.count || 0) + ' 项';
  }
  var emptyHint = document.getElementById('shipping-empty-hint');
  if (emptyHint) emptyHint.style.display = 'none';
  wmsShippingRenumber();
});

// 行编号：删除行或加载后重排 1..N（对齐 stock_in wmsStockInRenumber）
function wmsShippingRenumber() {
  document.querySelectorAll('.shipping-item-row').forEach(function (row, i) {
    var ln = row.querySelector('.line-num');
    if (ln) ln.textContent = i + 1;
  });
  wmsShippingCalcSummary();
}
