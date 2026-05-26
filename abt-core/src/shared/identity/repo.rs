use sqlx::Row;
use crate::shared::types::RepoResult;

use super::model::{Department, Role, RoleInfo, User};

pub struct IdentityRepo;

impl IdentityRepo {
    // -----------------------------------------------------------------------
    // User
    // -----------------------------------------------------------------------

    pub async fn insert_user(
        executor: &mut sqlx::postgres::PgConnection,
        username: &str,
        password_hash: &str,
        display_name: Option<&str>,
        is_super_admin: bool,
    ) -> RepoResult<User> {
        let row = sqlx::query(
            r#"
            INSERT INTO users (username, password_hash, display_name, is_active, is_super_admin)
            VALUES ($1, $2, $3, true, $4)
            RETURNING user_id, username, password_hash, display_name, is_active, is_super_admin, created_at, updated_at
            "#,
        )
        .bind(username)
        .bind(password_hash)
        .bind(display_name)
        .bind(is_super_admin)
        .fetch_one(&mut *executor)
        .await?;

        Self::row_to_user(&row)
    }

    pub async fn update_user(
        executor: &mut sqlx::postgres::PgConnection,
        user_id: i64,
        display_name: Option<&str>,
    ) -> RepoResult<User> {
        let row = sqlx::query(
            r#"
            UPDATE users
            SET display_name = $2, updated_at = NOW()
            WHERE user_id = $1
            RETURNING user_id, username, password_hash, display_name, is_active, is_super_admin, created_at, updated_at
            "#,
        )
        .bind(user_id)
        .bind(display_name)
        .fetch_one(&mut *executor)
        .await?;

        Self::row_to_user(&row)
    }

    pub async fn deactivate_user(
        executor: &mut sqlx::postgres::PgConnection,
        user_id: i64,
    ) -> RepoResult<()> {
        sqlx::query(
            "UPDATE users SET is_active = false, updated_at = NOW() WHERE user_id = $1",
        )
        .bind(user_id)
        .execute(&mut *executor)
        .await?;
        Ok(())
    }

    pub async fn get_user(
        executor: &mut sqlx::postgres::PgConnection,
        user_id: i64,
    ) -> RepoResult<User> {
        let row = sqlx::query(
            "SELECT user_id, username, password_hash, display_name, is_active, is_super_admin, created_at, updated_at \
             FROM users WHERE user_id = $1 AND is_active = true"
        )
        .bind(user_id)
        .fetch_one(&mut *executor)
        .await?;

        Self::row_to_user(&row)
    }

    pub async fn get_user_by_username(
        executor: &mut sqlx::postgres::PgConnection,
        username: &str,
    ) -> RepoResult<User> {
        let row = sqlx::query(
            "SELECT user_id, username, password_hash, display_name, is_active, is_super_admin, created_at, updated_at \
             FROM users WHERE username = $1 AND is_active = true"
        )
        .bind(username)
        .fetch_one(&mut *executor)
        .await?;

        Self::row_to_user(&row)
    }

    pub async fn list_users(
        executor: &mut sqlx::postgres::PgConnection,
        limit: i64,
        offset: i64,
    ) -> RepoResult<(Vec<User>, i64)> {
        let count_row = sqlx::query("SELECT COUNT(*) AS cnt FROM users WHERE is_active = true")
            .fetch_one(&mut *executor)
            .await?;
        let total: i64 = count_row.try_get("cnt")?;

        let rows = sqlx::query(
            "SELECT user_id, username, password_hash, display_name, is_active, is_super_admin, created_at, updated_at \
             FROM users WHERE is_active = true \
             ORDER BY user_id \
             LIMIT $1 OFFSET $2"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&mut *executor)
        .await?;

        let items: Vec<User> = rows.iter().map(Self::row_to_user).collect::<Result<Vec<_>, _>>()?;
        Ok((items, total))
    }

    pub async fn get_user_password_hash(
        executor: &mut sqlx::postgres::PgConnection,
        user_id: i64,
    ) -> RepoResult<String> {
        let row = sqlx::query("SELECT password_hash FROM users WHERE user_id = $1 AND is_active = true")
            .bind(user_id)
            .fetch_one(&mut *executor)
            .await?;
        Ok(row.try_get("password_hash")?)
    }

    // -----------------------------------------------------------------------
    // User-Role assignments
    // -----------------------------------------------------------------------

