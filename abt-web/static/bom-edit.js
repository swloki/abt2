// BOM Edit — collapse/expand, drag-drop reorder (SortableJS), level filter

var layerFilter = 0;
var collapsedNodes = {};
var allCollapsed = false;

// --- Storage helpers ---
function storageKey() { return 'bom-collapsed-' + location.pathname.split('/')[4]; }
function saveCollapsed() { try { sessionStorage.setItem(storageKey(), JSON.stringify(collapsedNodes)); } catch (e) {} }
function restoreCollapsed() { try { collapsedNodes = JSON.parse(sessionStorage.getItem(storageKey()) || '{}'); } catch (e) {} }

// --- Load products into add modal ---
window.bomLoadProducts = function () {
    var bomId = me('[name="bom_id"]')?.value || '0';
    htmx.ajax('GET', '/admin/md/boms/products?bom_id=' + bomId, { target: '#bom-edit-product-results', swap: 'innerHTML' });
};

// --- Collapse / Expand ---
function applyVisibility() {
    var tbody = me('#bom-sortable-tbody');
    if (!tbody) return;
    any('tr[data-node-id]', tbody).forEach(function (tr) {
        var level = Number(tr.dataset.level);
        if (layerFilter !== 0 && layerFilter !== level) { tr.style.display = 'none'; return; }
        var ancestors = tr.dataset.ancestors || '';
        if (ancestors) {
            var ids = ancestors.split(',');
            for (var i = 0; i < ids.length; i++) {
                if (collapsedNodes[ids[i]]) { tr.style.display = 'none'; return; }
            }
        }
        tr.style.display = '';
    });
    any('.bom-collapse-btn', tbody).forEach(function (btn) {
        var row = btn.closest('tr');
        if (!row) return;
        btn.classList.toggle('bom-collapsed', !!collapsedNodes[row.dataset.nodeId]);
    });
}

window.bomToggleCollapse = function (nodeId) {
    collapsedNodes[nodeId] = !collapsedNodes[nodeId];
    saveCollapsed();
    applyVisibility();
};

window.bomToggleAllCollapse = function () {
    allCollapsed = !allCollapsed;
    var tbody = me('#bom-sortable-tbody');
    if (!tbody) return;
    var parentIds = [];
    any('tr[data-node-id]', tbody).forEach(function (r) {
        var nid = Number(r.dataset.nodeId);
        if (tbody.querySelector('tr[data-parent-id="' + nid + '"]')) parentIds.push(nid);
    });
    parentIds.forEach(function (nid) { collapsedNodes[nid] = allCollapsed; });
    saveCollapsed();
    applyVisibility();
    me('#bom-collapse-all-btn').textContent = allCollapsed ? '全部展开' : '全部折叠';
};

// --- Drag & Drop (SortableJS) ---
function initSortable() {
    var tbody = me('#bom-sortable-tbody');
    if (!tbody) return;
    var bomId = location.pathname.split('/')[4];

    new Sortable(tbody, {
        handle: 'tr[data-node-id]',
        draggable: 'tr[data-node-id]',
        animation: 150,
        ghostClass: 'bom-dragging',
        onEnd: function (evt) {
            var dragNodeId = Number(evt.item.dataset.nodeId);
            var parentId = Number(evt.item.dataset.parentId);
            var beforeId = '';
            var next = evt.item.nextElementSibling;
            while (next) {
                if (Number(next.dataset.parentId) === parentId) {
                    beforeId = Number(next.dataset.nodeId);
                    break;
                }
                next = next.nextElementSibling;
            }
            fetch('/admin/md/boms/' + bomId + '/nodes/' + dragNodeId + '/move', {
                method: 'POST',
                headers: { 'HX-Request': 'true', 'Content-Type': 'application/x-www-form-urlencoded' },
                body: 'new_parent_id=' + parentId + '&before_sibling_id=' + beforeId
            }).then(function (resp) {
                if (resp.ok) {
                    var loc = resp.headers.get('HX-Redirect');
                    location.href = loc || location.pathname;
                } else {
                    resp.text().then(function (msg) { alert(msg || '移动失败'); location.reload(); });
                }
            }).catch(function () { alert('网络错误'); location.reload(); });
        }
    });
}

// --- Init ---
function init() {
    restoreCollapsed();
    initSortable();
    applyVisibility();
    var filter = me('#bom-level-filter');
    if (filter) filter.addEventListener('change', function () {
        layerFilter = parseInt(this.value) || 0;
        applyVisibility();
    });
}

if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', init);
else init();
