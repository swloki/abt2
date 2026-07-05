// 盘点单创建 drawer：行项目收集 + 库位选完后自动拉系统账面数量（只读快照）
// 依赖 app.js 的 binPickerOpen/binPickerSelect（选库位后派发 input 事件触发 ccRefreshSystemQty）。

window.ccCalcSummary = function () {
  var tbody = document.getElementById('cc-item-tbody');
  if (!tbody) return;
  var n = tbody.querySelectorAll('tr').length;
  var el = document.getElementById('cc-item-count');
  if (el) el.textContent = '共 ' + n + ' 项';
};

window.ccRenumber = function () {
  var tbody = document.getElementById('cc-item-tbody');
  if (!tbody) return;
  tbody.querySelectorAll('tr').forEach(function (row, i) {
    var ln = row.querySelector('.line-num');
    if (ln) ln.textContent = i + 1;
  });
  window.ccCalcSummary();
};

// bin 选完后拉系统账面数量（只读快照，GET 查询，对齐 wcShipRefreshStock 模式）
window.ccRefreshSystemQty = async function (row) {
  if (!row) return;
  var pid = row.querySelector('input[name="product_id"]');
  var bin = row.querySelector('input[name="bin_id"]');
  var sq = row.querySelector('input[name="system_qty"]');
  if (!pid || !bin || !sq || !bin.value) return;
  try {
    var url = '/admin/wms/work-center/cycle-counts/system-qty?product_id=' +
      encodeURIComponent(pid.value) + '&bin_id=' + encodeURIComponent(bin.value);
    var r = await fetch(url);  // allow: read-only snapshot query (display), not a form submit
    var d = await r.json();
    sq.value = d.system_qty || '0';
  } catch (e) { /* 静默：拉不到就保持 0，不阻断录入 */ }
};

window.cycleCountCollectItems = function () {
  var tbody = document.getElementById('cc-item-tbody');
  if (!tbody) return true;
  var rows = tbody.querySelectorAll('tr');
  var items = [];
  for (var i = 0; i < rows.length; i++) {
    var row = rows[i];
    var pid = row.querySelector('input[name="product_id"]');
    var bin = row.querySelector('input[name="bin_id"]');
    var bat = row.querySelector('input[name="batch_no"]');
    var sq = row.querySelector('input[name="system_qty"]');
    if (!pid || !pid.value) continue;
    if (!bin || !bin.value) { alert('请为每行物料选择库位'); return false; }
    items.push({
      product_id: pid.value,
      bin_id: bin.value,
      batch_no: (bat && bat.value) ? bat.value : null,
      system_qty: sq ? sq.value : '0'
    });
  }
  if (items.length === 0) { alert('请至少添加一个物料'); return false; }
  var j = document.getElementById('cc-items-json');
  if (j) j.value = JSON.stringify(items);
  return true;
};
