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

// 错误兜底：通过 POST /api/toast 显示错误 toast
document.addEventListener('htmx:afterRequest', function (e) {
    if (e.detail.successful) return;
    var xhr = e.detail.xhr;
    if (!xhr) return;
    // status===0：请求被 abort（通常是表单提交成功后页面跳转，或用户离开页面），非真实错误，不弹 toast
    if (xhr.status === 0) return;

    if (xhr.status === 401) {
        window.location.href = '/login';
        return;
    }

    var msg = (xhr.responseText || '').trim() || '操作失败';
    window.show_error_toast(msg);
});

// ── Export download handler ──
document.addEventListener('exportDone', function (e) {
    window.location.href = e.detail.url;
});

// Toast 生命周期：入场结束 → 3.5s 后离场 → 离场结束移除 DOM
document.addEventListener('animationend', function (e) {
    var el = e.target;
    if (!el.classList || !el.classList.contains('toast')) return;
    if (e.animationName === 'toast-in') {
        setTimeout(function () { el.classList.add('toast-dismiss'); }, 3500);
    }
    if (e.animationName === 'toast-out') {
        el.remove();
    }
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
    overlay.classList.add('open');
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
