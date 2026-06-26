htmx.config.disableInheritance=true;

// ── Smooth scroll to anchor (nav_chip 锚点条) ──
// hyperscript 不支持 JS 可选链 ?./对象字面量，故滚动逻辑放此，hyperscript 只 call 函数名。
window.scrollToAnchor = function (sel) {
  var el = document.querySelector(sel);
  if (el) el.scrollIntoView({ behavior: 'smooth', block: 'center' });
};


// ── Release drawer 流转卡增删（work-center 下达 drawer 批次规划）──
// .split-row 克隆增行；.split-remove 删行（至少保留 1 行）；每次操作后重新编号 splits[idx][batch_qty]
function renumberSplitRows(container) {
  container.querySelectorAll('.split-row').forEach(function (row, i) {
    var input = row.querySelector('.split-qty');
    if (input) input.name = 'splits[' + i + '][batch_qty]';
    var label = row.querySelector('.split-label');
    if (label) label.textContent = '流转卡 ' + (i + 1);
  });
}

window.addSplitRow = function (btn) {
  var container = (btn.closest('form') && btn.closest('form').querySelector('[data-splits]')) || btn.closest('[data-splits]');
  if (!container) return;
  var last = container.querySelector('.split-row:last-of-type');
  if (!last) return;
  var clone = last.cloneNode(true);
  var input = clone.querySelector('.split-qty');
  if (input) input.value = '';
  container.appendChild(clone);
  renumberSplitRows(container);
};

window.removeSplitRow = function (btn) {
  var row = btn && btn.closest('.split-row');
  var container = row && row.closest('[data-splits]');
  if (!container || container.querySelectorAll('.split-row').length <= 1) return;
  row.remove();
  renumberSplitRows(container);
};


// ── Demand Batch Action Bar (shared by mes work-center & demand-pool 订单明细 tab) ──
// 文档流内嵌批量栏：document 级 change/click 委托监听 .demand-cb，实时更新页面上所有
// .batch-bar（htmx 切 tab 动态出现的栏也能生效，无需 init 时机）。
// .batch-bar 结构约定：容器带 data-create-path；内含 .batch-count / .batch-create-btn / .batch-clear-btn。
function updateDemandBatchBars() {
  document.querySelectorAll('.batch-bar').forEach(function (bar) {
    // 每个 .batch-bar 按 closest('.batch-scope') 内的 demand-cb 独立计数
    // （订单明细 tab 一个 scope，每个物料展开区各自一个 scope，互不干扰）
    var scope = bar.closest('.batch-scope') || document;
    var checked = scope.querySelectorAll('input[type=checkbox].demand-cb:checked:not([disabled])');
    var count = checked.length;
    var createPath = bar.getAttribute('data-create-path');
    var countEl = bar.querySelector('.batch-count');
    var btn = bar.querySelector('.batch-create-btn');
    if (count === 0) { bar.classList.remove('show'); return; }
    bar.classList.add('show');
    if (countEl) countEl.textContent = count;
    if (!btn) return;
    var ids = [], productIds = new Set(), productName = '', productCode = '';
    checked.forEach(function (c) {
      ids.push(c.value);
      productIds.add(c.getAttribute('data-product-id'));
      if (!productName) productName = c.getAttribute('data-product-name') || '';
      if (!productCode) productCode = c.getAttribute('data-product-code') || '';
    });
    if (productIds.size > 1) {
      btn.onclick = function (e) { e.preventDefault(); alert('请选择同一物料的需求进行批量创建生产计划。'); };
    } else {
      var pid = productIds.size === 1 ? [...productIds][0] : null;
      var href = createPath + '?demand_ids=' + ids.join(',');
      if (pid && pid !== 'null' && pid !== 'undefined') {
        href += '&product_id=' + pid +
          '&product_name=' + encodeURIComponent(productName) +
          '&product_code=' + encodeURIComponent(productCode);
      }
      btn.setAttribute('href', href);
      btn.onclick = null;
    }
  });
}

// 全选 checkbox（订单明细 thead）联动
window.toggleAllDemands = function (master, table) {
  var cbs = table.querySelectorAll('input.demand-cb:not([disabled])');
  cbs.forEach(function (c) {
    c.checked = master.checked;
    var tr = c.closest('tr');
    if (tr) tr.classList.toggle('demand-row-selected', master.checked);
  });
  updateDemandBatchBars();
};