    pub async fn replace_user_roles(
        executor: &mut sqlx::postgres::PgConnection,
        user_id: i64,
        role_ids: &[i64],
    ) -> RepoResult<()> {
        sqlx::query("DELETE FROM user_roles WHERE user_id = $1")
            .bind(user_id)
            .execute(&mut *executor)
            .await?;

        for &role_id in role_ids {
            sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES ($1, $2)")
                .bind(user_id)
                .bind(role_id)
                .execute(&mut *executor)
                .await?;
        }
        Ok(())
    }

    pub async fn get_user_role_ids(
        executor: &mut sqlx::postgres::PgConnection,
        user_id: i64,
    ) -> RepoResult<Vec<i64>> {
        let rows = sqlx::query("SELECT role_id FROM user_roles WHERE user_id = $1")
            .bind(user_id)
            .fetch_all(&mut *executor)
            .await?;
        rows.iter().map(|r| r.try_get("role_id").map_err(Into::into)).collect()
    }

    pub async fn get_user_role_codes(
        executor: &mut sqlx::postgres::PgConnection,
        user_id: i64,
    ) -> RepoResult<Vec<String>> {
        let rows = sqlx::query(
            "SELECT r.role_code FROM user_roles ur JOIN roles r ON r.role_id = ur.role_id WHERE ur.user_id = $1"
        )
        .bind(user_id)
        .fetch_all(&mut *executor)
        .await?;
        rows.iter().map(|r| r.try_get("role_code").map_err(Into::into)).collect()
    }

    // -----------------------------------------------------------------------
    // User-Department assignments
    // -----------------------------------------------------------------------

    pub async fn replace_user_departments(
        executor: &mut sqlx::postgres::PgConnection,
        user_id: i64,
        dept_ids: &[i64],
    ) -> RepoResult<()> {
        sqlx::query("DELETE FROM user_departments WHERE user_id = $1")
            .bind(user_id)
            .execute(&mut *executor)
            .await?;

        for &dept_id in dept_ids {
            sqlx::query("INSERT INTO user_departments (user_id, department_id) VALUES ($1, $2)")
                .bind(user_id)
                .bind(dept_id)
                .execute(&mut *executor)
                .await?;
        }
        Ok(())
    }

    pub async fn remove_user_departments(
        executor: &mut sqlx::postgres::PgConnection,
        user_id: i64,
        dept_ids: &[i64],
    ) -> RepoResult<()> {
        for &dept_id in dept_ids {
            sqlx::query("DELETE FROM user_departments WHERE user_id = $1 AND department_id = $2")
                .bind(user_id)
                .bind(dept_id)
                .execute(&mut *executor)
                .await?;
        }
        Ok(())
    }

    pub async fn get_user_department_ids(
        executor: &mut sqlx::postgres::PgConnection,
        user_id: i64,
    ) -> RepoResult<Vec<i64>> {
        let rows = sqlx::query("SELECT department_id FROM user_departments WHERE user_id = $1")
            .bind(user_id)
            .fetch_all(&mut *executor)
            .await?;
        rows.iter().map(|r| r.try_get("department_id").map_err(Into::into)).collect()
    }

    // -----------------------------------------------------------------------
    // Role
    // -----------------------------------------------------------------------

    pub async fn insert_role(
        executor: &mut sqlx::postgres::PgConnection,
        role_name: &str,
        role_code: &str,
        description: Option<&str>,
        parent_role_id: Option<i64>,
    ) -> RepoResult<Role> {
        let row = sqlx::query(
            r#"
            INSERT INTO roles (role_name, role_code, is_system_role, parent_role_id, description)
            VALUES ($1, $2, false, $3, $4)
            RETURNING role_id, role_name, role_code, is_system_role, parent_role_id, description, created_at, updated_at
            "#,
        )
        .bind(role_name)
        .bind(role_code)
        .bind(parent_role_id)
        .bind(description)
        .fetch_one(&mut *executor)
        .await?;

        Self::row_to_role(&row)
    }

    pub async fn update_role(
        executor: &mut sqlx::postgres::PgConnection,
        role_id: i64,
        role_name: &str,
        description: Option<&str>,
    ) -> RepoResult<Role> {
        let row = sqlx::query(
            r#"
            UPDATE roles
            SET role_name = $2, description = $3, updated_at = NOW()
            WHERE role_id = $1
            RETURNING role_id, role_name, role_code, is_system_role, parent_role_id, description, created_at, updated_at
            "#,
        )
        .bind(role_id)
        .bind(role_name)
        .bind(description)
        .fetch_one(&mut *executor)
        .await?;

        Self::row_to_role(&row)
    }

