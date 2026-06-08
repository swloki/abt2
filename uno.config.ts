import { defineConfig, presetWind4 } from "unocss";

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
    radius: {
      sm: "var(--radius-sm)",
      DEFAULT: "var(--radius-md)",
      lg: "var(--radius-lg)",
      xl: "var(--radius-xl)",
      pill: "var(--radius-pill)",
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
