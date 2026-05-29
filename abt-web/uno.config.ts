import { defineConfig, presetWind4 } from "unocss";

// ────────────────────────────────────────────
// Preflight CSS: all hand-written styles
// injected into static/app.css by UnoCSS build
// ────────────────────────────────────────────

const designTokens = `
:root {
  --bg: #ffffff;
  --surface: #f5f7fa;
  --surface-raised: #fafbfc;
  --surface-warm: #e6f4ff;
  --fg: #141b2d;
  --fg-2: #4a5568;
  --muted: #697386;
  --meta: #1677ff;
  --border: #e2e8f0;
  --border-soft: #f0f2f5;
  --accent: #1677ff;
  --accent-on: #ffffff;
  --accent-hover: #4096ff;
  --accent-active: #0958d9;
  --accent-bg: rgba(22, 119, 255, 0.06);
  --success: #52c41a;
  --success-bg: rgba(82, 196, 26, 0.06);
  --warn: #faad14;
  --warn-bg: rgba(250, 173, 20, 0.06);
  --danger: #ff4d4f;
  --danger-bg: rgba(255, 77, 79, 0.06);
  --info: #1677ff;

  --font-body: "Plus Jakarta Sans", "Ant Sans", Inter, -apple-system, sans-serif;
  --font-mono: "JetBrains Mono", "SF Mono", ui-monospace, Menlo, monospace;

  --text-xs: 12px;
  --text-sm: 14px;
  --text-base: 15px;
  --text-lg: 17px;
  --text-xl: 21px;
  --text-2xl: 28px;
  --text-3xl: 36px;

  --space-1: 4px;
  --space-2: 8px;
  --space-3: 12px;
  --space-4: 16px;
  --space-5: 20px;
  --space-6: 24px;
  --space-8: 32px;
  --space-10: 40px;
  --space-12: 48px;

  --radius-sm: 4px;
  --radius-md: 6px;
  --radius-lg: 8px;
  --radius-xl: 12px;
  --radius-pill: 9999px;

  --sidebar-w: 240px;
  --header-h: 60px;

  --shadow-xs: 0 1px 2px rgba(0, 0, 0, 0.04);
  --shadow-sm: 0 1px 3px rgba(0, 0, 0, 0.06), 0 1px 2px rgba(0, 0, 0, 0.04);
  --shadow-md: 0 4px 12px rgba(0, 0, 0, 0.06), 0 1px 3px rgba(0, 0, 0, 0.04);
  --shadow-lg: 0 10px 32px rgba(0, 0, 0, 0.08), 0 2px 6px rgba(0, 0, 0, 0.04);
  --shadow-xl: 0 20px 48px rgba(0, 0, 0, 0.12), 0 4px 12px rgba(0, 0, 0, 0.06);
  --shadow-focus: 0 0 0 3px rgba(22, 119, 255, 0.15);

  --motion-fast: 150ms;
  --motion-base: 240ms;
  --motion-slow: 360ms;
  --ease-standard: cubic-bezier(0.2, 0, 0, 1);
  --ease-decelerate: cubic-bezier(0, 0, 0.2, 1);
  --ease-bounce: cubic-bezier(0.34, 1.56, 0.64, 1);
}
`;

const baseStyles = `
html { font-size: var(--text-sm); scroll-behavior: smooth; }
body {
  margin: 0;
  background: var(--surface);
  color: var(--fg);
  font-family: var(--font-body);
  font-size: var(--text-sm);
  line-height: 1.55;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}
button { cursor: pointer; }
input, select, textarea { font-family: inherit; font-size: inherit; line-height: 1.4; box-sizing: border-box; }
p { margin: 0; }

/* ─── Scrollbar ─── */
::-webkit-scrollbar { width: 6px; height: 6px; }
::-webkit-scrollbar-track { background: transparent; }
::-webkit-scrollbar-thumb { background: var(--border); border-radius: 3px; }
::-webkit-scrollbar-thumb:hover { background: var(--muted); }

/* ─── Alpine.js cloak ─── */
[x-cloak] { display: none !important; }

/* ─── Utility ─── */
.font-mono { font-family: var(--font-mono); font-variant-numeric: tabular-nums; }
`;