    pub async fn delete_role(
        executor: &mut sqlx::postgres::PgConnection,
        role_id: i64,
    ) -> RepoResult<()> {
        // Delete role permissions first
        sqlx::query("DELETE FROM role_permissions WHERE role_id = $1")
            .bind(role_id)
            .execute(&mut *executor)
            .await?;
        // Delete user-role assignments
        sqlx::query("DELETE FROM user_roles WHERE role_id = $1")
            .bind(role_id)
            .execute(&mut *executor)
            .await?;
        // Delete the role
        sqlx::query("DELETE FROM roles WHERE role_id = $1")
            .bind(role_id)
            .execute(&mut *executor)
            .await?;
        Ok(())
    }

    pub async fn list_roles(
        executor: &mut sqlx::postgres::PgConnection,
    ) -> RepoResult<Vec<Role>> {
        let rows = sqlx::query(
            "SELECT role_id, role_name, role_code, is_system_role, parent_role_id, description, created_at, updated_at \
             FROM roles ORDER BY role_id"
        )
        .fetch_all(&mut *executor)
        .await?;

        rows.iter().map(Self::row_to_role).collect::<Result<Vec<_>, _>>()
    }

    // -----------------------------------------------------------------------
    // Role-Permission assignments
    // -----------------------------------------------------------------------

    pub async fn assign_permissions(
        executor: &mut sqlx::postgres::PgConnection,
        role_id: i64,
        permissions: &[(String, String)],
    ) -> RepoResult<()> {
        for (resource_code, action) in permissions {
            sqlx::query(
                "INSERT INTO role_permissions (role_id, resource_code, action) VALUES ($1, $2, $3) \
                 ON CONFLICT (role_id, resource_code, action) DO NOTHING"
            )
            .bind(role_id)
            .bind(resource_code)
            .bind(action)
            .execute(&mut *executor)
            .await?;
        }
        Ok(())
    }

    pub async fn remove_permissions(
        executor: &mut sqlx::postgres::PgConnection,
        role_id: i64,
        permissions: &[(String, String)],
    ) -> RepoResult<()> {
        for (resource_code, action) in permissions {
            sqlx::query(
                "DELETE FROM role_permissions WHERE role_id = $1 AND resource_code = $2 AND action = $3"
            )
            .bind(role_id)
            .bind(resource_code)
            .bind(action)
            .execute(&mut *executor)
            .await?;
        }
        Ok(())
    }

    pub async fn get_all_role_permissions(
        executor: &mut sqlx::postgres::PgConnection,
    ) -> RepoResult<Vec<(i64, String, String)>> {
        let rows = sqlx::query(
            "SELECT role_id, resource_code, action FROM role_permissions"
        )
        .fetch_all(&mut *executor)
        .await?;

        rows.iter()
            .map(|r| {
                Ok((
                    r.try_get("role_id")?,
                    r.try_get("resource_code")?,
                    r.try_get("action")?,
                ))
            })
            .collect()
    }

    pub async fn get_role_parent_map(
        executor: &mut sqlx::postgres::PgConnection,
    ) -> RepoResult<Vec<(i64, Option<i64>)>> {
        let rows = sqlx::query("SELECT role_id, parent_role_id FROM roles")
            .fetch_all(&mut *executor)
            .await?;
        rows.iter()
            .map(|r| Ok((r.try_get("role_id")?, r.try_get("parent_role_id")?)))
            .collect()
    }

    pub async fn get_role_permissions_by_ids(
        executor: &mut sqlx::postgres::PgConnection,
        role_ids: &[i64],
    ) -> RepoResult<Vec<(i64, String, String)>> {
        let rows = sqlx::query(
            "SELECT role_id, resource_code, action FROM role_permissions WHERE role_id = ANY($1)"
        )
        .bind(role_ids)
        .fetch_all(&mut *executor)
        .await?;

        rows.iter()
            .map(|r| {
                Ok((
                    r.try_get("role_id")?,
                    r.try_get("resource_code")?,
                    r.try_get("action")?,
                ))
            })
            .collect()
    }

    // -----------------------------------------------------------------------
    // Department
    // -----------------------------------------------------------------------

