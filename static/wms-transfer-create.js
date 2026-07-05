// 调拨单创建 drawer：行项目收集 + 源仓变更后刷新各行可用量（ATP）
// 依赖 app.js 的 wcGenIdempotencyKey（幂等键）、product_picker（添加物料行）。

window.transferRenumber = function () {
  var tbody = document.getElementById('transfer-item-tbody');
  if (!tbody) return;
  tbody.querySelectorAll('tr').forEach(function (row, i) {
    var ln = row.querySelector('.line-num');
    if (ln) ln.textContent = i + 1;
  });
  var el = document.getElementById('transfer-item-count');
  if (el) el.textContent = '共 ' + tbody.querySelectorAll('tr').length + ' 项';
};

// 源仓变更 / 新增行后：批量查各行产品在源仓的可用量（ATP），填 [data-avail]
window.transferRefreshAvail = function (form) {
  if (!form) return;
  var whEl = form.querySelector('select[name="from_warehouse_id"]');
  var wh = whEl ? whEl.value : '';
  var rows = form.querySelectorAll('[data-row]');
  if (!rows.length) return;
  var pids = [];
  rows.forEach(function (r) {
    var pid = r.getAttribute('data-pid');
    if (pid) pids.push(Number(pid));
  });
  if (!pids.length) return;
  if (!wh) {
    // 无源仓：清空可用量显示
    rows.forEach(function (r) {
      var el = r.querySelector('[data-avail]');
      if (el) el.textContent = '—';
    });
    return;
  }
  fetch('/admin/wms/work-center/transfer-stock-avail?warehouse_id=' + encodeURIComponent(wh) +
        '&product_ids=' + encodeURIComponent(JSON.stringify(pids)))  // allow: read-only ATP query (display), not a form submit
    .then(function (res) { return res.json(); })
    .then(function (map) {
      rows.forEach(function (r) {
        var pid = r.getAttribute('data-pid');
        var el = r.querySelector('[data-avail]');
        if (!el || !pid) return;
        var avail = map[pid];
        el.textContent = (avail === undefined || avail === null) ? '—' : avail;
      });
    })
    .catch(function () {});
};

window.transferCollectItems = function () {
  var tbody = document.getElementById('transfer-item-tbody');
  if (!tbody) return true;
  var rows = tbody.querySelectorAll('tr');
  var items = [];
  for (var i = 0; i < rows.length; i++) {
    var row = rows[i];
    var pid = row.querySelector('input[name="product_id"]');
    var qty = row.querySelector('input[name="quantity"]');
    var bat = row.querySelector('input[name="batch_no"]');
    if (!pid || !pid.value) continue;
    var q = parseFloat(qty ? qty.value : '0');
    if (isNaN(q) || q <= 0) { alert('每行调拨数量必须大于 0'); return false; }
    items.push({
      product_id: pid.value,
      quantity: qty ? qty.value : '0',
      batch_no: (bat && bat.value) ? bat.value : null
    });
  }
  if (items.length === 0) { alert('请至少添加一个物料'); return false; }
  var j = document.getElementById('transfer-items-json');
  if (j) j.value = JSON.stringify(items);
  return true;
};