document.addEventListener('change', function (e) {
  if (e.target.type !== 'checkbox' || !e.target.classList.contains('demand-cb')) return;
  var tr = e.target.closest('tr');
  if (tr) tr.classList.toggle('demand-row-selected', e.target.checked);
  updateDemandBatchBars();
});

// 展开区懒加载 demand-cb（默认勾选）后，按 scope 刷新就地批量栏
document.addEventListener('htmx:afterSettle', function (e) {
  if (e.target && e.target.querySelector && e.target.querySelector('.demand-cb')) {
    updateDemandBatchBars();
  }
});

// 清除选择（document delegate，动态出现的栏也生效）
document.addEventListener('click', function (e) {
  if (!e.target.closest('.batch-clear-btn')) return;
  document.querySelectorAll('input[type=checkbox]').forEach(function (c) {
    if (!c.disabled && (c.classList.contains('demand-cb') || c.title === '全选')) {
      c.checked = false;
      var tr = c.closest('tr');
      if (tr) tr.classList.remove('demand-row-selected');
    }
  });
  updateDemandBatchBars();
});


// ── Floating Dropdown Positioning ──
// Positions a dropdown relative to its trigger, auto-flipping when near edges.
// Usage: call positionDropdown(triggerEl, dropdownEl) when dropdown becomes visible.

window.positionDropdown = function (trigger, dropdown) {
    var rect = trigger.getBoundingClientRect();
    var menuRect = dropdown.getBoundingClientRect();
    var viewH = window.innerHeight;
    var viewW = window.innerWidth;

    // Default: below trigger, right-aligned
    var top = rect.bottom + 4;
    var left = rect.right - menuRect.width;

    // Flip above if not enough space below
    if (top + menuRect.height > viewH - 8 && rect.top - menuRect.height - 4 > 8) {
        top = rect.top - menuRect.height - 4;
    }
    // Clamp horizontal
    if (left + menuRect.width > viewW - 8) left = viewW - menuRect.width - 8;
    if (left < 8) left = 8;

    dropdown.style.position = 'fixed';
    dropdown.style.top = top + 'px';
    dropdown.style.left = left + 'px';
    dropdown.style.right = 'auto';
};


// ── Toast helpers ──
// 通过 HTMX POST /api/toast 显示 toast 提示（服务端渲染）
window.show_toast = function (msg, type) {
    htmx.ajax('POST', '/api/toast', {target: '.toast-container', swap: 'innerHTML', values: {msg: msg, type: type || 'success'}});
};
window.show_success_toast = function (msg) { show_toast(msg, 'success'); };
window.show_error_toast = function (msg) { show_toast(msg, 'error'); };
window.show_warning_toast = function (msg) { show_toast(msg, 'warning'); };
window.show_info_toast = function (msg) { show_toast(msg, 'info'); };

// ── HTMX global error handling ──

// 错误兜底：仅在真实 HTTP 错误响应（4xx/5xx）时弹 toast。
// 注意：HX-Redirect 成功响应（200 + 空 body + 重定向头）会被 htmx 标记 successful=false，
// 但它不是错误；status===0 是请求被页面跳转/离开打断，也不是错误。两者都必须豁免，
// 否则每次提交成功都会闪一个"操作失败"toast。这样无需在每个 form 上加 hx-sync。
document.addEventListener('htmx:afterRequest', function (e) {
    if (e.detail.successful) return;
    var xhr = e.detail.xhr;
    if (!xhr) return;
    // status===0：请求被 abort（页面跳转/离开），非真实错误
    if (xhr.status === 0) return;
    // 2xx/3xx（HX-Redirect 成功响应、被 hx-sync 丢弃等）不是真实错误
    if (xhr.status < 400) return;

    if (xhr.status === 401) {
        window.location.href = '/login';
        return;
    }

    var msg = (xhr.responseText || '').trim() || '操作失败';
    window.show_error_toast(msg);
});