const layoutStyles = `
/* ─── Layout Shell ─── */
:root { --sidebar-w: 240px; --header-h: 60px; }
.app-shell {
  display: grid;
  grid-template-columns: var(--sidebar-w) 1fr;
  min-height: 100vh;
  transition: grid-template-columns var(--motion-base) var(--ease-standard);
}
.app-shell.sidebar-collapsed {
  grid-template-columns: 56px 1fr;
}

/* ─── Sidebar ─── */
#sidebar {
  background: linear-gradient(180deg, #0a1628 0%, #0f1d32 100%);
  color: rgba(255,255,255,0.85);
  display: flex;
  flex-direction: row;
  position: sticky;
  top: 0;
  height: 100vh;
  overflow: hidden;
  z-index: 20;
  transition: width var(--motion-base) var(--ease-standard);
}
#sidebar.sidebar-collapsed {
  width: 56px;
  min-width: 56px;
  overflow: visible;
}

/* ── Icon Rail ── */
.sidebar-rail {
  width: 56px; min-width: 56px;
  background: #070f1e;
  display: flex; flex-direction: column; align-items: center;
  padding: var(--space-3) 0;
  border-right: 1px solid rgba(255,255,255,0.04);
  flex-shrink: 0;
}
.rail-brand {
  width: 36px; height: 36px;
  border-radius: var(--radius-md);
  background: linear-gradient(135deg, var(--accent) 0%, #4096ff 100%);
  display: grid; place-items: center;
  margin-bottom: var(--space-5);
  box-shadow: 0 2px 8px rgba(22, 119, 255, 0.35);
  cursor: default;
}
.rail-brand svg { width: 18px; height: 18px; stroke: #fff; }
.rail-modules {
  flex: 1; display: flex; flex-direction: column; align-items: center;
  gap: 2px; width: 100%; overflow-y: auto; padding: 0 var(--space-1);
}
.rail-item {
  width: 44px; display: flex; flex-direction: column; align-items: center;
  gap: 3px; padding: 8px 0 6px; border: none; background: transparent;
  border-radius: var(--radius-sm); color: rgba(255,255,255,0.4);
  cursor: pointer; transition: all var(--motion-fast) var(--ease-standard);
  position: relative; text-decoration: none; font-size: inherit; font-family: inherit;
}
.rail-item:hover { color: rgba(255,255,255,0.85); background: rgba(255,255,255,0.06); }
.rail-item.active { color: #fff; background: rgba(22, 119, 255, 0.15); }
.rail-item.active::before {
  content: ''; position: absolute; left: -4px; top: 50%; transform: translateY(-50%);
  width: 3px; height: 20px; background: var(--accent); border-radius: 0 3px 3px 0;
}
.rail-icon { width: 20px; height: 20px; display: grid; place-items: center; }
.rail-icon svg { width: 18px; height: 18px; }
.rail-label { font-size: 10px; line-height: 1; white-space: nowrap; letter-spacing: 0.01em; }
.rail-item.active .rail-icon svg { stroke: var(--accent); }
.rail-bottom {
  display: flex; flex-direction: column; align-items: center; width: 100%;
  padding-top: var(--space-3); border-top: 1px solid rgba(255,255,255,0.06); margin-top: var(--space-2);
}
.rail-bottom .rail-item { color: rgba(255,255,255,0.25); }
.rail-bottom .rail-item:hover { color: rgba(255,255,255,0.6); }
.rail-collapse svg { width: 16px !important; height: 16px !important; opacity: 0.7; }
.rail-collapse:hover svg { opacity: 1; }

/* ── Sidebar Body ── */
.sidebar-body {
  flex: 1; min-width: 0; display: flex; flex-direction: column;
  overflow-y: auto; transition: width var(--motion-base) var(--ease-standard), opacity var(--motion-fast);
}
.sidebar-collapsed .sidebar-body { display: none; }
.sidebar-collapsed .sidebar-rail { border-right: none; }
.sidebar-module-header {
  padding: var(--space-4) var(--space-5); font-size: var(--text-sm); font-weight: 700;
  color: rgba(255,255,255,0.9); letter-spacing: -0.01em;
  border-bottom: 1px solid rgba(255,255,255,0.06);
  display: flex; align-items: center; gap: var(--space-2); flex-shrink: 0;
}
.module-header-icon { width: 18px; height: 18px; display: grid; place-items: center; }
.module-header-icon svg { width: 16px; height: 16px; stroke: var(--accent); }
.module-header-name { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }

/* ── Sidebar Nav ── */
.sidebar-nav { flex: 1; overflow-y: auto; padding: var(--space-2) 0; }
.sidebar-item {
  display: flex; align-items: center; gap: var(--space-3);
  padding: 9px var(--space-5); font-size: var(--text-sm); color: rgba(255,255,255,0.6);
  transition: all var(--motion-fast) var(--ease-standard);
  border-radius: var(--radius-sm); margin: 2px var(--space-3);
  cursor: pointer; position: relative; white-space: nowrap;
}
.sidebar-item:hover { background: rgba(255,255,255,0.06); color: rgba(255,255,255,0.95); }
.sidebar-item.active { background: rgba(22, 119, 255, 0.15); color: #fff; font-weight: 600; }
.sidebar-item.active::before {
  content: ''; position: absolute; left: 0; top: 50%; transform: translateY(-50%);
  width: 3px; height: 20px; background: var(--accent); border-radius: 0 3px 3px 0;
}
.sidebar-item svg { width: 18px; height: 18px; flex-shrink: 0; opacity: 0.55; transition: opacity var(--motion-fast); }
.sidebar-item:hover svg { opacity: 0.8; }
.sidebar-item.active svg { opacity: 1; color: var(--accent); stroke: var(--accent); }
.sidebar-item-text { overflow: hidden; text-overflow: ellipsis; }

/* ── Sidebar User ── */
.sidebar-user {
  margin-top: auto; padding: var(--space-4) var(--space-5);
  border-top: 1px solid rgba(255,255,255,0.06);
  display: flex; align-items: center; gap: var(--space-3);
}
.sidebar-user-avatar {
  width: 34px; height: 34px; border-radius: 50%;
  background: linear-gradient(135deg, var(--accent) 0%, #4096ff 100%);
  display: grid; place-items: center; font-size: 13px; font-weight: 700; color: #fff; flex-shrink: 0;
}
.sidebar-user-info { flex: 1; min-width: 0; }
.sidebar-user-name { font-size: var(--text-sm); font-weight: 600; color: #fff; }
.sidebar-user-role { font-size: 11px; color: rgba(255,255,255,0.4); }

/* ─── Main Content ─── */
.main-content { display: flex; flex-direction: column; min-height: 100vh; background: var(--surface); }

/* ─── Top Header ─── */
.top-header {
  height: var(--header-h); background: var(--bg); border-bottom: 1px solid var(--border-soft);
  display: flex; align-items: center; justify-content: space-between;
  padding: 0 var(--space-8); position: sticky; top: 0; z-index: 10; box-shadow: var(--shadow-xs);
}
.top-header-left { display: flex; align-items: center; gap: var(--space-4); }
.top-header-right { display: flex; align-items: center; gap: var(--space-4); }
.breadcrumb { display: flex; align-items: center; gap: var(--space-2); font-size: var(--text-sm); color: var(--muted); }
.breadcrumb-sep { color: var(--border); font-size: 12px; }
.header-icon-btn {
  width: 36px; height: 36px; border-radius: var(--radius-sm);
  border: 1px solid var(--border-soft); background: var(--bg);
  display: grid; place-items: center;
  transition: all var(--motion-fast) var(--ease-standard);
  position: relative; cursor: pointer;
}
.header-icon-btn:hover { background: var(--surface); border-color: var(--border); }
.header-icon-btn svg { width: 18px; height: 18px; color: var(--muted); }
.header-dot {
  position: absolute; top: 7px; right: 7px; width: 7px; height: 7px;
  border-radius: 50%; background: var(--danger); border: 2px solid var(--bg);
}
.avatar {
  width: 34px; height: 34px; border-radius: 50%;
  background: linear-gradient(135deg, var(--accent) 0%, #4096ff 100%);
  display: grid; place-items: center; font-size: 12px; font-weight: 700;
  color: #fff; letter-spacing: 0.02em;
}

/* ─── Page Content ─── */
.page-content { flex: 1; padding: var(--space-8); }

/* ─── Stat Icons ─── */
.stat-icon { width: 48px; height: 48px; border-radius: var(--radius-md); display: grid; place-items: center; flex-shrink: 0; }
.stat-icon.blue { background: linear-gradient(135deg, #e6f4ff 0%, #d6e8ff 100%); color: var(--accent); }
.stat-icon.green { background: linear-gradient(135deg, #f0fff0 0%, #e0ffe0 100%); color: var(--success); }
.stat-icon.orange { background: linear-gradient(135deg, #fff8eb 0%, #fff0d6 100%); color: var(--warn); }
.stat-icon.red { background: linear-gradient(135deg, #fff2f0 0%, #ffe8e6 100%); color: var(--danger); }
.stat-value { font-size: var(--text-2xl); font-weight: 700; line-height: 1.1; letter-spacing: -0.02em; }
.stat-label { font-size: 12px; color: var(--muted); margin-top: 3px; font-weight: 500; }

/* ─── Mobile ─── */
.mobile-menu-btn {
  display: none; width: 38px; height: 38px; border: none; background: transparent;
  border-radius: var(--radius-sm); place-items: center; cursor: pointer;
  flex-shrink: 0; transition: background var(--motion-fast);
}
.mobile-menu-btn:hover { background: var(--surface); }
.mobile-menu-btn svg { width: 22px; height: 22px; color: var(--fg); }
.mobile-nav {
  display: none; position: fixed; bottom: 0; left: 0; right: 0;
  height: 60px; background: var(--bg); border-top: 1px solid var(--border-soft);
  z-index: 30; box-shadow: 0 -2px 10px rgba(0,0,0,0.06);
}
.mobile-nav-scroll { height: 100%; overflow-x: auto; -webkit-overflow-scrolling: touch; scrollbar-width: none; }
.mobile-nav-scroll::-webkit-scrollbar { display: none; }
.mobile-nav-inner { display: flex; height: 100%; min-width: max-content; padding: 0 var(--space-1); }
.mobile-nav-item {
  display: flex; flex-direction: column; align-items: center; justify-content: center;
  gap: 3px; padding: 0 14px; font-size: 10px; color: var(--muted);
  text-decoration: none; white-space: nowrap; min-width: 60px; transition: color var(--motion-fast);
}
.mobile-nav-item svg { width: 20px; height: 20px; }
.mobile-nav-item.active { color: var(--accent); font-weight: 600; }
.mobile-nav-item.active svg { stroke: var(--accent); }
.mobile-sidebar-overlay {
  display: none; position: fixed; inset: 0; background: rgba(0,0,0,0.45);
  z-index: 50; backdrop-filter: blur(2px);
}
.mobile-sidebar-overlay.open { display: block; }

/* ─── Responsive Layout ─── */
@media (max-width: 768px) {
  .mobile-menu-btn { display: grid; }
  .mobile-nav { display: block; }
  .app-shell { grid-template-columns: 1fr !important; }
  #sidebar {
    position: fixed; left: 0; top: 0; bottom: 0; width: 280px;
    transform: translateX(-100%); z-index: 55;
    transition: transform var(--motion-base) var(--ease-standard);
  }
  #sidebar.mobile-open { transform: translateX(0); }
  .main-content { padding-bottom: 68px; }
  .page-content { padding: var(--space-4); }
  .top-header { padding: 0 var(--space-4); }
  .page-header { flex-direction: column; align-items: stretch; gap: var(--space-3); }
}
`;

