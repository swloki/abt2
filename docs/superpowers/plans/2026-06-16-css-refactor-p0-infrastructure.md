# P0: CSS 基础设施重构实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 `:root` 变量、reset、`@keyframes` 从 `base.css` 迁移到 `uno.config.ts` 的 preflights 和 theme.animation 中，为后续 P1-P9 的原子化迁移建立地基。

**Architecture:** P0 不删除 base.css 中的任何内容，也不修改任何 Maud 模板。只在 `uno.config.ts` 中新增 preflights 和 theme.animation 定义，让 UnoCSS 生成的 app.css 顶部包含这些全局样式。由于 UnoCSS preflight layer 优先级低于 base.css 的显式规则，两者暂时共存不会冲突。

**Tech Stack:** UnoCSS v66.7.0 + presetWind4, Node.js

**设计文档:** `docs/superpowers/specs/2026-06-16-css-architecture-atomic-refactor-design.md`

---

### Task 1: 在 uno.config.ts 中添加 preflights 块

**Files:**
- Modify: `uno.config.ts:3-5`（在 `presets` 之后、`theme` 之前插入 preflights）

- [ ] **Step 1: 在 uno.config.ts 中添加 preflights 块**

在 `presets: [presetWind4()],` 之后插入：

```typescript
  // preflights: 全局样式（:root 变量 + reset + scrollbar + [x-cloak]）
  // 内容从 base.css 行 1-104 迁移，P0 阶段与 base.css 共存不冲突
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
```

- [ ] **Step 2: 验证 TypeScript 语法正确**

Run: `cd E:/work/abt && node -e "require('./uno.config.ts')" 2>&1 || npx tsc --noEmit uno.config.ts 2>&1 | head -5`

如果报 TS 解析错误，改用 tsx 验证：`cd E:/work/abt && npx tsx -e "import('./uno.config.ts')" 2>&1 | head -5`

Expected: 无错误（或仅有模块解析警告，不影响 UnoCSS CLI 读取）

---

### Task 2: 在 theme 中添加 animation 块

**Files:**
- Modify: `uno.config.ts`（在 `theme.radius` 之后、`shortcuts` 之前插入 `animation`）

- [ ] **Step 1: 在 theme 块中添加 animation 定义**

在 `radius: { ... },` 之后插入：

```typescript
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
```

注意：`spin` 是 UnoCSS presetWind4 内置动画，不需要手动定义 durations/timingFns/counts（默认 `1s linear infinite`）。但 keyframes 需要显式定义因为 preflight layer 不会自动注入。

- [ ] **Step 2: 验证 animation 配置被 UnoCSS 正确识别**

Run:
```bash
cd E:/work/abt && node -e "
const { createGenerator } = require('unocss');
(async () => {
  const config = require('./uno.config.ts').default || require('./uno.config.ts');
  const uno = await createGenerator(config);
  const { css } = await uno.generate('animate-toast-in animate-spin animate-pulse-active', { preflights: false });
  console.log(css);
})();
" 2>&1 | head -20
```

如果 require 无法加载 TS 文件，改用：
```bash
cd E:/work/abt && node_modules/.bin/unocss.exe --config uno.config.ts 2>&1 | tail -3
```

Expected: 输出包含 `@keyframes toast-in`、`@keyframes spin`、`@keyframes pulse-active` 和对应的 `.animate-*` 规则。

---

### Task 3: 构建并验证 app.css 不破坏现有页面

**Files:**
- 无文件修改（验证步骤）

- [ ] **Step 1: 重新构建 CSS**

Run: `cd E:/work/abt && npm run build:css`

Expected: `[success] N utilities generated to static/app.css`

- [ ] **Step 2: 验证 app.css 包含 preflights 内容**

Run（使用 search 工具）:
- 搜索 `static/app.css` 中是否包含 `::-webkit-scrollbar` 和 `[x-cloak]`

Expected: 两者在 app.css 的 preflight layer 中都存在。

- [ ] **Step 3: 确认 app.css 行数变化合理**

Run: `wc -l static/app.css`

Expected: 行数应从 ~5235 行增加（因为 preflights 在 base.css 拼接基础上又加了一份，暂时重复）。这是正常的——P0 阶段允许重复，后续 P1-P9 迁移时会逐步从 base.css 删除对应内容。

- [ ] **Step 4: 用 agent-browser 验证页面渲染正常**

Run:
```bash
agent-browser --cdp 9222 open "http://localhost:8000/admin/purchase/settings"
agent-browser --cdp 9222 eval "JSON.stringify({
  bodyBg: getComputedStyle(document.body).backgroundColor,
  bodyColor: getComputedStyle(document.body).color,
  bodyFont: getComputedStyle(document.body).fontFamily.substring(0, 50),
  scrollbar: getComputedStyle(document.querySelector('::-webkit-scrollbar')).width || 'N/A',
  accentVar: getComputedStyle(document.documentElement).getPropertyValue('--accent'),
})"
```

Expected:
- `bodyBg` 为 `rgb(240, 242, 247)`（var(--surface)）
- `bodyColor` 为 `rgb(15, 23, 42)`（var(--fg)）
- `accentVar` 为 `#2563eb`

- [ ] **Step 5: cargo clippy 验证编译**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

Expected: 无 error 输出（warnings 是已有的，不算）

---

### Task 4: 提交

- [ ] **Step 1: Git 提交**

```bash
cd E:/work/abt && git add uno.config.ts static/app.css && git commit -m "refactor(css): P0 — add preflights and theme.animation to uno.config.ts

Migrate :root variables, reset, scrollbar, [x-cloak] from base.css
into UnoCSS preflights. Add 7 @keyframes to theme.animation.
base.css remains untouched — coexistence verified, no page breakage."
```
