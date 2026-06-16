# CSS 架构重构设计：从手写 base.css 到 100% 纯原子 UnoCSS

> 日期: 2026-06-16
> 状态: 已确认
> 方案: B — 100% 纯原子，零手写组件 CSS

## 一、问题诊断

### 现状

```
base.css (4476行, 1065 class)  ──┐
                                  ├──► UnoCSS CLI ──► app.css (5235行)
uno.config.ts (80 shortcuts)  ──┘                       │
                                                        ▼
173 个 Maud 页面 (8193 class= 引用) ◄── 页面只加载 app.css
```

三套规则源（base.css 手写 class、uno.config.ts shortcuts、UnoCSS 内置 utility）混在同一个 app.css 中。

### 核心缺陷

1. **特异性战争**：`.form-field label{display:block}`（特异性 0-1-1）覆盖 `.checkbox-row{display:flex}`（0-1-0），只要组件在 `.form-field` 内手写规则就赢
2. **同名冲突**：`flow-step`、`board-stats`、`progress-bar`、`progress-fill`、`login-shell`、`login-panel` 在 shortcuts 和 base.css 中各定义一套
3. **死代码积累**：删一个组件后对应的 CSS 永远留在 base.css 中
4. **改一个 class 要翻 4476 行文件**

### 现状数据

| 分类 | class 数 | 占比 | 特征 |
|---|---|---|---|
| 简单可替代 | ~350 | 33% | display+padding+border 组合，无伪元素/动画 |
| 中等（需状态/过渡） | ~400 | 38% | 含 `:focus`/`:hover` transition、`active` 状态切换 |
| 复杂（含伪元素/动画） | ~315 | 30% | 含 `::before/::after`、`@keyframes`、`backdrop-filter`、`@media` |

base.css 中的复杂 CSS 统计：91 处伪元素、8 个 `@keyframes`、25 处 `@media`、12 处 `calc()`、1474 条嵌套选择器。

## 二、目标架构

### 文件结构

| 文件 | 内容 | 行数(估) | 维护方式 |
|---|---|---|---|
| `uno.config.ts` | theme（colors/fontSize/spacing/radius/animation）+ preflights（:root 变量 + reset + scrollbar + `[x-cloak]`） | ~250 | 手动编辑 |
| `static/app.css` | UnoCSS CLI 纯输出（preflights + utilities） | ~2000 | 自动生成 |
| `static/base.css` | **删除** | 0 | — |

### 关键变化

1. **`:root` 变量 + reset** → 移入 `uno.config.ts` 的 `preflights`
2. **8 个 `@keyframes`** → 移入 `uno.config.ts` 的 `theme.animation.keyframes`
3. **91 处伪元素** → Maud 中用 `before:*` / `after:*` 前缀表达
4. **25 处 `@media`** → UnoCSS 的 `sm:` / `md:` / `lg:` 响应式前缀
5. **80 个 shortcuts** → 全部清空，组件样式直接内联到 Maud 的 `class=""`
6. **CLI entry.patterns** → 移除 `static/base.css`，只扫描 `abt-web/**/*.rs`

### `uno.config.ts` 目标结构

