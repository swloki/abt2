/* 委外单创建页 — 工单选择器 + 物料行管理 */

// 监听 #routing-zone DOM 变化 → 自动初始化
(function () {
  var rz = document.getElementById('routing-zone');
  if (rz) {
    new MutationObserver(function () {
      if (document.getElementById('wo-summary-data')) {
        omInitWorkOrderPicker();
      }
    }).observe(rz, { childList: true, subtree: true });
  }
})();

function omInitWorkOrderPicker() {
  var dataEl = document.getElementById('wo-summary-data');
  if (!dataEl) return;
  var pid = dataEl.dataset.pid || '';
  var pname = dataEl.dataset.pname || '';
  var pq = dataEl.dataset.pq || '';
  var se = dataEl.dataset.se || '';
  dataEl.remove();

  // 回填表单字段
  var setVal = function (sel, v) {
    var el = document.querySelector(sel);
    if (el) el.value = v;
  };
  setVal('#product-id-hidden', pid);
  setVal('#product-name-display', pname);
  setVal('[name="planned_qty"]', pq);
  setVal('[name="scheduled_date"]', se);
  setVal('[name="outsourcing_type"]', '2');

  // 工序选择 → 自动填单价 + 触发发料明细加载
  var rs = document.getElementById('routing-select');
  if (rs) {
    rs.addEventListener('change', function () {
      var opt = rs.options[rs.selectedIndex];
      var price = opt && opt.dataset.price ? opt.dataset.price : '';
      setVal('[name="unit_price"]', price);
      if (opt && opt.value && typeof htmx !== 'undefined') {
        htmx.trigger('#material-loader', 'routingSelected');
      }
    });
  }

  // 显示全部工序 checkbox + 选项重建
  var cb = document.getElementById('show-all-routings');
  var dataJson = document.getElementById('routings-json');
  var data = dataJson ? JSON.parse(dataJson.value || '[]') : [];

  function rebuild(all) {
    var sel = document.getElementById('routing-select');
    if (!sel) return;
    var cur = sel.value;
    sel.innerHTML = '<option value="">请选择工序</option>';
    data.forEach(function (r) {
      if (all || r[3]) {
        var o = document.createElement('option');
        o.value = r[0];
        o.dataset.name = r[2];
        o.dataset.price = r[4] || '';
        o.textContent = r[1] + ' - ' + r[2];
        sel.appendChild(o);
      }
    });
    // 恢复选中 或 自动选中第一个
    if (cur) {
      sel.value = cur;
    } else if (sel.options.length === 2) {
      sel.value = sel.options[1].value;
      setVal('[name="unit_price"]', sel.options[1].dataset.price || '');
    }
    // 通知发料明细区加载
    if (sel.value && typeof htmx !== 'undefined') {
      htmx.trigger('#material-loader', 'routingSelected');
    }
  }

  if (cb) {
    cb.addEventListener('change', function () { rebuild(cb.checked); });
  }
  rebuild(false);
}

// ── 物料行管理 ──

function omAddMaterialRow() {
  document.querySelector('#modal-product-id').value = '';
  document.querySelector('#modal-planned-qty').value = '';
  document.querySelector('#material-modal').classList.toggle('is-open');
}

function omConfirmMaterial() {
  var sel = document.querySelector('#modal-product-id');
  var pid = sel.value;
  var pname = sel.options[sel.selectedIndex] ? sel.options[sel.selectedIndex].textContent.trim() : '';
  var qty = parseFloat(document.querySelector('#modal-planned-qty').value) || 0;
  if (!pid || qty <= 0) return;

  var tbody = document.querySelector('#material-tbody');
  var tr = document.createElement('tr');
  tr.setAttribute('oninput', 'omUpdateMaterialJson()');
  tr.innerHTML = '<td>' + pname + '<input type="hidden" name="m_product_id" value="' + pid + '"></td>' +
    '<td><input class="w-[100px] text-right px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" type="number" step="any" name="m_planned_qty" value="' + qty + '"></td>' +
    '<td><button type="button" class="w-7 h-7 border-none text-muted rounded-sm cursor-pointer grid place-items-center hover:bg-surface transition-colors duration-150" title="删除" onclick="this.closest(\'tr\').remove();omUpdateMaterialJson()"><svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg></button></td>';
  tbody.appendChild(tr);
  omUpdateMaterialJson();
  document.querySelector('#material-modal').classList.remove('is-open');
}

function omUpdateMaterialJson() {
  var rows = Array.from(document.querySelectorAll('#material-tbody tr'));
  var items = [];
  rows.forEach(function (tr) {
    var pid = tr.querySelector('[name=m_product_id]');
    var qty = tr.querySelector('[name=m_planned_qty]');
    if (pid && qty) {
      items.push({
        product_id: parseInt(pid.value),
        planned_qty: qty.value,
        unit_cost: null
      });
    }
  });
  document.querySelector('#materials-json').value = JSON.stringify(items);
}

function omValidatePack(el) {
  var mp = parseFloat(el.dataset.minPack);
  var qty = parseFloat(el.value);
  var hint = el.closest('tr').querySelector('.pack-hint');
  if (mp && mp > 0 && qty && qty % mp !== 0) {
    el.style.borderColor = 'var(--danger, #f53f3f)';
    if (hint) {
      hint.textContent = '需 ' + mp + ' 的整数倍（当前 ' + qty + '）';
      hint.style.color = 'var(--danger, #f53f3f)';
    }
    return false;
  }
  if (hint) hint.style.color = '';
  return true;
}

function omValidateAllPacks() {
  var ok = true;
  document.querySelectorAll('#material-tbody input[name=m_planned_qty]').forEach(function (el) {
    if (!omValidatePack(el)) ok = false;
  });
  return ok;
}
