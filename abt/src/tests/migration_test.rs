//! RBAC 迁移测试模块
//!
//! 测试 RBAC 相关的数据库迁移是否正确执行

#[cfg(test)]
mod tests {
    use sqlx::postgres::PgPool;

    /// 测试数据库连接并验证迁移表结构
    async fn get_test_pool() -> PgPool {
        let database_url = std::env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set for tests");
        PgPool::connect(&database_url)
            .await
            .expect("Failed to connect to test database")
    }

    /// Step 1 测试: 验证 roles 表存在且结构正确
    #[tokio::test]
    async fn test_roles_table_exists() {
        let pool = get_test_pool().await;

        // 验证表存在
        let table_exists: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS (
                SELECT 1 FROM information_schema.tables
                WHERE table_name = 'roles'
            )
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("Failed to check if roles table exists");

        assert!(table_exists, "roles table should exist");

        // 验证表结构
        let columns: Vec<(String, String)> = sqlx::query_as(
            r#"
            SELECT column_name, data_type
            FROM information_schema.columns
            WHERE table_name = 'roles'
            ORDER BY ordinal_position
            "#,
        )
        .fetch_all(&pool)
        .await
        .expect("Failed to get roles table columns");

        let column_names: Vec<&str> = columns.iter().map(|(name, _)| name.as_str()).collect();
        assert!(column_names.contains(&"role_id"), "roles should have role_id column");
        assert!(column_names.contains(&"role_name"), "roles should have role_name column");
        assert!(column_names.contains(&"role_code"), "roles should have role_code column");
        assert!(column_names.contains(&"is_system_role"), "roles should have is_system_role column");
    }

    /// Step 1 测试: 验证 users 表存在且结构正确
    #[tokio::test]
    async fn test_users_table_exists() {
        let pool = get_test_pool().await;

        let table_exists: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS (
                SELECT 1 FROM information_schema.tables
                WHERE table_name = 'users'
            )
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("Failed to check if users table exists");

        assert!(table_exists, "users table should exist");
    }

    /// Step 1 测试: 验证 user_roles 关联表存在
    #[tokio::test]
    async fn test_user_roles_table_exists() {
        let pool = get_test_pool().await;

        let table_exists: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS (
                SELECT 1 FROM information_schema.tables
                WHERE table_name = 'user_roles'
            )
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("Failed to check if user_roles table exists");

        assert!(table_exists, "user_roles table should exist");
    }

    /// Step 1 测试: 验证 role_permissions 关联表存在
    #[tokio::test]
    async fn test_role_permissions_table_exists() {
        let pool = get_test_pool().await;

        let table_exists: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS (
                SELECT 1 FROM information_schema.tables
                WHERE table_name = 'role_permissions'
            )
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("Failed to check if role_permissions table exists");

        assert!(table_exists, "role_permissions table should exist");
    }

    /// Step 1 测试: 验证 permission_audit_logs 表存在
    #[tokio::test]
    async fn test_permission_audit_logs_table_exists() {
        let pool = get_test_pool().await;

        let table_exists: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS (
                SELECT 1 FROM information_schema.tables
                WHERE table_name = 'permission_audit_logs'
            )
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("Failed to check if permission_audit_logs table exists");

        assert!(table_exists, "permission_audit_logs table should exist");
    }

    /// Step 2 测试: 验证索引创建
    #[tokio::test]
    async fn test_indexes_created() {
        let pool = get_test_pool().await;

        // 验证 user_roles 索引
        let idx_user_exists: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS (
                SELECT 1 FROM pg_indexes
                WHERE indexname = 'idx_user_roles_user'
            )
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("Failed to check idx_user_roles_user");

        let idx_role_exists: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS (
                SELECT 1 FROM pg_indexes
                WHERE indexname = 'idx_user_roles_role'
            )
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("Failed to check idx_user_roles_role");

        assert!(idx_user_exists, "idx_user_roles_user should exist");
        assert!(idx_role_exists, "idx_user_roles_role should exist");
    }

    /// Step 3 测试: 验证预置角色数据
    #[tokio::test]
    async fn test_preset_roles_exist() {
        let pool = get_test_pool().await;

        let role_codes: Vec<String> = sqlx::query_scalar(
            "SELECT role_code FROM roles ORDER BY role_id"
        )
        .fetch_all(&pool)
        .await
        .expect("Failed to fetch roles");

        assert!(role_codes.contains(&"super_admin".to_string()), "super_admin role should exist");
        assert!(role_codes.contains(&"admin".to_string()), "admin role should exist");
        assert!(role_codes.contains(&"user".to_string()), "user role should exist");
        assert_eq!(role_codes.len(), 3, "Should have exactly 3 preset roles");
    }

    /// Step 3 测试: 验证系统角色标记正确
    #[tokio::test]
    async fn test_system_roles_marked() {
        let pool = get_test_pool().await;

        let system_roles: Vec<String> = sqlx::query_scalar(
            "SELECT role_code FROM roles WHERE is_system_role = true"
        )
        .fetch_all(&pool)
        .await
        .expect("Failed to fetch system roles");

        assert_eq!(system_roles.len(), 3, "All 3 preset roles should be system roles");
    }
}
