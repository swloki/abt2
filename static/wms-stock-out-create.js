/* 出库单创建页 — 发货申请/领料单 来源选择 + 折叠卡片明细 + 库位选择 + 收集提交 */
/* 销售出库：选多个发货申请 → 每个一张折叠卡片带出明细(含待出库余量) → 逐条填出库数量/选库位 → 提交。
   生产领料：选领料单(单选) → 一张卡片带出明细 → 同上。
   无来源：下方「手动添加物料」(product_picker) 直接添加行。
   注意：本页为独立出库登记，直接 record 扣减库存，不驱动发货申请/领料单状态机。 */

// ── 出库类型联动：销售出库→选择发货申请 / 生产领料→选择领料单（两按钮切换显隐）──
function wmsStockOutToggleSourceBtn() {
  var v = document.getElementById('txn-type').value;
  var isMaterial = v === 'MaterialIssue';
  var shippingBtn = document.getElementById('shipping-btn');
  var mrBtn = document.getElementById('mr-btn');
  if (shippingBtn) shippingBtn.classList.toggle('hidden', isMaterial);
  if (mrBtn) mrBtn.classList.toggle('hidden', !isMaterial);
}

function wmsStockOutUpdateSourceHint() {
  var n = document.querySelectorAll('.source-card').length;
  var hint = document.getElementById('source-selected-hint');
  if (!hint) return;
  hint.textContent = n > 0
    ? ('已选择 ' + n + ' 个来源单据')
    : '未选择来源单据；也可在下方手动添加物料';
}

// confirm 端点 swap #source-cards 后（sourceCardsUpdated 事件）/ 删除来源卡片后触发：重编号 + 汇总 + 更新提示
function wmsStockOutOnCardsUpdated() {
  wmsStockOutRenumber();
  wmsStockOutCalcSummary();
  wmsStockOutUpdateSourceHint();
}

// ── 库位选择弹窗（按产品+仓库，仅有库存库位）──
function wmsStockOutOpenBinPicker(btn) {
  var row = btn.closest('tr');
  if (!row) return;
  var pidEl = row.querySelector('input[name="product_id"]');
  var productId = pidEl ? pidEl.value : '';
  var whEl = row.querySelector('select[name="warehouse_id"]');
  var warehouseId = whEl ? whEl.value : '';
  if (!warehouseId) { alert('请先为本行选择来源仓库'); return; }
  if (!productId) { alert('该行缺少产品信息'); return; }
  window.__binPickerRow = row;
  htmx.ajax('GET', '/admin/wms/stock-out/create/suggest-bins', {
    target: '#bin-picker-results',
    swap: 'innerHTML',
    values: { product_id: productId, warehouse_id: warehouseId }
  });
  document.getElementById('bin-picker').classList.add('is-open');
}

// 选定库位：填回当前行的 hidden bin_id + 显示标签
function wmsStockOutPickBin(binId, label) {
  var row = window.__binPickerRow;
  if (row) {
    var input = row.querySelector('input[name="bin_id"]');
    var labelEl = row.querySelector('.bin-label');
    if (input) input.value = binId;
    if (labelEl) labelEl.textContent = label;
  }
  document.getElementById('bin-picker').classList.remove('is-open');
}

// ── 明细行校验 / 小计 / 汇总 / 收集 ──

// 出库数量不得超过待出库余量（来源明细行带 data-pending）
function wmsStockOutValidateRow(input) {
  var pending = parseFloat(input.dataset.pending);
  var qty = parseFloat(input.value);
  var hint = input.closest('tr') ? input.closest('tr').querySelector('.pending-hint') : null;
  if (isNaN(pending)) return true;
  if (!isNaN(qty) && qty > pending) {
    input.style.borderColor = 'var(--danger, #f53f3f)';
    if (hint) { hint.textContent = '超过待出库余量 ' + pending + '（当前 ' + qty + '）'; hint.style.color = 'var(--danger, #f53f3f)'; }
    return false;
  }
  input.style.borderColor = '';
  if (hint && hint.dataset.pending) hint.style.color = '';
  return true;
}

function wmsStockOutValidateAll() {
  var ok = true;
  document.querySelectorAll('.item-row input[name="quantity"][data-pending]').forEach(function (el) {
    if (!wmsStockOutValidateRow(el)) ok = false;
  });
  return ok;
}

