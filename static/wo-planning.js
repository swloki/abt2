/**
 * 工单规划 — 拆分行管理 + 数据收集 + 日期校验
 * 在规划 tab 的 HTML 之后加载（<script src="/static/wo-planning.js"></script>）
 */

/**
 * 收集所有勾选行的规划数据为 JSON 字符串
 * 每行必须有 class="wo-plan-row"，包含 .wo-check / .wo-qty / .wo-start / .wo-end
 * @returns {string} JSON array of WorkOrderPlanItem，校验失败返回空字符串
 */
function collectPlanItems() {
  var rows = document.querySelectorAll('.wo-plan-row');
  var items = [];
  for (var i = 0; i < rows.length; i++) {
    var row = rows[i];
    var checkbox = row.querySelector('.wo-check');
    if (!checkbox || !checkbox.checked) continue;

    var planItemId = parseInt(row.getAttribute('data-plan-item-id'));
    var productId = parseInt(row.getAttribute('data-product-id'));
    var qtyText = row.querySelector('.wo-qty').textContent.trim().replace(/,/g, '');
    var plannedQty = parseFloat(qtyText);
    var startVal = row.querySelector('.wo-start').value;
    var endVal = row.querySelector('.wo-end').value;

    // 日期校验
    if (endVal < startVal) {
      alert('排程结束日期不能早于开始日期（产品行 ' + (i + 1) + '）');
      return '';
    }

    items.push({
      plan_item_id: planItemId,
      product_id: productId,
      planned_qty: plannedQty,
      scheduled_start: startVal,
      scheduled_end: endVal,
      routing_id: null,
      work_center_id: null,
    });
  }
  return JSON.stringify(items);
}

/**
 * 拆分行：将当前行的数量拆成两份，新增一行
 * @param {HTMLButtonElement} btn - 拆分按钮
 */
function splitRow(btn) {
  var row = btn.closest('tr');
  var qtyCell = row.querySelector('.wo-qty');
  var originalQty = parseFloat(qtyCell.textContent.trim().replace(/,/g, ''));

  var inputStr = prompt(
    '输入第一份的数量（总计 ' + originalQty + '）：\n剩余将自动作为第二份。',
    (originalQty / 2).toFixed(2)
  );
  if (inputStr === null) return;

  var firstQty = parseFloat(inputStr);
  if (isNaN(firstQty) || firstQty <= 0 || firstQty >= originalQty) {
    alert('数量必须大于 0 且小于总量 ' + originalQty);
    return;
  }
  var secondQty = originalQty - firstQty;

  // 更新当前行数量
  qtyCell.textContent = firstQty.toFixed(2).replace(/\.?0+$/, '');

  // 克隆行作为第二份
  var newRow = row.cloneNode(true);
  newRow.querySelector('.wo-qty').textContent = secondQty.toFixed(2).replace(/\.?0+$/, '');
  // 插入到当前行后面
  row.parentNode.insertBefore(newRow, row.nextSibling);
}

/**
 * 全选/取消全选（事件委托）
 */
document.addEventListener('change', function (e) {
  if (e.target.classList.contains('wo-check-all')) {
    var tbody = e.target.closest('table').querySelector('tbody');
    if (tbody) {
      var checkboxes = tbody.querySelectorAll('.wo-check');
      for (var i = 0; i < checkboxes.length; i++) {
        checkboxes[i].checked = e.target.checked;
      }
    }
  }
});
