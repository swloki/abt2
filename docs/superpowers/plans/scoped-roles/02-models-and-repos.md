# Task 02: Models and Repositories

**Goal:** Add the `DeptRole` model, `UserDepartmentRoleRepo` repository, and update `Role` model to include `parent_role_id`.

**Depends on:** Task 01 (migration)

**Files:**
- Modify: `abt/src/models/role.rs` — add `parent_role_id` field
- Modify: `abt/src/models/mod.rs` — no change needed (already re-exports)
- Create: `abt/src/models/dept_role.rs` — new model
- Create: `abt/src/repositories/user_department_role_repo.rs` — new repo
- Modify: `abt/src/repositories/mod.rs` — register new repo

## Steps

- [ ] **Step 1: Update Role model to include parent_role_id**

Modify `abt/src/models/role.rs` — add `parent_role_id` field:

```rust
// In struct Role, add after is_system_role:
pub parent_role_id: Option<i64>,
```

Update the `FromRow` impl:
```rust
parent_role_id: row.try_get("parent_role_id")?,
```

- [ ] **Step 2: Update RoleRepo queries to include parent_role_id**

Modify `abt/src/repositories/role_repo.rs` — all `query_as!(Role, ...)` calls must select `parent_role_id`:

In `find_by_id`, `find_by_id_with_executor`, `find_by_code`, `list_all` — add `parent_role_id` to SELECT:

```sql
SELECT role_id, role_name, role_code, is_system_role,
       parent_role_id, description, created_at, updated_at
FROM roles WHERE ...
```

Note: sqlx `query_as!` macro requires exact column match. Since `parent_role_id` is nullable, the macro should handle `Option<i64>` correctly.

- [ ] **Step 3: Create DeptRole model**

Create `abt/src/models/dept_role.rs`:

```rust
use serde::{Deserialize, Serialize};

/// 用户在某个部门的角色分配
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeptRole {
    pub department_id: i64,
    pub role_id: i64,
}

/// 用户在某个部门的角色分配（含部门名称和角色名称，用于 API 返回）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeptRoleDetail {
    pub department_id: i64,
    pub department_name: String,
    pub role_id: i64,
    pub role_name: String,
}

/// 分配用户部门角色的请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignDeptRolesRequest {
    pub assignments: Vec<DeptRole>,
}
```

- [ ] **Step 4: Register DeptRole model in mod.rs**

Modify `abt/src/models/mod.rs` — add:

```rust
mod dept_role;
// in existing pub use section:
pub use dept_role::*;
```

- [ ] **Step 5: Create UserDepartmentRoleRepo**

Create `abt/src/repositories/user_department_role_repo.rs`:

```rust
use anyhow::Result;
use sqlx::PgPool;

use crate::models::{DeptRole, DeptRoleDetail};
use crate::repositories::Executor;

pub struct UserDepartmentRoleRepo;

impl UserDepartmentRoleRepo {
    /// Assign roles to a user in specific departments (merge semantics)
    pub async fn assign(
        executor: Executor<'_>,
        user_id: i64,
        assignments: &[DeptRole],
    ) -> Result<()> {
        for dept_role in assignments {
            sqlx::query!(
                r#"
                INSERT INTO user_department_roles (user_id, department_id, role_id)
                VALUES ($1, $2, $3)
                ON CONFLICT (user_id, department_id, role_id) DO NOTHING
                "#,
                user_id,
                dept_role.department_id,
                dept_role.role_id,
            )
            .execute(&mut *executor)
            .await?;
        }
        Ok(())
    }

    /// Remove specific role assignments for a user
    pub async fn remove(
        executor: Executor<'_>,
        user_id: i64,
        assignments: &[DeptRole],
    ) -> Result<()> {
        for dept_role in assignments {
            sqlx::query!(
                r#"
                DELETE FROM user_department_roles
                WHERE user_id = $1 AND department_id = $2 AND role_id = $3
                "#,
                user_id,
                dept_role.department_id,
                dept_role.role_id,
            )
            .execute(&mut *executor)
            .await?;
        }
        Ok(())
    }

    /// Get all department-role assignments for a user
    pub async fn get_user_dept_roles(
        pool: &PgPool,
        user_id: i64,
    ) -> Result<Vec<DeptRole>> {
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

        Ok(rows
            .into_iter()
            .map(|(department_id, role_id)| DeptRole {
                department_id,
                role_id,
            })
            .collect())
    }

    /// Get all department-role assignments for a user (with names, for API)
    pub async fn get_user_dept_role_details(
        pool: &PgPool,
        user_id: i64,
    ) -> Result<Vec<DeptRoleDetail>> {
        let rows = sqlx::query_as!(
            DeptRoleDetail,
            r#"
            SELECT
                udr.department_id,
                d.department_name,
                udr.role_id,
                r.role_name
            FROM user_department_roles udr
            JOIN departments d ON d.department_id = udr.department_id
            JOIN roles r ON r.role_id = udr.role_id
            WHERE udr.user_id = $1
            ORDER BY udr.department_id, udr.role_id
            "#,
            user_id
        )
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    /// Get role IDs for a user in a specific department
    pub async fn get_user_dept_role_ids(
        pool: &PgPool,
        user_id: i64,
        department_id: i64,
    ) -> Result<Vec<i64>> {
        let ids: Vec<(i64,)> = sqlx::query_as(
            r#"
            SELECT role_id FROM user_department_roles
            WHERE user_id = $1 AND department_id = $2
            "#,
        )
        .bind(user_id)
        .bind(department_id)
        .fetch_all(pool)
        .await?;

        Ok(ids.into_iter().map(|(id,)| id).collect())
    }

    /// Remove all role assignments for a user in a department
    pub async fn remove_all_for_dept(
        executor: Executor<'_>,
        user_id: i64,
        department_id: i64,
    ) -> Result<()> {
        sqlx::query!(
            "DELETE FROM user_department_roles WHERE user_id = $1 AND department_id = $2",
            user_id,
            department_id,
        )
        .execute(&mut *executor)
        .await?;
        Ok(())
    }
}
```

- [ ] **Step 6: Register new repo in mod.rs**

Modify `abt/src/repositories/mod.rs` — add:

```rust
mod user_department_role_repo;
pub use user_department_role_repo::UserDepartmentRoleRepo;
```

- [ ] **Step 7: Build to verify**

Run: `cd e:/work/abt && cargo build -p abt`
Expected: Compiles successfully.

- [ ] **Step 8: Commit**

```bash
git add abt/src/models/dept_role.rs abt/src/models/role.rs abt/src/models/mod.rs abt/src/repositories/user_department_role_repo.rs abt/src/repositories/mod.rs
git commit -m "feat: add DeptRole model, UserDepartmentRoleRepo, and parent_role_id on Role"
```
