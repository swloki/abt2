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


// ── Toast helper ──
// 通过 HTMX POST /api/toast 显示 toast 提示（服务端渲染）
window.show_toast = function (msg, type) {
    htmx.ajax('POST', '/api/toast', {target: '.toast-container', swap: 'innerHTML', values: {msg: msg, type: type || 'success'}});
};

// ── HTMX global error handling ──

// 错误兜底：通过 POST /api/toast 显示错误 toast
document.addEventListener('htmx:afterRequest', function (e) {
    if (e.detail.successful) return;
    var xhr = e.detail.xhr;
    if (!xhr) return;

    if (xhr.status === 401) {
        window.location.href = '/login';
        return;
    }

    var msg = (xhr.responseText || '').trim() || '操作失败';
    window.show_toast(msg, 'error');
});

// ── Export download handler ──
document.addEventListener('exportDone', function (e) {
    window.location.href = e.detail.url;
});

// CSS 动画结束后自动移除 DOM 节点，防止长时间使用后堆积透明元素
document.addEventListener('animationend', function (e) {
    if (e.target.classList.contains('toast')) {
        e.target.remove();
    }
});


// ── HTMX: re-init for swapped content ──


// ── HTMX custom confirm dialog (replaces native confirm()) ──

document.addEventListener('htmx:confirm', function (e) {
    if (!e.detail.question) return;
    e.preventDefault();
    var dialog = me('#global-confirm-dialog');
    if (!dialog) {
        if (confirm(e.detail.question)) e.detail.issueRequest(true);
        return;
    }
    var overlay = dialog.querySelector('.dialog-overlay');
    var msg = me('#global-confirm-message');
    if (!overlay || !msg) {
        if (confirm(e.detail.question)) e.detail.issueRequest(true);
        return;
    }
    msg.innerHTML = e.detail.question;
    window._confirmIssueRequest = e.detail.issueRequest.bind(e.detail, true);
    overlay.classList.add('open');
});

// ── Surreal.js helpers (replaces hyperscript _= attributes) ──
// These wrap surreal.js me() API for use in onclick/onsubmit handlers from Maud templates.

// Toggle class on self
window.hsToggleSelf = function(el, cls) {
    me(el).classToggle(cls);
};

// Add class to target (selector or element)
window.hsAdd = function(el, selector, cls) {
    me(selector || el).classAdd(cls);
};

// Remove class from target
window.hsRemove = function(el, selector, cls) {
    me(selector || el).classRemove(cls);
};

// Remove class from closest ancestor matching selector
window.hsRemoveClosest = function(el, ancestorSelector, cls) {
    var ancestor = el.closest(ancestorSelector);
    if (ancestor) me(ancestor).classRemove(cls);
};

// Add class to target, remove from siblings (tab-style)
window.hsTake = function(el, siblingSelector, cls) {
    var parent = el.parentElement;
    if (parent) {
        parent.querySelectorAll(siblingSelector).forEach(function(s) {
            me(s).classRemove(cls);
        });
    }
    me(el).classAdd(cls);
};

// Close overlay on backdrop click (only if click target IS the overlay itself)
window.hsBackdropClose = function(el, e, cls) {
    if (e.target === el) me(el).classRemove(cls);
};

// Toggle class on target
window.hsToggle = function(el, selector, cls) {
    me(selector).classToggle(cls);
};

// Toggle sidebar collapsed + persist to localStorage
window.hsToggleSidebar = function() {
    var shell = me('.app-shell');
    me(shell).classToggle('sidebar-collapsed');
    if (shell.classList.contains('sidebar-collapsed')) {
        localStorage.setItem('sidebar-collapsed', 'true');
    } else {
        localStorage.removeItem('sidebar-collapsed');
    }
};

// Set value of input and trigger event
window.hsSetAndTrigger = function(selector, value, eventName) {
    var input = me(selector);
    if (input) {
        input.value = value;
        input.send(eventName || 'keyup');
    }
};

// Remove closest ancestor element of given tag
window.hsRemoveClosestEl = function(el, ancestorSelector) {
    var ancestor = el.closest(ancestorSelector);
    if (ancestor) ancestor.remove();
};

// ── Generic Line Item Calculator ──
// Usage: var calc = lineItemCalc('#order-item-tbody');
// calc.calcRow(tr) / calc.recalcTotals() / calc.collectItems()
// Or in HTML: oninput="lineItemCalc('#quotation-item-tbody').calcRow(this)"
window.lineItemCalc = function(tbodyId) {
    function calcRow(row) {
        var q = parseFloat(me('[name="quantity"]', row).value) || 0;
        var p = parseFloat(me('[name="unit_price"]', row).value) || 0;
        var d = parseFloat(me('[name="discount_rate"]', row).value) || 0;
        var cell = me('.line-total', row);
        if (cell) cell.textContent = (q * p * (1 - d / 100)).toFixed(2);
        recalcTotals();
    }
    function recalcTotals() {
        var tbody = me(tbodyId);
        if (!tbody) return;
        var subtotal = 0, disc = 0;
        any('tr', tbody).forEach(function (row) {
            var q = parseFloat(me('[name="quantity"]', row).value) || 0;
            var p = parseFloat(me('[name="unit_price"]', row).value) || 0;
            var d = parseFloat(me('[name="discount_rate"]', row).value) || 0;
            subtotal += q * p;
            disc += q * p * (d / 100);
            var cell = me('.line-total', row);
            if (cell) cell.textContent = (q * p * (1 - d / 100)).toFixed(2);
        });
        me('#subtotal-value').textContent = '¥ ' + subtotal.toFixed(2);
        me('#discount-value').textContent = '- ¥ ' + disc.toFixed(2);
        me('#grand-value').textContent = '¥ ' + (subtotal - disc).toFixed(2);
    }
    function collectItems() {
        var tbody = me(tbodyId);
        if (!tbody) return;
        var items = [];
        any('tr', tbody).forEach(function (row) {
            var obj = {};
            any('input, select, textarea', row).forEach(function (el) {
                var name = el.attribute('name');
                if (name) obj[name] = el.value;
            });
            items.push(obj);
        });
        me('#items-json').value = JSON.stringify(items);
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
