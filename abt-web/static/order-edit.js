(function () {
    'use strict';

    // ── Row-level recalc ──
    // Recomputes line total for a single <tr> row.
    function recalcRow(row) {
        var q = parseFloat(row.querySelector('[name="quantity"]').value) || 0;
        var p = parseFloat(row.querySelector('[name="unit_price"]').value) || 0;
        var d = parseFloat(row.querySelector('[name="discount_rate"]').value) || 0;
        var lineTotal = q * p * (1 - d / 100);
        var cell = row.querySelector('.line-total');
        if (cell) cell.textContent = lineTotal.toFixed(2);
    }

    // ── Totals bar recalc ──
    // Sums all rows and updates subtotal / discount / grand total.
    function recalcTotals() {
        var tbody = me('#order-item-tbody');
        if (!tbody) return;
        var rows = tbody.querySelectorAll('tr');
        var subtotal = 0, disc = 0;
        rows.forEach(function (row) {
            var q = parseFloat(row.querySelector('[name="quantity"]').value) || 0;
            var p = parseFloat(row.querySelector('[name="unit_price"]').value) || 0;
            var d = parseFloat(row.querySelector('[name="discount_rate"]').value) || 0;
            var lineTotal = q * p * (1 - d / 100);
            var cell = row.querySelector('.line-total');
            if (cell) cell.textContent = lineTotal.toFixed(2);
            subtotal += q * p;
            disc += q * p * (d / 100);
        });
        var subtotalEl = me('#subtotal-value');
        var discountEl = me('#discount-value');
        var grandEl = me('#grand-value');
        if (subtotalEl) subtotalEl.textContent = '\u00a5 ' + subtotal.toFixed(2);
        if (discountEl) discountEl.textContent = '- \u00a5 ' + disc.toFixed(2);
        if (grandEl) grandEl.textContent = '\u00a5 ' + (subtotal - disc).toFixed(2);
    }

    // ── Collect items on form submit ──
    // Reads each <tr> in the tbody and builds the items JSON.
    function collectItems() {
        var tbody = me('#order-item-tbody');
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
        var jsonEl = me('#items-json');
        if (jsonEl) jsonEl.value = JSON.stringify(items);
    }

    // Expose for inline event handlers
    window.OrderEdit = { recalcRow: recalcRow, recalcTotals: recalcTotals, collectItems: collectItems };

    // ── Delegated input listener for .num-input ──
    document.addEventListener('input', function (e) {
        if (e.target.classList.contains('num-input')) {
            var row = e.target.closest('tr');
            if (row && row.closest('#order-item-tbody')) {
                recalcRow(row);
                recalcTotals();
            }
        }
    });

    // ── Initial recalc on page load ──
    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', recalcTotals);
    } else {
        recalcTotals();
    }

    // ── Re-init after HTMX swaps ──
    document.addEventListener('htmx:afterSettle', function (e) {
        if (e.target.querySelector && e.target.querySelector('#order-item-tbody')) {
            recalcTotals();
        }
    });
})();
