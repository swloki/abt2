# UnoCSS 项目约定与易错点

> 本文档是 abt-web **样式层的系统性正文**，与 [`htmx-patterns.md`](htmx-patterns.md)（交互）、[`hyperscript-patterns.md`](hyperscript-patterns.md)（脚本）并列，凑成前端三件套。
>
> **定位**：不教原子类语法（AI 会），只讲 **ABT 项目特有的 UnoCSS 约定 + AI 高发易错点**。配置唯一源：[`uno.config.ts`](../../uno.config.ts)（presetWind4 + presetIcons + transformer-variant-group）。构建：`npm run build:css` 扫描 `abt-web/**/*.rs` → `static/app.css`。

---

## 0. 心智模型：三层样式 + 双写 token

ABT 样式 100% 原子化，写在 Maud `class=""`。按复用度分三层：

| 层 | 用途 | 定义位置 | 示例 |
|---|---|---|---|
| **原子类** | 日常一切样式 | presetWind4 内置 + 项目 theme | `flex items-center gap-4 text-fg bg-bg` |
| **shortcuts** | 高频复用模式（10+ 文件） | `uno.config.ts` shortcuts | `data-card` / `form-field` |
| **preflights** | 不可原子化的全局/状态 CSS | `uno.config.ts` preflights getCSS | `:root` 变量、`.app-shell` grid、`.drawer-overlay.open` |

**token 双写**：颜色/字号/间距/圆角/阴影既是 `theme`（供原子类 `bg-accent` 解析），又是 `preflights :root` 的 CSS 变量（供 `shadow-[var(--shadow-focus)]` 引用）。**两者都不可省**（见 §5）。

---

## 1. 颜色 token 体系（禁硬编码）

6 个语义色 × Tailwind 50–900 色阶（`uno.config.ts:110-129`），`DEFAULT` = 600 档：

| 语义色 | 基色 | DEFAULT(600) | 半透明背景 |
|---|---|---|---|
| `danger` | red | `#dc2626` | `danger-bg` |
| `success` | green | `#16a34a` | `success-bg` |
| `warn` | amber | `#d97706` | `warn-bg` |
| `accent` | blue | `#2563eb` | `accent-bg` |
| `purple` | violet | `#7c3aed` | `purple-bg` |
| `info` | = accent | — | — |

中性色（slate 色阶）：`fg`=slate-900、`fg-2`=slate-?、`muted`=slate-500、`border`=slate-200、`bg`=#fff、`surface`/`surface-raised`/`surface-warm`。特化：`sidebar`(bg/rail 深色)。

**用法**：

```rust
// ✅ 语义 token
p class="text-danger-500" { "已删除" }
div class="bg-accent-50 border border-warn-200 rounded-md" { ... }
span class="text-muted text-xs" { "提示" }
```

**关键约定（AI 易错）**：

- **禁止硬编码 `[#hex]`**（`bg-[#dc2626]` 等）。全仓仅 2 处例外：`#ff0` 纯黄高亮（特化视觉）。改颜色一律走语义 token。
- **`*-bg`（半透明 rgba）≠ `*-50/100`（实色浅档）**，语义不同不互替：`*-bg` 用于通用淡背景 / hover；`*-50/100` 用于卡片底等实色场景。
- 换肤 / 暗色模式的唯一机制是改 `:root` 变量，**颜色变量绝不能省**（见 §5）。

---

## 2. 图标体系（presetIcons）—— AI 高发坑

**架构**（`uno.config.ts:10-16` + `abt-web/src/components/icon.rs`）：`presetIcons`（mask 模式）+ `@iconify-json/lucide`；`icon.rs` 是 ~68 个**薄封装**函数，全仓 900+ 处调用走封装、不内联。

