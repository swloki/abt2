# Repository Guidelines


## 不要使用puppeteer测试
## Constraints

- **Use Chinese (中文)** for all communication
- **Do not use `cargo run` to start the server** — it's already running. Verify correctness with `cargo clippy`
- **Code navigation**: Prefer `lsp` (definition / references / hover / type_definition); forbidden to use text search as a substitute for LSP lookups
 - **Before writing `abt-web/` components**, must read `abt-web/CLAUDE.md` first (component three principles, anti-fragmentation practices, etc.)
 - **Rust 2024 edition — Maud prefix literal 陷阱**：`class="cascade-product" style="..."` 中，字符串 `"-product"` 后直接跟 `style` 会被 Rust 2024 lexer 误解析为 prefix literal。解法：`class="cascade-product "`（字符串末尾加空格）。仅影响 pattern `"<值>-<标识符>" <下一属性>`。

## Project Overview

ABT is a BOM (Bill of Materials) and inventory management system built in Rust. It covers the full manufacturing lifecycle: sales CRM, procurement, warehouse management (WMS), manufacturing execution (MES), quality management (QMS), financial management (FMS), outsourcing (OM), and workflow-driven approval processes. Backend is PostgreSQL via `sqlx`; web frontend is server-rendered HTML via Axum + Maud + HTMX + UnoCSS.

**Communication**: Use Chinese (中文) for all interaction.


## New Feature Development Workflow (Mandatory Sequence)

 New features **must** follow this order — no steps may be skipped:

 **Interface + Model Design → Review & Confirm → Interaction Design → Implementation**

 1. **Interface first** — Define clear, stable Service traits. Once confirmed, do not change casually.
 2. **Model first** — Simultaneously design domain models (request/response structs, entities, value objects). Semantics must be clear, boundaries explicit, responsibilities single.
 3. **Design interaction based on interfaces** — Do not design frontend interactions or UI before interfaces are defined.
 4. **Documentation** — Interfaces and models use `docs/uml-design/` design documents as the skeleton and shared language.
 5. **Page prototypes** — Frontend page prototypes (Open Design) are stored at `C:\Users\weichen\AppData\Roaming\Open Design\namespaces\release-stable-win\data\projects\63ce2980-2f4e-45a7-9b34-8050e32135c2`. Use these as interaction reference when implementing UI.

## Design Authority

 `docs/uml-design/` is the **sole authoritative design documentation**. Code and design docs must stay **bidirectionally synchronized** — no drift allowed:

 - **All implementations must strictly follow** the design documents in `docs/uml-design/` (interface signatures, data models, component relationships).
 - **Change code → must update design docs** — Any code change (interface signatures, data models, component relationships, adding/removing methods) must simultaneously update the corresponding design document.
 - **Change design docs → must update code** — Any design document change must simultaneously update the code implementation.
 - **If implementation reveals design mismatch** — Must update design docs first (with user confirmation), then modify code.
 - **Never deviate from design without updating docs** — Including but not limited to: modifying interface signatures, adding/removing methods, changing data models, adjusting component relationships.
 - **Design document changes require user confirmation** — Do not unilaterally modify design documents.
 - **Self-check on every commit**: Are design docs still in sync? If not, update docs first.

 **Before implementing shared infrastructure, must read `docs/uml-design/README.md`** (interface signatures, type definitions, integration rules for AuditAction / SideEffect / EventPublishRequest etc.).

## Architecture & Data Flow

```
┌─────────────────────────────────────────────────────────┐
│  abt-web (Axum + Maud + HTMX + UnoCSS)                  │
│  SSR pages, HTMX partials, Hyperscript UI interactions │
│  Calls abt-core Service traits via AppState factory fns  │
└──────────────────────┬──────────────────────────────────┘
                       │ Service trait calls
┌──────────────────────▼──────────────────────────────────┐
│  abt-core (business logic library)                       │
│  10 business domains + shared infrastructure layer       │
│  Each domain: Service trait → implt.rs → repo.rs → DB   │
│  Shared: state machine, event bus, audit, identity, ...  │
└──────────────────────┬──────────────────────────────────┘
                       │ sqlx (compile-time checked SQL)
┌──────────────────────▼──────────────────────────────────┐
│  PostgreSQL (abt_v2)                                     │
│  Migrations in abt-core/migrations/ (27 files)           │
└─────────────────────────────────────────────────────────┘
```

