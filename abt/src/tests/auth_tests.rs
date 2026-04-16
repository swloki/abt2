//! Auth system unit tests

#[cfg(test)]
mod auth_tests {
    use std::collections::HashMap;

    use crate::models::AuthContext;

    fn make_auth(system_role: &str, dept_roles: HashMap<String, Vec<i64>>) -> AuthContext {
        AuthContext {
            user_id: 1,
            username: "test".to_string(),
            system_role: system_role.to_string(),
            dept_roles,
            current_department_id: None,
        }
    }

    #[test]
    fn test_is_super_admin() {
        let admin = make_auth("super_admin", HashMap::new());
        assert!(admin.is_super_admin());

        let user = make_auth("user", HashMap::new());
        assert!(!user.is_super_admin());
    }

    #[test]
    fn test_belongs_to_department() {
        let mut dept_roles = HashMap::new();
        dept_roles.insert("1".to_string(), vec![10]);
        dept_roles.insert("2".to_string(), vec![20]);

        let user = make_auth("user", dept_roles);

        assert!(user.belongs_to_department(1));
        assert!(user.belongs_to_department(2));
        assert!(!user.belongs_to_department(3));
    }

    #[test]
    fn test_super_admin_belongs_to_any_department() {
        let admin = make_auth("super_admin", HashMap::new());
        assert!(admin.belongs_to_department(999));
    }

    #[test]
    fn test_get_dept_role_ids() {
        let mut dept_roles = HashMap::new();
        dept_roles.insert("1".to_string(), vec![10, 20]);
        dept_roles.insert("2".to_string(), vec![30]);

        let user = make_auth("user", dept_roles);

        assert_eq!(user.get_dept_role_ids(1), vec![10, 20]);
        assert_eq!(user.get_dept_role_ids(2), vec![30]);
        assert!(user.get_dept_role_ids(3).is_empty());
    }

    #[test]
    fn test_empty_dept_roles() {
        let user = make_auth("user", HashMap::new());
        assert!(!user.is_super_admin());
        assert!(!user.belongs_to_department(1));
        assert!(user.get_dept_role_ids(1).is_empty());
    }
}