// ── HTMX 表单校验失败兜底 ──
// HTMX 在提交前调用 form.checkValidity()，必填字段未填会触发 htmx:validation:halted
// 并静默中止请求（checkValidity 不弹气泡）。当提交按钮在 <form> 外（hyperscript
// trigger submit 派发非可信事件）时，浏览器原生校验气泡也不显示，用户只见“点击无响应”。
// 这里主动找到第一个无效字段，聚焦 + 滚动 + toast，给出明确反馈。
document.addEventListener('htmx:validation:halted', function (e) {
    var elt = (e.detail && e.detail.elt) ? e.detail.elt : e.target;
    var form = elt && elt.tagName === 'FORM' ? elt : (elt && elt.closest ? elt.closest('form') : null);
    var invalid = form ? form.querySelector(':invalid') : null;
    if (invalid) {
        invalid.focus();
        invalid.scrollIntoView({ behavior: 'smooth', block: 'center' });
    }
    var msg = invalid && invalid.validationMessage ? invalid.validationMessage : '请填写所有必填字段';
    window.show_error_toast(msg);
});

// ── Export download handler ──
document.addEventListener('exportDone', function (e) {
    window.location.href = e.detail.url;
});



// ── HTMX: re-init for swapped content ──


// ── HTMX custom confirm dialog (replaces native confirm()) ──

document.addEventListener('htmx:confirm', function (e) {
    if (!e.detail.question) return;
    e.preventDefault();
    var dialog = document.querySelector('#global-confirm-dialog');
    if (!dialog) {
        if (confirm(e.detail.question)) e.detail.issueRequest(true);
        return;
    }
    var overlay = dialog.querySelector('.dialog-overlay');
    var msg = document.querySelector('#global-confirm-message');
    if (!overlay || !msg) {
        if (confirm(e.detail.question)) e.detail.issueRequest(true);
        return;
    }
    msg.innerHTML = e.detail.question;
    window._confirmIssueRequest = e.detail.issueRequest.bind(e.detail, true);
    overlay.style.display = 'grid';
});

// ── UI interactions now use hyperscript _= attributes (see AGENTS.md) ──
// All former surreal.js hs* helpers have been removed; callers migrated to _="on click ..."


// ── Generic Line Item Calculator ──
// Usage: var calc = lineItemCalc('#order-item-tbody');
// calc.calcRow(tr) / calc.recalcTotals() / calc.collectItems()
// Or in HTML: oninput="lineItemCalc('#quotation-item-tbody').calcRow(this)"
window.lineItemCalc = function(tbodyId) {
    function calcRow(row) {
        var q = parseFloat(row.querySelector('[name="quantity"]').value) || 0;
        var p = parseFloat(row.querySelector('[name="unit_price"]').value) || 0;
        var d = parseFloat(row.querySelector('[name="discount_rate"]').value) || 0;
        var cell = row.querySelector('.line-total');
        if (cell) cell.textContent = (q * p * (1 - d / 100)).toFixed(2);
        recalcTotals();
    }
    function recalcTotals() {
        var tbody = document.querySelector(tbodyId);
        if (!tbody) return;
        var subtotal = 0, disc = 0;
        tbody.querySelectorAll('tr').forEach(function (row) {
            var q = parseFloat(row.querySelector('[name="quantity"]').value) || 0;
            var p = parseFloat(row.querySelector('[name="unit_price"]').value) || 0;
            var d = parseFloat(row.querySelector('[name="discount_rate"]').value) || 0;
            subtotal += q * p;
            disc += q * p * (d / 100);
            var cell = row.querySelector('.line-total');
            if (cell) cell.textContent = (q * p * (1 - d / 100)).toFixed(2);
        });
        document.querySelector('#subtotal-value').textContent = '¥ ' + subtotal.toFixed(2);
        document.querySelector('#discount-value').textContent = '- ¥ ' + disc.toFixed(2);
        document.querySelector('#grand-value').textContent = '¥ ' + (subtotal - disc).toFixed(2);
    }
    function collectItems() {
        var tbody = document.querySelector(tbodyId);
        if (!tbody) return;
        var items = [];
        tbody.querySelectorAll('tr').forEach(function (row) {
            var obj = {};
            row.querySelectorAll('input, select, textarea').forEach(function (el) {
                var name = el.getAttribute('name');
                if (name) obj[name] = el.value;
            });
            items.push(obj);
        });
        document.querySelector('#items-json').value = JSON.stringify(items);
    }
    return { calcRow: calcRow, recalcTotals: recalcTotals, collectItems: collectItems };
};

