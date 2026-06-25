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