```rust
// icon.rs：内部 helper
fn icon(ic: &str, c: &str) -> Markup {
    html! { i class=(format!("{ic} {c}")) {} }   // <i> 元素，class = 图标 + 附加原子类
}
// 每个 pub fn 写【完整字面】图标 class
pub fn box_icon(c: &str) -> Markup { icon("i-lucide-box", c) }
pub fn home_icon(c: &str) -> Markup { icon("i-lucide-house", c) }
// 通用（传完整 class，供未预定义图标）
pub fn raw(ic: &str, c: &str) -> Markup { icon(ic, c) }
```

**尺寸/颜色控制**：用自定义 `icon:` variant（`uno.config.ts:238-242`），等价 `[&_[class*=i-lucide]]`：

```rust
(box_icon("icon:w-4.5 icon:text-accent"))   // 图标 4.5 尺寸 + accent 色
```

颜色由 mask 模式 `background-color: currentColor` 跟 `text-*` 继承（与旧 `stroke="currentColor"` 一致）。

### 三大坑（务必遵守）

1. **图标 class 必须完整字面** —— presetIcons **按需生成**，内容扫描器只认源码里的完整字面 class。`icon.rs` 每个 fn 写全 `"i-lucide-box"`，调用方零改动。**运行时 `format!` 拼接的 class（如 `format!("i-lucide-{}", name)`）扫描器看不见 → 不生成 CSS → 图标不显示**。所以新增图标必须加一个写全字面的封装函数，或用 `raw("i-lucide-xxx", c)` 传完整字面。
2. **扫描器扫整个文件文本（含注释/字符串）** —— 注释里出现 `i-lucide-` 子串会被当 class 提取，触发空图标 warn。注释里别写完整图标前缀。
3. **图标元素是 `<i>` 不是 `<svg>`** —— mask 模式挂在普通盒模型元素上。迁移前用 `[&_svg]` 控制图标的地方，迁移后**失效**，要改 `icon:` variant。**只有真正内联的 `<svg>`（toast、not_found、layout::page 等少量）才保留 `[&_svg]`**。

---

## 3. 自定义 variants（状态 class 联动）

`uno.config.ts:227-243` 定义了 **8 个项目 variant**，前缀匹配元素的**状态 class**（不是伪类），selector 拼接：

| variant | 匹配的 class | 用途 |
|---|---|---|
| `act:` | `.active` | 导航项 / Tab 激活 |
| `show:` | `.show` | 折叠面板 / Toast 展开 |
| `is-open:` | `.is-open` | dropdown / drawer 打开 |
| `is-visible:` | `.is-visible` | 隐藏内容显示 |
| `expanded:` | `.expanded` | 分类树 / 折叠组展开 |
| `open:` | `.open` | drawer / 分组 / 行展开 |
| `toast-dismiss:` | `.toast-dismiss` | toast 退出动画 |
| `icon:` | 后代 `[class*=i-lucide]` | 图标尺寸/颜色（见 §2） |

**机制**：`act:bg-accent` → 元素有 `.active` 时才应用 `bg-accent`。状态 class 由 Hyperscript 切换（`add .is-open` / `take .active`），variant 负责该状态下的样式。

```rust
// dropdown：有 .is-open 时显示（block），配合 [&.is-open]:opacity 等
div class="dropdown is-open:block opacity-0 ..." { ... }
// Tab：有 .active 时高亮
a class="tab act:text-accent act:font-semibold" { "Tab 1" }
```

> ⚠ `act:`/`is-open:` 是**状态 class** variant，和 `hover:`/`focus:`（伪类）是两套东西，别混。状态由 JS/Hyperscript 切，伪类由浏览器触发。

---

## 4. shortcuts（高频复用）

`uno.config.ts:355-369` 定义 6 个 shortcut（复用 10+ 文件才新增，class >100 字符）：

| shortcut | 用途 | 复用 |
|---|---|---|
| `data-table` | 表格全样式（th/td/tbody tr hover 等，用 variant-group 分组） | 107+ 文件 |
| `data-card` | 卡片容器 `bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]` | 107+ |
| `form-field` | 表单字段（label + input/select/textarea + focus 态） | 59+ |
| `form-section` | 表单分区容器 | 22+ |
| `field-full` | `col-span-full`（跨列） | 18+ |
| `status-pill` | 状态标签形状（颜色调用方由 `status_color()` 给） | 65+ |