**Data flow**: Browser → HTMX request → Axum handler → `AppState.xxx_service()` → abt-core Service trait → repo (raw SQL via sqlx) → PostgreSQL. Response is HTML rendered by Maud macros, swapped inline by HTMX.

## Key Directories

| Directory | Purpose |
|-----------|---------|
| `abt-core/src/` | Business logic library — 10 domain modules + shared infrastructure |
| `abt-core/src/shared/` | Cross-cutting services: state_machine, event_bus, audit_log, identity, document_sequence, document_link, inventory_reservation, cost_entry, idempotency, notification, scheduled_task, excel, enums |
| `abt-core/src/shared/types/` | Core types: `ServiceContext`, `DomainError`, `PageParams`, `PgExecutor`, `BatchResult`, `TransactionMode` |
| `abt-core/src/shared/enums/` | Shared enums (all `#[repr(i16)]`): `DocumentType` (42 variants), `DomainEventType` (63 variants), `SideEffect`, `CostType`, `LinkType`, etc. |
| `abt-core/migrations/` | 27 SQL migration files for PostgreSQL schema |
| `abt-web/src/` | Web frontend — Axum server, Maud HTML templates, HTMX pages |
| `abt-web/src/pages/` | 130 page rendering modules (Maud HTML), organized by business domain |
| `abt-web/src/routes/` | 51 route modules exposing `router()` functions |
| `abt-web/src/components/` | Shared UI components (modal, drawer, pagination, tabs, icons, etc.) |
| `abt-web/src/layout/` | Page shell, admin layout, sidebar, header |
| `static/` | 静态资源目录（项目根级）：`app.css`（UnoCSS 生成的纯原子 utility）, JS 文件 (`app.js`, `hyperscript.min.js`, `bom-edit.js`, `htmx.min.js`) |
| `abt-macros/src/` | Proc-macro crate: `#[require_permission("RESOURCE", "action")]` |
| `docs/uml-design/` | System design documents (HTML UML), authoritative source of truth |
| `docs/plans/` | Test plans and implementation plans (MES, WMS testing) |
| `scripts/` | Data migration scripts (TypeScript/SQL/Bash), test data SQL |

## Development Commands

```bash
# Build & verify (primary verification method)
cargo build                    # Build all crates
cargo clippy                   # Lint — main verification tool
cargo test                     # Run all tests
cargo test -p abt-core         # Test core library only
cargo test -p abt-core -- test_name  # Single test

# Web frontend
 # Web frontend
 cargo watch -x run            # 自动重编译重启（推荐用于开发，stderr 重新输出到终端）
 cargo run -p abt-web          # 单次启动（默认端口 8000）— 已有服务运行时禁止使用
 
 # CSS build（项目根级运行，static/ 在根目录）
 npm run build:css             # Build UnoCSS → static/app.css（修改 uno.config.ts 或 Maud 模板后必须执行）
 npm run watch                 # Watch mode for CSS changes
 ```
 
 **Important**: 服务由 `cargo watch -x run` 管理，CSS 变更不会触发 Rust 重编译 — 必须手动 `npm run build:css` 后服务自动重载静态文件。验证正确性用 `cargo clippy`。

## Code Conventions & Common Patterns

### Module Structure (abt-core)

Every business module follows a consistent file layout:

```
abt-core/src/<domain>/<module>/
├── mod.rs       # Exports + factory function (e.g., new_xxx_service)
├── service.rs   # Service trait definition (#[async_trait])
├── implt.rs     # Service trait implementation
├── model.rs     # Data models (request/response/entity)
└── repo.rs      # Database access (raw SQL via sqlx)
```

**Business domains**: `sales` (5 sub-modules), `purchase` (6), `wms` (15), `mes` (8), `fms` (4), `om` (2), `qms` (5), `master_data` (10), `workflow` (8 files), `h3yun` (integration).

### Service Trait Pattern

