// TODO: Rewrite bomEdit Alpine component to vanilla JS
// Migrated from Alpine.js to vanilla JS. All state is managed here;
// the HTML template no longer uses x-data/x-model/x-show etc.

(function () {
    'use strict';

    // ── State ──
    var layerFilter = 0;
    var collapsedNodes = {};
    var allCollapsed = false;
    var addModalOpen = false;
    var addParentId = 0;
    var addProductId = 0;
    var addProductCode = '';
    var addProductName = '';
    var addProductUnit = '';


    var saveAsOpen = false;
    var saveAsName = '';
    var deleteOpen = false;
    var publishOpen = false;

    // ── Storage helpers ──

    function storageKey() {
        return 'bom-collapsed-' + window.location.pathname.split('/')[4];
    }

    function saveCollapsed() {
        try {
            var data = {};
            for (var k in collapsedNodes) {
                if (collapsedNodes[k]) data[k] = true;
            }
            sessionStorage.setItem(storageKey(), JSON.stringify(data));
        } catch (e) { }
    }

    function restoreCollapsed() {
        try {
            var raw = sessionStorage.getItem(storageKey());
            if (raw) collapsedNodes = JSON.parse(raw);
        } catch (e) { }
    }

    // ── BOM ID helper ──

    function bomId() {
        return window.location.pathname.split('/')[4];
    }

    // ── Modal helpers ──

    function openModal(id) {
        var el = me('#' + id);
        if (el) el.classList.add('is-open');
    }

    function closeModal(id) {
        var el = me('#' + id);
        if (el) el.classList.remove('is-open');
    }

    // ── Add Node ──

    function selectAddProduct(product) {
        addProductId = product.product_id;
        addProductCode = product.product_code;
        addProductName = product.product_name;
        addProductUnit = product.unit || '';
        submitAddNode();
    }

    // Expose to inline onclick from product search results
    window.bomSelectProduct = selectAddProduct;

    function submitAddNode() {
        if (!addProductId) return;
        htmx.ajax('POST', '/admin/md/boms/' + bomId() + '/nodes', {
            values: {
                product_id: addProductId,
                parent_id: addParentId,
                quantity: '1',
                unit: addProductUnit
            },
            swap: 'none',
            headers: { 'HX-Request': 'true' }
        }).then(function () {
            closeModal('bom-add-modal');
        });
    }



    // ── Add Child ──

    function openAddChild(parentId) {
        addParentId = parentId;
        addProductId = 0;
        openModal('bom-add-modal');
    }

    window.bomOpenAddChild = openAddChild;

    // ── Edit Node ──

    function openEdit(nodeId, quantity, lossRate, unit, workCenter, position, remark) {
        var form = me('#bom-edit-node-form');
        if (!form) return;
        form.querySelector('[name="quantity"]').value = quantity;
        form.querySelector('[name="loss_rate"]').value = lossRate;
        form.querySelector('[name="unit"]').value = unit;
        form.querySelector('[name="work_center"]').value = workCenter;
        form.querySelector('[name="position"]').value = position;
        form.querySelector('[name="remark"]').value = remark;
        form.setAttribute('hx-post', '/admin/md/boms/' + bomId() + '/nodes/' + nodeId);
        htmx.process(form);
        openModal('bom-edit-modal');
    }

    window.bomOpenEdit = openEdit;

    // ── Delete Node ──

    function openDelete(nodeId) {
        var form = me('#bom-node-delete-form');
        if (!form) return;
        form.setAttribute('hx-delete', '/admin/md/boms/' + bomId() + '/nodes/' + nodeId);
        htmx.process(form);
        me('#bom-delete-dialog').classList.add('open');
    }

    window.bomOpenDelete = openDelete;

    // ── Collapse / Expand ──

    function toggleCollapse(nodeId) {
        collapsedNodes[nodeId] = !collapsedNodes[nodeId];
        saveCollapsed();
        applyVisibility();
    }

    window.bomToggleCollapse = toggleCollapse;

    function toggleAllCollapse() {
        allCollapsed = !allCollapsed;
        var tbody = me('#bom-sortable-tbody');
        if (!tbody) return;
        var parentIds = [];
        tbody.querySelectorAll('tr[data-node-id]').forEach(function (r) {
            var nid = Number(r.dataset.nodeId);
            var child = tbody.querySelector('tr[data-parent-id="' + nid + '"]');
            if (child) parentIds.push(nid);
        });
        parentIds.forEach(function (nid) {
            collapsedNodes[nid] = allCollapsed;
        });
        saveCollapsed();
        applyVisibility();

        // Update button text
        var btn = me('#bom-collapse-all-btn');
        if (btn) btn.textContent = allCollapsed ? '全部展开' : '全部折叠';
    }

    window.bomToggleAllCollapse = toggleAllCollapse;

    // ── Level Filter ──

    function applyVisibility() {
        var tbody = me('#bom-sortable-tbody');
        if (!tbody) return;
        tbody.querySelectorAll('tr[data-node-id]').forEach(function (tr) {
            var level = Number(tr.dataset.level);
            var nodeId = Number(tr.dataset.nodeId);

            // Level filter
            if (layerFilter !== 0 && layerFilter !== level) {
                tr.style.display = 'none';
                return;
            }

            // Ancestor collapse check
            var ancestors = tr.dataset.ancestors || '';
            if (ancestors) {
                var ids = ancestors.split(',');
                for (var i = 0; i < ids.length; i++) {
                    if (collapsedNodes[ids[i]]) {
                        tr.style.display = 'none';
                        return;
                    }
                }
            }
            tr.style.display = '';
        });
    }

    function initLevelFilter() {
        var select = me('#bom-level-filter');
        if (!select) return;
        select.addEventListener('change', function () {
            layerFilter = parseInt(select.value) || 0;
            applyVisibility();
        });
    }

    // ── Collapse button styling ──

    function updateCollapseButtons() {
        var tbody = me('#bom-sortable-tbody');
        if (!tbody) return;
        tbody.querySelectorAll('.bom-collapse-btn').forEach(function (btn) {
            var row = btn.closest('tr');
            if (!row) return;
            var nodeId = row.dataset.nodeId;
            if (collapsedNodes[nodeId]) {
                btn.classList.add('bom-collapsed');
            } else {
                btn.classList.remove('bom-collapsed');
            }
        });
    }

    // ── Sortable / Drag-and-Drop ──

    function initSortable() {
        restoreCollapsed();

        var tbody = me('#bom-sortable-tbody');
        if (!tbody) return;
        var table = tbody.closest('table');

        var dragNodeId = null;
        var dragParentId = null;
        var descendantIds = new Set();
        var cachedGaps = [];
        var currentGapIndex = -1;

        // Fixed-position overlay
        var indicator = document.createElement('div');
        indicator.className = 'bom-drop-indicator';
        indicator.style.display = 'none';
        document.body.appendChild(indicator);

        function getDescendants(nodeId) {
            var ids = new Set([nodeId]);
            var changed = true;
            while (changed) {
                changed = false;
                tbody.querySelectorAll('tr[data-node-id]').forEach(function (r) {
                    var pid = Number(r.dataset.parentId);
                    var nid = Number(r.dataset.nodeId);
                    if (ids.has(pid) && !ids.has(nid)) {
                        ids.add(nid);
                        changed = true;
                    }
                });
            }
            return ids;
        }

        function cacheGaps() {
            cachedGaps = [];
            var allRows = Array.from(tbody.querySelectorAll('tr[data-node-id]'));
            var siblings = allRows.filter(function (r) {
                return Number(r.dataset.parentId) === dragParentId
                    && !descendantIds.has(Number(r.dataset.nodeId));
            });
            for (var i = 0; i < siblings.length; i++) {
                var rect = siblings[i].getBoundingClientRect();
                cachedGaps.push({ y: rect.top, row: siblings[i], pos: 'top' });
                if (i + 1 < siblings.length) {
                    var nextRect = siblings[i + 1].getBoundingClientRect();
                    cachedGaps.push({ y: (rect.bottom + nextRect.top) / 2, row: siblings[i], pos: 'bottom' });
                } else {
                    cachedGaps.push({ y: rect.bottom, row: siblings[i], pos: 'bottom' });
                }
            }
        }

        function findNearestGap(clientY) {
            var best = -1;
            var bestDist = Infinity;
            for (var i = 0; i < cachedGaps.length; i++) {
                var dist = Math.abs(clientY - cachedGaps[i].y);
                if (dist < bestDist) {
                    bestDist = dist;
                    best = i;
                }
            }
            return best;
        }

        function hideIndicator() {
            indicator.style.display = 'none';
            currentGapIndex = -1;
        }

        function showIndicatorAt(gapIndex) {
            if (gapIndex === currentGapIndex) return;
            currentGapIndex = gapIndex;
            var gap = cachedGaps[gapIndex];
            var tableRect = table.getBoundingClientRect();
            indicator.style.display = 'block';
            indicator.style.top = (gap.y - 24) + 'px';
            indicator.style.left = tableRect.left + 'px';
            indicator.style.width = tableRect.width + 'px';
        }

        tbody.addEventListener('dragstart', function (e) {
            var row = e.target.closest('tr[data-node-id]');
            if (!row) return;
            dragNodeId = Number(row.dataset.nodeId);
            dragParentId = Number(row.dataset.parentId);
            descendantIds = getDescendants(dragNodeId);
            tbody.querySelectorAll('tr[data-node-id]').forEach(function (r) {
                if (descendantIds.has(Number(r.dataset.nodeId))) {
                    r.classList.add('bom-dragging');
                }
            });
            e.dataTransfer.effectAllowed = 'move';
            e.dataTransfer.setData('text/plain', String(dragNodeId));
            cacheGaps();
        });

        tbody.addEventListener('dragend', function () {
            tbody.querySelectorAll('.bom-dragging').forEach(function (r) {
                r.classList.remove('bom-dragging');
            });
            hideIndicator();
            dragNodeId = null;
            dragParentId = null;
            descendantIds = new Set();
            cachedGaps = [];
        });

        tbody.addEventListener('dragover', function (e) {
            e.preventDefault();
            e.dataTransfer.dropEffect = 'move';
            if (!dragNodeId || cachedGaps.length === 0) return;
            var gapIndex = findNearestGap(e.clientY);
            if (gapIndex >= 0) showIndicatorAt(gapIndex);
        });

        tbody.addEventListener('dragleave', function (e) {
            var related = e.relatedTarget;
            if (related && tbody.contains(related)) return;
            hideIndicator();
        });

        tbody.addEventListener('drop', function (e) {
            e.preventDefault();
            if (!dragNodeId || currentGapIndex < 0) { hideIndicator(); return; }
            var gap = cachedGaps[currentGapIndex];
            var targetRow = gap.row;
            var isTop = gap.pos === 'top';
            hideIndicator();

            var tid = Number(targetRow.dataset.nodeId);
            if (descendantIds.has(tid)) return;
            if (Number(targetRow.dataset.parentId) !== dragParentId) return;

            var allRows = Array.from(tbody.querySelectorAll('tr[data-node-id]'));
            var tIdx = allRows.indexOf(targetRow);
            var beforeSiblingId;

            if (isTop) {
                beforeSiblingId = tid;
            } else {
                beforeSiblingId = '';
                for (var i = tIdx + 1; i < allRows.length; i++) {
                    if (Number(allRows[i].dataset.parentId) === dragParentId) {
                        beforeSiblingId = Number(allRows[i].dataset.nodeId);
                        break;
                    }
                }
            }

            fetch('/admin/md/boms/' + bomId() + '/nodes/' + dragNodeId + '/move', {
                method: 'POST',
                headers: { 'HX-Request': 'true', 'Content-Type': 'application/x-www-form-urlencoded' },
                body: 'new_parent_id=' + dragParentId + '&before_sibling_id=' + beforeSiblingId
            }).then(function (resp) {
                if (resp.ok) {
                    var loc = resp.headers.get('HX-Redirect');
                    window.location.href = loc || window.location.pathname;
                } else {
                    return resp.text().then(function (msg) {
                        alert(msg || '移动失败');
                    });
                }
            }).catch(function () { alert('网络错误，请重试'); });
        });
    }

    function bindEditButtons() {
        any('button[title="编辑"]').forEach(function (btn) {
            btn.addEventListener('htmx:afterRequest', function () {
                me('#bom-edit-modal').classList.add('is-open');
            });
        });
    }
    // ── Init ──

    function init() {
        initSortable();
        initLevelFilter();
        applyVisibility();
        updateCollapseButtons();

        // "Add root node" button
        var addRootBtn = me('#bom-add-root-btn');
        if (addRootBtn) {
            addRootBtn.addEventListener('click', function () {
                addParentId = 0;
                addProductId = 0;
                openModal('bom-add-modal');
            });
        }

        // "Save As" button
        var saveAsBtn = me('#bom-save-as-btn');
        if (saveAsBtn) {
            saveAsBtn.addEventListener('click', function () {
                saveAsName = (saveAsBtn.dataset.name || '') + '_副本';
                var input = me('#bom-save-as-modal [name="new_name"]');
                if (input) input.value = saveAsName;
                openModal('bom-save-as-modal');
            });
        }

        // Publish/unpublish buttons
        var publishBtn = me('#bom-publish-btn');
        if (publishBtn) {
            publishBtn.addEventListener('click', function () {
                var overlay = me('#bom-publish-dialog');
                if (overlay) overlay.classList.add('open');
            });
        }

        // Edit button: open modal after HTMX loads content
        bindEditButtons();
        // Reload .data-card on nodeUpdated (edit/delete node)
        document.body.addEventListener('nodeUpdated', function () {
            var card = me('.data-card');
            if (card) {
                htmx.ajax('GET', window.location.pathname, {
                    target: card,
                    swap: 'outerHTML',
                    select: '.data-card'
                }).then(function () {
                    bindEditButtons();
                    applyVisibility();
                });
            }
            // Also close edit modal if open
            var modal = me('#bom-edit-modal');
            if (modal) {
                modal.classList.remove('is-open');
                modal.innerHTML = '';
            }
        });

    }

    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', init);
    } else {
        init();
    }
})();