**variant-group 分组写法**（`transformer-variant-group`，`uno.config.ts:21`）：`prefix:(a b c)` → `prefix:a prefix:b prefix:c`，让 shortcut 内的子选择器分组：

```ts
// data-table 用 [&_th]:(...) 把多个 th 样式收进一组
"data-table": "... [&_th]:(py-2.5 px-4 text-left font-semibold ...) ..."
```

页面里也可用：`hover:(bg-accent text-white)`、`focus:(border-accent shadow-[var(--shadow-focus)])`。

---

## 5. token 变量（`:root`）+ `var()` 引用

`:root` 变量（`uno.config.ts:34-132`）被 `.rs` 里**几百处 `var()` 任意值**直接引用：

| token | 引用次数（约） | 用法 |
|---|---|---|
| `--shadow-focus` | 179+ | `shadow-[var(--shadow-focus)]` |
| `--shadow-card` | 74+ | `shadow-[var(--shadow-card)]` |
| `--shadow-sm` | 55+ | — |
| `--radius-pill` | 10+ | `radius-[var(--radius-pill)]` |
| `--text-xs` 等 | 9+ | `text-[var(--text-xs)]` |

**重要约束（AI 易错）**：

- **`:root` 变量别想精简** —— 判断标准是**「是否被运行时 `var()` 引用」**，不是「颜色 vs 非颜色」。spacing/radius/shadow/text 变量虽是非颜色，但被几百处 `var()` 引用，直写 theme 会断引用。唯一安全的是删**未被引用**的死变量。
- **颜色变量更不能省** —— 它是换肤/暗色模式的唯一机制。
- 真要减变量，只能把 `.rs` 的 `var()` 迁到 utility（300+ 处），不推荐。

---

## 6. preflights（不可原子化的全局 CSS）

`uno.config.ts:31-223` 的 `getCSS` 保留这些**无法用原子类表达**的规则：

- **`:root` 变量 + reset**（html/body/button/input/scrollbar）+ `[x-cloak]` + `.font-mono` + `.no-scrollbar`
- **兄弟联动**：`.field-input:focus ~ .field-icon`（UnoCSS 不支持 `focus:[&~.xxx]`）
- **自定义控件**：`.perm-cell input:checked::after`（CSS border 画对勾）、`select` 的 chevron（data URI background-image）
- **多元素联动**：`.cat-row.cat-active`（背景 + ::before 竖条 + .cat-name 色联动，单 class 便于 Hyperscript `take` 整体切）
- **JS 驱动布局**：`.app-shell` grid（sidebar-collapsed 切换列宽）、`.drawer-overlay.open`（display+transform）、`.grp.open` / `tr.open` / `.mat-expand.expanded`（状态 class 显隐）
- **动画 keyframes**：`toast-in/out/progress`

### `@apply` 实测无效（别再尝试）

曾想用 `transformer-directives` 的 `@apply` 把这些状态 class 原子化，**实测无效**：

- preflights 的 `getCSS` 返回的原始 CSS **不经过 transformer 管道**（transformer 只处理被扫描的 `.rs`）；
- 且 CLAUDE.md 架构**禁止新建 CSS 文件**（`@apply` 唯一有效场景）。

→ 这些状态 class 样式只能手写 CSS 属性（是它们的本质归宿）。**别加 `@apply`，别装 `transformer-directives`**。

---

## 7. AI 易错点纠错（核心）

写样式前先看这里：

| 易错 | 正确 |
|---|---|
| 硬编码 `bg-[#dc2626]` | 用语义 token `bg-danger` |
| `format!("i-lucide-{}", n)` 拼图标 class | 加 icon.rs 封装（完整字面）或 `raw("i-lucide-x", c)` |
| `[&_svg]:w-4` 控制图标 | 用 `icon:w-4`（图标是 `<i>`，不是 svg） |
| 删 `:root` 里"看似没用"的变量 | 先 grep `var(--x)` 确认未被引用才能删 |
| 在 preflights 用 `@apply hidden` | 手写 `display:none`（@apply 无效，见 §6） |
| 把 `act:`/`is-open:` 当伪类 | 它们是状态 class variant，靠 Hyperscript `add .active` 切 |
| 单元素写两个 `class="..."` | Maud 双 class 陷阱 —— 浏览器只认第一个，合并成 `class="A B"` |

