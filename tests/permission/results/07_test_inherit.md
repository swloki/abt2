# Test Report: test_inherit (Inheritance Chain: derived_role -> base_role)

## User Info
- **Username**: test_inherit
- **Password**: test1234
- **Role**: derived_role (parent: base_role)
- **Inheritance Chain**: test_inherit -> derived_role -> base_role

### Permission Breakdown
| Resource | base_role (inherited) | derived_role (own) | Expected Combined |
|----------|----------------------|--------------------|-------------------|
| PRODUCT:read | YES | - | YES (inherited) |
| PRODUCT:create | - | YES | YES (own) |
| CATEGORY:read | YES | - | YES (inherited) |
| PRODUCT:update | NO | NO | NO |
| PRODUCT:delete | NO | NO | NO |
| CUSTOMER:* | NO | NO | NO |
| WAREHOUSE:* | NO | NO | NO |

## Test Date: 2026-06-10

## Test Results

### TP-INHERIT-01: Login
| Step | Action | Expected | Actual | Result |
|------|--------|----------|--------|--------|
| 1 | Login as test_inherit | Redirect to /admin | Redirected to /admin, sales overview shown | PASS |
| 2 | Username display | Show "测试-继承链" | Shows "承链 | 测试-继承链" | PASS |

### TP-INHERIT-02: Page Access (Inheritance Chain Verification)
| Page | Permission Required | Source | HTTP Status | Result |
|------|-------------------|--------|-------------|--------|
| /admin/md/products | PRODUCT:read | **Inherited from base_role** | 200 | **PASS** |
| /admin/md/categories | CATEGORY:read | **Inherited from base_role** | 200 | **PASS** |
| /admin/customers | CUSTOMER:read | None | 403 | PASS |
| /admin/system/users | USER:read | None | 403 | PASS |
| /admin/wms/warehouses | WAREHOUSE:read | None | 403 | PASS |

**Inheritance chain WORKS at server level**: derived_role successfully inherits PRODUCT:read and CATEGORY:read from base_role.

### TP-INHERIT-03: Write Operations (Permission Granularity)
| Operation | Permission | Source | Expected | Actual | Result |
|-----------|-----------|--------|----------|--------|--------|
| GET /admin/md/products/new | PRODUCT:create | derived_role own | 200 | 200 | **PASS** |
| Product edit (GET /admin/md/products/1/edit) | PRODUCT:update | None | 403 | 403 | **PASS** |
| Product delete (POST /admin/md/products/1/delete) | PRODUCT:delete | None | 403 | 403 | **PASS** |

**Permission granularity WORKS correctly**: Only PRODUCT:create (own) is allowed, PRODUCT:update and PRODUCT:delete are correctly denied.

### TP-INHERIT-04: Sidebar NavFilter
| Step | Action | Expected | Actual | Result |
|------|--------|----------|--------|--------|
| 1 | Check sidebar modules | Only show master data (only module with permissions) | Shows ALL 6 modules | **FAIL** |

### TP-INHERIT-05: Button-Level Permission Control
| Button | Permission Required | Should Show | Actually Shows | Result |
|--------|-------------------|-------------|----------------|--------|
| "New Product" | PRODUCT:create | YES (has it) | YES | PASS* |
| Row "Edit" | PRODUCT:update | NO | YES | **FAIL** |
| Row "Copy" | PRODUCT:create | YES (has it) | YES | PASS* |
| Row "Delete" | PRODUCT:delete | NO | YES | **FAIL** |
| Row "Set Price" | PRICE:* | NO | YES | **FAIL** |
| Row "Follow" | Unknown | NO | YES | **FAIL** |

*These buttons show correctly only because ALL buttons always show, not because of actual permission filtering.

### TP-INHERIT-06: Full Permission Summary
| Endpoint | Status | Permission Check | Inherited? |
|----------|--------|-----------------|------------|
| GET /admin/md/products | 200 | PRODUCT:read | YES (from base_role) |
| GET /admin/md/products/new | 200 | PRODUCT:create | NO (derived_role own) |
| GET /admin/md/products/1/edit | 403 | PRODUCT:update | N/A (not granted) |
| POST /admin/md/products/1/delete | 403 | PRODUCT:delete | N/A (not granted) |
| GET /admin/md/categories | 200 | CATEGORY:read | YES (from base_role) |
| GET /admin/customers | 403 | CUSTOMER:read | N/A (not granted) |

## Summary

| Category | Status | Notes |
|----------|--------|-------|
| Login | PASS | Login works correctly |
| Inheritance Chain (read) | **PASS** | base_role -> derived_role inheritance works correctly |
| Inheritance Chain (own permissions) | **PASS** | derived_role's own PRODUCT:create works alongside inherited PRODUCT:read |
| Permission Granularity | **PASS** | PRODUCT:update and PRODUCT:delete correctly denied |
| Sidebar NavFilter | **FAIL** | All modules shown |
| Button Permission Control | **FAIL** | All action buttons visible (edit, delete, set price shown despite no permission) |

## Key Findings

1. **Role inheritance chain WORKS correctly**: derived_role successfully inherits PRODUCT:read and CATEGORY:read from its parent base_role. The server correctly resolves the full permission chain.

2. **Own + inherited permissions merge correctly**: derived_role gets PRODUCT:read (from base_role) + PRODUCT:create (own). Both permissions are active simultaneously.

3. **Permission granularity is enforced at server level**: Only the specific actions granted are allowed. PRODUCT:update and PRODUCT:delete are correctly denied even though PRODUCT:read and PRODUCT:create are granted.

4. **Client-side shows everything**: Same as all other users - no button or menu filtering. This user can see "Edit" and "Delete" buttons that will fail with 403 if clicked.