```rust
// service.rs — trait definition
#[async_trait]
pub trait XxxService: Send + Sync {
    async fn create(&self, ctx: &ServiceContext, db: impl PgExecutor<'_>, req: CreateXxxReq) -> Result<Xxx, DomainError>;
}

// implt.rs — implementation
pub struct XxxServiceImpl { repo: XxxRepo, pool: PgPool }

// mod.rs — factory function
pub fn new_xxx_service(pool: PgPool) -> impl XxxService {
    XxxServiceImpl::new(pool)
}
```

### Shared Service Access (On-Demand Factory)

Each shared module's `mod.rs` exposes a factory function returning `impl Trait`:

```rust
// shared/audit_log/mod.rs
pub fn new_audit_log_service(pool: PgPool) -> impl AuditLogService {
    implt::AuditLogServiceImpl::new(pool)
}
```

**Rules for consumer Service implementations:**

1. **Consumer struct holds only `PgPool`** — no `Arc<dyn Trait>` fields:
   ```rust
   pub struct XxxServiceImpl { repo: XxxRepo, pool: PgPool }
   ```

2. **Method body depends only on trait interfaces** — obtain via factory, never depend on implementation types:
   ```rust
   // ✓ Correct: use imports trait + factory, code uses short names
   use crate::shared::audit_log::{new_audit_log_service, service::AuditLogService};
   use crate::shared::state_machine::{new_state_machine_service, service::StateMachineService};

   impl XxxService for XxxServiceImpl {
       async fn some_method(&self, ...) -> Result<()> {
           new_audit_log_service(self.pool.clone())
               .record(ctx, db, ...).await?;
           new_state_machine_service(self.pool.clone())
               .transition(ctx, db, ...).await?;
       }
   }

   // ✗ Forbidden: fully-qualified path
   crate::shared::audit_log::new_audit_log_service(self.pool.clone())

   // ✗ Forbidden: depend on implementation type
   use crate::shared::audit_log::implt::AuditLogServiceImpl;
   ```

3. **Core principles**: struct holds `PgPool` only, methods program against trait interfaces, factory functions imported via `use` with short names, shared services created on-demand and discarded (no upfront injection).


### Error Handling

- `DomainError` enum (thiserror) with variants: `NotFound`, `Duplicate`, `Unauthorized`, `PermissionDenied`, `BusinessRule`, `Validation`, `ConcurrentConflict`, `InvalidStateTransition`, `Internal`
- Always return `Result<T, DomainError>` from service methods
- **Never silently discard errors** — no `let _ = expr.await;` or `let _ = result;`
- Web layer maps `DomainError` to HTTP responses via `WebError`

### Module Boundaries

- Cross-module calls only through Service trait + Model — never access another module's Repository or `implt` directly
- Within the same module, internal Repository access is unrestricted

### Web Frontend Patterns

> **Authoritative source for HTMX / interaction patterns:** [`docs/frontend/htmx-patterns.md`](docs/frontend/htmx-patterns.md) (decision trees, composition modes, anti-patterns, gotchas). This section keeps the English cross-cutting constraints and the Surreal→Hyperscript migration table; for full worked examples, the attribute quick-reference, and the `HX-Trigger` vs `HX-Trigger-After-Settle` timing decision, refer to the authoritative doc.

#### Data Access Layer (Mandatory)

**`abt-web` is forbidden from direct database access.** All data operations must go through `abt-core` Service traits:

- **Forbidden**: `sqlx::query`, `sqlx::query_as`, `sqlx::query_scalar`, or direct `PgPool`/`PgConnection` queries in abt-web
- **Required**: Access via `AppState` service instances (e.g., `state.customer_service()`, `state.bom_command_service()`)
- **Required**: Follow `abt-core` Service trait signatures including `ServiceContext` parameter
- If `abt-core` lacks a needed interface, add it there first, then call from abt-web

#### TypedPath Routing (Mandatory)

Always use `TypedPath` — never hardcode URL strings:

```rust
#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/admin/md/products/:id")]
pub struct ProductDetailPath { pub id: i64 }
```

#### Component Three Principles

All interactive components must follow these rules:

1. **Absolute Cohesion** — `hx-target="this"` + `hx-swap="outerHTML"`. Component is its own replacement boundary, no external IDs:
   ```rust
   div class="counter" {
       span { (count) }
       button hx-post=(path) hx-target="this" hx-swap="outerHTML" { "+1" }
   }
   ```
2. **State Travels With Element** — Use `hx-vals` to bind Rust context data on the HTML node, no global state:
   ```rust
   tr hx-vals=(format!("{{\"item_id\": {id}, \"status\": \"{status}\"}}"))
      hx-post=(path) hx-target="this" hx-swap="outerHTML" { ... }
   ```
3. **Visual Closure** — Embed loading/indicator HTML inside component via `hx-indicator`, HTMX controls visibility automatically.

#### Anti-Fragmentation: TypedPath + hx-target="this"

- Handler **always returns the complete component** — no awareness of request origin needed
- Component itself is the swap boundary — no hardcoded `#id` targets
- **One URL, one Handler** — forbidden to create extra handlers for partial refresh
- When `this` is insufficient, use `closest <selector>` or similar relative positioning

#### HTMX vs Hyperscript Boundary (Hybrid Islands)

| Layer | Responsibility | Technology |
|-------|---------------|------------|
| Pure frontend UI | Modal open/close, dropdown, tab switch, toggle | Hyperscript `_="on click ..."` attribute |
| Server interaction | Form submit, search, pagination | HTMX `hx-post`/`hx-get` |
| Complex frontend state | Drag-sort, line-item calc, persistent state | Standalone JS files (`app.js`, `bom-edit.js`) |
| Data bridging | `input type="hidden" name="items_json"` | JS `lineItemCalc().collectItems()` |
| Success navigation | Server returns `HX-Redirect` | HTMX auto-redirect |
| Error display | `htmx:responseError` → toast | HTMX + JS |

**Rules**:
- HTMX for server-state interactions only. Never use HTMX for purely visual changes
- Hyperscript for pure frontend UI. Never use `fetch()` for server calls
- **Never use `onclick`/`<script>me().on(...)`/Surreal.js `me()` for UI** — use Hyperscript `_=` attribute

#### Hyperscript Pattern (`_` attribute)

Hyperscript lives in the `_` attribute as a declarative, sentence-like script. No `<script>` tag, no `maud::PreEscaped` wrapper — just a plain string attribute. In Maud:

```rust
button _="on click add .is-open to #modal" { "打开" }
button _="on click remove .is-open from closest .modal-overlay" { "关闭" }
```

**Surreal.js / hs\* → Hyperscript 迁移对照表**:

| 旧 (Surreal / hs\*) | 新 (Hyperscript `_=`) |
|-----|-----|
| `onclick="me('#m').classAdd('is-open')"` / `hsAdd(null,'#m','is-open')` | `_="on click add .is-open to #m"` |
| `onclick="hsRemove(null,'#m','is-open')"` | `_="on click remove .is-open from #m"` |
| `onclick="hsRemoveClosest(this,'.overlay','is-open')"` | `_="on click remove .is-open from closest .overlay"` |
| `onclick="hsBackdropClose(this,event,'is-open')"` | `_="on click[me is event.target] remove .is-open"` |
| `onclick="hsTake(this,'.tab','active')"` | `_="on click take .active from .tab"` |
| `onclick="hsToggle(null,'#m','is-open')"` | `_="on click toggle .is-open on #m"` |
| `onclick="hsRemoveClosestEl(this,'tr')"` | `_="on click remove closest tr"` |
| `hsRemoveClosest(...) + form.reset()` | `_="on click remove .is-open from closest .modal-overlay then reset (closest form)"` |
| `hx-on::after-request="hsAdd(...)"` 成功后打开 | 放在触发元素上 `_="on htmx:after-request[detail.xhr.status < 400] add .open to #drawer"` |
| `onkeydown="if(event.key==='Escape')..."` | `_="on keydown[event.key is 'Escape'] remove .open"` |

**核心语法速查**（完整参考 https://hyperscript.org/reference/ ，版本 `hyperscript.org@0.9.91`）:

| 语法 | 含义 | 示例 |
|-----|------|------|
| `on <event>` | 事件监听 | `on click` / `on change` / `on input` / `on submit` |
| `on <event>[filter]` | 带条件的事件过滤器 | `on click[me is event.target]` / `on keydown[event.key is 'Escape']` |
| `then` | 命令链式（隐式目标 `me`） | `add .a then remove .b` |
| `me` / `my` / `I` | 当前元素 | `add .active to me` |
| `it` / `its` / `result` | 上一条命令的结果 | `fetch /x as JSON then put it into #out` |
| `event` `target` `detail` `sender` | 事件对象（handler 内） | `if event.target is me` / `log detail.xhr.status` |
| `add .cls to <target>` | 加 class | `add .is-open to #modal` |
| `remove .cls from <target>` | 删 class | `remove .active from .tab` |
| `toggle .cls [on <target>]` | 切换 class | `toggle .expanded` / `toggle .open on #drawer` |
| `take .cls from <set>` | 抢占 class（移除同组其他元素，加给自己） | `take .active from .rail-item` |
| `closest <sel>` | 最近匹配祖先 | `remove .is-open from closest .modal-overlay` |
| `next` / `previous` `<sel>` | 相邻兄弟元素 | `toggle .show on next <div/>` |
| `<button/>` | query 选择器引用 | `add .x to <.active in me.parentElement/>` |
| `#id` / `.cls` | id / class 引用 | `#modal` / `.active` |
| `reset <form>` | 重置表单 | `reset #my-form` / `reset (closest form)` |
| `call <js>` / `get <js>` | 执行 JS 表达式 | `call alert('hi')` / `call myJsFn()` |
| `set <sym> to <val>` | 赋局部变量 | `set x to 0` |
| `put <val> into <target>` | 写入属性/变量 | `put '1' into #shift's value` |
| `if <cond> then ...` | 条件分支 | `if no <.results/> then exit` |
| `halt` / `halt the event` | 阻止冒泡/默认行为 | `on click halt the event then ...` |
| `send` / `trigger <event> to <target>` | 触发自定义事件 | `send cartUpdated to body` |
| `wait <time>` | 等待时间或事件 | `wait 2s then remove me` |
| `as <Type>` | 类型转换 | `"10" as Int` / 表单 `as Values` |
| `show` / `hide` | 显示/隐藏元素 | `hide #spinner` |
| `exit` | 提前退出 handler | `if x is null exit` |

**Magic values**: `me`(当前元素) · `it`(上次结果) · `event`/`target`/`detail`/`sender`(事件) · `body` · `cookies`。

**复杂逻辑的处理**：表单行收集（`lineItemCalc`）、拖拽排序、checkbox 全选/反选等**仍用 `static/app.js` 中的全局 JS 函数**，Hyperscript 通过 `call` 调用，避免在 `_` 里写大段逻辑：

```rust
form _="on submit call collectItems() then put it into #items_json" hx-post=(path) { ... }
```

**HTMX + Hyperscript combo**: HTMX swap 进来的新内容，其中的 `_` 属性会被 hyperscript 自动处理（它在 DOM 变化后扫描 `[_]` 节点并初始化）。需要靠 HTMX 结果驱动打开弹窗时，把 `_="on htmx:after-request ..."` 放在发起请求的元素上，或用 `on htmx:afterSettle`。

#### HX-Trigger Event-Driven Decoupling

When one interaction needs to refresh multiple independent components, avoid "aggregation routes":
1. Active component sends POST (e.g., `/cart/add`)
2. Server responds with `HX-Trigger: "cartUpdated"` header
3. Passive components declare `hx-trigger="cartUpdated from:body"` pointing to their own TypedPath

#### Form Development

- **Forbidden**: `fetch()` to submit forms — use HTMX `hx-post`
- **Forbidden**: `onclick` custom JS for UI — use Hyperscript `_="on click ..."` attribute
- **Forbidden**: `<script>me().on(...)` / Surreal.js `me()` — 已废弃，改用 Hyperscript `_=`
- Use `<form hx-post>` instead of `onclick="htmx.ajax(...)"` — no JS needed
- `hx-include="[name='parent_id']"` to auto-include hidden inputs from page

#### Standalone JS Files