const componentStyles = `
/* ─── Page Header ─── */
.page-actions { display: flex; gap: var(--space-3); }

/* ─── Customer Stats Grid ─── */
.customer-stats { display: grid; grid-template-columns: repeat(4, 1fr); gap: var(--space-5); margin-bottom: var(--space-6); }

/* ─── Filter Bar ─── */
.filter-bar { display: flex; align-items: center; gap: var(--space-3); margin-bottom: var(--space-6); flex-wrap: wrap; }
.search-input {
  width: 260px; padding: 8px 14px 8px 38px; border: 1px solid var(--border);
  border-radius: var(--radius-sm); background: var(--bg); font-size: var(--text-sm);
  color: var(--fg); outline: none; transition: all var(--motion-fast) var(--ease-standard);
}
.search-input:focus { border-color: var(--accent); box-shadow: var(--shadow-focus); }
.search-input::placeholder { color: var(--muted); opacity: 0.7; }
.search-wrap { position: relative; }
.search-wrap svg { position: absolute; left: 12px; top: 50%; transform: translateY(-50%); width: 16px; height: 16px; color: var(--muted); }
.filter-select {
  padding: 8px 32px 8px 14px; border: 1px solid var(--border);
  border-radius: var(--radius-sm); background: var(--bg); font-size: var(--text-sm);
  color: var(--fg); outline: none; transition: all var(--motion-fast) var(--ease-standard);
  appearance: none;
  background-image: url("data:image/svg+xml,%3Csvg width='10' height='6' viewBox='0 0 10 6' fill='none' xmlns='http://www.w3.org/2000/svg'%3E%3Cpath d='M1 1l4 4 4-4' stroke='%23697386' stroke-width='1.5' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E");
  background-repeat: no-repeat; background-position: right 12px center; cursor: pointer;
}
.filter-select:hover { border-color: var(--accent); }
.filter-select:focus { border-color: var(--accent); box-shadow: var(--shadow-focus); }

/* ─── Status Tabs ─── */
.status-tabs { display: flex; gap: var(--space-1); margin-bottom: var(--space-6); border-bottom: 1px solid var(--border-soft); }
.status-tab {
  padding: var(--space-3) var(--space-4); font-size: var(--text-sm); color: var(--muted);
  border-bottom: 2px solid transparent; cursor: pointer;
  transition: all var(--motion-fast); white-space: nowrap;
  border-top: none; border-left: none; border-right: none;
  background: none; text-decoration: none; display: inline-flex;
  align-items: center; gap: 6px;
}
.status-tab:hover { color: var(--fg); }
.status-tab.active { color: var(--accent); border-bottom-color: var(--accent); font-weight: 600; }
.status-tab .count {
  font-size: 11px; background: var(--surface); padding: 1px 7px;
  border-radius: var(--radius-pill); margin-left: var(--space-1); color: var(--muted); font-weight: 500;
}
.status-tab.active .count { background: var(--accent-bg); color: var(--accent); }

/* ─── Data Card / Table ─── */
.data-card-scroll { overflow-x: auto; -webkit-overflow-scrolling: touch; scrollbar-width: thin; scrollbar-color: var(--border) transparent; }
.data-card-scroll::-webkit-scrollbar { height: 6px; }
.data-card-scroll::-webkit-scrollbar-thumb { background: var(--border); border-radius: 3px; }
.data-card-head {
  padding: var(--space-4) var(--space-5); border-bottom: 1px solid var(--border-soft);
  display: flex; justify-content: space-between; align-items: center;
}
.data-card-head h3 { font-size: var(--text-base); font-weight: 600; color: var(--fg); margin: 0; }
.data-card-body { padding: var(--space-4); }
.data-table { width: 100%; min-width: 860px; font-size: var(--text-sm); }
.data-table th {
  padding: 11px var(--space-4); text-align: left; font-weight: 600; color: var(--muted);
  background: var(--surface-raised); font-size: 12px; white-space: nowrap;
  border-bottom: 1px solid var(--border-soft); text-transform: uppercase; letter-spacing: 0.04em;
}
.data-table td { padding: 13px var(--space-4); border-bottom: 1px solid var(--border-soft); vertical-align: middle; white-space: nowrap; }
.data-table tbody tr { transition: all var(--motion-fast) var(--ease-standard); cursor: pointer; }
.data-table tbody tr:hover { background: var(--accent-bg); }
.data-table tbody tr:last-child td { border-bottom: none; }
.data-table .mono { font-family: var(--font-mono); font-variant-numeric: tabular-nums; }
.data-table .link-cell {
  color: var(--accent); font-weight: 600; font-family: var(--font-mono);
  font-variant-numeric: tabular-nums; transition: color var(--motion-fast);
}
.data-table .link-cell:hover { color: var(--accent-hover); }
.data-table .row-actions { opacity: 0; transition: opacity var(--motion-fast); display: flex; gap: var(--space-1); }
.data-table tbody tr:hover .row-actions { opacity: 1; }
.row-action-btn {
  width: 28px; height: 28px; border: none; background: var(--surface);
  border-radius: var(--radius-sm); display: grid; place-items: center;
  cursor: pointer; transition: all var(--motion-fast);
}
.row-action-btn:hover { background: var(--accent-bg); color: var(--accent); }
.row-action-btn svg { width: 14px; height: 14px; }

/* ─── Credit Bar ─── */
.credit-bar { width: 60px; height: 4px; background: var(--border-soft); border-radius: 2px; overflow: hidden; }
.credit-bar-fill { height: 100%; border-radius: 2px; transition: width var(--motion-base); }

/* ─── Pagination ─── */
.pagination { display: flex; align-items: center; justify-content: space-between; padding: var(--space-4) var(--space-5); font-size: var(--text-xs); color: var(--muted); }
.pagination-pages { display: flex; gap: var(--space-1); }
.page-btn {
  width: 34px; height: 34px; display: grid; place-items: center;
  border: 1px solid var(--border-soft); border-radius: var(--radius-sm);
  background: var(--bg); color: var(--fg); font-size: var(--text-sm);
  cursor: pointer; transition: all var(--motion-fast) var(--ease-standard); text-decoration: none;
}
.page-btn:hover { border-color: var(--accent); color: var(--accent); background: var(--accent-bg); }
.page-btn.active { background: var(--accent); border-color: var(--accent); color: #fff; box-shadow: 0 1px 4px rgba(22, 119, 255, 0.25); }

/* ─── Status Pills ─── */
.status-pill {
  display: inline-flex; align-items: center; gap: 5px; padding: 3px 12px;
  border-radius: var(--radius-pill); font-size: 12px; font-weight: 500;
  line-height: 20px; white-space: nowrap; letter-spacing: 0.01em;
}
.status-pill::before { content: ''; width: 6px; height: 6px; border-radius: 50%; flex-shrink: 0; }
.status-draft { background: var(--surface); color: var(--muted); }
.status-draft::before { background: var(--muted); }
.status-info { background: #e8f4ff; color: #0958d9; }
.status-info::before { background: #0958d9; }
.status-accepted { background: #fff8eb; color: #d46b08; }
.status-accepted::before { background: #d46b08; }
.status-rejected { background: #fff2f0; color: #cf1322; }
.status-rejected::before { background: #cf1322; }
.status-progress { background: #fff8eb; color: #d46b08; }
.status-progress::before { background: #d46b08; }
.status-completed { background: #f0fff0; color: #389e0d; }
.status-completed::before { background: #389e0d; }
.status-shipped { background: #e8f4ff; color: #0958d9; }
.status-shipped::before { background: #0958d9; }

/* ─── Modal ─── */
.modal-overlay {
  display: none; position: fixed; inset: 0; background: rgba(15, 23, 42, 0.5);
  backdrop-filter: blur(4px); z-index: 1000; place-items: center;
}
.modal-overlay.is-open { display: grid; }
.modal {
  background: var(--bg); border-radius: var(--radius-lg); width: 680px;
  max-height: 85vh; overflow-y: auto; box-shadow: var(--shadow-xl);
  animation: modalSlideIn var(--motion-base) var(--ease-decelerate);
}
.modal-head {
  padding: var(--space-5) var(--space-6); border-bottom: 1px solid var(--border-soft);
  display: flex; justify-content: space-between; align-items: center;
}
.modal-head h2 { font-size: var(--text-lg); font-weight: 600; margin: 0; }
.modal-body { padding: var(--space-6); }
.modal-foot {
  padding: var(--space-4) var(--space-6); border-top: 1px solid var(--border-soft);
  display: flex; justify-content: flex-end; gap: var(--space-3); background: var(--surface-raised);
}
@keyframes modalFadeIn { from { opacity: 0 } to { opacity: 1 } }
@keyframes modalSlideIn { from { opacity: 0; transform: translateY(12px) scale(0.98) } to { opacity: 1; transform: translateY(0) scale(1) } }
.modal-lg { width: 900px; max-width: 94vw; }

/* ─── Back Link ─── */
.back-link {
  display: inline-flex; align-items: center; gap: 6px;
  color: var(--muted); font-size: var(--text-sm); margin-bottom: var(--space-3);
  transition: color var(--motion-fast);
}
.back-link:hover { color: var(--accent); }
.back-link svg { width: 16px; height: 16px; }

/* ─── Line Items (Create Page) ─── */
.line-num { font-family: var(--font-mono); color: var(--muted); font-size: var(--text-xs); text-align: center; }
.line-total { text-align: right; font-family: var(--font-mono); font-variant-numeric: tabular-nums; font-weight: 600; color: var(--fg); white-space: nowrap; }
.btn-remove-row {
  width: 28px; height: 28px; border: none; background: transparent;
  color: var(--muted); border-radius: var(--radius-sm); cursor: pointer;
  display: grid; place-items: center; transition: all var(--motion-fast);
}
.btn-remove-row:hover { background: #fff1f0; color: var(--danger); }
.btn-remove-row svg { width: 14px; height: 14px; }
.add-row-bar {
  padding: var(--space-3); border-top: 1px dashed var(--border);
  display: flex; align-items: center; gap: var(--space-2);
}
.btn-add-row {
  display: inline-flex; align-items: center; gap: var(--space-2);
  padding: 6px 14px; border: 1px dashed var(--border); border-radius: var(--radius-sm);
  background: transparent; color: var(--accent); font-size: var(--text-sm);
  cursor: pointer; transition: all var(--motion-fast);
}
.btn-add-row:hover { border-color: var(--accent); background: var(--accent-bg); }
.btn-add-row svg { width: 14px; height: 14px; }

/* ─── Totals Bar ─── */
.totals-bar {
  display: flex; justify-content: flex-end; padding: var(--space-4) var(--space-5);
  background: var(--surface); border-top: 1px solid var(--border); gap: var(--space-8);
}
.totals-item { display: flex; align-items: baseline; gap: var(--space-3); }
.totals-label { font-size: var(--text-sm); color: var(--muted); }
.totals-value { font-family: var(--font-mono); font-variant-numeric: tabular-nums; font-size: var(--text-lg); font-weight: 700; color: var(--fg); }
.totals-value.grand { color: var(--accent); font-size: var(--text-xl); }

/* ─── Create Action Bar (sticky bottom) ─── */
.create-action-bar {
  display: flex; align-items: center; justify-content: space-between;
  padding: var(--space-4) var(--space-6); background: var(--bg);
  border-top: 1px solid var(--border); position: sticky; bottom: 0;
  box-shadow: 0 -2px 8px rgba(0,0,0,0.04);
}

/* ─── Detail Header ─── */
.detail-header {
  display: flex; align-items: flex-start; justify-content: space-between;
  margin-bottom: var(--space-6);
}
.detail-title-row { display: flex; align-items: center; gap: var(--space-4); }
.detail-no { font-size: var(--text-xl); font-weight: 700; color: var(--fg); letter-spacing: -0.01em; }

/* ─── Workflow Steps ─── */
.workflow-steps { display: flex; align-items: center; gap: 0; margin-bottom: var(--space-6); }
.wf-step { display: flex; align-items: center; gap: var(--space-2); font-size: var(--text-xs); color: var(--muted); }
.wf-step.completed { color: var(--success); }
.wf-step.current { color: var(--accent); font-weight: 600; }
.wf-dot {
  width: 10px; height: 10px; border-radius: 50%; background: var(--border);
  transition: all var(--motion-base);
}
.wf-step.completed .wf-dot { background: var(--success); }
.wf-step.current .wf-dot { background: var(--accent); box-shadow: 0 0 0 4px var(--accent-bg); }
.wf-line {
  width: 48px; height: 2px; background: var(--border);
  margin: 0 var(--space-2); border-radius: 1px;
}
.wf-line.completed { background: var(--success); }

/* ─── Info Card (Detail Page) ─── */
.info-card {
  background: var(--bg); border: 1px solid var(--border-soft); border-radius: var(--radius-md);
  padding: var(--space-6); margin-bottom: var(--space-6);
  box-shadow: 0 1px 2px rgba(0,0,0,0.03); transition: box-shadow var(--motion-base);
}
.info-card:hover { box-shadow: 0 2px 8px rgba(0,0,0,0.06); }
.info-card-title {
  font-size: var(--text-base); font-weight: 600; color: var(--fg);
  margin-bottom: var(--space-4); padding-bottom: var(--space-3);
  border-bottom: 1px solid var(--border-soft);
}

/* ─── Info Grid (Detail Page) ─── */
.info-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(200px, 1fr)); gap: var(--space-5); }
.info-item { display: flex; flex-direction: column; gap: 4px; }
.info-label { font-size: 12px; font-weight: 500; color: var(--muted); letter-spacing: 0.02em; }
.info-value { font-size: var(--text-sm); color: var(--fg); font-weight: 500; }
.info-value.mono { font-family: var(--font-mono); font-variant-numeric: tabular-nums; }

/* ─── Amount Summary (Detail Page) ─── */
.amount-summary {
  display: flex; justify-content: flex-end; gap: var(--space-8);
  padding: var(--space-5) var(--space-6);
  border-top: 1px solid var(--border-soft); background: var(--surface-raised);
  border-radius: 0 0 var(--radius-md) var(--radius-md);
}
.amount-row { display: flex; align-items: baseline; gap: var(--space-3); }
.amount-label { font-size: var(--text-sm); color: var(--muted); }
.amount-value {
  font-family: var(--font-mono); font-variant-numeric: tabular-nums;
  font-size: var(--text-lg); font-weight: 700; color: var(--fg);
}
.amount-value.accent { color: var(--accent); }

/* ─── Product Select List (Modal) ─── */
.product-select-list { padding: var(--space-2) 0; }
.product-select-item {
  display: flex; align-items: center; justify-content: space-between;
  padding: var(--space-3) var(--space-5);
  border-bottom: 1px solid var(--border-soft);
  transition: background var(--motion-fast);
}
.product-select-item:last-child { border-bottom: none; }
.product-select-item:hover { background: var(--accent-bg); }
.product-select-info { flex: 1; min-width: 0; }
.product-select-name { font-size: var(--text-sm); font-weight: 500; color: var(--fg); margin-bottom: 2px; }
.product-select-meta { font-size: 12px; color: var(--muted); display: flex; align-items: center; gap: 6px; flex-wrap: wrap; }
.product-select-code { font-family: var(--font-mono); font-variant-numeric: tabular-nums; background: var(--surface); padding: 1px 6px; border-radius: var(--radius-sm); }
.product-select-sep { color: var(--border); }

/* ─── Product Search Bar (Modal) ─── */
.product-search-bar {
  display: flex; gap: var(--space-4);
  padding: var(--space-4) var(--space-5);
  border-bottom: 1px solid var(--border-soft);
}
.product-search-field { flex: 1; display: flex; flex-direction: column; gap: 4px; }
.product-search-label { font-size: 12px; font-weight: 500; color: var(--fg-2); }
.product-search-input {
  width: 100%; padding: 7px 10px; border: 1px solid var(--border);
  border-radius: var(--radius-sm); font-size: var(--text-sm);
  background: var(--bg); color: var(--fg); outline: none;
  transition: border-color var(--motion-fast), box-shadow var(--motion-fast);
}
.product-search-input:focus { border-color: var(--accent); box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 12%, transparent); }
.product-search-input::placeholder { color: var(--muted); opacity: 0.6; }
.product-search-clear {
  padding: 7px 14px; border: 1px solid var(--border); border-radius: var(--radius-sm);
  background: var(--bg); color: var(--fg-2); font-size: var(--text-sm);
  cursor: pointer; transition: all var(--motion-fast); white-space: nowrap;
  align-self: flex-end;
}
.product-search-clear:hover { border-color: var(--accent); color: var(--accent); background: var(--accent-bg); }

/* ─── Confirm Dialog ─── */
.dialog-overlay {
  display: none; position: fixed; inset: 0;
  background: rgba(15, 23, 42, 0.45); backdrop-filter: blur(6px);
  z-index: 1100; place-items: center;
  animation: modalFadeIn var(--motion-fast) var(--ease-decelerate);
}
.dialog-overlay.open { display: grid; }
.dialog {
  background: var(--bg); border-radius: var(--radius-md);
  width: 420px; max-width: 92vw;
  box-shadow: 0 25px 60px rgba(0,0,0,0.15), 0 8px 20px rgba(0,0,0,0.08);
  animation: dialogSlideIn 0.25s cubic-bezier(0.2, 0, 0, 1);
}
.dialog-icon-wrap {
  width: 56px; height: 56px; border-radius: 50%;
  background: color-mix(in srgb, var(--danger) 10%, transparent);
  display: flex; align-items: center; justify-content: center;
  margin: 0 auto var(--space-5);
}
.dialog-icon-wrap svg { width: 28px; height: 28px; color: var(--danger); }
.dialog-title { font-size: var(--text-lg); font-weight: 600; color: var(--fg); text-align: center; margin: 0 0 var(--space-2); }
.dialog-desc { font-size: var(--text-sm); color: var(--muted); text-align: center; margin: 0; line-height: 1.6; }
.dialog-desc strong { color: var(--fg); font-weight: 600; }
.dialog-body { padding: var(--space-8) var(--space-8) var(--space-6); display: flex; flex-direction: column; align-items: center; }
.dialog-foot {
  padding: var(--space-4) var(--space-6);
  border-top: 1px solid var(--border-soft);
  display: flex; justify-content: center; gap: var(--space-3);
  background: var(--surface); border-radius: 0 0 var(--radius-md) var(--radius-md);
}
.dialog-foot .btn { min-width: 100px; justify-content: center; }
.btn-danger { background: var(--danger) !important; color: #fff !important; border-color: var(--danger) !important; }
.btn-danger:hover { background: color-mix(in oklab, var(--danger), black 10%) !important; box-shadow: 0 4px 12px color-mix(in srgb, var(--danger) 25%, transparent); }
@keyframes dialogSlideIn { from { opacity: 0; transform: translateY(-16px) scale(0.96); } to { opacity: 1; transform: translateY(0) scale(1); } }

/* ─── Form ─── */
.form-section-title {
  font-size: var(--text-sm); font-weight: 600; color: var(--fg);
  margin-bottom: var(--space-4); padding-bottom: var(--space-2);
  border-bottom: 1px solid var(--border-soft);
}
.form-grid { display: grid; grid-template-columns: 1fr 1fr; gap: var(--space-4) var(--space-6); margin-bottom: var(--space-6); }
.form-grid .field-full { grid-column: 1 / -1; }
.form-field label { display: block; font-size: 12px; font-weight: 500; color: var(--fg-2); margin-bottom: var(--space-1); }
.form-field input,
.form-field select,
.form-field textarea {
  width: 100%; padding: 8px 12px; border: 1px solid var(--border);
  border-radius: var(--radius-sm); font-size: var(--text-sm); font-family: var(--font-body);
  transition: all var(--motion-fast) var(--ease-standard); background: #fff; color: var(--fg);
}
.form-field input:focus,
.form-field select:focus,
.form-field textarea:focus { outline: none; border-color: var(--accent); box-shadow: var(--shadow-focus); }
.form-field textarea { resize: vertical; min-height: 72px; }
.checkbox-label { display: inline-flex !important; align-items: center; gap: var(--space-2); cursor: pointer; }
.checkbox-label input[type="checkbox"] { width: auto; accent-color: var(--accent); }

/* ─── Tag Chips ─── */
.tag-chip { display: inline-flex; align-items: center; padding: 2px 10px; border-radius: var(--radius-pill); font-size: 11px; font-weight: 500; letter-spacing: 0.01em; }
.tag-key { background: #e6f4ff; color: #1677ff; }
.tag-normal { background: var(--surface); color: #666; }
.tag-potential { background: #f0fff0; color: #52c41a; }
.tag-primary { background: #e8f4ff; color: #1677ff; }
.tag-inactive { background: #fff2f0; color: #ff4d4f; }

/* ─── Detail Page ─── */
.detail-top { display: flex; justify-content: space-between; align-items: flex-start; margin-bottom: var(--space-6); }
.customer-identity { display: flex; align-items: center; gap: var(--space-5); }
.customer-avatar {
  width: 56px; height: 56px; border-radius: var(--radius-md);
  background: var(--surface-warm); display: grid; place-items: center;
  font-size: var(--text-xl); font-weight: 700; color: var(--accent); flex-shrink: 0;
}
.customer-name { font-size: var(--text-xl); font-weight: 700; margin: 0 0 4px 0; display: inline; }
.customer-meta { display: flex; gap: var(--space-4); color: var(--muted); font-size: var(--text-xs); margin-top: 4px; }
.customer-meta span { display: flex; align-items: center; gap: 4px; }

.detail-grid { display: grid; grid-template-columns: 1fr 1fr 340px; gap: var(--space-5); }
.detail-card { background: #fff; border: 1px solid var(--border-soft); border-radius: var(--radius-md); padding: var(--space-5) var(--space-6); }
.detail-card-title {
  font-size: var(--text-sm); font-weight: 600; margin-bottom: var(--space-4);
  padding-bottom: var(--space-2); border-bottom: 1px solid var(--border-soft);
  display: flex; align-items: center; justify-content: space-between;
}
.detail-row { display: flex; padding: var(--space-2) 0; font-size: var(--text-sm); }
.detail-label { width: 90px; flex-shrink: 0; color: var(--muted); }
.detail-value { color: var(--fg); }

/* ─── Credit Ring ─── */
.credit-display { text-align: center; padding: var(--space-5) 0; }
.credit-ring { width: 120px; height: 120px; margin: 0 auto var(--space-3); position: relative; }
.credit-ring svg { width: 100%; height: 100%; transform: rotate(-90deg); }
.credit-ring-text { position: absolute; inset: 0; display: flex; flex-direction: column; align-items: center; justify-content: center; }
.credit-ring-value { font-size: var(--text-lg); font-weight: 700; }
.credit-ring-label { font-size: 11px; color: var(--muted); }

/* ─── History Table ─── */
.history-table { width: 100%; border-collapse: collapse; }
.history-table th { text-align: left; font-size: 11px; font-weight: 500; color: var(--muted); padding: var(--space-2) var(--space-3); border-bottom: 1px solid var(--border-soft); }
.history-table td { padding: var(--space-3); font-size: var(--text-sm); border-bottom: 1px solid var(--border-soft); }
.history-table tr:last-child td { border-bottom: none; }

/* ─── Contact Card ─── */
.contact-card {
  padding: var(--space-3) var(--space-4); border: 1px solid var(--border-soft);
  border-radius: var(--radius-sm); margin-bottom: var(--space-3); transition: all var(--motion-fast);
}
.contact-card:hover { border-color: var(--border); background: var(--surface-raised); }
.contact-card-head { display: flex; align-items: center; gap: var(--space-2); margin-bottom: var(--space-2); }
.contact-card-head strong { font-size: var(--text-sm); color: var(--fg); }
.contact-info-row { display: flex; align-items: center; gap: var(--space-2); font-size: var(--text-xs); color: var(--fg-2); margin-top: 2px; }
.contact-info-row svg { width: 14px; height: 14px; color: var(--muted); flex-shrink: 0; }
.contact-card-actions { display: flex; justify-content: flex-end; margin-top: var(--space-2); }

/* ─── Address Card ─── */
.address-card {
  padding: var(--space-3) var(--space-4); border: 1px solid var(--border-soft);
  border-radius: var(--radius-sm); margin-bottom: var(--space-3); transition: all var(--motion-fast);
}
.address-card:hover { border-color: var(--border); background: var(--surface-raised); }
.address-card-head { display: flex; align-items: center; gap: var(--space-2); margin-bottom: var(--space-2); }
.address-card-body { font-size: var(--text-sm); color: var(--fg-2); line-height: 1.5; }
.address-card-body p { margin: 0; }
.address-contact { display: flex; align-items: center; gap: var(--space-2); margin-top: var(--space-2); font-size: var(--text-xs); color: var(--muted); }
.address-contact svg { width: 14px; height: 14px; flex-shrink: 0; }
.address-card-actions { display: flex; justify-content: flex-end; margin-top: var(--space-2); }

/* ─── Empty State ─── */
.empty-state { text-align: center; padding: var(--space-8) var(--space-4); color: var(--muted); font-size: var(--text-sm); }

/* ─── Utility ─── */
.mono { font-family: var(--font-mono); font-variant-numeric: tabular-nums; }
.text-danger { color: var(--danger); }
.num-right { text-align: right; }

/* ─── Component Responsive ─── */
@media (max-width: 1024px) {
  .detail-grid { grid-template-columns: 1fr 1fr; }
  .detail-top { flex-direction: column; gap: var(--space-4); }
}
@media (max-width: 768px) {
  .filter-bar { flex-wrap: wrap; }
  .search-input { width: 100%; }
  .data-table { font-size: 13px; }
  .data-table th { padding: 9px var(--space-3); }
  .data-table td { padding: 10px var(--space-3); }
  .detail-grid { grid-template-columns: 1fr; }
  .form-grid { grid-template-columns: 1fr; }
  .modal { width: 95vw; }
  .detail-top { flex-direction: column; }
}

`;

