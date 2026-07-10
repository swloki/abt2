// BOM 工序拖拽排序（独立工序管理页 /admin/md/boms/{id}/operations）
// 复用 static/Sortable.min.js；onEnd 用 htmx.ajax 提交新顺序（非 fetch，遵循 no-fetch-submit）。
// 脚本随独立页 content 末尾加载一次；#ops-table 被 htmx 局部刷新后，通过 htmx:afterSwap 重新初始化。

(function () {
  function initBomOpSortable() {
    const tbody = document.getElementById('ops-tbody');
    if (!tbody || tbody.dataset.sortableInit === '1') return;
    if (typeof Sortable === 'undefined') return;
    tbody.dataset.sortableInit = '1';
    Sortable.create(tbody, {
      handle: '.handle',
      draggable: 'tr',
      animation: 150,
      ghostClass: 'opacity-30',
      onEnd() {
        const rows = Array.from(tbody.querySelectorAll('tr[data-step]'));
        if (rows.length === 0) return;
        const orders = rows.map((r, i) => ({
          step_order: parseInt(r.dataset.step, 10),
          new_order: i + 1,
        }));
        const reorderPath = location.pathname.replace(/\/$/, '') + '/reorder';
        if (typeof htmx === 'undefined') return;
        htmx.ajax('POST', reorderPath, {
          target: '#ops-table',
          swap: 'none',
          values: { orders: JSON.stringify(orders) },
        });
      },
    });
  }

  initBomOpSortable();
  document.body.addEventListener('htmx:afterSwap', (e) => {
    const t = e.detail && e.detail.target;
    if (t && t.id === 'ops-table') {
      // 新 tbody 进入，重置标记后重新初始化
      initBomOpSortable();
    }
  });
})();