```typescript
import { defineConfig, presetWind4 } from "unocss";

export default defineConfig({
  presets: [presetWind4()],

  // preflights: 全局样式（替代 base.css 的 :root + reset 部分）
  preflights: [
    {
      content: `
        :root {
          /* 颜色 token (18个) */
          --bg: #ffffff;
          --surface: #f0f2f7;
          /* ... 全部变量从 base.css 行 1-76 迁移 ... */

          /* 字体/字号/间距/圆角/布局/阴影/动画/玻璃 token */
        }

        html { font-size: var(--text-sm); scroll-behavior: smooth; }
        body { margin: 0; background: var(--bg); color: var(--fg);
               font-family: var(--font-body); line-height: 1.55;
               -webkit-font-smoothing: antialiased; }
        button { cursor: pointer; }
        input, select, textarea { font-family: inherit; box-sizing: border-box; }
        p { margin: 0; }
        ::-webkit-scrollbar { width: 6px; height: 6px; }
        ::-webkit-scrollbar-track { background: transparent; }
        ::-webkit-scrollbar-thumb { background: var(--border); border-radius: 3px; }
        [x-cloak] { display: none !important; }
      `,
    },
  ],

  theme: {
    colors: { /* 保持不变 */ },
    fontSize: { /* 保持不变 */ },
    spacing: { /* 保持不变 */ },
    radius: { /* 保持不变 */ },

    // 动画定义（从 base.css 的 8 个 @keyframes 迁移）
    animation: {
      keyframes: {
        spin: '{to{transform:rotate(360deg)}}',
        'toast-in': '{from{opacity:0;transform:translateY(8px)}to{opacity:1;transform:translateY(0)}}',
        'toast-out': '{from{opacity:1;transform:translateY(0)}to{opacity:0;transform:translateY(8px)}}',
        'toast-progress': '{from{width:100%}to{width:0%}}',
        'dialog-slide-in': '{from{opacity:0;transform:translateY(-12px) scale(0.97)}to{opacity:1;transform:translateY(0) scale(1)}}',
        'shimmer-bar': '{0%{background-position:-200% 0}100%{background-position:200% 0}}',
        'pulse-active': '{0%,100%{box-shadow:0 0 0 0 rgba(22,119,255,0.4)}50%{box-shadow:0 0 0 6px rgba(22,119,255,0)}}',
      },
      durations: {
        'toast-in': '0.3s', 'toast-out': '0.3s', 'toast-progress': '4s',
        'dialog-slide-in': '0.2s', 'shimmer-bar': '2s', 'pulse-active': '2s',
      },
      timingFns: {
        'toast-in': 'ease-out', 'toast-out': 'ease-in', 'toast-progress': 'linear',
        'dialog-slide-in': 'ease-out', 'shimmer-bar': 'linear', 'pulse-active': 'ease-in-out',
      },
      counts: {
        'toast-progress': '1', 'shimmer-bar': 'infinite', 'pulse-active': 'infinite',
      },
    },
  },

  // shortcuts: 清空（所有样式内联到 Maud class="" 中）
  shortcuts: {},

  cli: {
    entry: {
      patterns: ["abt-web/**/*.rs"],  // 移除 "static/base.css"
      outFile: "static/app.css",
    },
  },
});
```

## 三、复杂模式原子化映射规则

### 3.1 Status Pill（55 个 class → 1 个原子模式）

当前 `.status-pill::before` + 40+ 种 `.status-xxx::before{background}` 散落在 base.css 的 6 处定义。

**原子化写法：**

```rust
// Maud 中：
span class="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-xs
           before:content-[''] before:w-1.5 before:h-1.5 before:rounded-full
           before:bg-success" {
    "已完成"
}
```

**颜色映射表（status → UnoCSS 颜色）：**

| 状态语义 | 文字+伪元素颜色 | UnoCSS class |
|---|---|---|
| draft/muted/neutral/inactive | `--muted` | `text-muted before:bg-muted` |
| info/confirmed/sent/submitted | `--accent` | `text-accent before:bg-accent` |
| accepted/progress/inspecting/warn | `#d46b08` | `text-[#d46b08] before:bg-[#d46b08]` |
| completed/shipped/success/full | `--success` | `text-success before:bg-success` |
| rejected/cancelled/expired/danger/disputed | `--danger` | `text-danger before:bg-danger` |
| suspended/defect | `#cf1322` | `text-[#cf1322] before:bg-[#cf1322]` |
| partial | `--accent` | `text-accent before:bg-accent` |

### 3.2 动画（8 个 @keyframes → theme.animation）

```rust
// Maud 中直接用 animate-* class：
div class="animate-toast-in" { ... }       // toast 入场
div class="animate-spin" { ... }           // spinner 旋转
div class="animate-dialog-slide-in" { ... } // dialog 滑入
div class="animate-toast-progress" { ... }  // toast 进度条
div class="animate-shimmer-bar" { ... }     // 委外追踪 hero 条
div class="animate-pulse-active" { ... }    // 委外追踪活跃节点
```

### 3.3 毛玻璃效果（FMS Dashboard）

```rust
// 当前：.fms-dashboard .mes-stat-card { backdrop-filter: blur(12px); background: rgba(255,255,255,0.88); }
// 原子化：
div class="backdrop-blur-md bg-white/88 rounded-lg border border-white/40 shadow-lg" { ... }
```

