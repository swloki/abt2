# Test Report: test_guest (Read-only Guest)

## User Info
- **Username**: test_guest
- **Password**: test1234
- **Role**: readonly_guest (parent: viewer, which has ALL read permissions)
- **Expected**: Can view all pages (read only), no create/edit/delete buttons

## Test Date: 2026-06-10

## Test Results

### TP-GUEST-01: Login
| Step | Action | Expected | Actual | Result |
|------|--------|----------|--------|--------|
| 1 | Login as test_guest | Redirect to /admin | Redirected to /admin | PASS |
| 2 | Page loads without error | Dashboard visible | Sales overview page shown | PASS |

### TP-GUEST-02: Page Access (Inheritance from viewer)
| Step | Action | Expected | Actual | Result |
|------|--------|----------|--------|--------|
| 1 | Navigate to /admin/customers | Page loads (CUSTOMER:read inherited from viewer) | Page loads with 8 customers | PASS |
| 2 | Navigate to /admin/md/products | Page loads (PRODUCT:read inherited from viewer) | Page loads with 12319 products | PASS |
| 3 | Navigate to /admin/wms/warehouses | Page loads (WAREHOUSE:read inherited from viewer) | Page loads with 19 warehouses | PASS |

**Inheritance works at server level**: readonly_guest inherits viewer's read permissions, all pages accessible.

### TP-GUEST-03: Sidebar NavFilter
| Step | Action | Expected | Actual | Result |
|------|--------|----------|--------|--------|
| 1 | Check sidebar modules | Should only show modules with read permission | Shows ALL 6 modules: sales, purchase, inventory, production, master data, system | **FAIL** |

**NavFilter is completely broken**: All sidebar modules and sub-items are shown regardless of permissions.

### TP-GUEST-04: Button-Level Permission Control
| Page | Button | Should Show | Actually Shows | Result |
|------|--------|-------------|----------------|--------|
| /admin/customers | "New Customer" | NO | YES (visible) | **FAIL** |
| /admin/customers | Row "Edit" | NO | YES (visible) | **FAIL** |
| /admin/customers | Row "Delete" | NO | YES (visible) | **FAIL** |
| /admin/md/products | "New Product" | NO | YES (visible) | **FAIL** |
| /admin/md/products | Row "Edit" | NO | YES (visible) | **FAIL** |
| /admin/md/products | Row "Delete" | NO | YES (visible) | **FAIL** |
| /admin/md/products | Row "Copy" | NO | YES (visible) | **FAIL** |
| /admin/md/products | Row "Set Price" | NO | YES (visible) | **FAIL** |
| /admin/wms/warehouses | "New Warehouse" | NO | YES (visible) | **FAIL** |
| /admin/wms/warehouses | Row "Edit" | NO | YES (visible) | **FAIL** |
| /admin/wms/warehouses | Row "Delete" | NO | YES (visible) | **FAIL** |

**Button-level permission control is completely broken**: All action buttons are visible for a read-only user.

### TP-GUEST-05: Server-Side Enforcement
| Step | Action | Expected | Actual | Result |
|------|--------|----------|--------|--------|
| 1 | Direct fetch POST to /admin/wms/warehouses/create | 403 Forbidden | 403 "无权执行此操作: WAREHOUSE:write" | PASS |

**Server-side 403 enforcement works correctly**.

### TP-GUEST-06: Create Form Access
| Step | Action | Expected | Actual | Result |
|------|--------|----------|--------|--------|
| 1 | Click "New Warehouse" link | Should be blocked or hidden | Form opens successfully | **FAIL** |
| 2 | Fill form and submit | 403 from server | Form visible but submit may not trigger via HTMX | N/A |

**Client-side pages are accessible**, but server blocks actual data modification.

## Summary

| Category | Status | Notes |
|----------|--------|-------|
| Login | PASS | Login works correctly |
| Page Access (Inheritance) | PASS | viewer permissions inherited correctly by readonly_guest |
| Sidebar NavFilter | **FAIL** | All modules shown, no filtering |
| Button Permission Control | **FAIL** | All action buttons visible for read-only user |
| Server-Side 403 | PASS | Server correctly blocks write operations |
| Create Form Access | **FAIL** | New/create forms accessible via URL |

## Key Findings

1. **Inheritance WORKS at server level**: readonly_guest → viewer inheritance chain functions correctly. The user can access all pages that viewer has read permissions for.
2. **Client-side permission filtering is completely broken**: No buttons are hidden, no menu items are filtered.
3. **Server-side enforcement is the only working protection**: 403 responses are returned for unauthorized write operations.
4. **Defense-in-depth is absent**: The system relies entirely on server-side protection with zero client-side hardening.
