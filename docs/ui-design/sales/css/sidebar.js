function renderSidebar(activeItem) {
  const navItems = [
    { section: '概览' },
    { name: '仪表盘', icon: 'home', href: 'index.html', id: 'dashboard' },
    { section: '销售管理' },
    { name: '客户管理', icon: 'users', href: 'customer-list.html', id: 'customer' },
    { name: '报价单', icon: 'file', href: 'quotation-list.html', id: 'quotation' },
    { name: '销售订单', icon: 'package', href: 'order-list.html', id: 'order' },
    { name: '发货申请', icon: 'truck', href: 'shipping-list.html', id: 'shipping' },
    { name: '销售退货', icon: 'return', href: 'return-list.html', id: 'return' },
    { name: '月对账单', icon: 'check', href: 'reconciliation-list.html', id: 'reconciliation' },
    { section: '其他' },
    { name: '产品管理', icon: 'box', href: '#', id: 'product' },
    { name: '库存管理', icon: 'archive', href: '#', id: 'inventory' },
    { name: '仓库管理', icon: 'warehouse', href: '#', id: 'warehouse' },
  ];

  const icons = {
    home: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-4 0a1 1 0 01-1-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 01-1 1h-2z"/></svg>',
    users: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0zm6 3a2 2 0 11-4 0 2 2 0 014 0zM7 10a2 2 0 11-4 0 2 2 0 014 0z"/></svg>',
    file: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"/></svg>',
    package: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M20 7l-8-4-8 4m16 0l-8 4m8-4v10l-8 4m0-10L4 7m8 4v10M4 7v10l8 4"/></svg>',
    truck: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M13 16V6a1 1 0 00-1-1H4a1 1 0 00-1 1v10m10 0H3m10 0h2m4 0h2V9a1 1 0 00-.293-.707l-3-3A1 1 0 0017.586 5H15m-2 11v-4a1 1 0 011-1h4m-1 6a1 1 0 100-2 1 1 0 000 2zm-8 0a1 1 0 100-2 1 1 0 000 2z"/></svg>',
    return: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M3 10h10a5 5 0 015 5v2M3 10l4-4M3 10l4 4"/></svg>',
    check: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2m-6 9l2 2 4-4"/></svg>',
    box: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M20 7l-8-4-8 4m16 0l-8 4m8-4v10l-8 4m0-10L4 7m8 4v10M4 7v10l8 4"/></svg>',
    archive: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M5 8h14M5 8a2 2 0 110-4h14a2 2 0 110 4M5 8v10a2 2 0 002 2h10a2 2 0 002-2V8m-9 4h4"/></svg>',
    warehouse: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M19 21V5a2 2 0 00-2-2H7a2 2 0 00-2 2v16m14 0h2m-2 0h-5m-9 0H3m2 0h5M9 7h1m-1 4h1m4-4h1m-1 4h1m-5 10v-5a1 1 0 011-1h2a1 1 0 011 1v5m-4 0h4"/></svg>',
  };

  let html = `
  <div class="sidebar-brand">
    <div class="sidebar-brand-icon">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 7l-8-4-8 4m16 0l-8 4m8-4v10l-8 4m0-10L4 7m8 4v10M4 7v10l8 4"/></svg>
    </div>
    <span>ABT ERP</span>
  </div>`;

  navItems.forEach(item => {
    if (item.section) {
      html += `<div class="sidebar-section">${item.section}</div>`;
    } else {
      const isActive = item.id === activeItem ? ' active' : '';
      html += `<a href="${item.href}" class="sidebar-item${isActive}">${icons[item.icon] || ''}${item.name}</a>`;
    }
  });

  html += `
  <div class="sidebar-user">
    <div class="sidebar-user-avatar">ZM</div>
    <div class="sidebar-user-info">
      <div class="sidebar-user-name">张明</div>
      <div class="sidebar-user-role">销售经理</div>
    </div>
  </div>`;

  document.getElementById('sidebar').innerHTML = html;
}
