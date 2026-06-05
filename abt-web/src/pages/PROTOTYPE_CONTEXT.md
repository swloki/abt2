# Prototype Alignment — Shared Context

## Project Rules
- Axum + Maud (HTML templates) + HTMX + UnoCSS
- `#[require_permission("RESOURCE", "action")]` macro on all handlers
- `RequestContext` from `crate::utils` with fields: `conn, state, service_ctx, claims, is_htmx`
- `admin_page()` layout helper with module name `"system"`
- Route registration: `.route(TypedPath::PATH, get(handler))` pattern
- Match ergonomics: `match &value` not `match value` with ref patterns
- Define CSS classes in `uno.config.ts` componentStyles, NOT inline styles
- Maud boolean attribute: `checked[expr]` not `checked={expr}`
- NO inline `style` attributes in Maud templates — all styles as CSS classes in uno.config.ts

## Service Methods
- `state.user_service()`: `list_users_with_roles`, `get_user_with_roles`, `create_user`, `update_user`, `delete_user`, `update_user_status`, `batch_assign_roles`, `assign_roles`, `remove_roles`, `get_user_departments` (via dept_service), `change_password`
- `state.role_service()`: `list_roles`, `get_role_with_permissions`, `create_role(name, code, desc, parent_role_id)`, `update_role(id, name, desc)`, `assign_permissions`, `remove_permissions`, `list_roles_with_user_counts`
- `state.department_service()`: `list_departments`, `get_department`, `create_department(name, code, desc)`, `update_department(id, name, desc)`, `delete_department`, `assign_departments`, `remove_departments`, `get_user_departments`
- `state.permission_service()`: `get_user_permissions`

## Key Models
```rust
User { user_id, username, password_hash, display_name: Option<String>, is_active, is_super_admin, created_at, updated_at }
Role { role_id, role_name, role_code, is_system_role, parent_role_id: Option<i64>, description: Option<String>, created_at, updated_at }
Department { department_id, department_name, department_code, description: Option<String>, is_active, is_default, created_at, updated_at }
RoleInfo { role_id, role_name, role_code }
UserWithRoles { user: User, roles: Vec<RoleInfo> }
RoleWithPermissions { role: Role, permissions: Vec<String> } // "RESOURCE:action" strings
ResourceActionDef { resource_code, resource_name, description, action, action_name }
Claims { sub, username, display_name, system_role, role_ids, role_codes, department_ids, iss, exp, iat }
```

## Routes
- Users: `/admin/system/users` (list), `/admin/system/users/create` (create), `/admin/system/users/{id}` (detail), `/admin/system/users/{id}/edit` (edit)
- Roles: `/admin/system/roles` (list), `/admin/system/roles/create` (create), `/admin/system/roles/{id}` (detail), `/admin/system/roles/{id}/edit` (edit)
- Departments: `/admin/system/departments` (list), `/admin/system/departments/create` (create), `/admin/system/departments/{id}` (detail)
- Permissions: `/admin/system/permissions` (config page)

## Existing CSS Classes (uno.config.ts componentStyles)
data-card, data-table, data-card-scroll, filter-bar, filter-select, search-wrap, search-input, 
form-section, form-section-title, form-grid, form-field, form-input, form-select,
page-header, page-title, back-link, create-action-bar, btn, btn-primary, btn-default, btn-sm,
stat-card, pagination, status-tabs, status-pill, info-card, info-grid, info-item,
modal-overlay, modal, modal-lg, modal-head, modal-body, modal-foot, modal-close-btn,
perm-matrix-table series, perm-notice, perm-toggle, data-card, page-header, form-field
mono, status-active, status-inactive, stat-chip, tag-list, tag-chip, tag-normal

## Available Icon Functions (icon.rs)
user_icon, lock_icon, users_icon, shield_icon, building_icon, sliders_icon,
arrow_left_icon, return_arrow_icon, plus_icon, edit_icon, trash_icon, 
check_icon, eye_icon, eye_off_icon, key_icon, search_icon, refresh_icon
