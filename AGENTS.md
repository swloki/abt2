# Repository Guidelines


## 不要使用puppeteer测试
## Constraints

- **Use Chinese (中文)** for all communication
- **Do not use `cargo run` to start the server** — it's already running. Verify correctness with `cargo clippy`
- **Code navigation**: Prefer `lsp` (definition / references / hover / type_definition); forbidden to use text search as a substitute for LSP lookups
- **Before writing `abt-web/` components**, must read `abt-web/CLAUDE.md` first (component three principles, anti-fragmentation practices, etc.)

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
│  SSR pages, HTMX partials, Surreal.js UI interactions   │
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
| `static/` | 静态资源目录（项目根级）：编译后 CSS (`base.css` 手写 + `app.css` 由 UnoCSS 生成), JS 文件 (`app.js`, `surreal.js`, `bom-edit.js`, `htmx.min.js`) |
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
cargo run -p abt-web           # Start server (default port 8000) — DO NOT use if server is already running

# CSS build (static/ 目录位于项目根级)
cd abt-web && npm run build:css   # Build UnoCSS → static/app.css
cd abt-web && npm run watch       # Watch mode for CSS changes
```

**Important**: Do not use `cargo run` to start the server if it's already running. Verify correctness with `cargo clippy`.

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

#### HTMX vs Surreal.js Boundary (Hybrid Islands)

| Layer | Responsibility | Technology |
|-------|---------------|------------|
| Pure frontend UI | Modal open/close, dropdown, tab switch | Surreal.js `<script>me().on(...)` inline |
| Server interaction | Form submit, search, pagination | HTMX `hx-post`/`hx-get` |
| Complex frontend state | Drag-sort, persistent state | Standalone JS files (SortableJS) |
| Data bridging | `input type="hidden" name="items_json"` | JS `lineItemCalc().collectItems()` |
| Success navigation | Server returns `HX-Redirect` | HTMX auto-redirect |
| Error display | `htmx:responseError` → toast | HTMX + JS |

**Rules**:
- HTMX for server-state interactions only. Never use HTMX for purely visual changes
- Surreal.js for pure frontend UI. Never use `fetch()` for server calls
- Never use `onclick` calling custom JS functions for UI — use Surreal.js inline

#### Surreal.js Inline Pattern

`me()` inside `<script>` returns the **parent element**. Wrap with `maud::PreEscaped()`:

```rust
(maud::PreEscaped(r#"<script>me().on('click',function(){me('#modal').classAdd('is-open')})</script>"#))
```

| API | Description |
|-----|-------------|
| `me()` | Parent element of `<script>` |
| `me(selector)` | `document.querySelector(selector)` |
| `me(selector, start)` | Search from `start` element |
| `any(selector)` | All matching elements as array |
| `me(el).on(event, fn)` | `addEventListener` |
| `me(el).classAdd/Remove/Toggle(cls)` | Class manipulation |
| `me(el).attribute(name, val?)` | Read/write attribute |
| `me(el).remove()` | Remove element |

**HTMX + Surreal.js combo**: `<script>` must be placed **outside** the modal container — HTMX swap replaces innerHTML and destroys inner listeners. Use `htmx:afterSettle` (fires on **target element**) to open modal after successful load.

#### HX-Trigger Event-Driven Decoupling

When one interaction needs to refresh multiple independent components, avoid "aggregation routes":
1. Active component sends POST (e.g., `/cart/add`)
2. Server responds with `HX-Trigger: "cartUpdated"` header
3. Passive components declare `hx-trigger="cartUpdated from:body"` pointing to their own TypedPath

#### Form Development

- **Forbidden**: `fetch()` to submit forms — use HTMX `hx-post`
- **Forbidden**: `onclick` custom JS for UI — use Surreal.js `<script>me().on(...)`
- **Forbidden**: `script { "..." }` in Maud (HTML-escaped) — use `maud::PreEscaped("<script>...</script>")`
- Use `<form hx-post>` instead of `onclick="htmx.ajax(...)"` — no JS needed
- `hx-include="[name='parent_id']"` to auto-include hidden inputs from page

#### Standalone JS Files

Only for interactions that cannot be expressed inline:
- `static/bom-edit.js` — SortableJS drag-sort + collapse/expand state persistence
- `static/app.js` — `lineItemCalc` row calculator, `hs*` compatibility helpers, category tree

#### HTMX 2.x Event Model

- `htmx:afterRequest` fires on **trigger element** (the one making the request)
- `htmx:afterSettle` fires on **target element** (the swap target)
- `hx-select` is inherited by child elements — add `hx-disinherit="hx-select"` on parent to prevent

### CSS Management
样式文件位于项目根级 `static/` 目录：
- **`static/base.css`** — 手写 CSS，包含 CSS 变量、重置、布局、组件样式、复杂选择器等。**可直接编辑**
- **`static/app.css`** — UnoCSS (`uno.config.ts`) 生成的输出文件。**禁止手动修改**，仅通过 `npm run build:css` 生成
- **`uno.config.ts`**（项目根级）— UnoCSS shortcuts 配置，新增工具类组合优先在此添加
**禁止**在 `static/` 下新建独立 CSS 文件。禁止在 Maud 模板中使用 `style` 属性内联样式（`<col>` 元素例外）。
Key shortcuts defined in `uno.config.ts`: `data-card`, `data-table`, `form-section`, `form-grid`, `form-field`, `form-input`, `form-select`, `filter-bar`, `filter-select`, `search-wrap`, `search-input`, `page-header`, `page-title`, `modal-overlay`, `modal`/`modal-lg`, `btn-primary`, `btn-danger`, `status-pill`, `info-card`, `info-grid`, `info-item`, `workflow-steps`, `stat-card`, `pagination`, `kanban-*`, etc. Refer to `abt-web/CLAUDE.md` for the full 80-entry class name reference table.


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
| `uno.config.ts` | UnoCSS configuration with ~80 shortcuts and design tokens (项目根级) |
| `abt-macros/src/lib.rs` | `#[require_permission]` proc macro |
| `docs/uml-design/` | Authoritative design documents — code must stay in sync |

## Runtime/Tooling Preferences

- **Language**: Rust (edition 2024 for abt-core and abt-web; edition 2024 for abt-macros)
- **Database**: PostgreSQL (abt_v2)
- **Package manager**: npm for abt-web CSS tooling; bun for scripts
- **Async runtime**: tokio (full features)
- **HTML templating**: Maud (compile-time macros, not string templates)
- **CSS framework**: UnoCSS with Tailwind v4 preset (`presetWind4`)
- **Frontend interactivity**: HTMX 2.x (server-state) + Surreal.js (pure UI)
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

#### Login & Session Setup

```bash
# First-time login — save auth profile
agent-browser auth save abt --url https://localhost:8000/login --username admin --password chenxi0514

# Start browser and login
agent-browser --session-name abt open https://localhost:8000/login
agent-browser snapshot -i
agent-browser fill @e<username_input> "admin"
agent-browser fill @e<password_input> "chenxi0514"
agent-browser click @e<login_button>
agent-browser wait 2000
```

The `--session-name abt` flag auto-saves/restores cookies so subsequent opens reuse the session.

#### Testing a Page

```bash
# Navigate to target page
agent-browser open https://localhost:8000/admin/md/products
agent-browser snapshot -i              # Get interactive elements with @eN refs
agent-browser screenshot --full        # Full page screenshot for visual verification

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
| List page renders | `open <url> && snapshot -i && screenshot --full` |
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
| `screenshot [path]` | Viewport screenshot (auto-displayed) |
| `screenshot --full [path]` | Full page screenshot |
| `wait <sel\|ms>` | Wait for element or milliseconds |
| `console [--clear]` | View/clear console logs |
| `errors [--clear]` | View/clear page errors |
| `get text @eN` | Get element text content |
| `back` / `reload` | Navigation |
| `close [--all]` | Close browser |

#### Headed Mode (Visible Browser)

Add `--headed` flag to watch the browser in real time during debugging:

```bash
agent-browser --headed open https://localhost:8000/admin/md/products
agent-browser --headed snapshot -i
```

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