// 行小计 = 数量 × 单位成本
function wmsStockOutCalcRow(row) {
  var qtyInput = row.querySelector('input[name="quantity"]');
  var costInput = row.querySelector('input[name="unit_cost"]');
  var totalCell = row.querySelector('.line-subtotal');
  var qty = parseFloat(qtyInput ? qtyInput.value : '0') || 0;
  var cost = parseFloat(costInput ? costInput.value : '0') || 0;
  var subtotal = qty * cost;
  if (totalCell) totalCell.textContent = subtotal > 0 ? '¥' + subtotal.toFixed(2) : '—';
  wmsStockOutCalcSummary();
}

function wmsStockOutCalcSummary() {
  var rows = document.querySelectorAll('.item-row');
  var kinds = rows.length;
  var totalQty = 0;
  var totalAmount = 0;
  rows.forEach(function (row) {
    var q = row.querySelector('input[name="quantity"]');
    var c = row.querySelector('input[name="unit_cost"]');
    var qty = parseFloat(q ? q.value : '0') || 0;
    var cost = parseFloat(c ? c.value : '0') || 0;
    totalQty += qty;
    totalAmount += qty * cost;
  });
  var k = document.getElementById('stockout-summary-kinds'); if (k) k.textContent = kinds;
  var tq = document.getElementById('stockout-summary-qty'); if (tq) tq.textContent = totalQty;
  var ta = document.getElementById('stockout-summary-amount'); if (ta) ta.textContent = '¥' + totalAmount.toFixed(2);
  var ic = document.getElementById('stockout-item-count'); if (ic) ic.textContent = '共 ' + kinds + ' 项';
}

function wmsStockOutRenumber() {
  var rows = document.querySelectorAll('.item-row');
  rows.forEach(function (row, i) {
    var ln = row.querySelector('.line-num');
    if (ln) ln.textContent = i + 1;
  });
  wmsStockOutCalcSummary();
}

// 收集所有明细行（来源卡片行 + 手动物料行）→ items_json，每行带 per-item source
function wmsStockOutCollectItems() {
  if (!wmsStockOutValidateAll()) {
    alert('存在出库数量超过待出库余量的明细，请检查标红的行');
    return false;
  }
  var rows = document.querySelectorAll('.item-row');
  var items = [];
  var missingDoc = false;
  rows.forEach(function (row) {
    var pid = row.querySelector('input[name="product_id"]');
    if (!pid) return;
    var qtyEl = row.querySelector('input[name="quantity"]');
    var whEl = row.querySelector('select[name="warehouse_id"]');
    var binEl = row.querySelector('input[name="bin_id"]');
    var costEl = row.querySelector('input[name="unit_cost"]');
    var srcId = row.querySelector('input[name="source_id"]');
    var srcDoc = row.querySelector('input[name="source_doc_number"]');
    var srcType = row.querySelector('input[name="source_type"]');
    var qty = qtyEl ? qtyEl.value : '0';
    if (!qty || parseFloat(qty) <= 0) return; // 跳过未填数量的行
    if (!srcDoc || !srcDoc.value) { missingDoc = true; return; } // 关联单号必填
    items.push({
      product_id: pid.value,
      quantity: qty,
      unit_cost: (costEl && costEl.value) ? costEl.value : null,
      warehouse_id: (whEl && whEl.value) ? whEl.value : null,
      bin_id: (binEl && binEl.value) ? binEl.value : null,
      source_id: (srcId && srcId.value) ? srcId.value : null,
      source_doc_number: srcDoc.value,
      source_type: (srcType && srcType.value) ? srcType.value : null
    });
  });
  if (missingDoc) {
    alert('每行物料必须填写关联单号（来源明细自带，手动物料需手填）');
    return false;
  }
  var json = document.getElementById('stockout-items-json');
  if (json) json.value = JSON.stringify(items);
  if (items.length === 0) {
    alert('请至少添加一个物料并填写出库数量');
    return false;
  }
  return true;
}

// ── 初始化：手动物料表行变化时自动重编号 ──
(function () {
  var manualTbody = document.getElementById('stockout-item-tbody');
  if (manualTbody) {
    new MutationObserver(function () {
      wmsStockOutRenumber();
    }).observe(manualTbody, { childList: true });
  }
})();

// ── confirm 端点 HX-Trigger 事件监听 ──
document.body.addEventListener('closeShippingPicker', function () {
  var picker = document.getElementById('shipping-picker');
  if (picker) picker.classList.remove('is-open');
  document.querySelectorAll('#shipping-search-results input:checked').forEach(function (cb) { cb.checked = false; });
});
document.body.addEventListener('closeMrPicker', function () {
  var picker = document.getElementById('mr-picker-modal');
  if (picker) picker.classList.remove('is-open');
});
document.body.addEventListener('sourceCardsUpdated', wmsStockOutOnCardsUpdated);
