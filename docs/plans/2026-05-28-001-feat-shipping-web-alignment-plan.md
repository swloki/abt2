# feat: 发货申请页面原型对齐 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Align the three shipping request web pages (list, detail, create) with the prototype designs in the Open Design project.

**Architecture:** Axum + Maud SSR for page rendering, HTMX for server-state interactions (filtering, pagination, form submit), Alpine.js for pure frontend UI state (modal, dynamic line items, calculations). Existing `ShippingRequestService` trait handles business logic; pages migrate from direct SQL to service calls. A new Alpine.js form function (`shippingForm`) manages the create page's complex client-side interactions, bridging state to the server via hidden input + `hx-post`.

**Tech Stack:** Rust (Axum, Maud, sqlx), HTMX, Alpine.js, UnoCSS

**Origin doc:** `docs/superpowers/specs/2026-05-28-shipping-web-design.md`

---

## Scope Boundaries

### In Scope
- List page UI alignment (create button, row actions, search placeholder, order links, tab counts)
- Detail page UI alignment (back-link, detail-header, info-card, item table columns, separate remarks)
- New create page (full Alpine.js + HTMX form with customer auto-fill, order picker modal, dynamic line items)
- Service factory functions in abt-core and AppState
- Minimal backend additions: `customer_id` filter in `ShippingQuery`, status count query

### Not In Scope
- Editing existing shipping requests (deferred)
- Priority field (backend model lacks it)
- Changes to service trait signatures beyond adding `customer_id` to `ShippingQuery`
- New CSS files — all styling via UnoCSS shortcuts/preflights only

### Deferred to Follow-Up Work
- Inline editing of shipping requests on list page
- Bulk operations (bulk confirm, bulk cancel)
- Print/export functionality for shipping documents

---

## Key Technical Decisions

1. **Add `customer_id` to `ShippingQuery`** — The service's `list()` method doesn't support customer filtering. Adding `Option<i64> customer_id` to the query model and one WHERE clause to the repo is a minimal change that enables migrating the list page from direct SQL to the service trait. This is a model/repo change, not a service logic change.

2. **Keep items query as direct SQL** — `ShippingRequestService` doesn't expose `list_items()`. Fetching items via `find_by_shipping_request_id` repo call or direct SQL is acceptable for the detail page. Adding a service method would be a larger change than needed.

3. **Soft-delete via direct SQL for draft** — The service has no `delete()` method. For draft-status shipping requests, a simple `UPDATE SET deleted_at = NOW()` is sufficient. This follows the existing pattern in sales_order_list.

4. **Migrate status transitions to service trait** — The detail page's current direct SQL (`UPDATE SET status = X`) bypasses inventory deduction, audit logging, state machine validation, and event publishing. Must be changed to call `service.confirm/pick/ship/cancel`.

5. **Order picker modal uses HTMX for order list** — When user opens the order picker modal, HTMX fetches orders filtered by customer_id. This follows the product search modal pattern in `order-create.js`. The modal itself is Alpine.js controlled (open/close), content is HTMX loaded.

---

## Implementation Units

### U1. Service Infrastructure & Backend Additions

**Goal:** Create factory functions for shipping and warehouse services, add them to AppState, and extend `ShippingQuery` with `customer_id` filter.

**Dependencies:** None

**Files:**
- Modify: `abt-core/src/sales/shipping_request/mod.rs` — add `new_shipping_request_service()` factory
- Modify: `abt-core/src/sales/shipping_request/model.rs` — add `customer_id: Option<i64>` to `ShippingQuery`
- Modify: `abt-core/src/sales/shipping_request/repo.rs` — add customer_id WHERE clause in `query()`
- Modify: `abt-core/src/wms/warehouse/mod.rs` — add `new_warehouse_service()` factory
- Modify: `abt-web2/src/state.rs` — add `shipping_service()` and `warehouse_service()` methods

**Approach:**

