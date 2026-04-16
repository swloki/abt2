# Task 05: require_permission Macro Update

**Goal:** Update the `require_permission` macro's generated code to implement the new two-step permission check: department归属校验 → department resource visibility → scoped role permissions.

**Depends on:** Task 04 (new AuthContext with dept_roles)

**Files:**
- Modify: `abt-macros/src/lib.rs` — rewrite generated check code
- Modify: `abt-grpc/src/permissions/mod.rs` — add helper functions for the macro to call
- Modify: `abt/src/models/auth.rs` — add `check_scoped_permission` method (or put in a separate helper)
- Modify: `abt/src/models/resources.rs` — no change needed (already has is_system_resource)

## Steps

- [ ] **Step 1: Create permission check helper**

Create a helper module or function that the macro can call. The macro generates raw code, so it needs a simple function interface.

Add to `abt/src/models/auth.rs` or create `abt-grpc/src/permissions/scoped_check.rs`:

The cleanest approach: add a `check_scoped_permission` method to `AuthContext` and a free function the macro calls.

First, add `use` imports and method to `AuthContext` in `abt/src/models/auth.rs`:

```rust
impl AuthContext {
    pub fn is_super_admin(&self) -> bool { /* already added */ }
    pub fn belongs_to_department(&self, department_id: i64) -> bool { /* already added */ }
    pub fn get_dept_role_ids(&self, department_id: i64) -> Vec<i64> { /* already added */ }
}
```

Then, create the actual check function that the macro will call. Add to `abt-grpc/src/permissions/mod.rs`:

```rust
use crate::generated::abt::v1::{Action, Resource};
use std::collections::HashMap;

/// Check scoped permission for a business resource.
/// Called by require_permission macro for business resources.
///
/// Flow:
/// 1. If super_admin and no dept_roles → allow (super_admin doesn't belong to departments)
/// 2. Extract department_id from gRPC metadata
/// 3. Check user belongs to this department
/// 4. Check department has access to this resource type
/// 5. Check user's roles in this department have the required permission
pub fn check_business_permission(
    auth: &abt::AuthContext,
    resource_code: &str,
    action_code: &str,
    department_id: Option<i64>,
    dept_resource_access: &HashMap<i64, Vec<String>>,
) -> Result<(), String> {
    // Super admin with no dept assignments = full access
    if auth.is_super_admin() && auth.dept_roles.is_empty() {
        return Ok(());
    }

    // Must have a department context
    let dept_id = department_id.ok_or_else(|| {
        "department_id is required for business resource operations".to_string()
    })?;

    // Step 0: Department归属校验
    if !auth.belongs_to_department(dept_id) {
        return Err(format!("User does not belong to department {}", dept_id));
    }

    // Super admin who belongs to the department → allow
    if auth.is_super_admin() {
        return Ok(());
    }

    // Step 1: Department resource visibility
    let accessible = dept_resource_access.get(&dept_id)
        .ok_or_else(|| format!("Department {} has no resource access configured", dept_id))?;
    if !accessible.contains(&resource_code.to_string()) {
        return Err(format!("Department {} does not have access to resource {}", dept_id, resource_code));
    }

    // Step 2: Check role permissions via cache
    let role_ids = auth.get_dept_role_ids(dept_id);
    if role_ids.is_empty() {
        return Err(format!("User has no roles in department {}", dept_id));
    }

    let cache = abt::get_permission_cache();
    // Use try_get to avoid blocking in sync context — the cache is always loaded at startup
    let has_perm = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            cache.has_permission(&role_ids, resource_code, action_code).await
        })
    });

    if has_perm {
        Ok(())
    } else {
        Err(format!("No permission for {}:{} in department {}", resource_code, action_code, dept_id))
    }
}

/// Check system resource permission.
/// Called by require_permission macro for system resources.
pub fn check_system_permission(
    auth: &abt::AuthContext,
    resource_code: &str,
    action_code: &str,
    system_role_permissions: &[&str],
) -> Result<(), String> {
    if auth.is_super_admin() {
        return Ok(());
    }

    let required = format!("{}:{}", resource_code, action_code);
    if system_role_permissions.contains(&required.as_str()) {
        Ok(())
    } else {
        Err(format!("No system permission for {}:{}", resource_code, action_code))
    }
}
```

