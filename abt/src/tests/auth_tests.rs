//! Auth system unit tests

#[cfg(test)]
mod auth_tests {
    use crate::models::AuthContext;

    #[test]
    fn test_check_permission_granted() {
        let auth = AuthContext {
            user_id: 1,
            username: "test".to_string(),
            is_super_admin: false,
            permissions: vec![
                "product:read".to_string(),
                "product:write".to_string(),
                "warehouse:read".to_string(),
            ],
        };

        assert!(auth.check_permission("product", "read").is_ok());
        assert!(auth.check_permission("product", "write").is_ok());
        assert!(auth.check_permission("warehouse", "read").is_ok());
    }

    #[test]
    fn test_check_permission_denied() {
        let auth = AuthContext {
            user_id: 2,
            username: "limited".to_string(),
            is_super_admin: false,
            permissions: vec!["product:read".to_string()],
        };

        assert!(auth.check_permission("product", "read").is_ok());
        assert!(auth.check_permission("product", "write").is_err());
        assert!(auth.check_permission("warehouse", "read").is_err());
        assert!(auth.check_permission("bom", "delete").is_err());
    }

    #[test]
    fn test_super_admin_bypasses_all() {
        let auth = AuthContext {
            user_id: 0,
            username: "admin".to_string(),
            is_super_admin: true,
            permissions: vec![], // super_admin has empty permissions list
        };

        assert!(auth.check_permission("product", "read").is_ok());
        assert!(auth.check_permission("product", "write").is_ok());
        assert!(auth.check_permission("product", "delete").is_ok());
        assert!(auth.check_permission("user", "write").is_ok());
        assert!(auth.check_permission("role", "delete").is_ok());
        assert!(auth.check_permission("anything", "anyaction").is_ok());
    }

    #[test]
    fn test_empty_permissions_denies_all() {
        let auth = AuthContext {
            user_id: 3,
            username: "noperm".to_string(),
            is_super_admin: false,
            permissions: vec![],
        };

        assert!(auth.check_permission("product", "read").is_err());
        assert!(auth.check_permission("anything", "read").is_err());
    }
}
