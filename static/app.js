htmx.config.disableInheritance=true;

// ── Smooth scroll to anchor (nav_chip 锚点条) ──
// hyperscript 不支持 JS 可选链 ?./对象字面量，故滚动逻辑放此，hyperscript 只 call 函数名。
window.scrollToAnchor = function (sel) {
  var el = document.querySelector(sel);
  if (el) el.scrollIntoView({ behavior: 'smooth', block: 'center' });
};


// ── Release drawer 生产批次增删（work-center 下达 drawer 批次规划）──
// .split-row 克隆增行；.split-remove 删行（至少保留 1 行）；重新编号仅更新 label（数量由 collectReleaseSplits 收集成 JSON）
function renumberSplitRows(container) {
  var rows = container.querySelectorAll('.split-row');
  var single = rows.length <= 1;
  rows.forEach(function (row, i) {
    var label = row.querySelector('.split-label');
    if (label) label.textContent = '生产批次 ' + (i + 1);
    // 仅 1 行时禁用删除（至少保留 1 行）；多行时启用
    var rm = row.querySelector('.split-remove');
    if (rm) rm.disabled = single;
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
  // cloneNode 不触发 hyperscript 重新绑定，手动给克隆出的删除按钮挂 click
  var rm = clone.querySelector('.split-remove');
  if (rm) rm.addEventListener('click', function () { window.removeSplitRow(rm); });
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

// 收集 split 行为 JSON 字符串（htmx config-request 时注入 splits_json 参数）。
// serde_urlencoded 不支持 Vec<Struct>，故用 JSON 桥接（同 lineItemCalc 模式）。
window.collectReleaseSplits = function (form) {
  var rows = form.querySelectorAll('.split-row');
  var arr = [];
  for (var i = 0; i < rows.length; i++) {
    var q = rows[i].querySelector('.split-qty');
    if (q && q.value && parseFloat(q.value) > 0) {
      arr.push({ batch_qty: q.value });
    }
  }
  return JSON.stringify(arr);
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
      // 改用 drawer 就地创建（不跳转）：hx-get 加载 create-plan-drawer → afterSettle 打开 #create-plan-overlay
      var drawerUrl = '/admin/mes/work-center/create-plan-drawer/' + pid + '?demand_ids=' + ids.join(',');
      btn.setAttribute('hx-get', drawerUrl);
      btn.removeAttribute('href');
      if (window.htmx) htmx.process(btn);
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


// ── Purchase work-center 采购明细批量栏（.pc-demand-cb，同供应商多物料 → 一张 PO）──
// 与 MES .demand-cb 隔离：MES 强制单物料（productIds.size>1 报错），采购允许同供应商
// 多物料多选。document 级 change/click 委托，htmx 切 tab/翻页动态出现的栏也生效。
// .pc-batch-bar 结构约定：容器带 data-supplier-id；内含 .pc-batch-count / .pc-batch-create-btn / .pc-batch-clear-btn。
function pcUpdateBatchBar() {
  document.querySelectorAll('.pc-batch-bar').forEach(function (bar) {
    var scope = bar.closest('.pc-batch-scope') || document;
    var checked = scope.querySelectorAll('input[type=checkbox].pc-demand-cb:checked:not([disabled])');
    var count = checked.length;
    var countEl = bar.querySelector('.pc-batch-count');
    var btn = bar.querySelector('.pc-batch-create-btn');
    if (count === 0) { bar.classList.remove('show'); return; }
    bar.classList.add('show');
    if (countEl) countEl.textContent = count;
    if (!btn) return;
    var ids = [];
    checked.forEach(function (c) { ids.push(c.value); });
    var sid = bar.getAttribute('data-supplier-id');
    // 批量转单 drawer URL（同供应商多物料多选 → 一张 PO），由 .pc-batch-create-btn 的 hx-target 装载
    btn.setAttribute('hx-get', '/admin/purchase/work-center/batch-convert/' + sid + '/drawer?demand_ids=' + ids.join(','));
    if (window.htmx) htmx.process(btn);
  });
}

// 全选 checkbox（采购明细 thead）联动当前表所有 .pc-demand-cb
window.pcToggleAllDemands = function (master, table) {
  table.querySelectorAll('input.pc-demand-cb:not([disabled])').forEach(function (c) {
    c.checked = master.checked;
  });
  pcUpdateBatchBar();
};

// 行内 checkbox 变化 → 刷新采购批量栏
document.addEventListener('change', function (e) {
  if (e.target.type !== 'checkbox' || !e.target.classList.contains('pc-demand-cb')) return;
  pcUpdateBatchBar();
});

// htmx 替换表格后重新计算批量栏（翻页/切 tab 后选中态重置）
document.addEventListener('htmx:afterSettle', function (e) {
  if (e.target && e.target.querySelector && e.target.querySelector('.pc-demand-cb')) {
    pcUpdateBatchBar();
  }
});

// 清除选择（采购明细批量栏「清除」按钮）
document.addEventListener('click', function (e) {
  if (!e.target.closest('.pc-batch-clear-btn')) return;
  document.querySelectorAll('input[type=checkbox]').forEach(function (c) {
    if (!c.disabled && (c.classList.contains('pc-demand-cb') || c.title === '全选')) c.checked = false;
  });
  pcUpdateBatchBar();
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

// ── 供应商价格补录：提交成功后回填到原报价行 ──
// 由补录 modal 的 form 上的 hyperscript 调用：call fillPriceRowFromForm(me)
window.fillPriceRowFromForm = function (formEl) {
    var pidEl = formEl.querySelector('[name="product_id"]');
    var priceEl = formEl.querySelector('[name="price"]');
    if (!pidEl || !priceEl) return;
    var pid = pidEl.value;
    var newPrice = priceEl.value;
    var rows = document.querySelectorAll('#pq-item-tbody tr');
    for (var i = 0; i < rows.length; i++) {
        var rowPidEl = rows[i].querySelector('input[name="item_product_id"]');
        if (rowPidEl && rowPidEl.value === pid) {
            var target = rows[i].querySelector('input[name="item_unit_price"]');
            if (target) {
                target.value = newPrice;
                target.dispatchEvent(new Event('input', { bubbles: true }));
            }
            break;
        }
    }
};

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

// wcCollectWorkers：报工 modal 工人表格收集（[data-worker-row] 行，字段 [data-k]）
// → JSON 字符串 [{worker_id, completed_qty}]，由 hx-on:htmx:config-request 注入 workers_json 参数。
// worker_id 必须 Number()：后端 WorkerReportItem.worker_id 是 i64，字符串会导致 serde_json 解析失败。
window.wcCollectWorkers = function (form) {
    var rows = form.querySelectorAll('[data-worker-row]');
    var arr = [];
    for (var i = 0; i < rows.length; i++) {
        var idEl = rows[i].querySelector('[data-k="worker_id"]');
        var qtyEl = rows[i].querySelector('[data-k="completed_qty"]');
        var qty = qtyEl ? (qtyEl.value || '').trim() : '';
        var defEl = rows[i].querySelector('[data-k="defect_qty"]');
        var defect = defEl ? (defEl.value || '').trim() : '';
        if (idEl && idEl.value && qty && parseFloat(qty) > 0) {
            arr.push({ worker_id: Number(idEl.value), completed_qty: qty, defect_qty: defect || '0' });
        }
    }
    return JSON.stringify(arr);
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

// ── 弹窗式库位选择（bin_picker_modal 公共控件）──

// binPickerOpen：行内按钮点击 → 记录当前行 + 写 product_id/mode → 打开弹窗；
// 若本行已设仓库（统一仓库批量设过），打开后自动选中左侧对应仓库加载其库位。
// binPickerOpen：行内按钮点击 → 拉产品信息/库存分布/之前存储位置 → 填产品条 + 标注仓库 + 自动定位。
// 自动选中优先级：本行已设仓库 > 第一个有库存仓（之前存的）> 第一个仓。
window.binPickerOpen = async function (btn) {
  var row = btn.closest('tr');
  if (!row) return;
  window.__binPickerRow = row;
  var modal = document.getElementById('bin-picker-modal');
  if (!modal) return;
  var whList = document.getElementById('bin-picker-modal-wh-list');
  var pidInput = document.getElementById('bin-picker-modal-product-id');
  var modeInput = document.getElementById('bin-picker-modal-mode');
  var infoBox = document.getElementById('bin-picker-modal-product-info');
  var pid = btn.getAttribute('data-product-id') || '';
  if (pidInput) pidInput.value = pid;
  if (modeInput) modeInput.value = btn.getAttribute('data-mode') || 'inbound';

  // 清左侧旧标注/高亮 + 搜索框 + 右侧旧列表 + 产品信息条
  if (whList) {
    whList.querySelectorAll('.wh-item').forEach(function (el) {
      el.classList.remove('active', 'has-stock');
      var badge = el.querySelector('.stock-badge');
      if (badge) badge.remove();
    });
  }
  var search = modal.querySelector('input[placeholder^="搜索库位"]');
  if (search) search.value = '';
  var binsHolder = document.getElementById('bin-picker-modal-bins');
  if (binsHolder) binsHolder.innerHTML = '<div class="text-center text-muted py-10 text-sm">选择左侧仓库后加载库位列表</div>';
  if (infoBox) infoBox.textContent = '加载产品信息…';
  modal.classList.add('is-open');

  // 拉产品信息 + 库存分布 + 之前存储位置
  var data = null;
  if (pid) {
    try {
      var r = await fetch('/api/bin-picker/product-info?product_id=' + encodeURIComponent(pid));
      data = await r.json();
    } catch (e) {}
  }

  // 产品信息条：编码/名称 + 之前存储位置（可展开列表，每行点击回填）
  if (infoBox && data) {
    function sugRow(s) {
      return '<button type="button" class="suggested-pick self-start px-2 py-0.5 rounded-sm bg-accent-bg/60 text-accent text-[11px] font-medium border border-accent/20 cursor-pointer hover:bg-accent/15 text-left"'
        + ' data-wh-id="' + s.warehouse_id + '" data-wh-name="' + s.warehouse_name + '"'
        + ' data-bin-id="' + s.bin_id + '" data-bin-code="' + s.bin_code + '">'
        + s.warehouse_name + ' / ' + s.bin_code + ' · 库存 ' + s.qty
        + '</button>';
    }
    var html = '<div class="flex items-center gap-2 flex-wrap">';
    if (data.product_code) html += '<span class="font-mono text-fg">' + data.product_code + '</span>';
    if (data.product_name) html += '<span class="text-fg-2 truncate max-w-[260px]">' + data.product_name + '</span>';
    html += '</div>';
    if (data.stocks && data.stocks.length) {
      html += '<div class="mt-1.5 flex flex-col gap-1 items-start">';
      html += '<span class="text-[11px] text-muted">之前存储（点击回填）：</span>';
      html += sugRow(data.stocks[0]);
      if (data.stocks.length > 1) {
        html += '<button type="button" class="sug-toggle text-[11px] text-muted hover:text-accent bg-transparent border-none cursor-pointer">共 ' + data.stocks.length + ' 处 ▾</button>';
        html += '<div class="sug-more hidden flex flex-col gap-1">';
        for (var i = 1; i < data.stocks.length; i++) {
          html += sugRow(data.stocks[i]);
        }
        html += '</div>';
      }
      html += '</div>';
    }
    infoBox.innerHTML = html;
    infoBox.querySelectorAll('.suggested-pick').forEach(function (btn) {
      btn.addEventListener('click', function () {
        window.binPickerPickSuggested(
          btn.getAttribute('data-wh-id'),
          btn.getAttribute('data-wh-name'),
          btn.getAttribute('data-bin-id'),
          btn.getAttribute('data-bin-code')
        );
      });
    });
    var toggle = infoBox.querySelector('.sug-toggle');
    if (toggle) {
      toggle.addEventListener('click', function () {
        var more = infoBox.querySelector('.sug-more');
        if (!more) return;
        var hidden = more.classList.toggle('hidden');
        toggle.textContent = hidden ? ('共 ' + data.stocks.length + ' 处 ▾') : '收起 ▴';
      });
    }
  }

  // 仓库标注（库存分布）+ 排前
  var stockMap = {};
  if (data && data.stock_by_warehouse) {
    data.stock_by_warehouse.forEach(function (it) { stockMap[it.warehouse_id] = it.qty; });
  }
  if (whList) {
    var items = Array.prototype.slice.call(whList.querySelectorAll('.wh-item'));
    items.forEach(function (el) {
      var wid = el.getAttribute('data-warehouse-id');
      if (stockMap[wid]) {
        el.classList.add('has-stock');
        var b = document.createElement('span');
        b.className = 'stock-badge ml-1.5 text-[11px] text-success font-mono';
        b.textContent = '库存 ' + stockMap[wid];
        el.appendChild(b);
      }
    });
    // 有库存的仓库排前（reverse 保持各自原顺序）
    items
      .filter(function (el) { return el.classList.contains('has-stock'); })
      .reverse()
      .forEach(function (el) { whList.insertBefore(el, whList.firstChild); });
  }

  // 自动选中：本行已设仓库 > 第一个 has-stock > 第一个仓
  var whInput = row.querySelector('input[name="warehouse_id"]');
  var whId = whInput ? whInput.value : '';
  var target = whId && whList ? whList.querySelector('.wh-item[data-warehouse-id="' + whId + '"]') : null;
  if (!target && whList) target = whList.querySelector('.wh-item.has-stock');
  if (!target && whList) target = whList.querySelector('.wh-item');
  if (target && window.htmx) {
    window.htmx.trigger(target, 'click');
  }
};

// binPickerPickSuggested：点「一键选中」之前存储位置 → 填回当前行 warehouse_id+bin_id+按钮文字 + 关弹窗。
// 出库行（含 [data-avail]）额外刷新「可用」列。
window.binPickerPickSuggested = function (whId, whName, binId, binCode) {
  var row = window.__binPickerRow;
  if (!row) return;
  var whInput = row.querySelector('input[name="warehouse_id"]');
  var binInput = row.querySelector('input[name="bin_id"]');
  if (whInput) whInput.value = whId;
  if (binInput) binInput.value = binId;
  var btn = row.querySelector('.bin-cell-btn');
  if (btn) btn.textContent = whName + ' / ' + binCode;
  var modal = document.getElementById('bin-picker-modal');
  if (modal) modal.classList.remove('is-open');
  if (row.querySelector('[data-avail]') && window.wcShipRefreshStock) {
    window.wcShipRefreshStock(row);
  }
};

// binPickerFilterBins：右侧库位搜索框 oninput → 按编码/名称过滤当前库位列表
window.binPickerFilterBins = function (input) {
  var kw = (input.value || '').toLowerCase().trim();
  var bins = document.getElementById('bin-picker-modal-bins');
  if (!bins) return;
  bins.querySelectorAll('button[data-bin-id]').forEach(function (b) {
    var code = (b.getAttribute('data-bin-code') || '').toLowerCase();
    var name = (b.getAttribute('data-bin-name') || '').toLowerCase();
    b.style.display = (!kw || code.indexOf(kw) >= 0 || name.indexOf(kw) >= 0) ? '' : 'none';
  });
};

// binPickerSelect：库位列表点击 → 填回当前行 warehouse_id + bin_id + 按钮文字；
// 发货 drawer（行含 [data-avail]）额外刷新「可用」列。
window.binPickerSelect = function (binId, binCode, binName) {
  var row = window.__binPickerRow;
  if (!row) return;
  var activeWh = document.querySelector('#bin-picker-modal-wh-list .wh-item.active');
  var whId = activeWh ? activeWh.getAttribute('data-warehouse-id') : '';
  var whName = activeWh ? activeWh.getAttribute('data-warehouse-name') : '';
  var whInput = row.querySelector('input[name="warehouse_id"]');
  var binInput = row.querySelector('input[name="bin_id"]');
  if (whInput) { whInput.value = whId; whInput.dispatchEvent(new Event('input', { bubbles: true })); }
  if (binInput) { binInput.value = binId; binInput.dispatchEvent(new Event('input', { bubbles: true })); }
  var btn = row.querySelector('.bin-cell-btn');
  if (btn) btn.textContent = whName + ' / ' + binCode;
  document.getElementById('bin-picker-modal').classList.remove('is-open');
  // 发货 drawer：选完库位刷新该行「可用」列
  if (row.querySelector('[data-avail]') && window.wcShipRefreshStock) {
    window.wcShipRefreshStock(row);
  }
};

// wcGenIdempotencyKey：收货 drawer body 加载时生成幂等键（防双击重复入库）
window.wcGenIdempotencyKey = function (el) {
  el.value = crypto.randomUUID ? crypto.randomUUID() : (Date.now() + Math.random()).toString(36);
};

// wcShipRefreshStock：发货 drawer 选仓库后查各产品 ATP → 刷新「可用」列。
// source 为单行 → 只刷该行；否则刷 source 所在 form 的全部 [data-row]。无 warehouse_id 的行跳过。
window.wcShipRefreshStock = function (source) {
  var rows;
  if (source && source.matches && source.matches('[data-row]')) {
    rows = [source];
  } else {
    var form = source && source.closest ? source.closest('form') : null;
    rows = form ? form.querySelectorAll('[data-row]') : [];
  }
  if (!rows || !rows.length) return;
  rows.forEach(function (r) {
    var pid = r.getAttribute('data-pid');
    var need = parseFloat(r.getAttribute('data-need')) || 0;
    var whEl = r.querySelector('[data-k="warehouse_id"]');
    var wh = whEl ? whEl.value : '';
    if (!wh || !pid) return;
    fetch('/admin/wms/work-center/ship-stock-avail?warehouse_id=' + encodeURIComponent(wh) +
          '&product_ids=' + encodeURIComponent(JSON.stringify([pid])))
      .then(function (res) { return res.json(); })
      .then(function (map) {
        var avail = parseFloat(map[pid]) || 0;
        var el = r.querySelector('[data-avail]');
        if (!el) return;
        if (avail < need) {
          el.innerHTML = '<span class="text-danger">' + avail + ' 缺</span>';
        } else {
          el.innerHTML = '<span class="text-muted">' + avail + '</span>';
        }
      })
      .catch(function () {});
  });
};

// wcShipCollectRows：发货 drawer onsubmit — 校验（qty>0）→ 收集 [data-k] → items_json
window.wcShipCollectRows = function (form) {
  var rows = form.querySelectorAll('[data-row]');
  var items = [];
  for (var i = 0; i < rows.length; i++) {
    var r = rows[i];
    var qtyEl = r.querySelector('[data-k="qty"]');
    var qty = qtyEl ? parseFloat(qtyEl.value) : NaN;
    if (isNaN(qty) || qty <= 0) { alert('每行实发数量必须大于 0'); return false; }
    var whEl = r.querySelector('[data-k="warehouse_id"]');
    if (whEl && !whEl.value) { alert('每行必须选择发货仓库'); return false; }
    var o = {};
    r.querySelectorAll('[data-k]').forEach(function (el) {
      o[el.getAttribute('data-k')] = el.value;
    });
    items.push(o);
  }
  var j = form.querySelector('[name="items_json"]');
  if (j) j.value = JSON.stringify(items);
  return true;
};

// wcApplyWarehouseAll：顶部「统一仓库」批量 → 设各行 hidden warehouse_id + 更新按钮文字 + 清 bin_id。
// 行内已无 select，不再 trigger HTMX change；库位由用户各行点按钮开弹窗选。
// 发货 drawer（form 内有 [data-avail]）额外刷新「可用」列。
window.wcApplyWarehouseAll = function (topSel) {
  var form = topSel.closest('form');
  if (!form) return;
  var wh = topSel.value;
  var opt = topSel.options[topSel.selectedIndex];
  var whName = opt ? opt.textContent : '';
  var isShip = !!form.querySelector('[data-avail]');
  form.querySelectorAll('[data-row]').forEach(function (row) {
    var whInput = row.querySelector('[data-k="warehouse_id"]');
    var binInput = row.querySelector('[data-k="bin_id"]');
    var btn = row.querySelector('.bin-cell-btn');
    if (whInput) whInput.value = wh;
    if (binInput) binInput.value = '';
    if (btn) btn.textContent = wh ? ((whName || '已选仓库') + ' / 选择库位') : '选择仓库 / 库位';
  });
  if (isShip && window.wcShipRefreshStock) window.wcShipRefreshStock(form);
};

// ── WMS 工作中心：待出库批量发货栏（复用 MES .show 显隐范式）──
// wcUpdateBatchBar：收集 .wc-ship-cb:checked → 填 #wc-batch-bar 的 ids + 计数 + 显隐
window.wcUpdateBatchBar = function () {
    var card = document.getElementById('wc-domain-card');
    if (!card) return;
    var bar = document.getElementById('wc-batch-bar');
    if (!bar) return;
    var checked = card.querySelectorAll('input[type=checkbox].wc-ship-cb:checked');
    var ids = [];
    checked.forEach(function (cb) { ids.push(cb.value); });
    var idsInput = bar.querySelector('input[name="ids"]');
    if (idsInput) idsInput.value = ids.join(',');
    var countEl = bar.querySelector('.wc-batch-count');
    if (countEl) countEl.textContent = ids.length;
    if (ids.length > 0) { bar.classList.add('show'); } else { bar.classList.remove('show'); }
    // 全选 checkbox 状态同步（当前页可发单全勾 = 全选）
    var master = card.querySelector('.wc-select-all');
    var allShip = card.querySelectorAll('input[type=checkbox].wc-ship-cb');
    if (master) master.checked = allShip.length > 0 && checked.length === allShip.length;
};

// wcToggleAll：表头全选 checkbox 联动当前页所有可发单
window.wcToggleAll = function (master) {
    var card = document.getElementById('wc-domain-card');
    if (!card) return;
    card.querySelectorAll('input[type=checkbox].wc-ship-cb').forEach(function (cb) {
        cb.checked = master.checked;
    });
    window.wcUpdateBatchBar();
};

// wcClearBatch：清除所有勾选（批量栏「清除」按钮）
window.wcClearBatch = function () {
    var card = document.getElementById('wc-domain-card');
    if (!card) return;
    card.querySelectorAll('input[type=checkbox].wc-ship-cb:checked').forEach(function (cb) {
        cb.checked = false;
    });
    window.wcUpdateBatchBar();
};

// 行内 checkbox 变化 → 刷新批量栏
document.addEventListener('change', function (e) {
    if (e.target.type !== 'checkbox' || !e.target.classList.contains('wc-ship-cb')) return;
    window.wcUpdateBatchBar();
});

// 批量发货/切 tab 后 htmx 替换 #wc-domain-card → 重新初始化批量栏状态
document.addEventListener('htmx:afterSettle', function (e) {
    if (e.target && e.target.querySelector && e.target.querySelector('.wc-ship-cb')) {
        window.wcUpdateBatchBar();
    }
});
