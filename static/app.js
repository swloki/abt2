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


window.showToast = function (message, type) {
    var container = me('.toast-container');
    if (!container) return;
    type = type || 'success';

    var icons = {
        success: '<span><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="toast-icon"><path d="M22 11.08V12a10 10 0 11-5.93-9.14"/><polyline points="22 4 12 14.01 9 11.01"/></svg></span>',
        error: '<span><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="toast-icon"><circle cx="12" cy="12" r="10"/><line x1="15" y1="9" x2="9" y2="15"/><line x1="9" y1="9" x2="15" y2="15"/></svg></span>',
        warning: '<span><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="toast-icon"><path d="M10.29 3.86L1.82 18a2 2 0 001.71 3h16.94a2 2 0 001.71-3L13.71 3.86a2 2 0 00-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg></span>'
    };

    var div = document.createElement('div');
    div.className = 'toast toast-show toast-' + type;
    div.innerHTML = icons[type] || icons.success;

    var span = document.createElement('span');
    span.className = 'toast-message';
    span.textContent = message;
    div.appendChild(span);

    var btn = document.createElement('button');
    btn.className = 'toast-close';
    btn.textContent = '\u00d7';
    btn.onclick = function () { if (div.parentNode) div.parentNode.removeChild(div); };
    div.appendChild(btn);

    container.appendChild(div);

    setTimeout(function () {
        if (div.parentNode) div.parentNode.removeChild(div);
    }, 4000);
};

// ── HTMX global error handling ──

document.addEventListener('htmx:afterRequest', function (e) {
    if (e.detail.successful) return;
    var xhr = e.detail.xhr;
    if (!xhr) return;

    if (xhr.status === 401) {
        window.location.href = '/login';
        return;
    }

    var msg = (xhr.responseText || '').trim() || '操作失败';
    window.showToast(msg, 'error');
});

// ── Export download handler ──

document.addEventListener('exportDone', function (e) {
    window.location.href = e.detail.url;
    window.showToast('导出完成', 'success');
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
        me('#subtotal-value').textContent = '\u00a5 ' + subtotal.toFixed(2);
        me('#discount-value').textContent = '- \u00a5 ' + disc.toFixed(2);
        me('#grand-value').textContent = '\u00a5 ' + (subtotal - disc).toFixed(2);
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