(function () {
    'use strict';

    function loadOrderData() {
        var dataEl = document.getElementById('pr-order-data');
        if (!dataEl) return;
        var supplierName = dataEl.getAttribute('data-supplier-name') || '—';
        var contact = dataEl.getAttribute('data-contact') || '—';
        var phone = dataEl.getAttribute('data-phone') || '—';
        document.getElementById('pr-supplier-name').value = supplierName;
        document.getElementById('pr-contact').value = contact;
        document.getElementById('pr-phone').value = phone;

        var orderId = document.getElementById('pr-order-select').value;
        var hiddenInput = document.querySelector('#pr-form input[name="order_id"]');
        if (hiddenInput) hiddenInput.value = orderId;

        var section = document.getElementById('pr-items-section');
        var tbody = document.getElementById('pr-item-tbody');
        if (!section || !tbody) return;

        var itemDivs = dataEl.querySelectorAll('div[data-item]');
        if (itemDivs.length === 0) {
            section.style.display = 'none';
            return;
        }

        section.style.display = '';
        tbody.innerHTML = '';

        itemDivs.forEach(function (div, idx) {
            var data = JSON.parse(div.getAttribute('data-item'));
            var tr = document.createElement('tr');
            tr.innerHTML =
                '<td class="line-num">' + (idx + 1) + '</td>' +
                '<td class="mono">' + (data.product_code || '') + '</td>' +
                '<td>' + (data.product_name || '') + '</td>' +
                '<td>' + (data.specification || '') + '</td>' +
                '<td>' + (data.unit || '') + '</td>' +
                '<td class="num-right">' + (data.order_qty || '0') + '</td>' +
                '<td class="num-right">' + (data.received_qty || '0') + '</td>' +
                '<td><input class="return-qty" type="number" step="1" min="0" name="returned_qty" value="' + (data.returned_qty || '') + '" style="width:100px;text-align:right;padding:5px 8px;font-size:13px;font-family:var(--font-mono);border:1px solid var(--border);border-radius:var(--radius-sm)"></td>' +
                '<td class="num-right unit-price">' + (data.unit_price || '0') + '</td>' +
                '<td class="num-right line-amount">' + (data.returned_qty ? (parseFloat(data.returned_qty) * parseFloat(data.unit_price || 0)).toFixed(2) : '0.00') + '</td>' +
                '<td><button type="button" class="btn-remove-row" title="删除行" onclick="this.closest(\'tr\').remove();PRCreate.updateTotals();">×</button></td>' +
                '<input type="hidden" name="order_item_id" value="' + data.order_item_id + '">' +
                '<input type="hidden" name="product_id" value="' + data.product_id + '">';
            tbody.appendChild(tr);
        });

        updateTotals();
    }

    function updateTotals() {
        var tbody = document.getElementById('pr-item-tbody');
        if (!tbody) return;
        var totalQty = 0;
        var totalAmount = 0;
        tbody.querySelectorAll('tr').forEach(function (row) {
            var qtyInput = row.querySelector('.return-qty');
            var priceCell = row.querySelector('.unit-price');
            var amountCell = row.querySelector('.line-amount');
            if (qtyInput && priceCell && amountCell) {
                var qty = parseFloat(qtyInput.value) || 0;
                var price = parseFloat(priceCell.textContent) || 0;
                var amount = qty * price;
                amountCell.textContent = amount.toFixed(2);
                totalQty += qty;
                totalAmount += amount;
            }
        });
        var totalQtyEl = document.getElementById('pr-total-qty');
        var totalAmountEl = document.getElementById('pr-total-amount');
        if (totalQtyEl) totalQtyEl.textContent = totalQty;
        if (totalAmountEl) totalAmountEl.textContent = totalAmount.toFixed(2);
    }

    // Expose for inline event handlers
    window.PRCreate = { updateTotals: updateTotals };

    // Auto-fill supplier info & render items after HTMX loads order data
    document.addEventListener('htmx:afterRequest', function (e) {
        if (e.detail.elt && e.detail.elt.id === 'pr-order-select') {
            setTimeout(loadOrderData, 0);
        }
    });

    // Recompute on qty input changes (delegated)
    document.addEventListener('input', function (e) {
        if (e.target.classList && e.target.classList.contains('return-qty')) {
            var row = e.target.closest('tr');
            if (!row) return;
            var priceCell = row.querySelector('.unit-price');
            var amountCell = row.querySelector('.line-amount');
            if (priceCell && amountCell) {
                var qty = parseFloat(e.target.value) || 0;
                var price = parseFloat(priceCell.textContent) || 0;
                amountCell.textContent = (qty * price).toFixed(2);
            }
            updateTotals();
        }
    });
})();
