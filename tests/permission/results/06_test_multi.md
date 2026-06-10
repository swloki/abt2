# Test Report: test_multi (Multi-Role: sales_manager + warehouse_keeper)

## User Info
- **Username**: test_multi
- **Password**: test1234
- **Roles**: sales_manager + warehouse_keeper (dual role assignment)
- **Expected**: Combined permissions from both roles

### Permission Breakdown
| Resource | sales_manager | warehouse_keeper | Expected Combined |
|----------|---------------|------------------|-------------------|
| CUSTOMER | CRUD | - | CRUD |
| SALES_ORDER | CRUD | - | CRUD |
| SHIPPING | CRUD | - | CRUD |
| PRODUCT | read | read | read |
| CATEGORY | read | read | read |
| PRICE | read | - | read |
| WAREHOUSE | - | CRUD | CRUD |
| LOCATION | - | CRUD | CRUD |
| INVENTORY | - | CRUD | CRUD |

## Test Date: 2026-06-10

## Test Results

### TP-MULTI-01: Login
| Step | Action | Expected | Actual | Result |
|------|--------|----------|--------|--------|
| 1 | Login as test_multi | Redirect to /admin | Redirected to /admin, sales overview shown | PASS |

### TP-MULTI-02: Page Access (Multi-Role Merging)
| Page | Permission Source | HTTP Status | Result |
|------|-------------------|-------------|--------|
| /admin/customers | sales_manager: CUSTOMER:read | 200 | PASS |
| /admin/wms/warehouses | warehouse_keeper: WAREHOUSE:read | 200 | PASS |
| /admin/md/products | both: PRODUCT:read | 200 | PASS |
| /admin/md/categories | both: CATEGORY:read | 200 | PASS |
| /admin/system/users | neither | 403 | PASS |
| /admin/purchase/orders | neither | 403 | PASS |

**Multi-role permission merging WORKS correctly**: User gets combined permissions from both roles.

### TP-MULTI-03: Write Operations
| Operation | Permission | Action Code in Seed | Action Code in Page | Expected | Actual | Result |
|-----------|-----------|--------------------|--------------------|----------|--------|--------|
| GET /admin/customers/create | CUSTOMER:create | "create" | "create" | 200/400 (form) | 400 (GET not expected) | PARTIAL |
| POST /admin/wms/warehouses/create | WAREHOUSE:create | "create" | **"write"** | Should work | **403** | **FAIL** |
| GET /admin/wms/warehouses/create | WAREHOUSE:read | "read" | "read" | 200 | 200 | PASS |

**Critical Finding**: WAREHOUSE create page GET handler checks "read" (passes), but POST handler checks "write" (fails because permission is defined as "create"). This is an **action code mismatch** between the permission system and the page handlers.

### TP-MULTI-04: Sidebar NavFilter
| Step | Action | Expected | Actual | Result |
|------|--------|----------|--------|--------|
| 1 | Check sidebar modules | Only show sales + WMS modules | Shows ALL 6 modules | **FAIL** |

### TP-MULTI-05: Button-Level Permission Control
| Page | Button | Should Show | Actually Shows | Result |
|------|--------|-------------|----------------|--------|
| /admin/customers | "New Customer" | YES (CUSTOMER:create) | YES | PASS* |
| /admin/customers | Row "Edit"/"Delete" | YES (CUSTOMER:update/delete) | YES | PASS* |
| /admin/wms/warehouses | "New Warehouse" | YES (WAREHOUSE:create) | YES | PASS* |

*Buttons show correctly but only because ALL buttons always show — not because of actual permission filtering.

## Summary

| Category | Status | Notes |
|----------|--------|-------|
| Login | PASS | Login works correctly |
| Multi-Role Merging | **PASS** | Combined permissions from both roles work at server level |
| Write Operations | **FAIL** | WAREHOUSE write fails due to action code mismatch ("write" vs "create") |
| Sidebar NavFilter | **FAIL** | All modules shown, no filtering |
| Button Permission Control | **FAIL** | All buttons show (no filtering at all) |

## Key Findings

1. **Multi-role permission merging WORKS**: The server correctly combines permissions from sales_manager and warehouse_keeper. The user can access pages from both roles.
2. **Action code mismatch is a critical bug**: WMS pages use `require_permission("WAREHOUSE", "write")` but the permission system defines "create"/"update"/"delete" actions. This means warehouse_keeper's CRUD permissions on WAREHOUSE are effectively useless for write operations — only read works.
3. **Page GET vs POST permission split is inconsistent**: Warehouse create page GET checks "read" (allowing form display), but POST checks "write" (blocking actual creation). This creates confusing UX where the form opens but can't be submitted.
4. **Client-side shows everything regardless**: Same as other users — no button/menu filtering.
