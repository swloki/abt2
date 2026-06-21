/* 入库单创建页 — PO 多选弹窗 + 折叠卡片明细 + 库位级联 + 收集提交 */
/* 采购入库：选多个 PO → 每个 PO 一张折叠卡片带出明细(含待入库余量) → 逐条填入库数量/库位 → 提交。
   生产入库/无 PO：下方「手动添加物料」(product_picker) 直接添加行。 */

// ── 库位级联（Warehouse → Zone → Bin）──
function wmsUpdateZones() {
  var whId = document.getElementById('warehouse-select').value;
  var zoneSelect = document.getElementById('zone-select');
  var options = zoneSelect.querySelectorAll('option[data-wh]');
  var firstOpt = zoneSelect.querySelector('option:not([data-wh])');
  options.forEach(function (opt) {
    opt.style.display = (!whId || opt.dataset.wh === whId) ? '' : 'none';
  });
  zoneSelect.value = '';
  if (firstOpt) firstOpt.textContent = whId ? '请选择库区' : '请先选择仓库';
  wmsUpdateBins();
}

function wmsUpdateBins() {
  var zoneId = document.getElementById('zone-select').value;
  var binSelect = document.getElementById('bin-select');
  var options = binSelect.querySelectorAll('option[data-zone]');
  var firstOpt = binSelect.querySelector('option:not([data-zone])');
  options.forEach(function (opt) {
    opt.style.display = (!zoneId || opt.dataset.zone === zoneId) ? '' : 'none';
  });
  binSelect.value = '';
  if (firstOpt) firstOpt.textContent = zoneId ? '按上架策略分配' : '请先选择库区';
}

// 给所有空的 .row-bin-select 填充库位选项（从 #bin-select 的 option 读取）
function wmsFillBinSelects() {
  var binSelect = document.getElementById('bin-select');
  if (!binSelect) return;
  var opts = [];
  binSelect.querySelectorAll('option[data-zone]').forEach(function (o) {
    opts.push({ id: o.value, label: (o.textContent || '').trim() });
  });
  document.querySelectorAll('.row-bin-select').forEach(function (sel) {
    if (sel.options.length > 0) return; // 已填充则跳过
    sel.innerHTML = '<option value="">自动</option>';
    opts.forEach(function (o) {
      var op = document.createElement('option');
      op.value = o.id;
      op.textContent = o.label;
      sel.appendChild(op);
    });
  });
}

// ── PO 多选弹窗 ──

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
    : '未选择采购订单；也可在下方「入库物料明细」手动添加物料（如生产入库）';
}

// 监听结果区 checkbox 勾选 → 更新计数（结果区是 HTMX 动态替换的，用事件委托）
document.addEventListener('change', function (e) {
  if (e.target && e.target.classList && e.target.classList.contains('po-pick-cb')) {
    wmsStockInUpdateSelectedCount();
  }
});