For `new_shipping_request_service()`: Follow `new_sales_order_service()` pattern in `abt-core/src/sales/sales_order/mod.rs`. The `ShippingRequestServiceImpl::new()` constructor takes 13 dependencies — repos are constructed directly, shared services use their own factory functions wrapped in `Arc`.

For `new_warehouse_service()`: `WarehouseServiceImpl::new(pool: Arc<PgPool>)` is simple. Wrap pool in Arc and construct.

For `ShippingQuery`: Add `pub customer_id: Option<i64>` field. In the repo's `query()` method, add the same conditional-param pattern used for `order_id`:
```rust
let customer_param = if let Some(cid) = filter.customer_id {
    param_idx += 1;
    conditions.push(format!("customer_id = ${param_idx}"));
    Some(cid)
} else {
    None
};
```

For AppState: Follow existing pattern. `shipping_service()` calls `new_shipping_request_service(self.pool.clone())`, `warehouse_service()` calls `new_warehouse_service(self.pool.clone())`.

**Patterns to follow:**
- `abt-core/src/sales/sales_order/mod.rs` — factory function pattern
- `abt-web2/src/state.rs` — existing service factory methods

**Test scenarios:**
- `cargo clippy` passes after all changes
- Factory functions compile and return correct trait impl types
- `ShippingQuery` with `customer_id: Some(id)` correctly filters results in repo query
- `ShippingQuery` with `customer_id: None` returns all results (no filter applied)

**Verification:** `cargo clippy` and `cargo build` pass. The new factories are callable from `state.rs`.

---

### U2. Shipping List Page Alignment

**Goal:** Align the shipping list page with prototype design — add create button, edit/delete row actions, expanded search, clickable order links, and per-status tab counts.

**Dependencies:** U1 (for service factory and `ShippingQuery.customer_id`)

**Files:**
- Modify: `abt-web2/src/pages/shipping_list.rs` — all list page changes
- Modify: `abt-web2/src/routes/shipping.rs` — add delete route

**Approach:**

**Page header:** Add `div class="page-actions"` with a primary button linking to `ShippingCreatePath::PATH`, using `icon::plus_icon("w-4 h-4")`.

**Status tab counts:** After the main query, run a single SQL query `SELECT status, COUNT(*) FROM shipping_requests WHERE deleted_at IS NULL [AND customer_id = $1] GROUP BY status` to get per-status counts. Map into `TabItem { count: Some(n) }` for each tab.

**Search placeholder:** Change to `"搜索发货单号、客户名称…"`.

**Filter bar:** Keep customer dropdown as-is. The `ShippingQuery` now supports `customer_id`.

**Source order column:** Change from plain text to `<a href={order_detail_path} style="color:var(--info)">{doc_number}</a>`. Need `OrderDetailPath` from the order routes module.

**Row actions:** For Draft-status rows, show `row-actions` div containing:
- Edit button (`icon::edit_icon`) — links to create page with `?edit={id}` query param (or just detail page for now, since editing is deferred)
- Delete button (`icon::trash_icon`) — triggers Alpine.js confirm_dialog

For non-Draft rows, show only view button (`icon::eye_icon`).

**Delete handler:** Add `POST /admin/shipping/{id}/delete` handler that soft-deletes a Draft shipping request:
```sql
UPDATE shipping_requests SET deleted_at = NOW(), updated_at = NOW() WHERE id = $1 AND status = 1 AND deleted_at IS NULL
```
Return `HX-Redirect` to list page on success.

**Service migration:** Replace `query_shipping_requests()` direct SQL with `shipping_svc.list(ctx, conn, ShippingQuery{..}, page)`. Replace `resolve_customer_names` and `resolve_order_numbers` helpers — keep these as they resolve display names from IDs.

**Patterns to follow:**
- `abt-web2/src/pages/sales_order_list.rs` — row actions, confirm dialog, page header, tab counts
- `abt-web2/src/components/confirm_dialog.rs` — delete confirmation
- `abt-web2/src/components/tabs.rs` — `TabItem` with `count` field

