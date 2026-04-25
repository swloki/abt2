use super::*;
use crate::generated::abt::v1::{Action, Resource};

const ALL_RESOURCES: [Resource; 13] = [
    Resource::Product,
    Resource::Term,
    Resource::Bom,
    Resource::Warehouse,
    Resource::Location,
    Resource::Inventory,
    Resource::Price,
    Resource::LaborProcess,
    Resource::User,
    Resource::Role,
    Resource::Permission,
    Resource::Department,
    Resource::Excel,
];

const ALL_ACTIONS: [Action; 3] = [Action::Read, Action::Write, Action::Delete];

fn make_auth(system_role: &str, role_ids: Vec<i64>) -> abt::AuthContext {
    abt::AuthContext {
        user_id: 1,
        username: "testuser".to_string(),
        system_role: system_role.to_string(),
        role_ids,
        role_codes: vec![],
    }
}

#[test]
fn resource_code_returns_uppercase() {
    assert_eq!(Resource::Product.code(), "PRODUCT");
    assert_eq!(Resource::Warehouse.code(), "WAREHOUSE");
    assert_eq!(Resource::LaborProcess.code(), "LABOR_PROCESS");
    assert_eq!(Resource::Excel.code(), "EXCEL");
}

#[test]
fn action_code_returns_uppercase() {
    assert_eq!(Action::Read.code(), "READ");
    assert_eq!(Action::Write.code(), "WRITE");
    assert_eq!(Action::Delete.code(), "DELETE");
}

#[test]
fn all_resource_codes_match_resources_rs() {
    use abt::models::resources::collect_all_resources;

    let defined_resources: std::collections::HashSet<&str> = collect_all_resources()
        .iter()
        .map(|r| r.resource_code)
        .collect();

    for variant in ALL_RESOURCES {
        let code = variant.code();
        assert!(
            defined_resources.contains(code.as_str()),
            "Resource variant {:?} (code={:?}) not found in resources.rs",
            variant,
            code
        );
    }
}

#[test]
fn all_action_codes_exist_in_resources_rs() {
    use abt::models::resources::collect_all_resources;

    let defined_actions: std::collections::HashSet<&str> = collect_all_resources()
        .iter()
        .map(|r| r.action)
        .collect();

    for variant in ALL_ACTIONS {
        let code = variant.code();
        assert!(
            defined_actions.contains(code.as_str()),
            "Action variant {:?} (code={:?}) not found in resources.rs",
            variant,
            code
        );
    }
}

// ============================================================================
// Permission check function tests
// ============================================================================

#[test]
fn super_admin_has_full_access() {
    let auth = make_auth("super_admin", vec![]);
    // super_admin bypasses all permission checks
    assert!(
        check_permission_for_resource(&auth, "user", "write").is_ok(),
        "super_admin should have user:write"
    );
    assert!(
        check_permission_for_resource(&auth, "role", "delete").is_ok(),
        "super_admin should have role:delete"
    );
    assert!(
        check_permission_for_resource(&auth, "product", "write").is_ok(),
        "super_admin should have product:write"
    );
    assert!(
        check_permission_for_resource(&auth, "bom", "delete").is_ok(),
        "super_admin should have bom:delete"
    );
}

#[test]
fn normal_user_denied_without_role_permission() {
    // All resources (system and business) require role-based permission via cache.
    let auth = make_auth("user", vec![1]);
    assert!(
        check_permission_for_resource(&auth, "user", "read").is_err(),
        "normal user without role permission should NOT have user:read"
    );
    assert!(
        check_permission_for_resource(&auth, "department", "read").is_err(),
        "normal user without role permission should NOT have department:read"
    );
    assert!(
        check_permission_for_resource(&auth, "user", "write").is_err(),
        "normal user without role permission should NOT have user:write"
    );
    assert!(
        check_permission_for_resource(&auth, "product", "read").is_err(),
        "normal user without role permission should NOT have product:read"
    );
}

#[test]
fn empty_role_ids_denied() {
    let auth = make_auth("user", vec![]);
    assert!(
        check_permission_for_resource(&auth, "product", "read").is_err(),
        "user with empty role_ids should be denied product:read"
    );
    assert!(
        check_permission_for_resource(&auth, "user", "read").is_err(),
        "user with empty role_ids should be denied user:read"
    );
}

#[test]
fn super_admin_role_code_grants_full_access() {
    let mut auth = make_auth("user", vec![1]);
    auth.role_codes = vec!["super_admin".to_string()];
    assert!(
        check_permission_for_resource(&auth, "product", "write").is_ok(),
        "user with super_admin role_code should have product:write"
    );
    assert!(
        check_permission_for_resource(&auth, "user", "delete").is_ok(),
        "user with super_admin role_code should have user:delete"
    );
}
