/**
 * 工单规划 — 拆分行管理 + 数据收集 + 日期校验
 * 在规划 tab 的 HTML 之后加载（<script src="/wo-planning.js"></script>）
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
 * 打开拆分弹窗，记录当前操作行，默认填入总量的一半（向下取整）
 * @param {HTMLButtonElement} btn - 拆分按钮
 */
function openSplitDialog(btn) {
  var row = btn.closest('tr');
  window._splitRow = row;
  // 默认填入总量的一半（向下取整，至少 1）
  var qtyCell = row.querySelector('.wo-qty');
  var total = parseFloat(qtyCell.textContent.trim().replace(/,/g, ''));
  var half = Math.floor(total / 2);
  if (half < 1) half = 1;
  var input = document.getElementById('split-input');
  if (input) input.value = half;
  document.getElementById('split-dialog').classList.add('is-open');
}

/**
 * 拆分行：从 split-dialog modal 获取输入值，拆分 window._splitRow 指向的行
 * 由 input_dialog 组件的确认按钮触发：on click call doSplit()
 */
function doSplit() {
  var row = window._splitRow;
  if (!row) return;

  var qtyCell = row.querySelector('.wo-qty');
  var originalQty = parseFloat(qtyCell.textContent.trim().replace(/,/g, ''));

  var input = document.getElementById('split-input');
  if (!input) return;
  var firstQty = parseFloat(input.value);

  if (isNaN(firstQty) || firstQty < 1 || firstQty >= originalQty) {
    alert('数量必须大于 0 且小于总量 ' + originalQty);
    return;
  }

  var secondQty = originalQty - firstQty;

  // 更新当前行数量
  qtyCell.textContent = firstQty.toFixed(0);

  // 克隆行作为第二份
  var newRow = row.cloneNode(true);
  newRow.querySelector('.wo-qty').textContent = secondQty.toFixed(0);
  // 插入到当前行后面
  row.parentNode.insertBefore(newRow, row.nextSibling);

  // 关闭弹窗 + 清空输入
  input.value = '';
  document.getElementById('split-dialog').classList.remove('is-open');
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
