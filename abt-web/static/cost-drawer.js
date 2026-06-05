// Cost drawer temp-price handler (vanilla JS, no Alpine)
// Manages temporary price overrides for material items missing unit prices.
// Data is persisted in localStorage per BOM.

(function () {
    'use strict';

    function storageKey(bomId) {
        return 'bom-cost-temp-prices:' + bomId;
    }

    function loadTempPrices(bomId) {
        try {
            var raw = localStorage.getItem(storageKey(bomId));
            return raw ? JSON.parse(raw) : {};
        } catch (e) { return {}; }
    }

    function saveTempPrices(bomId, map) {
        localStorage.setItem(storageKey(bomId), JSON.stringify(map));
    }

    function fmtCurrency(val) {
        if (val == null) return '-';
        return '\u00a5' + Number(val).toFixed(6);
    }

    function fmtAmount(price, qty) {
        return '\u00a5' + (parseFloat(price) * parseFloat(qty)).toFixed(6);
    }

    function initCostDrawer(container) {
        var bomId = container.dataset.bomId;
        if (!bomId) return;

        var tempPrices = loadTempPrices(bomId);

        // Fill in temp price cells for items without unit_price
        var priceCells = container.querySelectorAll('.cost-price-cell');
        priceCells.forEach(function (cell) {
            var productId = cell.dataset.productId;
            var tempPrice = tempPrices[productId];

            if (tempPrice) {
                // Show temp price badge
                cell.innerHTML = '<span class="temp-price-badge"><span>\u00a5' +
                    tempPrice + '</span><span class="temp-tag">临时</span></span>';
            } else {
                // Show input
                cell.innerHTML = '<span class="temp-price-input-wrap">' +
                    '<span class="missing-price">缺失</span>' +
                    '<input type="text" class="temp-price-input" placeholder="输入临时单价" data-product-id="' +
                    productId + '"></span>';
            }
        });

        // Fill in amount cells
        var amountCells = container.querySelectorAll('.cost-amount-cell');
        amountCells.forEach(function (cell) {
            var productId = cell.dataset.productId;
            var tr = cell.closest('tr');
            var quantity = tr ? tr.dataset.quantity : '1';
            var tempPrice = tempPrices[productId];

            if (tempPrice) {
                cell.innerHTML = '<span class="font-mono amount-warn">' + fmtAmount(tempPrice, quantity) + '</span>';
            } else {
                cell.innerHTML = '<span class="missing-price">-</span>';
            }
        });

        // Update temp price notice
        updateTempNotice(container, tempPrices);

        // Bind input events (using event delegation)
        container.addEventListener('keydown', function (e) {
            if (e.key === 'Enter' && e.target.classList.contains('temp-price-input')) {
                handleTempPriceInput(e.target, container, bomId);
            }
        });
        container.addEventListener('blur', function (e) {
            if (e.target.classList.contains('temp-price-input')) {
                handleTempPriceInput(e.target, container, bomId);
            }
        }, true);
        container.addEventListener('click', function (e) {
            if (e.target.classList.contains('temp-price-input')) {
                e.stopPropagation();
            }
        });
    }

    function handleTempPriceInput(input, container, bomId) {
        var val = (input.value || '').trim();
        if (!val) return;
        var num = parseFloat(val);
        if (isNaN(num) || num < 0) {
            input.value = '';
            return;
        }
        var productId = input.dataset.productId;
        var tempPrices = loadTempPrices(bomId);
        tempPrices[productId] = val;
        saveTempPrices(bomId, tempPrices);

        // Re-init the whole drawer to update display
        initCostDrawer(container);
    }

    function updateTempNotice(container, tempPrices) {
        var notice = container.querySelector('#temp-price-notice');
        if (!notice) return;

        var keys = Object.keys(tempPrices);
        if (keys.length > 0) {
            notice.style.display = '';
            var countEl = notice.querySelector('#temp-price-count');
            if (countEl) countEl.textContent = keys.length;
        } else {
            notice.style.display = 'none';
        }
    }

    // Global clear function (called from onclick handler)
    window.costDrawerClearTemp = function () {
        var container = me('[data-bom-id]');
        if (!container) return;
        var bomId = container.dataset.bomId;
        localStorage.removeItem(storageKey(bomId));
        initCostDrawer(container);
    };

    // Init on HTMX swap and on DOMContentLoaded
    function tryInit(target) {
        var container = target.querySelector('[data-bom-id]');
        if (container) initCostDrawer(container);
    }

    document.addEventListener('htmx:afterSettle', function (e) {
        tryInit(e.target);
    });

    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', function () {
            tryInit(document);
        });
    } else {
        tryInit(document);
    }
})();
