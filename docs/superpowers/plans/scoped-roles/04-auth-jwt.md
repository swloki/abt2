# Task 04: Auth/JWT — Claims, AuthContext, Login/Refresh

**Goal:** Refactor JWT Claims and AuthContext to use the new scoped roles structure. Update login, refresh_token, and add switch_department.

**Depends on:** Task 02 (models + repos), Task 03 (permission cache)

**Files:**
- Modify: `abt/src/models/auth.rs` — new Claims + AuthContext
- Modify: `abt/src/service/auth_service.rs` — add switch_department
- Modify: `abt/src/implt/auth_service_impl.rs` — rewrite login/refresh/build_claims, add switch_department
- Modify: `abt/src/repositories/auth_repo.rs` — new query for dept_roles

## Steps

- [ ] **Step 1: Rewrite Claims struct**

Modify `abt/src/models/auth.rs` — replace the entire Claims struct:

```rust
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// JWT Claims 结构 (Scoped Roles)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// 用户 ID
    pub sub: i64,
    /// 用户名
    pub username: String,
    /// 显示名
    pub display_name: String,
    /// 系统角色: "super_admin" | "user"
    pub system_role: String,
    /// 部门-角色映射: department_id (as string key) -> list of role_ids
    pub dept_roles: HashMap<String, Vec<i64>>,
    /// 当前部门上下文 ID
    pub current_department_id: Option<i64>,
    /// 过期时间 (UNIX timestamp)
    pub exp: u64,
    /// 签发时间 (UNIX timestamp)
    pub iat: u64,
}

/// 从 gRPC request extensions 中提取的认证上下文
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user_id: i64,
    pub username: String,
    pub system_role: String,
    pub dept_roles: HashMap<String, Vec<i64>>,
    pub current_department_id: Option<i64>,
}

impl AuthContext {
    /// 是否超级管理员
    pub fn is_super_admin(&self) -> bool {
        self.system_role == "super_admin"
    }

    /// 检查用户是否属于指定部门
    pub fn belongs_to_department(&self, department_id: i64) -> bool {
        self.is_super_admin()
            || self.dept_roles.contains_key(&department_id.to_string())
    }

    /// 获取用户在指定部门的角色 ID 列表
    pub fn get_dept_role_ids(&self, department_id: i64) -> Vec<i64> {
        self.dept_roles
            .get(&department_id.to_string())
            .cloned()
            .unwrap_or_default()
    }
}

/// 资源操作定义（代码注册，非数据库）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceActionDef {
    pub resource_code: &'static str,
    pub resource_name: &'static str,
    pub description: &'static str,
    pub action: &'static str,
    pub action_name: &'static str,
}
```

- [ ] **Step 2: Update auth_repo to fetch dept_roles**

Modify `abt/src/repositories/auth_repo.rs` — replace `get_user_permission_codes` with `get_user_dept_roles`:

```rust
use anyhow::Result;
use sqlx::PgPool;
use std::collections::HashMap;

use crate::models::User;
use crate::repositories::UserRepo;

pub struct AuthRepo;

impl AuthRepo {
    pub async fn find_user_by_username(pool: &PgPool, username: &str) -> Result<Option<User>> {
        UserRepo::find_by_username(pool, username).await
    }

    pub async fn find_user_by_id(pool: &PgPool, user_id: i64) -> Result<Option<User>> {
        UserRepo::find_by_id(pool, user_id).await
    }

    /// Get user's department-role mappings as a nested map:
    /// { department_id_string => [role_id, ...] }
    pub async fn get_user_dept_roles(pool: &PgPool, user_id: i64) -> Result<HashMap<String, Vec<i64>>> {
        let rows: Vec<(i64, i64)> = sqlx::query_as(
            r#"
            SELECT department_id, role_id
            FROM user_department_roles
            WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;

        let mut map: HashMap<String, Vec<i64>> = HashMap::new();
        for (dept_id, role_id) in rows {
            map.entry(dept_id.to_string())
                .or_default()
                .push(role_id);
        }
        Ok(map)
    }
}
```

- [ ] **Step 3: Update AuthService trait**

Modify `abt/src/service/auth_service.rs` — add `switch_department`:

```rust
use anyhow::Result;
use async_trait::async_trait;
use crate::models::{Claims, ResourceActionDef};

#[async_trait]
pub trait AuthService: Send + Sync {
    async fn login(&self, username: &str, password: &str) -> Result<(String, i64, Claims)>;
    async fn refresh_token(&self, token: &str) -> Result<(String, i64, Claims)>;
    async fn get_user_claims(&self, user_id: i64) -> Result<Claims>;
    fn list_resources(&self) -> Vec<ResourceActionDef>;

    /// Switch current department context, returns updated token
    async fn switch_department(&self, user_id: i64, department_id: i64) -> Result<(String, i64, Claims)>;
}
```

- [ ] **Step 4: Rewrite auth_service_impl**

Modify `abt/src/implt/auth_service_impl.rs` — update `build_claims`, `login`, `refresh_token`, `get_user_claims`, add `switch_department`:

Key changes to `build_claims`:
```rust
fn build_claims(
    user_id: i64,
    username: String,
    display_name: String,
    system_role: String,
    dept_roles: HashMap<String, Vec<i64>>,
    current_department_id: Option<i64>,
    now: u64,
    expiration_hours: u64,
) -> Claims {
    Claims {
        sub: user_id,
        username,
        display_name,
        system_role,
        dept_roles,
        current_department_id,
        iat: now,
        exp: now + expiration_hours * SECONDS_PER_HOUR,
    }
}
```

Key changes to `login`:
```rust
async fn login(&self, username: &str, password: &str) -> Result<(String, i64, Claims)> {
    // 1-3. Same: find user, check active, verify password

    // 4. Determine system_role
    let system_role = if user.is_super_admin {
        "super_admin".to_string()
    } else {
        "user".to_string()
    };

    // 5. Get dept_roles from user_department_roles
    let dept_roles = AuthRepo::get_user_dept_roles(self.pool.as_ref(), user.user_id).await?;

    // 6. Determine current_department_id
    let current_department_id = self.resolve_default_department(&dept_roles).await?;

    // 7. Build and sign JWT
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let display_name = user.display_name.clone().unwrap_or_default();
    let claims = Self::build_claims(
        user.user_id,
        user.username.clone(),
        display_name,
        system_role,
        dept_roles,
        current_department_id,
        now,
        self.jwt_expiration_hours,
    );

    let expires_at = claims.exp as i64;
    let token = self.sign_jwt(&claims)?;
    Ok((token, expires_at, claims))
}
```

Add helper `resolve_default_department`:
```rust
/// Resolve default department: if only one dept, auto-select; else None (frontend chooses).
async fn resolve_default_department(
    &self,
    dept_roles: &HashMap<String, Vec<i64>>,
) -> Result<Option<i64>> {
    let dept_ids: Vec<i64> = dept_roles.keys()
        .filter_map(|k| k.parse::<i64>().ok())
        .collect();

    if dept_ids.len() == 1 {
        return Ok(Some(dept_ids[0]));
    }

    // Multiple or zero departments — use default department if no assignments
    if dept_ids.is_empty() {
        let default_id = DepartmentResourceAccessRepo::get_default_department_id(
            self.pool.as_ref(),
        ).await?;
        return Ok(default_id);
    }

    // Multiple departments — frontend will choose
    Ok(None)
}
```

Add `switch_department`:
```rust
async fn switch_department(&self, user_id: i64, department_id: i64) -> Result<(String, i64, Claims)> {
    // 1. Verify user exists and is active
    let user = AuthRepo::find_user_by_id(self.pool.as_ref(), user_id)
        .await?
        .ok_or_else(|| anyhow!("User not found"))?;
    if !user.is_active {
        return Err(anyhow!("User account is disabled"));
    }

    // 2. Verify user belongs to this department
    let dept_roles = AuthRepo::get_user_dept_roles(self.pool.as_ref(), user_id).await?;
    if !dept_roles.contains_key(&department_id.to_string()) {
        return Err(anyhow!("User does not belong to department {}", department_id));
    }

    // 3. Build new claims with updated current_department_id
    let system_role = if user.is_super_admin { "super_admin" } else { "user" }.to_string();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let display_name = user.display_name.clone().unwrap_or_default();
    let claims = Self::build_claims(
        user.user_id,
        user.username.clone(),
        display_name,
        system_role,
        dept_roles,
        Some(department_id),
        now,
        self.jwt_expiration_hours,
    );

    let expires_at = claims.exp as i64;
    let token = self.sign_jwt(&claims)?;
    Ok((token, expires_at, claims))
}
```

Update `refresh_token` and `get_user_claims` similarly — use the same pattern as login (build Claims with system_role + dept_roles).

- [ ] **Step 5: Update auth_interceptor**

Modify `abt-grpc/src/interceptors/auth.rs` — update `auth_interceptor` to build new AuthContext:

```rust
pub fn auth_interceptor(mut request: Request<()>) -> Result<Request<()>, Status> {
    let claims = decode_jwt_from_request(&request)?;

    let auth_ctx = abt::AuthContext {
        user_id: claims.sub,
        username: claims.username,
        system_role: claims.system_role,
        dept_roles: claims.dept_roles,
        current_department_id: claims.current_department_id,
    };

    request.extensions_mut().insert(auth_ctx);
    Ok(request)
}
```

- [ ] **Step 6: Build to verify**

Run: `cd e:/work/abt && cargo build`
Expected: Compiles. There will be compile errors in the macro-generated code and handlers that use `auth.check_permission(...)` — these will be fixed in Task 05 and 06.

- [ ] **Step 7: Commit**

```bash
git add abt/src/models/auth.rs abt/src/repositories/auth_repo.rs abt/src/service/auth_service.rs abt/src/implt/auth_service_impl.rs abt-grpc/src/interceptors/auth.rs
git commit -m "feat: refactor Claims/AuthContext for scoped roles, add switch_department"
```
