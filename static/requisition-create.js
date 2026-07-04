// requisition-create.js — 领料单新建交互（工单 picker 联动 + 行收集）
// 行内「仓库/库位」cell 复用全局 binPickerOpen/binPickerSelect（app.js，warehouse_bin_cell 渲染）；
// 顶部统一仓 change 复用 wcApplyWarehouseAll（app.js，批量设各行 hidden + 按钮文案 + 清 bin）。

function reqCalcSummary() {
  var tbody = document.getElementById('req-item-tbody');
  if (!tbody) return;
  var rows = tbody.querySelectorAll('tr[data-row]');
  var count = document.getElementById('req-item-count');
  if (count) count.textContent = '共 ' + rows.length + ' 项';
}

function reqRenumber() {
  var tbody = document.getElementById('req-item-tbody');
  if (!tbody) return;
  tbody.querySelectorAll('tr[data-row]').forEach(function (row, i) {
    var ln = row.querySelector('.line-num');
    if (ln) ln.textContent = i + 1;
  });
  reqCalcSummary();
}

// 收集明细行 → items_json。跳过本次领料量 ≤ 0 的行（对齐 shipping 跳过空行语义）。
// 每行读 hidden product_id + data-k（requested_qty/warehouse_id/bin_id/batch_no）。
function reqCollectItems() {
  var tbody = document.getElementById('req-item-tbody');
  if (!tbody) return false;
  var items = [];
  tbody.querySelectorAll('tr[data-row]').forEach(function (row) {
    var pidEl = row.querySelector('input[name="product_id"]');
    var qtyEl = row.querySelector('[data-k="requested_qty"]');
    if (!pidEl || !qtyEl) return;
    var qty = parseFloat(qtyEl.value);
    if (!qty || qty <= 0) return;
    var whEl = row.querySelector('[data-k="warehouse_id"]');
    var binEl = row.querySelector('[data-k="bin_id"]');
    var batchEl = row.querySelector('[data-k="batch_no"]');
    items.push({
      product_id: pidEl.value,
      requested_qty: qtyEl.value,
      warehouse_id: whEl ? whEl.value : '',
      bin_id: binEl ? binEl.value : '',
      batch_no: batchEl ? batchEl.value : ''
    });
  });
  var hidden = document.getElementById('req-items-json');
  if (hidden) hidden.value = JSON.stringify(items);
  if (items.length === 0) {
    alert('请至少添加一条领料明细（且本次领料量 > 0）');
    return false;
  }
  return true;
}

// 选工单 → 后端渲染整组 BOM 行（HX-Trigger-After-Settle: woItemsLoaded）→ 重编号 + 汇总
document.addEventListener('woItemsLoaded', function () {
  reqRenumber();
});

// 提交成功关 drawer —— form 的 hyperscript `on 'htmx:afterRequest'` 对 drawer body swap 进来的
// 元素监听 htmx 驼峰事件不可靠（实测：手动 dispatch 触发，真实 htmx 派发不触发），用 document 级
// 原生事件委托兜底（宿主页预载本 JS，form 在 drawer 打开后才进 DOM，委托式监听不受影响）。
document.addEventListener('htmx:afterRequest', function (e) {
  var d = e.detail || {};
  var form = d.elt;
  var xhr = d.xhr;
  if (form && form.id === 'requisitionForm' && xhr && xhr.status < 400 && (xhr.responseText || '').length === 0) {
    var overlay = document.getElementById('wc-requisition-create-overlay');
    if (overlay) overlay.classList.remove('open');
  }
});

