/* 入库单创建页 — PO 多选弹窗 + 折叠卡片明细 + 库位选择弹窗 + 收集提交 */
/* 采购入库：选多个 PO → 每个 PO 一张折叠卡片带出明细(含待入库余量) → 逐条填入库数量/选库位 → 提交。
   生产入库/无 PO：下方「手动添加物料」(product_picker) 直接添加行。 */

function wmsStockInOpenPoPicker() {
  // 弹窗由 hyperscript 打开；这里只同步已选计数
  wmsStockInUpdateSelectedCount();
}

function wmsStockInUpdateSelectedCount() {
  var n = document.querySelectorAll('#po-search-results .po-pick-cb:checked').length;
  var el = document.getElementById('po-selected-count');
  if (el) el.textContent = '已选 ' + n + ' 个采购订单';
}

function wmsStockInUpdatePoHint() {
  var n = document.querySelectorAll('.po-card').length;
  var hint = document.getElementById('po-selected-hint');
  if (!hint) return;
  hint.textContent = n > 0
    ? ('已选择 ' + n + ' 个采购订单')
    : '未选择采购订单；也可在下方手动添加物料（如生产入库）';
}

// 监听结果区 checkbox 勾选 → 更新计数（结果区是 HTMX 动态替换的，用事件委托）
document.addEventListener('change', function (e) {
  if (e.target && e.target.classList && e.target.classList.contains('po-pick-cb')) {
    wmsStockInUpdateSelectedCount();
  }
});

// confirm 端点 swap #po-cards 后（poCardsUpdated 事件）/ 删除 PO 卡片后触发：填库位 + 重编号 + 汇总 + 更新提示
function wmsStockInOnCardsUpdated() {
  wmsStockInRenumber();
  wmsStockInCalcSummary();
  wmsStockInUpdatePoHint();
}

// ── 明细行校验 / 汇总 / 收集 ──

// 入库数量不得超过待入库余量（PO 明细行带 data-pending）
function wmsStockInValidateRow(input) {
  var pending = parseFloat(input.dataset.pending);
  var qty = parseFloat(input.value);
  var hint = input.closest('tr') ? input.closest('tr').querySelector('.pending-hint') : null;
  if (isNaN(pending)) return true;
  if (!isNaN(qty) && qty > pending) {
    input.style.borderColor = 'var(--danger, #f53f3f)';
    if (hint) { hint.textContent = '超过待入库余量 ' + pending + '（当前 ' + qty + '）'; hint.style.color = 'var(--danger, #f53f3f)'; }
    return false;
  }
  input.style.borderColor = '';
  if (hint && hint.dataset.pending) {
    // 还原提示（由后端 fragment 初始渲染的 data-pending 推导）
    hint.style.color = '';
  }
  return true;
}

function wmsStockInValidateAll() {
  var ok = true;
  document.querySelectorAll('.item-row input[name="quantity"][data-pending]').forEach(function (el) {
    if (!wmsStockInValidateRow(el)) ok = false;
  });
  return ok;
}

function wmsStockInCalcRow() {
  wmsStockInCalcSummary();
}

function wmsStockInCalcSummary() {
  var rows = document.querySelectorAll('.item-row');
  var kinds = rows.length;
  var totalQty = 0;
  rows.forEach(function (row) {
    var q = row.querySelector('input[name="quantity"]');
    if (q) totalQty += parseFloat(q.value) || 0;
  });
  var ic = document.getElementById('stockin-item-count');
  if (ic) {
    ic.textContent = kinds > 0 ? ('共 ' + kinds + ' 项 · ' + totalQty + ' 件') : '共 0 项';
  }
}

function wmsStockInRenumber() {
  var rows = document.querySelectorAll('.item-row');
  rows.forEach(function (row, i) {
    var ln = row.querySelector('.line-num');
    if (ln) ln.textContent = i + 1;
  });
  wmsStockInCalcSummary();
}

// 收集所有明细行（PO 卡片行 + 手动物料行）→ items_json，每行带 per-item source
function wmsStockInCollectItems() {
  if (!wmsStockInValidateAll()) {
    alert('存在入库数量超过待入库余量的明细，请检查标红的行');
    return false;
  }
  var rows = document.querySelectorAll('.item-row');
  var items = [];
  var missingDoc = false;
  rows.forEach(function (row) {
    var pid = row.querySelector('input[name="product_id"]');
    if (!pid) return;
    var qtyEl = row.querySelector('input[name="quantity"]');
    var whEl = row.querySelector('input[name="warehouse_id"]');
    var binEl = row.querySelector('input[name="bin_id"]');
    var srcId = row.querySelector('input[name="source_id"]');
    var srcDoc = row.querySelector('input[name="source_doc_number"]');
    var qty = qtyEl ? qtyEl.value : '0';
    if (!qty || parseFloat(qty) <= 0) return; // 跳过未填数量的行
    if (!srcDoc || !srcDoc.value) { missingDoc = true; return; } // 关联单号必填
    items.push({
      product_id: pid.value,
      quantity: qty,
      warehouse_id: (whEl && whEl.value) ? whEl.value : null,
      bin_id: (binEl && binEl.value) ? binEl.value : null,
      source_id: (srcId && srcId.value) ? srcId.value : null,
      source_doc_number: srcDoc.value
    });
  });
  if (missingDoc) {
    alert('每行物料必须填写关联单号（PO/工单明细自带，手动物料需手填）');
    return false;
  }
  var json = document.getElementById('stockin-items-json');
  if (json) json.value = JSON.stringify(items);
  if (items.length === 0) {
    alert('请至少添加一个物料并填写入库数量');
    return false;
  }
  return true;
}

// ── 初始化：手动物料表行变化时自动重编号 + 填库位 ──
(function () {
  var manualTbody = document.getElementById('stockin-item-tbody');
  if (manualTbody) {
    new MutationObserver(function () {
      wmsStockInRenumber();
    }).observe(manualTbody, { childList: true });
  }
})();

// ── 入库日期默认今天 ──
(function () {
  var d = document.getElementById('posting-date');
  if (d && !d.value) {
    var n = new Date();
    d.value = n.getFullYear() + '-' + String(n.getMonth() + 1).padStart(2, '0') + '-' + String(n.getDate()).padStart(2, '0');
  }
})();

// ── 入库日期默认今天 ──

// ── confirm 端点 HX-Trigger 事件监听 ──
// HTMX 将 HX-Trigger 事件 dispatch 到触发元素（确认按钮）后冒泡至 body；
// hyperscript "from body" 仅匹配 target===body 的事件、捕获不到冒泡事件，故用原生监听
document.body.addEventListener('closePoPicker', function () {
  var picker = document.getElementById('po-picker');
  if (picker) picker.classList.remove('is-open');
  document.querySelectorAll('#po-search-results input:checked').forEach(function (cb) { cb.checked = false; });
});
document.body.addEventListener('poCardsUpdated', wmsStockInOnCardsUpdated);