// ── Page-specific aliases for inline handlers ──
var _qc = lineItemCalc('#quotation-item-tbody');
window.quotationCalcRow = _qc.calcRow;
window.quotationRecalcTotals = _qc.recalcTotals;
window.quotationSubmit = _qc.collectItems;

var _sc = lineItemCalc('#order-item-tbody');
window.salesOrderCalcRow = _sc.calcRow;
window.salesOrderRecalcTotals = _sc.recalcTotals;
window.salesOrderSubmit = _sc.collectItems;

// ── Generic Entity Picker (search-select modal) ──
// Called from Hyperscript: _="on click call entityPickerSelect(me)"
// Reads data-id / data-label from clicked option, fills hidden input + display,
// closes the modal, and fires a custom event for cascade triggers.
window.entityPickerSelect = function (el) {
    var modal = el.closest('.modal-overlay');
    if (!modal) return;

    var targetId = modal.querySelector('input[name="target_id"]').value;
    var displayId = modal.querySelector('input[name="display_id"]').value;
    var eventName = modal.querySelector('input[name="event_name"]').value;

    var hidden = document.getElementById(targetId);
    var display = document.getElementById(displayId);
    if (hidden) hidden.value = el.dataset.id;
    if (display) {
        display.textContent = el.dataset.label;
        display.classList.remove('placeholder');
    }

    modal.classList.remove('is-open');

    if (eventName) {
        document.body.dispatchEvent(new CustomEvent(eventName));
    }
};

// ===== Purchase Order Line Calculation =====

window.formatMoney = function(v) {
    return Number(v || 0).toFixed(2).replace(/\B(?=(\d{3})+(?!\d))/g, ',');
};

window.calcPurchaseLine = function(row) {
    const qtyEl = row.querySelector('[data-field="qty"]');
    const priceEl = row.querySelector('[data-field="price"]');
    const discountEl = row.querySelector('[data-field="discount"]');
    const taxSelect = row.querySelector('[data-field="tax_rate_id"]');

    const qty = parseFloat(qtyEl?.value) || 0;
    const price = parseFloat(priceEl?.value) || 0;
    const discount = parseFloat(discountEl?.value) || 0;

    let taxRate = 0;
    if (taxSelect) {
        const opt = taxSelect.selectedOptions[0];
        taxRate = parseFloat(opt?.dataset?.rate) || 0;
    }

    const subtotal = qty * price * (1 - discount / 100);
    const tax = subtotal * taxRate / 100;
    const total = subtotal + tax;

    const subtotalEl = row.querySelector('[data-field="subtotal"]');
    if (subtotalEl) subtotalEl.textContent = window.formatMoney(subtotal);

    return { subtotal, tax, total };
};

window.updatePurchaseSummary = function() {
    let untaxed = 0, tax = 0, total = 0;
    document.querySelectorAll('tr[data-item-row]').forEach(function(row) {
        const r = window.calcPurchaseLine(row);
        untaxed += r.subtotal;
        tax += r.tax;
        total += r.total;
    });
    const elU = document.getElementById('sum-untaxed');
    const elT = document.getElementById('sum-tax');
    const elTotal = document.getElementById('sum-total');
    if (elU) elU.textContent = window.formatMoney(untaxed);
    if (elT) elT.textContent = window.formatMoney(tax);
    if (elTotal) elTotal.textContent = window.formatMoney(total);
};

// Bind to PO item row input changes
document.addEventListener('input', function(e) {
    const row = e.target.closest('tr[data-item-row]');
    if (row && e.target.matches('[data-field="qty"], [data-field="price"], [data-field="discount"], [data-field="tax_rate_id"]')) {
        window.updatePurchaseSummary();
    }
});

// ===== Merge Purchase Orders =====

window.mergeSelectedPOs = function() {
    var ids = [];
    document.querySelectorAll('.po-checkbox:checked').forEach(function(cb) {
        ids.push(cb.value);
    });
    if (ids.length < 2) {
        alert('请至少选择两个订单进行合并');
        return;
    }
    if (!confirm('确认合并选中的 ' + ids.length + ' 个订单？')) return;

    var form = document.createElement('form');
    form.method = 'POST';
    form.action = '/admin/purchase/orders/merge';
    var input = document.createElement('input');
    input.type = 'hidden';
    input.name = 'order_ids';
    input.value = ids.join(',');
    form.appendChild(input);
    document.body.appendChild(form);
    form.submit();
};

