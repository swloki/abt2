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
    "btn-danger-ghost":
      "btn bg-transparent text-[#dc2626] border-[1px] border-[#fca5a5] hover-bg-[#fef2f2]",

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
    "status-active":
      "status-pill bg-[#f0fff0] text-[#389e0d] before-content-empty before-w-1.5 before-h-1.5 before-rounded-full before-bg-[#389e0d]",
    "status-inactive":
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
  cli: {
    entry: {
      patterns: ["src/**/*.rs", "static/base.css"],
      outFile: "static/app.css",
    },
  },
});
