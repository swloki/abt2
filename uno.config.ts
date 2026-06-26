import presetIcons from "@unocss/preset-icons";
import { defineConfig, presetWind4 } from "unocss";
import transformerVariantGroup from "@unocss/transformer-variant-group";

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

  // transformer-variant-group: 分组写法 prefix:(a b c) → prefix:a prefix:b prefix:c，
  // 用于收编 shortcut 和高频重复前缀。
  transformers: [transformerVariantGroup()],

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
  --border: #e2e8f0;
  --border-soft: #eef1f6;
  --accent: #2563eb;
  --accent-on: #ffffff;
  --accent-hover: #3b82f6;
  --accent-active: #1d4ed8;
  --accent-bg: rgba(37, 99, 235, 0.05);
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

  --motion-base: 240ms;
  --ease-standard: cubic-bezier(0.2, 0, 0, 1);

  /* ─── 色阶（Tailwind 标准色板，主色 = 现有 600 档）─── */
  /* danger (red): 600 = --danger */
  --danger-50: #fef2f2; --danger-100: #fee2e2; --danger-200: #fecaca; --danger-300: #fca5a5;
  --danger-400: #f87171; --danger-500: #ef4444; --danger-700: #b91c1c; --danger-800: #991b1b; --danger-900: #7f1d1d;
  /* success (green): 600 = --success */
  --success-50: #f0fdf4; --success-100: #dcfce7; --success-200: #bbf7d0; --success-300: #86efac;
  --success-400: #4ade80; --success-500: #22c55e; --success-700: #15803d; --success-800: #166534; --success-900: #14532d;
  /* warn (amber): 600 = --warn */
  --warn-50: #fffbeb; --warn-100: #fef3c7; --warn-200: #fde68a; --warn-300: #fcd34d;
  --warn-400: #fbbf24; --warn-500: #f59e0b; --warn-700: #b45309; --warn-800: #92400e; --warn-900: #78350f;
  /* accent (blue): 600 = --accent */
  --accent-50: #eff6ff; --accent-100: #dbeafe; --accent-200: #bfdbfe; --accent-300: #93c5fd;
  --accent-400: #60a5fa; --accent-500: #3b82f6; --accent-700: #1d4ed8; --accent-800: #1e40af; --accent-900: #1e3a8a;
  /* purple (violet): 600 = --purple */
  --purple-50: #f5f3ff; --purple-100: #ede9fe; --purple-200: #ddd6fe; --purple-300: #c4b5fd;
  --purple-400: #a78bfa; --purple-500: #8b5cf6; --purple-700: #6d28d9; --purple-800: #5b21b6; --purple-900: #4c1d95;
  /* slate (中性): fg=900, muted=500, border=200 */
  --slate-50: #f8fafc; --slate-100: #f1f5f9; --slate-200: #e2e8f0; --slate-300: #cbd5e1;
  --slate-400: #94a3b8; --slate-500: #64748b; --slate-600: #475569; --slate-700: #334155;
  --slate-800: #1e293b; --slate-900: #0f172a;
  /* sidebar 专用深色（特化视觉） */
  --sidebar-bg: #0b1829; --sidebar-rail: #070f1e;
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
/* cat-active: 分类树节点激活态（背景 + 左竖条 + 名称色联动），单 class 便于 hyperscript take 整体切换 */
.cat-row.cat-active { background-color: var(--accent-bg); }
.cat-row.cat-active::before { content: ''; position: absolute; left: 0; top: 0; bottom: 0; width: 3px; background: var(--accent); border-top-right-radius: 0.125rem; }
.cat-row.cat-active > .cat-name { color: var(--accent); font-weight: 600; }
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