### 3.4 追踪时间线竖线（`.tracking-timeline::before`）

```rust
// 当前：.tracking-timeline::before { content:''; position:absolute; left:17px; top:18px;
//        bottom:18px; width:2px; border-radius:1px;
//        background:linear-gradient(180deg, var(--success) 0%, ..., var(--border-soft) 100%); }
// 原子化：
div class="relative before:content-[''] before:absolute before:left-[17px]
     before:top-[18px] before:bottom-[18px] before:w-0.5 before:rounded-sm
     before:bg-gradient-to-b before:from-success before:via-accent before:to-border-soft" { ... }
```

### 3.5 Login 网格背景（`.brand-panel::before` + `.brand-panel::after`）

```rust
// 原子化：
div class="relative before:content-[''] before:absolute before:inset-0
     before:bg-[linear-gradient(rgba(255,255,255,0.05)_1px,transparent_1px),linear-gradient(90deg,rgba(255,255,255,0.05)_1px,transparent_1px)]
     before:bg-[size:40px_40px]
     after:content-[''] after:absolute after:inset-0
     after:bg-[radial-gradient(circle_at_50%_50%,rgba(37,99,235,0.15),transparent_70%)]" { ... }
```

### 3.6 Toast 进度条（`.toast::after`）

```rust
// 原子化：
div class="after:content-[''] after:absolute after:bottom-0 after:left-0 after:h-0.5
     after:bg-success after:animate-toast-progress" { ... }
```

### 3.7 FMS scoped 覆盖（`.fms-list-page .data-card`）

当前使用父选择器 scoped 覆盖全局 class 的毛玻璃效果。

**策略**：FMS 页面的 data-card 等 class 直接写 FMS 专属的原子 class 组合，不再依赖 scoped 级联覆盖。即在 FMS 页面的 Maud 模板中，`data-card` 替换为 FMS 版本的完整原子 class 串。

### 3.8 响应式（25 处 @media → UnoCSS 前缀）

```rust
// 当前：@media (max-width: 768px) { .detail-grid { grid-template-columns: 1fr; } }
// 原子化：
div class="grid grid-cols-3 md:grid-cols-1 gap-4" { ... }

// 当前：@media (max-width: 1024px) { .app-shell { grid-template-columns: 1fr; } }
// 原子化：
div class="grid grid-cols-[auto_1fr] lg:grid-cols-1" { ... }
```

## 四、分批迁移策略（渐进式）

### 迁移原则

- 从 Maud 引用最多的族开始，每批结束后页面立即可回归测试
- 每批完成后删除 base.css 中对应 class 定义，逐步缩减到 0
- P1-P8 可以并行执行（不同页面文件不冲突）
- P9 最后执行（依赖前面所有批次的模式）
- base.css 在 P9 结束后删除

### 批次定义

| 批次 | 族 | class 数 | Maud 引用 | 涉及文件 | 迁移内容 |
|---|---|---|---|---|---|
| **P0** | 基础设施 | — | — | `uno.config.ts` | 重构配置：移入 preflights + theme.animation；清空 shortcuts；移除 CLI 对 base.css 的扫描。验证不破坏现有页面 |
| **P1** | Form Controls | ~20 | ~500 | `pages/*_create.rs` `pages/*_edit.rs` `pages/purchase_settings.rs` 等 | form-field/form-input/form-select/form-label/form-grid/form-section/form-section-title/form-actions/form-textarea/form-hint/form-check/checkbox-row/section-desc |
| **P2** | Layout Shell + Page Header | ~12 | ~250 | `layout/page.rs` + 所有页面 | app-shell/main-content/page-content/page-header/page-title/page-actions/back-link |
| **P3** | Data Card + Table + Filter | ~15 | ~400 | 所有列表页 | data-card/data-table/filter-bar/filter-select/search-wrap/search-input/create-action-bar |
| **P4** | Status Pill | ~55 | ~200 | 所有含状态标签的页面 | 统一为 before:content-[''] 原子模式 + 颜色映射；删除 base.css 中 6 处散落定义 |
| **P5** | Info Card/Grid + Detail Layout | ~43 | ~330 | 所有详情页 | info-card/info-grid/info-item/detail-grid/detail-card/detail-row/detail-tabs |
| **P6** | Modal + Drawer + Dialog | ~34 | ~140 | `components/modal.rs` `components/drawer.rs` + 使用弹窗的页面 | modal-overlay/modal/drawer-overlay/drawer-panel/dialog-overlay/dialog |
| **P7** | Tag/Badge/Stat/Pagination | ~60 | ~300 | 所有页面 | tag-chip/stat-card/pagination/badge 变体 |
| **P8** | Sidebar + Header + User Menu | ~48 | ~45 | `layout/page.rs` | #sidebar/rail-*/sidebar-*/top-header/breadcrumb/avatar/user-menu-* |
| **P9** | 域专属（剩余全部） | ~250 | ~280 | MES/FMS/Sales/Outsourcing/Toast/Login/BOM/Cost/Permission/Dept/WMS/Demand | 各域专属 class + 删除 base.css 文件 |