// 确认多选 → 为每个新选的 PO 渲染折叠卡片并加载明细
function wmsStockInConfirmPoPicker() {
  var checked = document.querySelectorAll('#po-search-results .po-pick-cb:checked');
  var cards = document.getElementById('po-cards');
  if (!cards) return;
  var added = 0;
  checked.forEach(function (cb) {
    var poId = cb.dataset.id;
    if (!poId) return;
    if (cards.querySelector('.po-card[data-po-id="' + poId + '"]')) return; // 去重
    var card = document.createElement('div');
    card.className = 'po-card bg-surface border border-border-soft rounded-md';
    card.setAttribute('data-po-id', poId);
    var doc = cb.dataset.doc || poId;
    var supplier = cb.dataset.supplier || '-';
    var status = cb.dataset.status || '';
    card.innerHTML =
      '<div class="po-card-header flex items-center gap-3 px-4 py-3 border-b border-border-soft cursor-pointer hover:bg-surface/60">' +
        '<span class="po-toggle text-muted text-xs">▼</span>' +
        '<span class="text-sm font-semibold text-fg">' + doc + '</span>' +
        '<span class="text-xs text-muted">' + supplier + ' · ' + status + '</span>' +
        '<button type="button" class="ml-auto text-xs text-muted hover:text-danger" data-po-id="' + poId + '">删除</button>' +
      '</div>' +
      '<div class="po-card-body p-3 overflow-x-auto">' +
        '<table class="data-table"><thead><tr>' +
          '<th class="w-10">序号</th><th>产品</th><th class="w-[120px]">批次号</th>' +
          '<th class="w-[110px]">入库数量 <span class="text-danger">*</span></th><th class="w-[150px]">目标库位</th><th class="w-10"></th>' +
        '</tr></thead>' +
        '<tbody id="po-card-tbody-' + poId + '">' +
          '<tr><td colspan="6" class="text-center text-muted py-3 text-sm">加载明细中…</td></tr>' +
        '</tbody></table>' +
      '</div>';
    cards.appendChild(card);
    // header 点击折叠/展开（点删除按钮除外）
    card.querySelector('.po-card-header').addEventListener('click', function (e) {
      if (e.target.tagName === 'BUTTON') return;
      var body = card.querySelector('.po-card-body');
      if (!body) return;
      body.style.display = (body.style.display === 'none') ? '' : 'none';
      var toggle = card.querySelector('.po-toggle');
      if (toggle) toggle.textContent = (body.style.display === 'none') ? '▶' : '▼';
    });
    // 删除按钮
    card.querySelector('.po-card-header button').addEventListener('click', function () {
      wmsStockInRemovePoCard(poId);
    });
    // 加载该 PO 的物料明细（get_source_items 返回带待入库余量的行）
    htmx.ajax('GET', '/admin/wms/stock-in/create/source-items', {
      target: '#po-card-tbody-' + poId,
      swap: 'innerHTML',
      values: { source_type: 'purchase', source_id: poId }
    }).then(function () {
      setTimeout(function () { wmsFillBinSelects(); wmsStockInRenumber(); }, 50);
    });
    added++;
  });
  // 关闭弹窗、清空勾选
  document.querySelector('#po-picker').classList.remove('is-open');
  document.querySelectorAll('#po-search-results .po-pick-cb').forEach(function (cb) { cb.checked = false; });
  wmsStockInUpdateSelectedCount();
  wmsStockInUpdatePoHint();
  if (added === 0 && checked.length > 0) {
    // 全部是已存在的 PO
  }
}

function wmsStockInRemovePoCard(poId) {
  var card = document.querySelector('.po-card[data-po-id="' + poId + '"]');
  if (card) card.remove();
  wmsStockInRenumber();
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
  var k = document.getElementById('stockin-summary-kinds'); if (k) k.textContent = kinds;
  var tq = document.getElementById('stockin-summary-qty'); if (tq) tq.textContent = totalQty;
  var ic = document.getElementById('stockin-item-count'); if (ic) ic.textContent = '共 ' + kinds + ' 项';
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
  rows.forEach(function (row) {
    var pid = row.querySelector('input[name="product_id"]');
    if (!pid) return;
    var qtyEl = row.querySelector('input[name="quantity"]');
    var batchEl = row.querySelector('input[name="batch_no"]');
    var binEl = row.querySelector('select[name="bin_id"]') || row.querySelector('input[name="bin_id"]');
    var srcId = row.querySelector('input[name="source_id"]');
    var srcDoc = row.querySelector('input[name="source_doc_number"]');
    var qty = qtyEl ? qtyEl.value : '0';
    if (!qty || parseFloat(qty) <= 0) return; // 跳过未填数量的行
    items.push({
      product_id: pid.value,
      batch_no: batchEl ? (batchEl.value || null) : null,
      quantity: qty,
      bin_id: (binEl && binEl.value) ? binEl.value : null,
      source_id: (srcId && srcId.value) ? srcId.value : null,
      source_doc_number: (srcDoc && srcDoc.value) ? srcDoc.value : null
    });
  });
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
      wmsFillBinSelects();
    }).observe(manualTbody, { childList: true });
  }
})();