/* 作业中心 grp 分组折叠（.open 切换 grp-body 显隐 + chevron 旋转）*/
.grp > .grp-body { display: none; }
.grp.open > .grp-body { display: block; }
.grp-chev { transition: transform var(--motion-base) var(--ease-standard); }
.grp.open > .grp-head .grp-chev { transform: rotate(180deg); }
/* 行展开 chevron（tr.open 时旋转 90°）*/
.expand-btn svg { transition: transform var(--motion-base) var(--ease-standard); }
tr.open .expand-btn svg { transform: rotate(90deg); }
/* 需求物料行懒加载展开区（.expanded 切换显隐）*/
.mat-expand { display: none; }
.mat-expand.expanded { display: block; }
`,
    },
  ],

  // Custom variants: prefix matches state class (not pseudo-class)
  variants: [
    (matcher) => {
      const map = { 'act:': '.active', 'show:': '.show', 'is-open:': '.is-open', 'is-visible:': '.is-visible', 'expanded:': '.expanded', 'open:': '.open', 'toast-dismiss:': '.toast-dismiss' };
      for (const [prefix, cls] of Object.entries(map)) {
        if (matcher.startsWith(prefix)) {
          return { matcher: matcher.slice(prefix.length), selector: (s) => `${s}${cls}` };
        }
      }
    },
    // 图标后代选择器：icon:w-4 等价于 [&_[class*=i-lucide]]:w-4，
    // 作用域到当前元素内部的 lucide 图标（presetIcons 生成的 <i>）。大幅缩短 539 处调用。
    (matcher) => {
      if (matcher.startsWith("icon:")) {
        return { matcher: matcher.slice(5), selector: (s) => `${s} [class*=i-lucide]` };
      }
    },
  ],
  theme: {
    colors: {
      bg: "var(--bg)",
      accent: {
        50: "var(--accent-50)", 100: "var(--accent-100)", 200: "var(--accent-200)", 300: "var(--accent-300)",
        400: "var(--accent-400)", 500: "var(--accent-500)", DEFAULT: "var(--accent)",
        700: "var(--accent-700)", 800: "var(--accent-800)", 900: "var(--accent-900)",
        on: "var(--accent-on)", hover: "var(--accent-hover)", active: "var(--accent-active)", bg: "var(--accent-bg)",
      },
      fg: { DEFAULT: "var(--fg)", 2: "var(--fg-2)" },
      muted: "var(--muted)",
      surface: {
        DEFAULT: "var(--surface)", raised: "var(--surface-raised)", warm: "var(--surface-warm)",
      },
      border: {
        DEFAULT: "var(--border)", soft: "var(--border-soft)",
      },
      success: {
        50: "var(--success-50)", 100: "var(--success-100)", 200: "var(--success-200)", 300: "var(--success-300)",
        400: "var(--success-400)", 500: "var(--success-500)", DEFAULT: "var(--success)",
        700: "var(--success-700)", 800: "var(--success-800)", 900: "var(--success-900)", bg: "var(--success-bg)",
      },
      warn: {
        50: "var(--warn-50)", 100: "var(--warn-100)", 200: "var(--warn-200)", 300: "var(--warn-300)",
        400: "var(--warn-400)", 500: "var(--warn-500)", DEFAULT: "var(--warn)",
        700: "var(--warn-700)", 800: "var(--warn-800)", 900: "var(--warn-900)", bg: "var(--warn-bg)",
      },
      danger: {
        50: "var(--danger-50)", 100: "var(--danger-100)", 200: "var(--danger-200)", 300: "var(--danger-300)",
        400: "var(--danger-400)", 500: "var(--danger-500)", DEFAULT: "var(--danger)",
        700: "var(--danger-700)", 800: "var(--danger-800)", 900: "var(--danger-900)", bg: "var(--danger-bg)",
      },
      info: "var(--info)",
      purple: {
        50: "var(--purple-50)", 100: "var(--purple-100)", 200: "var(--purple-200)", 300: "var(--purple-300)",
        400: "var(--purple-400)", 500: "var(--purple-500)", DEFAULT: "var(--purple)",
        700: "var(--purple-700)", 800: "var(--purple-800)", 900: "var(--purple-900)", bg: "var(--purple-bg)",
      },
      slate: {
        50: "var(--slate-50)", 100: "var(--slate-100)", 200: "var(--slate-200)", 300: "var(--slate-300)",
        400: "var(--slate-400)", 500: "var(--slate-500)", 600: "var(--slate-600)", 700: "var(--slate-700)",
        800: "var(--slate-800)", 900: "var(--slate-900)",
      },
      sidebar: { bg: "var(--sidebar-bg)", rail: "var(--sidebar-rail)" },
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
    "data-table": "w-full border-collapse [&_th]:(py-2.5 px-4 text-left font-semibold text-muted text-xs uppercase tracking-wide bg-surface-raised [border-bottom:1px_solid_var(--border-soft)] whitespace-nowrap) [&_td]:(py-3 px-4 [border-bottom:1px_solid_var(--border-soft)] whitespace-nowrap align-middle) [&_tbody_tr]:(transition-colors cursor-pointer) [&_tbody_tr:hover]:bg-accent-bg [&_tbody_tr:last-child_td]:[border-bottom:none] [&_tbody_tr:hover_.row-actions]:opacity-100",
    // ─── Data Card ─── (container for tables, info sections)
    "data-card": "bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]",
    // ─── Form Section ─── (22+ files: customer/supplier/product/reconciliation create pages)
    "form-section": "bg-bg border border-border rounded-md p-6 mb-6",
    // ─── Form Field ─── (59+ files: label + input/select/textarea container)
    // label: block, xs, medium, fg-2, mb-1, nowrap; controls: w-full, px-3 py-2, border-border, rounded-sm, sm, bg-white, fg, focus→accent+shadow; textarea: resize-y, min-h-72px
    "form-field": "[&_label]:(block text-xs font-medium text-fg-2 mb-1 whitespace-nowrap) [&_input,&_select,&_textarea]:(w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150) [&_input:focus,&_select:focus,&_textarea:focus]:(border-accent [box-shadow:var(--shadow-focus)]) [&_textarea]:(resize-y min-h-[72px])",
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
