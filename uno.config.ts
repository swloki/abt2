import presetIcons from "@unocss/preset-icons";
import { defineConfig, presetWind4 } from "unocss";

export default defineConfig({
  presets: [
    presetWind4(),
    // 纯 CSS 图标：i-lucide-* 由 @iconify-json/lucide 提供，编译期按需生成 mask 模式 CSS。
    // mask 模式下 background-color: currentColor，颜色跟随 text-* 继承；尺寸默认 em 跟随 font-size。
    presetIcons({
      warn: true,
      extraProperties: {
        display: "inline-block",
        "vertical-align": "middle",
      },
    }),
  ],

  // rules: UnoCSS 规则引擎——项目高频任意值模式原子化（tailwindcss 没有的 UnoCSS 灵魂能力）
  rules: [
    // 财务/数量列等宽数字（替代手写 [font-variant-numeric:tabular-nums]）
    ["tabular", { "font-variant-numeric": "tabular-nums" }],
  ],

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
  --purple: #7c3aed;
  --purple-bg: rgba(124, 58, 237, 0.06);

  --font-body: -apple-system, BlinkMacSystemFont, "Segoe UI", "PingFang SC", "Microsoft YaHei", "Helvetica Neue", sans-serif;
  --font-mono: "JetBrains Mono", "SF Mono", ui-monospace, Menlo, monospace;

  --text-xs: 12px;
  --text-sm: 14px;
  --text-base: 15px;
  --text-lg: 17px;
  --text-xl: 21px;
  --text-2xl: 28px;
  --text-3xl: 36px;
  /* presetWind4 internal fontSize variables (override defaults) */
  --text-xs-fontSize: 12px; --text-xs-lineHeight: 1.4;
  --text-sm-fontSize: 14px; --text-sm-lineHeight: 1.55;
  --text-base-fontSize: 15px; --text-base-lineHeight: 1.55;
  --text-lg-fontSize: 17px; --text-lg-lineHeight: 1.5;
  --text-xl-fontSize: 21px; --text-xl-lineHeight: 1.35;
  --text-2xl-fontSize: 28px; --text-2xl-lineHeight: 1.25;
  --text-3xl-fontSize: 36px; --text-3xl-lineHeight: 1.2;

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
/* 隐藏滚动条但保留滚动（横向滚动区等）——伪元素不可原子化，归 preflights */
.no-scrollbar { -ms-overflow-style: none; scrollbar-width: none; }
.no-scrollbar::-webkit-scrollbar { display: none; }
[x-cloak] { display: none !important; }
.font-mono { font-family: var(--font-mono); font-variant-numeric: tabular-nums; }

/* ─── Truly irreducible CSS (cannot be expressed as atomic utilities) ─── */
/* field-input focus changes sibling icon color */
.field-input:focus ~ .field-icon, .field-input:focus + .field-icon { color: var(--accent); }
/* perm-cell custom checkbox :checked::after draws a CSS checkmark */
.perm-cell input { width: 16px; height: 16px; cursor: pointer; appearance: none; -webkit-appearance: none; border: 2px solid #d1d5db; border-radius: 3px; background: #fff; transition: all .15s; flex-shrink: 0; position: relative; }
.perm-cell input:checked { background: #22c55e; border-color: #22c55e; }
.perm-cell input:checked::after { content: ''; position: absolute; left: 4px; top: 1px; width: 5px; height: 9px; border: solid #fff; border-width: 0 2px 2px 0; transform: rotate(45deg); }
.perm-cell input.perm-readonly { cursor: default; pointer-events: none; }
/* Global select: custom chevron (matches prototype .filter-select) */
select {
  appearance: none;
  -webkit-appearance: none;
  -moz-appearance: none;
  background-image: url("data:image/svg+xml,%3Csvg width='10' height='6' viewBox='0 0 10 6' fill='none' xmlns='http://www.w3.org/2000/svg'%3E%3Cpath d='M1 1l4 4 4-4' stroke='%23697386' stroke-width='1.5' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E");
  background-repeat: no-repeat;
  background-position: right 12px center;
  padding-right: 32px !important;
  cursor: pointer;
}
select:disabled {
  background-image: url("data:image/svg+xml,%3Csvg width='10' height='6' viewBox='0 0 10 6' fill='none' xmlns='http://www.w3.org/2000/svg'%3E%3Cpath d='M1 1l4 4 4-4' stroke='%23cbd5e1' stroke-width='1.5' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E");
  cursor: default;
}
/* Allow elements that set their own background-image (e.g. native date input arrow) to override */
/* app-shell JS-driven sidebar collapse + mobile state */

/* Toast: keyframes + progress bar (::after) + dismiss animation */
@keyframes toast-in { from { opacity: 0; transform: translateX(40px) scale(0.95); } to { opacity: 1; transform: translateX(0) scale(1); } }
@keyframes toast-out { from { opacity: 1; transform: translateX(0) scale(1); } to { opacity: 0; transform: translateX(40px) scale(0.95); } }
@keyframes toast-progress { from { width: 100%; } to { width: 0%; } }
.toast { position: relative; animation: toast-in 0.3s ease forwards; }
.toast::after { content: ''; position: absolute; bottom: 0; left: 0; height: 3px; opacity: 0.4; animation: toast-progress 4s linear forwards; }
.toast-error::after { background: var(--danger); }
.toast-success::after { background: var(--success); }
.toast-warning::after { background: var(--warn); }
.toast-info::after { background: var(--info); }
.toast.toast-dismiss { animation: toast-out 0.3s ease forwards; }
.toast.toast-dismiss::after { display: none; }

.app-shell { display: grid; grid-template-columns: var(--sidebar-w) 1fr; min-height: 100vh; transition: grid-template-columns var(--motion-base) var(--ease-standard); }
.app-shell.sidebar-collapsed { grid-template-columns: 56px 1fr; }
.sidebar-collapsed .sidebar-body { display: none; }
.sidebar-collapsed .sidebar-rail { border-right: none; }
/* Drawer overlay + slide animation */
.drawer-overlay { display: none !important; opacity: 0 !important; transition: opacity 0.35s ease-out; }
.drawer-overlay.open { display: flex !important; opacity: 1 !important; }
.drawer-overlay .drawer-panel { transform: translateX(100%) !important; transition: transform 0.35s cubic-bezier(0.16, 1, 0.3, 1) !important; }
.drawer-overlay.open .drawer-panel { transform: translateX(0) !important; }
`,
    },
  ],

  // Custom variants: prefix matches state class (not pseudo-class)
  variants: [
    (matcher) => {
      const map = { 'act:': '.active', 'show:': '.show', 'is-open:': '.is-open', 'is-visible:': '.is-visible', 'expanded:': '.expanded', 'toast-dismiss:': '.toast-dismiss' };
      for (const [prefix, cls] of Object.entries(map)) {
        if (matcher.startsWith(prefix)) {
          return { matcher: matcher.slice(prefix.length), selector: (s) => `${s}${cls}` };
        }
      }
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
      purple: { DEFAULT: "var(--purple)", bg: "var(--purple-bg)" },
    },
    fontSize: {
      xs: ["var(--text-xs)", "1.4"],
      sm: ["var(--text-sm)", "1.55"],
      base: ["var(--text-base)", "1.55"],
      lg: ["var(--text-lg)", "1.5"],
      xl: ["var(--text-xl)", "1.35"],
      "2xl": ["var(--text-2xl)", "1.25"],
      "3xl": ["var(--text-3xl)", "1.2"],
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
        'dialog-slide-in': '{from{opacity:0;transform:translateY(-16px) scale(0.96)}to{opacity:1;transform:translateY(0) scale(1)}}',
        'shimmer-bar': '{0%,100%{background-position:0% 0}50%{background-position:100% 0}}',
        'pulse-active': '{0%,100%{box-shadow:0 0 0 5px rgba(37,99,235,0.08),0 3px 14px rgba(37,99,235,0.3)}50%{box-shadow:0 0 0 8px rgba(37,99,235,0.06),0 4px 18px rgba(37,99,235,0.35)}}',
      },
      durations: {
        'dialog-slide-in': '0.2s',
        'shimmer-bar': '6s',
        'pulse-active': '2.5s',
      },
      timingFns: {
        'dialog-slide-in': 'ease-out',
        'shimmer-bar': 'ease-in-out',
        'pulse-active': 'ease-in-out',
      },
      counts: {
        'shimmer-bar': 'infinite',
        'pulse-active': 'infinite',
      },
    },
  },

  shortcuts: {
    // ─── Data Table ─── (107+ files use this pattern)
    "data-table": "w-full border-collapse [&_th]:py-2.5 [&_th]:px-4 [&_th]:text-left [&_th]:font-semibold [&_th]:text-muted [&_th]:text-xs [&_th]:uppercase [&_th]:tracking-wide [&_th]:bg-surface-raised [&_th]:[border-bottom:1px_solid_var(--border-soft)] [&_th]:whitespace-nowrap [&_td]:py-3 [&_td]:px-4 [&_td]:[border-bottom:1px_solid_var(--border-soft)] [&_td]:whitespace-nowrap [&_td]:align-middle [&_tbody_tr]:transition-colors [&_tbody_tr]:cursor-pointer [&_tbody_tr:hover]:bg-accent-bg [&_tbody_tr:last-child_td]:[border-bottom:none] [&_tbody_tr:hover_.row-actions]:opacity-100",
    // ─── Data Card ─── (container for tables, info sections)
    "data-card": "bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]",
    // ─── Form Section ─── (22+ files: customer/supplier/product/reconciliation create pages)
    "form-section": "bg-bg border border-border rounded-md p-6 mb-6",
    // ─── Form Field ─── (59+ files: label + input/select/textarea container)
    // label: block, xs, medium, fg-2, mb-1, nowrap; controls: w-full, px-3 py-2, border-border, rounded-sm, sm, bg-white, fg, focus→accent+shadow; textarea: resize-y, min-h-72px
    "form-field": "[&_label]:block [&_label]:text-xs [&_label]:font-medium [&_label]:text-fg-2 [&_label]:mb-1 [&_label]:whitespace-nowrap [&_input,&_select,&_textarea]:w-full [&_input,&_select,&_textarea]:px-3 [&_input,&_select,&_textarea]:py-2 [&_input,&_select,&_textarea]:border [&_input,&_select,&_textarea]:border-border [&_input,&_select,&_textarea]:rounded-sm [&_input,&_select,&_textarea]:text-sm [&_input,&_select,&_textarea]:bg-white [&_input,&_select,&_textarea]:text-fg [&_input,&_select,&_textarea]:outline-none [&_input,&_select,&_textarea]:transition-all [&_input,&_select,&_textarea]:duration-150 [&_input:focus,&_select:focus,&_textarea:focus]:border-accent [&_input:focus,&_select:focus,&_textarea:focus]:[box-shadow:var(--shadow-focus)] [&_textarea]:resize-y [&_textarea]:min-h-[72px]",
    // ─── Field Full ─── (18+ files: grid-column 1/-1, spans all columns)
    "field-full": "col-span-full",
    // ─── Status Pill ─── (65+ files: inline status badge shape; color resolved at call site via crate::utils::status_color())
    "status-pill": "inline-flex items-center text-[11px] px-2 py-0.5 rounded-full font-medium whitespace-nowrap",
  },
  cli: {
    entry: {
      patterns: ["abt-web/**/*.rs"],
      outFile: "static/app.css",
    },
  },
});