**Test scenarios:**
- List page renders with create button
- Status tabs show counts per status
- Search placeholder shows "搜索发货单号、客户名称…"
- Draft rows show edit and delete buttons
- Non-draft rows show only view button
- Delete button triggers confirm dialog, confirm sends POST, success redirects to list
- Source order numbers are clickable links to order detail
- Customer filter works through service trait

**Verification:** Visual comparison with prototype `shipping-list.html`. `cargo clippy` passes.

---

### U3. Shipping Detail Page Alignment

**Goal:** Align the detail page with prototype design — back-link, detail-header layout, info-card structure, expanded item table columns, separate remarks section, and service-trait-based status transitions.

**Dependencies:** U1 (for service factory)

**Files:**
- Modify: `abt-web2/src/pages/shipping_detail.rs` — all detail page changes

**Approach:**

**Layout restructuring** (top to bottom):
1. `a.back-link` with `icon::chevron_left_icon` + "返回发货申请列表"
2. `div.detail-header` — left side: `div.detail-title-row` with `h1.detail-no.font-mono` (doc_number) + status pill; below it a muted line with "来源订单：" + clickable order link. Right side: `div.page-actions` with conditional action buttons.
3. Workflow steps — keep existing `workflow_steps()` but align CSS classes to match the `wf-step/wf-line/wf-dot` pattern used in sales order detail (the current shipping detail uses slightly different class names like `workflow-connector/workflow-step`). Keep the current implementation as it already works visually; only adjust if CSS class mismatch is apparent.
4. `div.info-card` — title "发货信息", `info-grid` with: 客户名称, 收货地址, 预计发货日期, 承运商, 物流单号, 操作员
5. `div.data-card` — item table with expanded columns: 行号, 产品编码, 产品名称, 规格描述, 单位, 申请数量, 已发货, 发货仓库
6. Conditional `div.info-card` for "备注" (only shown if remark non-empty)

**Data enrichment:**
- Fetch operator name: query users table by `operator_id`
- Fetch product details: query products by `product_id` (get code, name, spec, unit)
- Fetch warehouse name: query warehouses by `warehouse_id`

**Status transitions:** Replace direct SQL with service calls:
- `confirm_shipping` → `shipping_svc.confirm(ctx, conn, id)`
- `pick_shipping` → `shipping_svc.pick(ctx, conn, id)`
- `ship_shipping` → `shipping_svc.ship(ctx, conn, id)`
- `cancel_shipping` → `shipping_svc.cancel(ctx, conn, id)`

Keep `HX-Redirect` responses.

**Patterns to follow:**
- `abt-web2/src/pages/sales_order_detail.rs` — back-link, detail-header, info-card, workflow steps
- `abt-web2/src/components/icon.rs` — `chevron_left_icon`, `truck_icon`, etc.

**Test scenarios:**
- Back-link navigates to shipping list
- Detail header shows doc_number + status pill + source order link
- Info card shows 6 fields (客户名称, 收货地址, 预计发货日期, 承运商, 物流单号, 操作员)
- Item table shows 8 columns (行号, 产品编码, 产品名称, 规格描述, 单位, 申请数量, 已发货, 发货仓库)
- Remarks section only appears when remark is non-empty
- Status transitions use service trait (not direct SQL)
- Draft status shows "确认发货" button
- Confirmed shows "开始拣货"
- Picking shows "确认发出"
- Draft/Confirmed show "取消" button

**Verification:** Visual comparison with prototype `shipping-detail.html`. `cargo clippy` passes.

---

### U4. Shipping Create Page — Backend (Routes & Handlers)

**Goal:** Add all new routes and handlers for the shipping create page, including HTMX endpoints for customer contacts and order search.

**Dependencies:** U1 (for service factory)

**Files:**
- Create: `abt-web2/src/pages/shipping_create.rs` — create page handlers
- Modify: `abt-web2/src/routes/shipping.rs` — add new TypedPaths and routes

