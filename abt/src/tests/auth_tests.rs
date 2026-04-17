//! Auth system unit tests

#[cfg(test)]
mod auth_tests {
    use crate::models::{AuthContext, Claims};

    fn make_auth(system_role: &str, role_ids: Vec<i64>) -> AuthContext {
        AuthContext {
            user_id: 1,
            username: "test".to_string(),
            system_role: system_role.to_string(),
            role_ids,
        }
    }

    #[test]
    fn test_is_super_admin() {
        let admin = make_auth("super_admin", vec![]);
        assert!(admin.is_super_admin());

        let user = make_auth("user", vec![1, 2]);
        assert!(!user.is_super_admin());
    }

    #[test]
    fn test_has_role() {
        let user = make_auth("user", vec![10, 20]);
        assert!(user.has_role(10));
        assert!(user.has_role(20));
        assert!(!user.has_role(30));
    }

    #[test]
    fn test_empty_role_ids() {
        let user = make_auth("user", vec![]);
        assert!(!user.is_super_admin());
        assert!(!user.has_role(1));
    }

    #[test]
    fn test_super_admin_still_works_with_roles() {
        let admin = make_auth("super_admin", vec![1]);
        assert!(admin.is_super_admin());
        assert!(admin.has_role(1));
    }

    #[test]
    fn test_claims_serializes_role_ids() {
        let claims = Claims {
            sub: 42,
            username: "alice".to_string(),
            display_name: "Alice".to_string(),
            system_role: "user".to_string(),
            role_ids: vec![1, 5, 10],
            permissions: vec![],
            exp: 9999999999,
            iat: 1000000000,
        };

        let json = serde_json::to_string(&claims).expect("serialize claims");
        assert!(json.contains("\"role_ids\":[1,5,10]"), "role_ids should serialize as array");
    }

    #[test]
    fn test_claims_deserialize_role_ids() {
        // 旧 JWT 没有 permissions 字段，应能正常反序列化
        let json = r#"{
            "sub": 42,
            "username": "alice",
            "display_name": "Alice",
            "system_role": "user",
            "role_ids": [1, 5, 10],
            "exp": 9999999999,
            "iat": 1000000000
        }"#;

        let claims: Claims = serde_json::from_str(json).expect("deserialize claims");
        assert_eq!(claims.role_ids, vec![1, 5, 10]);
        assert_eq!(claims.sub, 42);
        assert_eq!(claims.system_role, "user");
        assert!(claims.permissions.is_empty(), "missing permissions should default to empty");
    }

    #[test]
    fn test_claims_empty_role_ids_roundtrip() {
        let claims = Claims {
            sub: 1,
            username: "bob".to_string(),
            display_name: "Bob".to_string(),
            system_role: "super_admin".to_string(),
            role_ids: vec![],
            permissions: vec![],
            exp: 9999999999,
            iat: 1000000000,
        };

        let json = serde_json::to_string(&claims).unwrap();
        let back: Claims = serde_json::from_str(&json).unwrap();
        assert!(back.role_ids.is_empty());
        assert_eq!(back.system_role, "super_admin");
    }
}