const loginStyles = `
/* ─── Login: Brand Panel ─── */
.brand-panel {
  background: linear-gradient(160deg, #0a1628 0%, #0f1d32 40%, #132847 100%);
  display: flex; flex-direction: column; justify-content: center; align-items: center;
  padding: 64px 56px; position: relative; overflow: hidden;
}
.brand-panel::before {
  content: ''; position: absolute; inset: 0;
  background-image:
    linear-gradient(rgba(22, 119, 255, 0.04) 1px, transparent 1px),
    linear-gradient(90deg, rgba(22, 119, 255, 0.04) 1px, transparent 1px);
  background-size: 48px 48px; pointer-events: none;
}
.brand-panel::after {
  content: ''; position: absolute; width: 400px; height: 400px; border-radius: 50%;
  background: radial-gradient(circle, rgba(22, 119, 255, 0.12) 0%, transparent 70%);
  top: 20%; right: -80px; pointer-events: none;
}
.brand-headline {
  font-size: var(--text-3xl); font-weight: 800; color: #fff;
  line-height: 1.2; letter-spacing: -0.03em; margin-bottom: 20px;
}
.brand-headline span {
  background: linear-gradient(135deg, var(--accent) 0%, #69b4ff 100%);
  -webkit-background-clip: text; -webkit-text-fill-color: transparent; background-clip: text;
}
.brand-desc { font-size: var(--text-base); color: rgba(255, 255, 255, 0.55); line-height: 1.7; margin-bottom: 48px; }

/* ─── Field input icon positioning ─── */
.field-icon {
  position: absolute; left: 13px; top: 50%; transform: translateY(-50%);
  width: 18px; height: 18px; color: var(--muted); pointer-events: none;
  transition: color var(--motion-fast);
}
.field-input:focus ~ .field-icon,
.field-input:focus + .field-icon { color: var(--accent); }

/* ─── Password toggle ─── */
.pw-toggle {
  position: absolute; right: 12px; top: 50%; transform: translateY(-50%);
  width: 32px; height: 32px; border: none; background: transparent;
  display: grid; place-items: center; cursor: pointer; color: var(--muted);
  border-radius: var(--radius-sm); transition: all var(--motion-fast);
}
.pw-toggle:hover { color: var(--fg-2); background: var(--surface); }

/* ─── Custom checkbox ─── */
.custom-checkbox {
  width: 16px; height: 16px; border-radius: 4px;
  border: 1.5px solid var(--border); appearance: none; -webkit-appearance: none;
  background: var(--bg); cursor: pointer; position: relative; transition: all var(--motion-fast);
}
.custom-checkbox:checked { background: var(--accent); border-color: var(--accent); }
.custom-checkbox:checked::after {
  content: ''; position: absolute; left: 4.5px; top: 1.5px;
  width: 5px; height: 9px; border: solid #fff; border-width: 0 2px 2px 0; transform: rotate(45deg);
}
.custom-checkbox:focus-visible { box-shadow: var(--shadow-focus); }

/* ─── Divider ─── */
.login-divider { display: flex; align-items: center; gap: 16px; margin: 28px 0; }
.login-divider::before, .login-divider::after { content: ''; flex: 1; height: 1px; background: var(--border); }

/* ─── Loading spinner ─── */
.spinner {
  width: 18px; height: 18px; border: 2px solid rgba(255, 255, 255, 0.3);
  border-top-color: #fff; border-radius: 50%; animation: spin 0.6s linear infinite;
}
@keyframes spin { to { transform: rotate(360deg); } }

/* ─── SSO button ─── */
.btn-sso {
  width: 100%; padding: 11px 24px; background: var(--bg); color: var(--fg-2);
  border: 1px solid var(--border); border-radius: var(--radius-md);
  font-size: var(--text-sm); font-weight: 500; font-family: var(--font-body);
  cursor: pointer; transition: all var(--motion-fast) var(--ease-standard);
  display: flex; align-items: center; justify-content: center; gap: 10px;
}
.btn-sso:hover { border-color: var(--accent); color: var(--accent); background: var(--accent-bg); }

/* ─── Toast ─── */
.toast {
  position: fixed; top: 24px; left: 50%; transform: translateX(-50%) translateY(-20px);
  padding: 14px 20px; border-radius: var(--radius-lg); font-size: 14px;
  font-weight: 500; z-index: 9999; box-shadow: 0 6px 24px rgba(0,0,0,0.12);
  opacity: 0; transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  display: flex; align-items: center; gap: 10px; max-width: 480px;
  backdrop-filter: blur(8px);
}
.toast.toast-show { opacity: 1; transform: translateX(-50%) translateY(0); }
.toast-icon { width: 20px; height: 20px; flex-shrink: 0; }
.toast-message { flex: 1; line-height: 1.5; }
.toast-close {
  background: none; border: none; color: inherit; cursor: pointer;
  font-size: 18px; line-height: 1; padding: 2px 4px; border-radius: 4px;
  opacity: 0.6; transition: opacity 0.15s;
}
.toast-close:hover { opacity: 1; background: rgba(255,255,255,0.15); }
.toast-error {
  background: rgba(255,77,79,0.95); color: #fff;
  border: 1px solid rgba(255,120,117,0.3);
}
.toast-success {
  background: rgba(82,196,26,0.95); color: #fff;
  border: 1px solid rgba(135,208,104,0.3);
}
.toast-warning {
  background: rgba(250,173,20,0.95); color: #fff;
  border: 1px solid rgba(255,212,100,0.3);
}

/* ─── Login Responsive ─── */
@media (max-width: 920px) {
  .login-shell { grid-template-columns: 1fr; }
  .brand-panel { display: none; }
  .login-panel { padding: 48px 28px; }
}
@media (max-width: 480px) {
  .login-panel { padding: 36px 20px; }
}
`;