**Approach:**

**New TypedPaths** (in `routes/shipping.rs`):
```
ShippingCreatePath          -> GET  /admin/shipping/create
ShippingCreatePostPath      -> POST /admin/shipping/create
ShippingDeletePath { id }   -> POST /admin/shipping/{id}/delete (from U2)
ShippingCustomerContactsPath -> GET  /admin/shipping/customer-contacts  (HTMX)
ShippingOrderSearchPath      -> GET  /admin/shipping/order-search       (HTMX)
```

**GET create handler** (`get_shipping_create`):
- Fetch customer list via `customer_svc.list()`
- Fetch warehouse list via `warehouse_svc.list()` (active only)
- Render the create form page using the template from U5

**POST create handler** (`post_shipping_create`):
- Receive `Form<ShippingCreateForm>` with `items_json: String`
- Parse `items_json` via `serde_json::from_str()` to get `Vec<CreateShippingItemReq>`
- Build `CreateFromOrderReq { order_id, expected_ship_date, shipping_address, items }`
- Call `shipping_svc.create_from_order(ctx, conn, req)`
- Return `HX-Redirect` to `ShippingDetailPath { id }`

**HTMX customer contacts** (`get_customer_contacts`):
- Query param: `customer_id`
- Fetch primary contact (name, phone) from `customer_contacts` table
- Fetch default shipping address from `customer_addresses` table
- Return HTML fragment with contact info display

**HTMX order search** (`get_order_search`):
- Query params: `customer_id`, `keyword`, `status`
- Call `sales_order_svc.list()` with filter by customer_id
- For each order, fetch items via `sales_order_svc.list_items()`
- Calculate remaining qty per item (ordered - shipped)
- Return HTML table fragment for modal body

**Patterns to follow:**
- `abt-web2/src/pages/sales_order_create.rs` — POST handler pattern
- `abt-web2/src/pages/quotation_create.rs` — product search HTMX handler
- `abt-web2/src/routes/shipping.rs` — existing TypedPath pattern

**Test scenarios:**
- GET create page renders with customer and warehouse dropdowns
- POST with valid data calls `create_from_order` and redirects
- POST with missing customer_id returns error toast
- POST with empty items returns error toast
- Customer contacts HTMX returns contact info for valid customer_id
- Customer contacts HTMX returns empty for unknown customer_id
- Order search HTMX returns orders for given customer_id
- Order search with keyword filters results

**Verification:** `cargo clippy` passes. Manual test: navigate to create page, select customer, see auto-filled contacts, open order modal, search orders.

---

### U5. Shipping Create Page — Frontend (Alpine.js & Maud Template)

**Goal:** Build the complete create page UI with Alpine.js state management, including customer auto-fill, order picker modal, dynamic line items, and summary calculations.

**Dependencies:** U4 (routes and handlers must exist)

**Files:**
- Modify: `abt-web2/src/pages/shipping_create.rs` — Maud template (created in U4, expanded here)
- Create: `abt-web2/static/shipping-create.js` — Alpine.js form function

**Approach:**

**Alpine.js function** (`shippingForm()`):
```javascript
function shippingForm() {
    return {
        customerId: '',
        orderId: '',
        orderModalOpen: false,
        items: [],

        // Auto-filled customer info
        contactName: '',
        contactPhone: '',
        shippingAddress: '',

        // Methods
        openOrderModal() { ... },
        closeOrderModal() { ... },
        selectOrder(orderId, orderItems) {
            // Parse order items, populate this.items
            // Each item: { order_item_id, product_id, product_code, product_name,
            //             specification, unit, ordered_qty, shipped_qty, ship_qty, warehouse_id }
        },
        addItem() { ... },
        removeItem(idx) { ... },

        // Computed
        get totalItems() { return this.items.length; },
        get totalQty() { return this.items.reduce((s, i) => s + (parseFloat(i.ship_qty) || 0), 0); },
        get itemsJson() { return JSON.stringify(this.items.map(...)); }
    };
}
```