    pub async fn insert_department(
        executor: &mut sqlx::postgres::PgConnection,
        name: &str,
        code: &str,
        description: Option<&str>,
    ) -> RepoResult<Department> {
        let row = sqlx::query(
            r#"
            INSERT INTO departments (department_name, department_code, description, is_active, is_default)
            VALUES ($1, $2, $3, true, false)
            RETURNING department_id, department_name, department_code, description, is_active, is_default, created_at, updated_at
            "#,
        )
        .bind(name)
        .bind(code)
        .bind(description)
        .fetch_one(&mut *executor)
        .await?;

        Self::row_to_department(&row)
    }

    pub async fn update_department(
        executor: &mut sqlx::postgres::PgConnection,
        dept_id: i64,
        name: &str,
        description: Option<&str>,
    ) -> RepoResult<Department> {
        let row = sqlx::query(
            r#"
            UPDATE departments
            SET department_name = $2, description = $3, updated_at = NOW()
            WHERE department_id = $1
            RETURNING department_id, department_name, department_code, description, is_active, is_default, created_at, updated_at
            "#,
        )
        .bind(dept_id)
        .bind(name)
        .bind(description)
        .fetch_one(&mut *executor)
        .await?;

        Self::row_to_department(&row)
    }

    pub async fn deactivate_department(
        executor: &mut sqlx::postgres::PgConnection,
        dept_id: i64,
    ) -> RepoResult<()> {
        sqlx::query(
            "UPDATE departments SET is_active = false, updated_at = NOW() WHERE department_id = $1"
        )
        .bind(dept_id)
        .execute(&mut *executor)
        .await?;
        Ok(())
    }

    pub async fn get_department(
        executor: &mut sqlx::postgres::PgConnection,
        dept_id: i64,
    ) -> RepoResult<Department> {
        let row = sqlx::query(
            "SELECT department_id, department_name, department_code, description, is_active, is_default, created_at, updated_at \
             FROM departments WHERE department_id = $1"
        )
        .bind(dept_id)
        .fetch_one(&mut *executor)
        .await?;
        Self::row_to_department(&row)
    }

    pub async fn list_departments(
        executor: &mut sqlx::postgres::PgConnection,
    ) -> RepoResult<Vec<Department>> {
        let rows = sqlx::query(
            "SELECT department_id, department_name, department_code, description, is_active, is_default, created_at, updated_at \
             FROM departments WHERE is_active = true ORDER BY department_id"
        )
        .fetch_all(&mut *executor)
        .await?;

        rows.iter().map(Self::row_to_department).collect::<Result<Vec<_>, _>>()
    }

    pub async fn get_user_departments(
        executor: &mut sqlx::postgres::PgConnection,
        user_id: i64,
    ) -> RepoResult<Vec<Department>> {
        let rows = sqlx::query(
            "SELECT d.department_id, d.department_name, d.department_code, d.description, d.is_active, d.is_default, d.created_at, d.updated_at \
             FROM departments d \
             INNER JOIN user_departments ud ON d.department_id = ud.department_id \
             WHERE ud.user_id = $1 AND d.is_active = true \
             ORDER BY d.department_id"
        )
        .bind(user_id)
        .fetch_all(&mut *executor)
        .await?;
        rows.iter().map(Self::row_to_department).collect::<Result<Vec<_>, _>>()
    }

    // -----------------------------------------------------------------------
    // Bulk / composite queries
    // -----------------------------------------------------------------------

    pub async fn get_users_by_ids(
        executor: &mut sqlx::postgres::PgConnection,
        user_ids: &[i64],
    ) -> RepoResult<Vec<User>> {
        let rows = sqlx::query(
            "SELECT user_id, username, password_hash, display_name, is_active, is_super_admin, created_at, updated_at \
             FROM users WHERE user_id = ANY($1) AND is_active = true"
        )
        .bind(user_ids)
        .fetch_all(&mut *executor)
        .await?;

        rows.iter().map(Self::row_to_user).collect::<Result<Vec<_>, _>>()
    }

    pub async fn get_role_info_for_user(
        executor: &mut sqlx::postgres::PgConnection,
        user_id: i64,
    ) -> RepoResult<Vec<RoleInfo>> {
        let rows = sqlx::query(
            "SELECT r.role_id, r.role_name, r.role_code \
             FROM user_roles ur JOIN roles r ON r.role_id = ur.role_id \
             WHERE ur.user_id = $1"
        )
        .bind(user_id)
        .fetch_all(&mut *executor)
        .await?;

        rows.iter().map(Self::row_to_role_info).collect::<Result<Vec<_>, _>>()
    }