- [ ] **Step 2: Update require_permission macro**

Modify `abt-macros/src/lib.rs` — change the generated `check_stmt`:

The macro currently generates:
```rust
auth.check_permission(resource.code(), action.code())
    .map_err(|_e| error::forbidden(resource.code(), action.code()))?;
```

Change to:
```rust
permissions::check_permission_for_resource(
    &auth,
    resource.code(),
    action.code(),
    request.metadata().get("x-department-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<i64>().ok()),
).map_err(|_e| error::forbidden(resource.code(), action.code()))?;
```

The macro should generate a single function call that dispatches to business vs system check internally. The function `check_permission_for_resource` will be in `abt-grpc/src/permissions/mod.rs` and will:
1. Check if resource_code is a system resource → call `check_system_permission`
2. Otherwise → call `check_business_permission`

The key change in the macro's `let check_stmt` line:

```rust
let check_stmt: Stmt = parse_quote! {
    crate::permissions::check_permission_for_resource(
        &auth,
        #resource.code(),
        #action.code(),
        #request_ident.metadata().get("x-department-id")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<i64>().ok()),
    ).map_err(|_e| error::forbidden(#resource.code(), #action.code()))?;
};
```

- [ ] **Step 3: Add `check_permission_for_resource` dispatcher**

Add to `abt-grpc/src/permissions/mod.rs`:

```rust
use abt::{is_system_resource, is_business_resource};

/// Main permission check entry point, called by require_permission macro.
/// Dispatches to system or business resource check based on resource type.
pub fn check_permission_for_resource(
    auth: &abt::AuthContext,
    resource_code: &str,
    action_code: &str,
    department_id: Option<i64>,
) -> Result<(), String> {
    if is_system_resource(resource_code) {
        // System resources: check system role permissions
        // user role has: user:read, department:read, permission:read
        let user_permissions = ["user:read", "department:read", "permission:read"];
        check_system_permission(auth, resource_code, action_code, &user_permissions)
    } else {
        // Business resources: scoped check with department context
        // Load dept_resource_access from a global cache or pass through
        let dept_access = get_dept_resource_access();
        check_business_permission(auth, resource_code, action_code, department_id, &dept_access)
    }
}
```

- [ ] **Step 4: Add department resource access cache**

The `check_business_permission` function needs access to `department_resource_access` data without hitting the database. Add a simple global cache similar to `RolePermissionCache`.

Create in `abt/src/permission_cache.rs` or add a separate `DeptResourceAccessCache`:

```rust
// In abt/src/permission_cache.rs, add:

/// Cache for department resource access (department_id -> list of resource_codes)
pub struct DeptResourceAccessCache {
    cache: RwLock<HashMap<i64, Vec<String>>>,
}

impl DeptResourceAccessCache {
    pub fn new() -> Self {
        Self { cache: RwLock::new(HashMap::new()) }
    }

    pub async fn load(&self, pool: &PgPool) -> Result<()> {
        let rows: Vec<(i64, String)> = sqlx::query_as(
            "SELECT department_id, resource_code FROM department_resource_access"
        )
        .fetch_all(pool)
        .await?;

        let mut map: HashMap<i64, Vec<String>> = HashMap::new();
        for (dept_id, code) in rows {
            map.entry(dept_id).or_default().push(code);
        }
        *self.cache.write().await = map;
        Ok(())
    }

    pub async fn get(&self, department_id: i64) -> Option<Vec<String>> {
        self.cache.read().await.get(&department_id).cloned()
    }

    /// Sync getter for use in blocking context (permission checks)
    pub fn get_sync(&self, cache: &HashMap<i64, Vec<String>>, department_id: i64) -> Option<Vec<String>> {
        cache.get(&department_id).cloned()
    }
}
```

Alternatively, combine both caches into a single struct for simplicity.

- [ ] **Step 5: Build to verify**

Run: `cd e:/work/abt && cargo build`
Expected: Compiles. All handlers using `#[require_permission]` should now pass.

- [ ] **Step 6: Commit**

```bash
git add abt-macros/src/lib.rs abt-grpc/src/permissions/mod.rs abt/src/permission_cache.rs abt/src/models/auth.rs
git commit -m "feat: update require_permission macro for scoped role permission checks"
```