**Maud template structure** (in `shipping_create.rs`):
1. Back-link to shipping list
2. Page header "新建发货申请" + "自动保存草稿" hint
3. **客户信息 form-section**: Customer select (triggers HTMX contact fill via `hx-get`), readonly contact/phone inputs, order picker input (readonly, opens modal on click), shipping address input
4. **发货信息 form-section**: Ship date, carrier select, default warehouse select, remark textarea
5. **发货产品明细 form-section**: Line items table (`template x-for`), add row button, summary bar (total items + total qty)
6. Bottom action bar: "保存草稿" + "提交申请" buttons

**Order picker modal**:
- Alpine.js controlled: `x-bind:class="{ 'is-open': orderModalOpen }"`
- Search input with `hx-get=ShippingOrderSearchPath::PATH` + `hx-target="#order-search-results"`
- Order list table with radio selection
- Confirm button calls `selectOrder()` to populate line items

**Key interactions:**
- Customer select `hx-get` → fetches contact info → fills readonly fields
- After customer selected, order picker input becomes enabled
- Order picker click → opens modal, HTMX loads orders for that customer
- Select order + confirm → items populated from order products, ship_qty defaults to (ordered - shipped)
- Line items rendered via `template x-for="(item, idx) in items"` with `x-model` bindings
- Hidden input `name="items_json" x-model="itemsJson"` bridges to HTMX
- Form submit: `hx-post=ShippingCreatePostPath::PATH hx-swap="none"`

**Patterns to follow:**
- `abt-web2/static/order-create.js` — Alpine form function structure
- `abt-web2/src/pages/sales_order_create.rs` — Maud template for create page
- `abt-web2/src/components/modal.rs` — modal component
- `abt-web2/CLAUDE.md` — Alpine.js form development pattern (x-data, x-for, x-model, hidden input bridge)

**Test scenarios:**
- Page loads with customer dropdown populated
- Selecting customer fills contact name, phone, and address
- Order picker is disabled until customer is selected
- Clicking order picker opens modal
- Modal shows orders for selected customer
- Searching in modal filters orders
- Selecting an order and confirming populates line items
- Line items show correct product details and default ship quantities
- Adding/removing rows works correctly
- Summary bar updates in real-time
- Form submit sends correct data to backend
- Error responses show toast notifications

**Verification:** Visual comparison with prototype `shipping-create.html`. Full flow test: select customer → pick order → verify items → submit → redirect to detail page.

---

### U6. Routes Wiring & Final Integration

**Goal:** Wire all new routes in the router, ensure all pages work together, and pass final lint checks.

**Dependencies:** U2, U3, U4, U5

**Files:**
- Modify: `abt-web2/src/routes/shipping.rs` — register all new routes
- Modify: `abt-web2/src/routes/mod.rs` — if shipping module isn't already registered
- Modify: `abt-web2/static/app.css` — UnoCSS rebuild if new shortcuts needed

**Approach:**

Add routes to `router()` function:
```rust
.route(ShippingCreatePath::PATH, get(shipping_create::get_shipping_create))
.route(ShippingCreatePostPath::PATH, post(shipping_create::post_shipping_create))
.route(ShippingDeletePath::PATH, post(shipping_list::delete_shipping))
.route(ShippingCustomerContactsPath::PATH, get(shipping_create::get_customer_contacts))
.route(ShippingOrderSearchPath::PATH, get(shipping_create::get_order_search))
```

Ensure all pages reference consistent TypedPaths. Verify no hardcoded URLs.

Run `cargo clippy` to catch any issues. Fix warnings.

**Test scenarios:**
- All routes are accessible without 404
- Navigation between list → detail → back works
- Navigation from list → create → submit → detail works
- Order detail link from shipping list navigates to correct order
- All HTMX interactions (filter, search, modal content) work without full page reload

**Verification:** `cargo clippy` passes clean. Full user flow works end-to-end.