### PowerShell 批量替换 token 的灾难性坑

跨文件批量替换颜色/token 字符串时，**用 hash + `[regex]::Replace`**：

```powershell
foreach ($k in $map.Keys) { $tok = $map[$k]; ... [regex]::Replace(...) }
```

**绝不要用嵌套数组 `@(@('old','new'))`** —— PowerShell 会展平成 `@('old','new')`，foreach 取到字符串 `'old'`，`$pair[0]/[1]` 变成**首字符索引**（`'b'`/`'g'`），`.Replace('b','g')` 会把文件里**所有该字符替换**（曾把 bom_edit.rs 所有 `b`→`g`：`bg-`→`gg-`、`border`→`gorder`）。靠 `git checkout` 恢复。

---

## 8. 常用原子类速查

| 场景 | 写法 |
|---|---|
| 任意颜色值（禁用，走 token） | `bg-[#0b1829]`（仅 sidebar 等特化） |
| CSS 变量引用 | `shadow-[var(--shadow-card)]`、`text-[var(--text-xs)]` |
| 任意 CSS shorthand | `[border-right:1px_solid_rgba(255,255,255,0.04)]` |
| 伪元素 | `before:content-['']`、`after:content-['✓']`、`before:absolute before:w-[3px] before:bg-accent` |
| 子元素控制 | `[&_svg]:w-4.5`、`[&_svg]:opacity-55 hover:[&_svg]:opacity-80`（svg 子元素；图标用 `icon:`） |
| 状态 class variant | `act:bg-accent`、`is-open:block`、`show:grid-rows-[1fr]`、`expanded:block` |
| 伪类 | `hover:bg-accent-bg`、`focus:border-accent focus:shadow-[var(--shadow-focus)]` |
| 响应式 | `md:grid-cols-1`、`max-[900px]:flex-col` |
| 分组（variant-group） | `hover:(bg-accent text-white)`、`[&_th]:(py-2.5 px-4)` |
| 等宽数字 | `tabular`（财务/数量列，`rules` 定义） |

---

## 附录：token 完整清单

**颜色**（`uno.config.ts:110-129, 245-288`）：`danger`/`success`/`warn`/`accent`/`purple`/`info` 各 50/100/200/300/400/500/DEFAULT(600)/700/800/900 + `*-bg`；`slate` 50-900；`fg`/`fg-2`/`muted`/`bg`/`surface`(+raised/warm)/`border`(+soft)/`sidebar`(bg/rail)/`white`。

**字号**：`xs/sm/base/lg/xl/2xl/3xl`（12/14/15/17/21/28/36px）。
**间距**：`1-12`（4/8/12/16/20/24/32/40/48px）。
**圆角**：`sm/DEFAULT(md)/lg/xl/pill`（6/8/12/16/9999px）。
**阴影**：`xs/sm/md/lg/xl/card/card-hover/focus/accent`。
**动画**：`spin`、`dialog-slide-in`、`shimmer-bar`、`pulse-active`、`badge-pulse`（+ toast keyframes 在 preflights）。

---

## 关联文档

- [`htmx-patterns.md`](htmx-patterns.md) — HTMX 交互范式（服务端状态层）
- [`hyperscript-patterns.md`](hyperscript-patterns.md) — Hyperscript 语法与用法（纯前端 UI 层）
- [`abt-web/CLAUDE.md`](../../abt-web/CLAUDE.md) — 前端强约束入口
- [`AGENTS.md`](../../AGENTS.md) — 英文通用约束
- 配置源：[`uno.config.ts`](../../uno.config.ts)