Only for interactions that cannot be expressed inline:
- `static/bom-edit.js` — SortableJS drag-sort + collapse/expand state persistence
- `static/app.js` — `lineItemCalc` row calculator, toast/export/confirm helpers, category tree

#### HTMX 2.x Event Model

- `htmx:afterRequest` fires on **trigger element** (the one making the request)
- `htmx:afterSettle` fires on **target element** (the swap target)
- `hx-select` is inherited by child elements — add `hx-disinherit="hx-select"` on parent to prevent

 ### CSS Management（100% 原子化 UnoCSS）
 样式文件位于项目根级 `static/` 目录：
 - **`static/base.css`** — **已删除**。不再存在手写 CSS 文件。所有样式通过 UnoCSS 原子类或 shortcuts 实现
 - **`static/app.css`** — UnoCSS CLI 生成的纯原子 utility 输出。**禁止手动修改**，仅通过 `npm run build:css` 生成

**核心规则：**
1. **所有样式写在 Maud 的 `class=""` 中** — 使用 UnoCSS 原子类组合，如 `class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded-md shadow-[var(--shadow-card)]"`
2. **禁止新建 CSS 文件** — 不在 `static/` 下新建独立 CSS，不在 Maud 模板中用 `style=""` 内联（`<col>` 元素例外）
3. **修改 CSS 变量** — 在 `uno.config.ts` 的 `preflights` 的 `:root { ... }` 块中修改
4. **新增动画** — 在 `uno.config.ts` 的 `theme.animation.keyframes` 中定义，Maud 中用 `animate-xxx` 引用
 5. **`shortcuts` 只允许高频组件模式** — 当前有 `data-table`（表格全样式）、`data-card`（卡片容器）、`form-field`（表单字段）、`form-section`（表单分区容器）、`field-full`（跨列）五个 shortcut，新增需满足：在 10+ 个 Maud 文件中重复使用且 class 字符串超过 100 字符

**UnoCSS 高级语法速查（项目实际使用）：**

| 语法 | 用途 | 示例 |
|---|---|---|
| `bg-[#0b1829]` | 任意颜色值 | 深色侧边栏背景 |
| `bg-[linear-gradient(...)]` | 任意渐变 | `bg-[linear-gradient(180deg,#0a1628,#0f1d32)]` |
| `shadow-[var(--shadow-card)]` | CSS 变量阴影 | data-card 阴影 |
| `[border-right:1px_solid_rgba(255,255,255,0.04)]` | 任意 CSS shorthand | 侧边栏边框（解决 border-r currentColor 继承问题） |
| `before:content-['']` / `after:content-['✓']` | 伪元素内容 | 状态指示条、勾选符号 |
| `before:absolute before:w-[3px] before:bg-accent` | 伪元素样式 | active 指示条 |
| `[&_svg]:w-4.5 [&_svg]:h-4.5` | 子元素 svg 尺寸控制 | 按钮/图标容器内的 SVG |
| `[&_svg]:opacity-55 hover:[&_svg]:opacity-80` | 子元素 hover 联动 | 导航项图标透明度 |
| `act:bg-accent act:text-white` | `.active` class 状态 | 自定义 variant：当元素同时有 `.active` class 时生效 |
| `show:grid-rows-[1fr]` | `.show` class 状态 | 折叠面板展开 |
| `is-open:block` | `.is-open` class 状态 | 下拉菜单展开 |
| `expanded:block` | `.expanded` class 状态 | 分类树展开 |
| `hover:bg-accent-bg` | hover 状态 | 行 hover 高亮 |
| `focus:border-accent focus:shadow-[var(--shadow-focus)]` | focus 状态 | 输入框聚焦 |
| `md:grid-cols-1` / `max-[900px]:flex-col` | 响应式断点 | 移动端布局 |

**自定义 variants（`uno.config.ts` 中定义）：**

| 前缀 | 匹配的 class | 用途 |
|---|---|---|
| `act:` | `.active` | 导航项/Tab 激活状态 |
| `show:` | `.show` | 折叠面板/Toast 展开状态 |
| `is-open:` | `.is-open` | 下拉菜单/抽屉打开状态 |
| `is-visible:` | `.is-visible` | 隐藏内容显示状态 |
| `expanded:` | `.expanded` | 分类树/折叠组展开状态 |