// ===== Category Tree Select Filter =====

window.filterCatOptions = function(searchInput) {
    var keyword = searchInput.value.toLowerCase().trim();
    var list = searchInput.closest('.cat-dropdown');
    if (!list) return;
    var options = list.querySelectorAll('.cat-option');
    options.forEach(function(opt) {
        var name = (opt.getAttribute('data-name') || opt.textContent || '').toLowerCase();
        opt.style.display = name.indexOf(keyword) !== -1 ? '' : 'none';
    });
};

window.selectCat = function(btn) {
    var wrapper = btn.closest('.cat-select');
    if (!wrapper) return;
    var hidden = wrapper.querySelector('input[type=hidden]');
    var label = wrapper.querySelector('.cat-label');
    var dropdown = wrapper.querySelector('.cat-dropdown');
    var backdrop = wrapper.querySelector('.cat-backdrop');
    var id = btn.getAttribute('data-id') || '';
    var name = btn.getAttribute('data-name') || btn.textContent.trim();
    if (hidden) {
        hidden.value = id;
        hidden.dispatchEvent(new Event('change', {bubbles: true}));
    }
    if (label) label.textContent = name;
    if (dropdown) dropdown.style.display = 'none';
    if (backdrop) backdrop.style.display = 'none';
};

// ===== Drawer Slide Animation =====

window.slideDrawerIn = function(panelSelector) {
    var panel = document.querySelector(panelSelector);
    var overlay = panel ? panel.parentElement : null;
    if (overlay) overlay.style.opacity = '1';
    if (!panel) return;
    panel.style.transition = 'transform 0.3s ease';
    panel.style.transform = 'translateX(100%)';
    requestAnimationFrame(function() {
        requestAnimationFrame(function() {
            panel.style.transform = 'translateX(0)';
        });
    });
};

window.slideDrawerOut = function(panelSelector, overlay) {
    var panel = document.querySelector(panelSelector);
    if (!panel) { if (overlay) overlay.style.display = 'none'; return; }
    panel.style.transition = 'transform 0.3s ease';
    panel.style.transform = 'translateX(100%)';
    setTimeout(function() {
        if (overlay) {
            overlay.style.opacity = '0';
            setTimeout(function() { overlay.style.display = 'none'; }, 300);
        }
    }, 300);
};

// ── Disclosure drill-down：展开目标区块并 smooth 滚动定位 ──
// Hyperscript 调用：_="on click call openAndScroll('d-info')"
// 工单工作台 detail-header 物料徽章 / 摘要带点击 drill-down 用。
// 注：hyperscript 的 `call #x.scrollIntoView() with {behavior:'smooth',...}` 是非法语法
// （with 不能跟在 call 方法调用后传 JS 对象），故收敛到此全局函数。
window.openAndScroll = function (id) {
    var el = document.getElementById(id);
    if (!el) return;
    el.classList.add('open');
    el.scrollIntoView({behavior: 'smooth', block: 'center'});
};

// ── 报工记录筛选（工单工作台 d-report）──
// initReportFilter：从 tbody 行的 data-* 提取去重的工序/班组，填充 select 选项。
// filterReports：按 关键词(报工人/批次) + 工序 + 班组 三条件筛选行。
window.initReportFilter = function () {
    var root = document.querySelector('#d-report');
    if (!root) return;
    var rows = root.querySelectorAll('tbody tr');
    var ops = {}, teams = {};
    rows.forEach(function (tr) {
        if (tr.dataset.op) ops[tr.dataset.op] = 1;
        if (tr.dataset.team) teams[tr.dataset.team] = 1;
    });
    var fill = function (selId, map, allLabel) {
        var sel = document.getElementById(selId);
        if (!sel) return;
        var keys = Object.keys(map);
        sel.innerHTML = '<option value="">' + allLabel + '</option>' +
            keys.map(function (k) { return '<option value="' + k + '">' + k + '</option>'; }).join('');
    };
    fill('rpt-op', ops, '全部工序');
    fill('rpt-team', teams, '全部班组');
};
window.filterReports = function () {
    var root = document.querySelector('#d-report');
    if (!root) return;
    var kw = (document.getElementById('rpt-kw').value || '').toLowerCase();
    var op = document.getElementById('rpt-op').value;
    var team = document.getElementById('rpt-team').value;
    root.querySelectorAll('tbody tr').forEach(function (tr) {
        var w = (tr.dataset.worker || '').toLowerCase();
        var b = (tr.dataset.batch || '').toLowerCase();
        var okKw = !kw || w.indexOf(kw) >= 0 || b.indexOf(kw) >= 0;
        var okOp = !op || tr.dataset.op === op;
        var okTeam = !team || tr.dataset.team === team;
        tr.style.display = (okKw && okOp && okTeam) ? '' : 'none';
    });
};