export default defineConfig({
  presets: [presetWind4()],

  theme: {
    colors: {
      accent: {
        DEFAULT: "var(--accent)",
        on: "var(--accent-on)",
        hover: "var(--accent-hover)",
        active: "var(--accent-active)",
        bg: "var(--accent-bg)",
      },
      fg: { DEFAULT: "var(--fg)", 2: "var(--fg-2)" },
      muted: "var(--muted)",
      surface: {
        DEFAULT: "var(--surface)",
        raised: "var(--surface-raised)",
        warm: "var(--surface-warm)",
      },
      border: {
        DEFAULT: "var(--border)",
        soft: "var(--border-soft)",
      },
      success: { DEFAULT: "var(--success)", bg: "var(--success-bg)" },
      warn: { DEFAULT: "var(--warn)", bg: "var(--warn-bg)" },
      danger: { DEFAULT: "var(--danger)", bg: "var(--danger-bg)" },
      info: "var(--info)",
    },
    fontSize: {
      xs: "var(--text-xs)",
      sm: "var(--text-sm)",
      base: "var(--text-base)",
      lg: "var(--text-lg)",
      xl: "var(--text-xl)",
      "2xl": "var(--text-2xl)",
      "3xl": "var(--text-3xl)",
    },
    spacing: {
      1: "var(--space-1)",
      2: "var(--space-2)",
      3: "var(--space-3)",
      4: "var(--space-4)",
      5: "var(--space-5)",
      6: "var(--space-6)",
      8: "var(--space-8)",
      10: "var(--space-10)",
      12: "var(--space-12)",
    },
    borderRadius: {
      sm: "var(--radius-sm)",
      DEFAULT: "var(--radius-md)",
      lg: "var(--radius-lg)",
      xl: "var(--radius-xl)",
      pill: "var(--radius-pill)",
    },
  },

  shortcuts: {
    // ─── Buttons ───
    btn: "inline-flex items-center gap-2 px-[18px] py-[9px] rounded-sm border border-transparent text-sm font-medium transition-all duration-150 cursor-pointer whitespace-nowrap relative",
    "btn-primary":
      "btn bg-accent text-accent-on border-accent shadow-[0_1px_3px_rgba(22,119,255,0.25)] hover-bg-accent-hover hover-border-accent-hover hover-shadow-[0_2px_8px_rgba(22,119,255,0.3)] hover--translate-y-px active-bg-accent-active active-translate-y-0",
    "btn-default":
      "btn bg-white text-fg-2 border-border hover-border-accent hover-text-accent hover-bg-accent-bg",
    "btn-text":
      "btn bg-transparent text-fg-2 border-transparent px-3 hover-text-accent hover-bg-accent-bg",
    "btn-sm": "px-3 py-[5px] text-xs",
    "btn-danger":
      "btn !bg-danger !text-white !border-danger hover-shadow-[0_4px_12px_rgba(255,77,79,0.25)]",

    // ─── Data Card / Table ───
    "data-card":
      "bg-white rounded-md border border-border-soft shadow-sm p-5 hover-shadow-md transition-shadow duration-240",
    "data-table": "w-full min-w-[860px] text-sm",
    "link-cell":
      "text-accent font-semibold font-mono tabular-nums hover-text-accent-hover transition-colors duration-150",

    // ─── Status ───
    "status-pill":
      "inline-flex items-center gap-[5px] px-3 py-[3px] rounded-pill text-xs font-medium leading-5 whitespace-nowrap",
    "status-draft":
      "status-pill bg-surface text-muted before-content-empty before-w-1.5 before-h-1.5 before-rounded-full before-bg-muted",
    "status-info":
      "status-pill bg-[#e8f4ff] text-[#0958d9] before-content-empty before-w-1.5 before-h-1.5 before-rounded-full before-bg-[#0958d9]",
    "status-progress":
      "status-pill bg-[#fff8eb] text-[#d46b08] before-content-empty before-w-1.5 before-h-1.5 before-rounded-full before-bg-[#d46b08]",
    "status-success":
      "status-pill bg-[#f0fff0] text-[#389e0d] before-content-empty before-w-1.5 before-h-1.5 before-rounded-full before-bg-[#389e0d]",
    "status-danger":
      "status-pill bg-[#fff2f0] text-[#cf1322] before-content-empty before-w-1.5 before-h-1.5 before-rounded-full before-bg-[#cf1322]",

    // ─── Form ───
    "form-input":
      "w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus-border-accent focus-shadow-[0_0_0_3px_rgba(22,119,255,0.15)]",
    "form-select": "form-input appearance-none cursor-pointer",

    // ─── Info Card ───
    "info-card":
      "bg-white border border-border-soft rounded-md p-6 mb-6 shadow-xs hover-shadow-sm transition-shadow duration-240",
    "info-card-flat":
      "bg-white border border-border-soft rounded-md p-5 shadow-xs transition-shadow duration-240",
    "info-label": "text-xs text-muted font-medium tracking-wide",
    "amount-value": "font-mono tabular-nums text-lg font-bold text-fg",
    "amount-value-accent": "amount-value text-accent",
    "section-title": "text-lg font-semibold text-fg tracking-tight",

    // ─── Dashboard ───
    "dash-stat": "info-card-flat",
    "quick-link":
      "flex flex-col items-center gap-2 px-4 py-6 bg-white border border-border rounded-md transition-all duration-140 text-center hover-border-accent hover-shadow-sm",
    "flow-step": "flex flex-col items-center gap-2 min-w-[100px]",
    "flow-step-icon": "w-10 h-10 rounded-full grid place-items-center",
    "activity-row":
      "flex items-center gap-3 px-5 py-4 border-b border-border-soft cursor-pointer hover-bg-accent-bg",

    // ─── Stat Card ───
    "stat-card":
      "bg-white border border-border-soft rounded-md px-6 py-5 flex items-center gap-4 shadow-xs hover-shadow-sm hover--translate-y-px transition-all duration-240",

    // ─── Page ───
    "page-title": "text-xl font-bold text-fg tracking-tight",
    "page-header": "flex items-center justify-between mb-6",

    // ─── Login ───
    "login-shell": "grid grid-cols-2 min-h-screen",
    "login-panel":
      "flex flex-col justify-center items-center px-12 py-16 bg-white relative",
    "field-input":
      "w-full py-[11px] px-[14px] pl-[42px] border border-border rounded-md bg-white text-sm text-fg outline-none transition-all duration-150 hover-border-[#c4cdd5] focus-border-accent focus-shadow-[0_0_0_3px_rgba(22,119,255,0.15)]",
    "btn-login":
      "w-full py-3 px-6 bg-accent text-accent-on border-none rounded-md text-[15px] font-semibold cursor-pointer transition-all duration-150 shadow-[0_2px_8px_rgba(22,119,255,0.3)] flex items-center justify-center gap-2 hover-bg-accent-hover hover-shadow-[0_4px_16px_rgba(22,119,255,0.35)] hover--translate-y-px active-bg-accent-active active-translate-y-0",
  },

  preflights: [
    {
      getCSS: () =>
        [
          designTokens,
          baseStyles,
          layoutStyles,
          componentStyles,
          loginStyles,
        ].join("\n"),
    },
  ],
});