**`preflights` 中保留的不可原子化 CSS（~15 条规则）：**
- `app-shell` grid 布局 + JS 驱动的 sidebar-collapsed 状态切换（`grid-template-columns` 动态变化 + 子元素显隐）
- `field-input:focus ~ .field-icon` 兄弟元素焦点联动（UnoCSS 不支持 `focus:[&~.xxx]:` 语法）
- `perm-cell input:checked::after` 自定义 checkbox 勾选符号（CSS border 绘制的对勾形状）
- `@media (max-width: 768px)` 移动端 sidebar 定位 + 多元素联动


### Enums

All shared enums are `#[repr(i16)]` stored as PostgreSQL `smallint`. They implement `sqlx::Type`, `sqlx::Encode`, `sqlx::Decode`, `serde::Serialize`, `serde::Deserialize`.

### Database Conventions

- Soft delete via `deleted_at` timestamp
- `Decimal(10,6)` for financial/quantity precision
- `operator_id` for audit trail
- JSONB for flexible metadata (e.g., `products.meta`, `boms.bom_detail`)
- `sqlx::query!` macro for compile-time SQL verification


## Important Files

| File | Role |
|------|------|
| `abt-core/src/lib.rs` | Crate root — declares 10 domain + shared modules |
| `abt-core/src/shared/types/context.rs` | `ServiceContext` — operation metadata (operator_id, department_id, data_scope, trace_id) |
| `abt-core/src/shared/types/error.rs` | `DomainError` — unified error type |
| `abt-core/src/shared/identity/model.rs` | Auth models, `RESOURCE_ACTION_DEFS` (72 permission entries) |
| `abt-web/src/main.rs` | Server entrypoint (Axum setup, session layer, router mount) |
| `abt-web/src/state.rs` | `AppState` — holds PgPool, 45+ service factory methods |
| `abt-web/src/utils.rs` | `RequestContext` axum extractor, serde helpers |
| `abt-web/src/routes/mod.rs` | Master router — merges all 51 domain routers |
 | `uno.config.ts` | UnoCSS configuration: preflights (:root variables + reset + component state CSS) + theme (colors/spacing/radius/shadow/animation) + custom variants (act:/show:/is-open:) + shortcuts (data-table, data-card, form-field, form-section, field-full) (项目根级) |
| `abt-macros/src/lib.rs` | `#[require_permission]` proc macro |
| `docs/uml-design/` | Authoritative design documents — code must stay in sync |

## Runtime/Tooling Preferences

- **Language**: Rust (edition 2024 for abt-core and abt-web; edition 2024 for abt-macros)
- **Database**: PostgreSQL (abt_v2)
- **Package manager**: npm for abt-web CSS tooling; bun for scripts
- **Async runtime**: tokio (full features)
- **HTML templating**: Maud (compile-time macros, not string templates)
- **CSS framework**: UnoCSS with Tailwind v4 preset (`presetWind4`)
- **Frontend interactivity**: HTMX 2.x (server-state) + Hyperscript 0.9.91 (pure UI, `_` attribute)
- **Session storage**: File-based via `tower-sessions` + `file-store`
- **Linting**: `cargo clippy` — primary verification
- **Environment** (`.env` file): `DATABASE_URL` (required, points to `abt_v2`), `JWT_SECRET` (required), `WEB_PORT` (default `8000`), `WEB_HOST` (default `0.0.0.0`), `MAX_CONNECTION` (default `20`)
- **Local auth**: username `admin`, password `chenxi0514`

## Testing & QA

### Build Verification

`cargo clippy` is the primary correctness gate. Run it after every code change:

```bash
cargo clippy                    # All crates
cargo clippy -p abt-core       # Core only
```

### Test Execution

```bash
cargo test                      # All tests
cargo test -p abt-core          # Core library tests
cargo test -p abt-core -- test_name  # Single test by name
```

### Test Data

SQL test data scripts in `scripts/`:
- `scripts/mes-test-data.sql` — MES module test data (6 plans, 9 work orders, 8 batches)
- `scripts/wms-test-data.sql` — WMS module test data (4 warehouses, zones, bins, inventory)
- `scripts/mes_test_data.sql` — Supplementary MES data (routings, reports, inspections)