// ── 作业中心 drawer 行级明细收集 ──
// 收货/拣货 drawer 的多行输入（每行 [data-row]，字段 [data-k="item_id"|"received_qty"|...]）
// 在 htmx submit 前收成 JSON 塞进 hidden items_json（onsubmit 触发，早于 htmx 的 submit 监听）
window.wcCollectItems = function (form) {
    var rows = form.querySelectorAll('[data-row]');
    var items = Array.prototype.map.call(rows, function (r) {
        var o = {};
        r.querySelectorAll('[data-k]').forEach(function (i) {
            o[i.getAttribute('data-k')] = i.value;
        });
        return o;
    });
    var j = form.querySelector('[name="items_json"]');
    if (j) j.value = JSON.stringify(items);
};

// ── 作业中心 收货 drawer：提交校验 + 库位选择弹窗 ──

// wcReceiveSubmit：收货 form 的 onsubmit 包装——校验（实收>0、仓库必填）→ wcCollectItems → 返回 bool（false 阻止提交）
window.wcReceiveSubmit = function (form) {
    var rows = form.querySelectorAll('[data-row]');
    for (var i = 0; i < rows.length; i++) {
        var r = rows[i];
        var qtyEl = r.querySelector('[data-k="received_qty"]');
        var whEl = r.querySelector('[data-k="warehouse_id"]');
        var qty = qtyEl ? parseFloat(qtyEl.value) : NaN;
        var wh = whEl ? whEl.value : '';
        if (isNaN(qty) || qty <= 0) { alert('每行实收数量必须大于 0'); return false; }
        if (!wh) { alert('每行必须选择目标仓库'); return false; }
    }
    window.wcCollectItems(form);
    return true;
};

// wcOpenBinPicker：读当前行 product/warehouse → htmx 加载 suggest_bins → 开 #bin-picker 弹窗
window.wcOpenBinPicker = function (btn) {
    var row = btn.closest('[data-row]');
    if (!row) return;
    var pidEl = row.querySelector('[data-k="product_id"]');
    var whEl = row.querySelector('[data-k="warehouse_id"]');
    var pid = pidEl ? pidEl.value : '';
    var wh = whEl ? whEl.value : '';
    if (!wh) { alert('请先为本行选择目标仓库'); return; }
    if (!pid) { alert('该行缺少产品信息'); return; }
    window.__binPickerRow = row;
    htmx.ajax('GET', '/admin/wms/stock-in/create/suggest-bins', {
        target: '#bin-picker-results',
        swap: 'innerHTML',
        values: { product_id: pid, warehouse_id: wh }
    });
    document.getElementById('bin-picker').classList.add('is-open');
};

// wmsPickBin：选定库位 → 填回 window.__binPickerRow 的 bin input + label。
// 兼容 work-center（data-k="bin_id"）与 stock-in/create（input[name="bin_id"]）两种行结构。
// suggest_bins_fragment 的库位按钮调用此函数（替代 stock-in 专用的 wmsStockInPickBin）。
window.wmsPickBin = function (binId, label) {
    var row = window.__binPickerRow;
    if (row) {
        var input = row.querySelector('[data-k="bin_id"], input[name="bin_id"]');
        var labelEl = row.querySelector('.bin-label');
        if (input) input.value = binId;
        if (labelEl) labelEl.textContent = label;
    }
    var picker = document.getElementById('bin-picker');
    if (picker) picker.classList.remove('is-open');
};

// wcResetBin：仓库 change 时清本行 bin_id + label（纯前端 UI，不发后端请求）
window.wcResetBin = function (sel) {
    var row = sel.closest('[data-row]');
    if (!row) return;
    var inp = row.querySelector('[data-k="bin_id"]');
    var lbl = row.querySelector('.bin-label');
    if (inp) inp.value = '';
    if (lbl) lbl.textContent = '自动分配';
};
