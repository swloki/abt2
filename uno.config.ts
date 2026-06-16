import { defineConfig, presetWind4 } from "unocss";

export default defineConfig({
  presets: [presetWind4()],

  // preflights: 全局样式（:root 变量 + reset + scrollbar + [x-cloak]）
  // 从 base.css 行 1-104 迁移
  preflights: [
    {
      getCSS: () => `
:root {
  --bg: #ffffff;
  --surface: #f0f2f7;
  --surface-raised: #f8f9fc;
  --surface-warm: #e6f4ff;
  --fg: #0f172a;
  --fg-2: #3b4a63;
  --muted: #64748b;
  --meta: var(--accent);
  --border: #e2e8f0;
  --border-soft: #eef1f6;
  --accent: #2563eb;
  --accent-on: #ffffff;
  --accent-hover: #3b82f6;
  --accent-active: #1d4ed8;
  --accent-bg: rgba(37, 99, 235, 0.05);
  --accent-glow: rgba(37, 99, 235, 0.12);
  --success: #16a34a;
  --success-bg: rgba(22, 163, 74, 0.06);
  --warn: #d97706;
  --warn-bg: rgba(217, 119, 6, 0.06);
  --danger: #dc2626;
  --danger-bg: rgba(220, 38, 38, 0.05);
  --info: #2563eb;

  --font-body: -apple-system, BlinkMacSystemFont, "Segoe UI", "PingFang SC", "Microsoft YaHei", "Helvetica Neue", sans-serif;
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

  --radius-sm: 6px;
  --radius-md: 8px;
  --radius-lg: 12px;
  --radius-xl: 16px;
  --radius-pill: 9999px;

  --sidebar-w: 240px;
  --header-h: 60px;

  --shadow-xs: 0 1px 2px rgba(15, 23, 42, 0.04);
  --shadow-sm: 0 1px 3px rgba(15, 23, 42, 0.05), 0 1px 2px rgba(15, 23, 42, 0.03);
  --shadow-md: 0 4px 16px rgba(15, 23, 42, 0.06), 0 1px 3px rgba(15, 23, 42, 0.04);
  --shadow-lg: 0 12px 40px rgba(15, 23, 42, 0.08), 0 4px 12px rgba(15, 23, 42, 0.04);
  --shadow-xl: 0 24px 56px rgba(15, 23, 42, 0.12), 0 8px 20px rgba(15, 23, 42, 0.06);
  --shadow-card: 0 1px 3px rgba(15, 23, 42, 0.04), 0 0 0 1px rgba(15, 23, 42, 0.03);
  --shadow-card-hover: 0 8px 30px rgba(15, 23, 42, 0.08), 0 0 0 1px rgba(15, 23, 42, 0.04);
  --shadow-focus: 0 0 0 3px rgba(37, 99, 235, 0.12);
  --shadow-accent: 0 4px 14px rgba(37, 99, 235, 0.25);

  --motion-fast: 150ms;
  --motion-base: 240ms;
  --motion-slow: 360ms;
  --ease-standard: cubic-bezier(0.2, 0, 0, 1);
  --ease-decelerate: cubic-bezier(0, 0, 0.2, 1);
  --ease-bounce: cubic-bezier(0.34, 1.56, 0.64, 1);
  --glass-bg: rgba(255, 255, 255, 0.72);
  --glass-border: rgba(255, 255, 255, 0.18);
  --glass-blur: 12px;
}

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
::-webkit-scrollbar { width: 6px; height: 6px; }
::-webkit-scrollbar-track { background: transparent; }
::-webkit-scrollbar-thumb { background: var(--border); border-radius: 3px; }
::-webkit-scrollbar-thumb:hover { background: var(--muted); }
[x-cloak] { display: none !important; }
.font-mono { font-family: var(--font-mono); font-variant-numeric: tabular-nums; }
`,
    },
  ],

  theme: {
    colors: {
      bg: "var(--bg)",
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
    radius: {
      sm: "var(--radius-sm)",
      DEFAULT: "var(--radius-md)",
      lg: "var(--radius-lg)",
      xl: "var(--radius-xl)",
      pill: "var(--radius-pill)",
    },
    boxShadow: {
      xs: "var(--shadow-xs)",
      sm: "var(--shadow-sm)",
      md: "var(--shadow-md)",
      lg: "var(--shadow-lg)",
      xl: "var(--shadow-xl)",
      card: "var(--shadow-card)",
      "card-hover": "var(--shadow-card-hover)",
      focus: "var(--shadow-focus)",
      accent: "var(--shadow-accent)",
    },
    animation: {
      keyframes: {
        spin: '{to{transform:rotate(360deg)}}',
        'toast-in': '{from{opacity:0;transform:translateX(40px) scale(0.95)}to{opacity:1;transform:translateX(0) scale(1)}}',
        'toast-out': '{from{opacity:1;transform:translateX(0) scale(1)}to{opacity:0;transform:translateX(40px) scale(0.95)}}',
        'toast-progress': '{from{width:100%}to{width:0%}}',
        'dialog-slide-in': '{from{opacity:0;transform:translateY(-16px) scale(0.96)}to{opacity:1;transform:translateY(0) scale(1)}}',
        'shimmer-bar': '{0%,100%{background-position:0% 0}50%{background-position:100% 0}}',
        'pulse-active': '{0%,100%{box-shadow:0 0 0 5px rgba(37,99,235,0.08),0 3px 14px rgba(37,99,235,0.3)}50%{box-shadow:0 0 0 8px rgba(37,99,235,0.06),0 4px 18px rgba(37,99,235,0.35)}}',
      },
      durations: {
        'toast-in': '0.3s',
        'toast-out': '0.3s',
        'toast-progress': '4s',
        'dialog-slide-in': '0.2s',
        'shimmer-bar': '6s',
        'pulse-active': '2.5s',
      },
      timingFns: {
        'toast-in': 'ease',
        'toast-out': 'ease',
        'toast-progress': 'linear',
        'dialog-slide-in': 'ease-out',
        'shimmer-bar': 'ease-in-out',
        'pulse-active': 'ease-in-out',
      },
      counts: {
        'toast-progress': '1',
        'shimmer-bar': 'infinite',
        'pulse-active': 'infinite',
      },
    },
  },

  shortcuts: {
    // ─── Dashboard ───
    "flow-step": "flex flex-col items-center gap-2 min-w-[100px]",
    "flow-step-icon": "w-10 h-10 rounded-full grid place-items-center",
    "activity-row":
      "flex items-center gap-3 px-5 py-4 border-b border-border-soft cursor-pointer hover-bg-accent-bg",

    // ─── Board Stats (Schedule Board) ───
    "board-stats": "grid grid-cols-5 gap-4 mb-6",
    "stat-card-value": "text-2xl font-bold font-mono tabular-nums",
    "stat-card-label": "text-sm text-muted mt-1",
    "stat-pending": "text-muted",
    "stat-progress": "text-[#fa8c16]",
    "stat-receipt": "text-accent",
    "stat-done": "text-[var(--success)]",

    // ─── Kanban Board ───
    "kanban-board": "grid grid-cols-4 gap-4",
    "kanban-column":
      "bg-[var(--surface)] rounded-md border border-border-soft min-h-[400px] flex flex-col",
    "kanban-col-header":
      "flex items-center justify-between px-4 py-3 border-b border-border-soft",
    "kanban-col-title": "text-sm font-semibold text-fg",
    "kanban-col-count":
      "text-xs text-muted bg-[rgba(0,0,0,0.04)] px-2 py-0.5 rounded-full",
    "kanban-col-body": "flex-1 p-3 flex flex-col gap-3 overflow-y-auto",
    "kanban-empty": "text-sm text-muted text-center py-8",
    "kanban-card":
      "block bg-white rounded-md border border-border-soft p-4 shadow-xs hover-shadow-md hover--translate-y-px transition-all duration-200 cursor-pointer",
    "kanban-card-top": "flex items-center justify-between mb-2",
    "kanban-card-no": "text-sm font-semibold",
    "kanban-card-pill": "text-[10px] px-2 py-0.5 rounded-full font-medium",
    "kanban-card-product": "text-sm text-fg-2 mb-2",
    "kanban-card-meta": "text-xs text-muted",
    "kanban-card-progress": "mt-2",
    "kanban-card-tag":
      "mt-2 text-[10px] text-muted bg-[rgba(0,0,0,0.03)] px-2 py-0.5 rounded",

    // ─── Progress Bar ───
    "progress-bar": "h-1.5 bg-[rgba(0,0,0,0.06)] rounded-full overflow-hidden",
    "progress-fill":
      "h-full bg-accent rounded-full transition-all duration-300",
    "progress-text": "text-[10px] text-muted mt-1",

    // ─── Kanban Card Pill Colors ───
    "pill-pending": "bg-[rgba(0,0,0,0.04)] text-muted",
    "pill-progress": "bg-[rgba(250,140,22,0.08)] text-[#d46b08]",
    "pill-suspended": "bg-[rgba(245,63,63,0.06)] text-[#f53f3f]",
    "pill-receipt": "bg-[rgba(22,119,255,0.08)] text-accent",
    "pill-done": "bg-[rgba(82,196,26,0.08)] text-[var(--success)]",

    // ─── Kanban Column Accent Colors ───
    "kanban-col-pending": "",
    "kanban-col-progress": "",
    "kanban-col-receipt": "",
    "kanban-col-done": "",

    // ─── Material Usage ───
    "usage-summary": "grid grid-cols-4 gap-4 mb-6",
    "text-center-empty": "text-center py-8 text-sm text-muted",
    "diff-indicator": "text-xs font-medium font-mono",
    "diff-positive": "text-[var(--danger)]",
    "diff-negative": "text-[var(--success)]",
    "diff-zero": "text-muted",

    // ─── Timeline ───
    timeline: "flex flex-col gap-0",
    "timeline-item":
      "flex gap-4 pb-4 border-l-2 border-border-soft pl-4 ml-1 relative",
    "timeline-dot":
      "w-2.5 h-2.5 rounded-full bg-accent absolute -left-[6px] top-1",
    "timeline-content": "flex-1 min-w-0",
    "timeline-time": "text-xs text-muted",
    "timeline-action": "text-sm font-medium text-fg mt-0.5",
    "timeline-desc": "text-sm text-muted mt-1",

    // ─── Login ───
    "login-shell": "grid grid-cols-2 min-h-screen",
    "login-panel":
      "flex flex-col justify-center items-center px-12 py-16 bg-white relative",
    "field-input":
      "w-full py-[11px] px-[14px] pl-[42px] border border-border rounded-md bg-white text-sm text-fg outline-none transition-all duration-150 hover-border-[#c4cdd5] focus-border-accent focus-shadow-[0_0_0_3px_rgba(22,119,255,0.15)]",
    "btn-login":
      "w-full py-3 px-6 bg-accent text-accent-on border-none rounded-md text-[15px] font-semibold cursor-pointer transition-all duration-150 shadow-[0_2px_8px_rgba(22,119,255,0.3)] flex items-center justify-center gap-2 hover-bg-accent-hover hover-shadow-[0_4px_16px_rgba(22,119,255,0.35)] hover--translate-y-px active-bg-accent-active active-translate-y-0",
  },
  cli: {
    entry: {
      patterns: ["abt-web/**/*.rs", "static/base.css"],
      outFile: "static/app.css",
    },
  },
});