### Design-Code Sync

Code changes must stay synchronized with `docs/uml-design/`. If implementation reveals design mismatches, update design docs (with user approval) before changing code. Every commit should pass the self-check: "Are design docs still in sync?"

### Documented Solutions

When available in `docs/solutions/`, consult existing solutions (organized with YAML frontmatter: `module`, `tags`, `problem_type`) before implementing or debugging in documented areas.

### Page Functional Testing (Agent Browser)

Use `agent-browser` CLI for end-to-end page testing. **Never use `curl`** for page verification.

> **🔌 CDP 连接已有浏览器**：用户已开启一个 Chrome 实例（CDP 端口 9222），所有 agent-browser 命令必须通过 `--cdp 9222` 连接到该实例。**禁止关闭/重启浏览器**（不可使用 `agent-browser close` / `close --all`）。禁止使用无头模式。
>
> **🚫 禁止截图**：当前模型不支持图片输入，禁止使用 `agent-browser screenshot` 或 `screenshot --full` 命令。页面验证改用 `snapshot -i`（无障碍树文本）+ `get text @eN`（元素文本内容）。
>

#### Login & Session Setup

```bash
# First-time login — save auth profile
agent-browser auth save abt --url http://localhost:8000/login --username admin --password chenxi0514

# Login via CDP (连接用户已开的浏览器)
agent-browser --cdp 9222 open http://localhost:8000/login
agent-browser snapshot -i
agent-browser fill @e<username_input> "admin"
agent-browser fill @e<password_input> "chenxi0514"
agent-browser click @e<login_button>
agent-browser wait 2000
```

#### Testing a Page

```bash
# Navigate to target page
agent-browser --cdp 9222 open http://localhost:8000/admin/md/products
agent-browser snapshot -i              # Get interactive elements with @eN refs

# Test interaction (click, fill, submit)
agent-browser click @e3                # Click element by snapshot ref
agent-browser snapshot -i              # Verify result after action

# Check for console errors
agent-browser console --clear
agent-browser errors
```

#### Common Testing Patterns

| Task | Commands |
|------|----------|
| List page renders | `open <url> && snapshot -i` |
| Create form submit | `open <create_url> && fill @eN "value" && click @eN && snapshot -i` |
| Search/filter | `fill @eN "query" && press Enter && snapshot -i` |
| Delete with confirm | `click @eN && snapshot -i && click @eN` |
| Pagination | `click @eN (next page) && snapshot -i` |
| Check page errors | `errors --clear` before action, then `errors` after |

#### Key `agent-browser` Commands

| Command | Purpose |
|---------|---------|
| `open <url>` | Navigate to URL |
| `snapshot -i` | Accessibility tree with interactive element refs (`@e1`, `@e2`, ...) |
| `click @eN` | Click element by ref |
| `fill @eN "text"` | Clear and fill input |
| `type @eN "text"` | Append text without clearing |
| `press Enter` | Press keyboard key |
| `select @eN "value"` | Select dropdown option |
| `screenshot` | **已禁用**（模型不支持图片输入） |
| `errors [--clear]` | View/clear page errors |
| `get text @eN` | Get element text content |
| `back` / `reload` | Navigation |
| `close [--all]` | **已禁用**（禁止关闭用户的浏览器） |

## Adding a New Feature

 1. In `abt-core/src/<domain>/<module>/` create the module files:
    - `model.rs` — Data models
    - `repo.rs` — Database access
    - `service.rs` — Service trait definition
    - `implt.rs` — Service trait implementation (struct holds only `PgPool`, shared services via on-demand factory)
    - `mod.rs` — Exports + factory function
 2. Add database migration in `abt-core/migrations/` (sequential numbered SQL file)
 3. Create page modules in `abt-web/src/pages/` (if UI is needed)
 4. Add route module in `abt-web/src/routes/` and register in `routes/mod.rs`
 5. Add service factory method to `abt-web/src/state.rs` `AppState`
 6. **Synchronize `docs/uml-design/` design documents** — mandatory, not optional