### 每批的执行步骤（以 P1 为例）

1. 在 base.css 中找到该批次所有 class 的 CSS 定义
2. 将每个 class 的属性翻译为 UnoCSS 原子 class 组合
3. 在 Maud 模板中将 `class="form-input"` 替换为原子 class 串
4. 从 base.css 中删除已迁移的 class 定义
5. 运行 `npm run build:css` 重新生成 app.css
6. 用 agent-browser 打开相关页面，DOM 检查计算样式是否一致
7. `cargo clippy` 验证编译通过

### 验收标准

- 每个 class 迁移后，页面渲染效果与迁移前一致（计算样式对比）
- `cargo clippy` 无新增错误
- `npm run build:css` 成功
- base.css 逐步缩减，P9 结束后文件删除
- app.css 行数从 5235 行降至 ~2000 行（纯 UnoCSS 输出）

## 五、风险与缓解

| 风险 | 影响 | 缓解措施 |
|---|---|---|
| 伪元素 class 串过长影响可读性 | Maud 模板中 `class=""` 可达 200+ 字符 | 对超长 class 串定义少量 UnoCSS rule（非 shortcut），或在 Maud 中用 `let cls = "...";` 变量提取 |
| FMS scoped 覆盖无法原子化 | `.fms-list-page .data-card` 父选择器级联 | 在 FMS 页面直接写 FMS 专属原子 class 组合，不依赖 scoped 级联 |
| 响应式断点不一致 | base.css 用 `768px`/`1024px`，UnoCSS 默认 `md:768px`/`lg:1024px` | 验证 UnoCSS presetWind4 默认断点是否匹配，不匹配则在 theme.breakpoints 中自定义 |
| 迁移期间两套体系并存 | 已迁移页面用原子 class，未迁移页面仍依赖 base.css | P0 保持 base.css 被 app.css 拼接，每批只删已迁移的 class，未迁移的不受影响 |
| Import/Export 族引用不存在的 CSS 变量 | `--primary-50`/`--slate-50` 等 Tailwind 变量名在当前系统中不存在 | P9 中迁移为正确的 UnoCSS utility 或项目 CSS 变量 |

## 六、P0 详细设计（立即执行的第一步）

P0 是整个重构的地基，必须在其他批次之前完成。

### P0 步骤

1. **重构 `uno.config.ts`**：
   - 添加 `preflights` 块，移入 base.css 行 1-103 的全部内容（:root 变量 + reset + scrollbar + [x-cloak]）
   - 添加 `theme.animation` 块，移入 8 个 @keyframes 定义
   - 清空 `shortcuts` 块（但暂时保留已冲突的 6 个 shortcut 的定义，直到对应批次迁移完成后再删）
   - 修改 `cli.entry.patterns`，暂时保留 `static/base.css` 扫描（直到 P9 才移除）

2. **验证不破坏现有页面**：
   - `npm run build:css`
   - 用 agent-browser 打开 5 个代表性页面（dashboard/列表/详情/表单/弹窗）
   - DOM 检查计算样式是否与迁移前一致
   - `cargo clippy`

3. **P0 不删除 base.css 中的任何内容**——只把 :root/reset/animation 在 UnoCSS 中重新定义，让两者暂时共存（UnoCSS preflights 优先级低于 base.css 的显式规则，不会覆盖）。