    pub async fn get_role_by_id(
        executor: &mut sqlx::postgres::PgConnection,
        role_id: i64,
    ) -> RepoResult<Role> {
        let row = sqlx::query(
            "SELECT role_id, role_name, role_code, is_system_role, parent_role_id, description, created_at, updated_at \
             FROM roles WHERE role_id = $1"
        )
        .bind(role_id)
        .fetch_one(&mut *executor)
        .await?;

        Self::row_to_role(&row)
    }

    pub async fn get_permissions_for_role(
        executor: &mut sqlx::postgres::PgConnection,
        role_id: i64,
    ) -> RepoResult<Vec<String>> {
        let rows = sqlx::query(
            "SELECT resource_code, action FROM role_permissions WHERE role_id = $1"
        )
        .bind(role_id)
        .fetch_all(&mut *executor)
        .await?;

        rows.iter()
            .map(|r| {
                let resource: String = r.try_get("resource_code")?;
                let action: String = r.try_get("action")?;
                Ok(format!("{}:{}", resource, action))
            })
            .collect()
    }

    pub async fn update_user_password(
        executor: &mut sqlx::postgres::PgConnection,
        user_id: i64,
        password_hash: &str,
    ) -> RepoResult<()> {
        sqlx::query(
            "UPDATE users SET password_hash = $2, updated_at = NOW() WHERE user_id = $1"
        )
        .bind(user_id)
        .bind(password_hash)
        .execute(&mut *executor)
        .await?;
        Ok(())
    }

    pub async fn update_user_status(
        executor: &mut sqlx::postgres::PgConnection,
        user_id: i64,
        is_active: bool,
    ) -> RepoResult<User> {
        let row = sqlx::query(
            "UPDATE users SET is_active = $2, updated_at = NOW() WHERE user_id = $1 \
             RETURNING user_id, username, password_hash, display_name, is_active, is_super_admin, created_at, updated_at"
        )
        .bind(user_id)
        .bind(is_active)
        .fetch_one(&mut *executor)
        .await?;
        Self::row_to_user(&row)
    }

    pub async fn add_user_roles(
        executor: &mut sqlx::postgres::PgConnection,
        user_id: i64,
        role_ids: &[i64],
    ) -> RepoResult<()> {
        for &role_id in role_ids {
            sqlx::query(
                "INSERT INTO user_roles (user_id, role_id) VALUES ($1, $2) \
                 ON CONFLICT (user_id, role_id) DO NOTHING"
            )
            .bind(user_id)
            .bind(role_id)
            .execute(&mut *executor)
            .await?;
        }
        Ok(())
    }

    pub async fn remove_user_roles(
        executor: &mut sqlx::postgres::PgConnection,
        user_id: i64,
        role_ids: &[i64],
    ) -> RepoResult<()> {
        for &role_id in role_ids {
            sqlx::query("DELETE FROM user_roles WHERE user_id = $1 AND role_id = $2")
                .bind(user_id)
                .bind(role_id)
                .execute(&mut *executor)
                .await?;
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Row mappers
    // -----------------------------------------------------------------------

    fn row_to_user(row: &sqlx::postgres::PgRow) -> RepoResult<User> {
        Ok(User {
            user_id: row.try_get("user_id")?,
            username: row.try_get("username")?,
            password_hash: row.try_get("password_hash")?,
            display_name: row.try_get("display_name")?,
            is_active: row.try_get("is_active")?,
            is_super_admin: row.try_get("is_super_admin")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }

    fn row_to_role(row: &sqlx::postgres::PgRow) -> RepoResult<Role> {
        Ok(Role {
            role_id: row.try_get("role_id")?,
            role_name: row.try_get("role_name")?,
            role_code: row.try_get("role_code")?,
            is_system_role: row.try_get("is_system_role")?,
            parent_role_id: row.try_get("parent_role_id")?,
            description: row.try_get("description")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }

    fn row_to_department(row: &sqlx::postgres::PgRow) -> RepoResult<Department> {
        Ok(Department {
            department_id: row.try_get("department_id")?,
            department_name: row.try_get("department_name")?,
            department_code: row.try_get("department_code")?,
            description: row.try_get("description")?,
            is_active: row.try_get("is_active")?,
            is_default: row.try_get("is_default")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }

    fn row_to_role_info(row: &sqlx::postgres::PgRow) -> RepoResult<RoleInfo> {
        Ok(RoleInfo {
            role_id: row.try_get("role_id")?,
            role_name: row.try_get("role_name")?,
            role_code: row.try_get("role_code")?,
        })
    }
}
