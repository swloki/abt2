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
    }
}

#[test]
fn resource_code_returns_lowercase() {
    assert_eq!(Resource::Product.code(), "product");
    assert_eq!(Resource::Warehouse.code(), "warehouse");
    assert_eq!(Resource::LaborProcess.code(), "labor_process");
    assert_eq!(Resource::Excel.code(), "excel");
}

#[test]
fn action_code_returns_lowercase() {
    assert_eq!(Action::Read.code(), "read");
    assert_eq!(Action::Write.code(), "write");
    assert_eq!(Action::Delete.code(), "delete");
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
            defined_resources.contains(code),
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
            defined_actions.contains(code),
            "Action variant {:?} (code={:?}) not found in resources.rs",
            variant,
            code
        );
    }
}

#[test]
fn business_system_resource_codes_cover_all_enums() {
    use abt::models::resources::{is_business_resource, is_system_resource};

    for variant in ALL_RESOURCES {
        let code = variant.code();
        let is_business = is_business_resource(code);
        let is_system = is_system_resource(code);
        assert!(
            is_business || is_system,
            "Resource {:?} (code={:?}) is neither business nor system resource",
            variant,
            code
        );
        assert!(
            !(is_business && is_system),
            "Resource {:?} (code={:?}) is both business and system resource",
            variant,
            code
        );
    }
}

// ============================================================================
// Permission check function tests
// ============================================================================

#[test]
fn super_admin_has_full_system_access() {
    let auth = make_auth("super_admin", vec![]);
    assert!(
        check_permission_for_resource(&auth, "user", "write").is_ok(),
        "super_admin should have user:write"
    );
    assert!(
        check_permission_for_resource(&auth, "role", "delete").is_ok(),
        "super_admin should have role:delete"
    );
    assert!(
        check_permission_for_resource(&auth, "department", "write").is_ok(),
        "super_admin should have department:write"
    );
}

#[test]
fn super_admin_has_full_business_access() {
    let auth = make_auth("super_admin", vec![]);
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
fn normal_user_system_read_access() {
    let auth = make_auth("user", vec![1]);
    assert!(
        check_permission_for_resource(&auth, "user", "read").is_ok(),
        "normal user should have user:read"
    );
    assert!(
        check_permission_for_resource(&auth, "department", "read").is_ok(),
        "normal user should have department:read"
    );
    assert!(
        check_permission_for_resource(&auth, "permission", "read").is_ok(),
        "normal user should have permission:read"
    );
    assert!(
        check_permission_for_resource(&auth, "role", "read").is_ok(),
        "normal user should have role:read"
    );
}

#[test]
fn normal_user_system_write_denied() {
    let auth = make_auth("user", vec![1]);
    assert!(
        check_permission_for_resource(&auth, "user", "write").is_err(),
        "normal user should NOT have user:write"
    );
    assert!(
        check_permission_for_resource(&auth, "role", "delete").is_err(),
        "normal user should NOT have role:delete"
    );
    assert!(
        check_permission_for_resource(&auth, "excel", "write").is_err(),
        "normal user should NOT have excel:write"
    );
}

#[test]
fn empty_role_ids_denied_business_resources() {
    let auth = make_auth("user", vec![]);
    assert!(
        check_permission_for_resource(&auth, "product", "read").is_err(),
        "user with empty role_ids should be denied product:read"
    );
    assert!(
        check_permission_for_resource(&auth, "bom", "write").is_err(),
        "user with empty role_ids should be denied bom:write"
    );
}

#[test]
fn check_permission_routes_system_vs_business() {
    // Verify system resources go through check_system_permission
    let admin = make_auth("super_admin", vec![]);
    let user = make_auth("user", vec![]);

    // admin can do everything on system resources
    for res in &["user", "role", "permission", "department", "excel"] {
        for act in &["read", "write", "delete"] {
            assert!(
                check_permission_for_resource(&admin, res, act).is_ok(),
                "admin should have {}:{}",
                res,
                act
            );
        }
    }

    // user can only read certain system resources
    assert!(check_permission_for_resource(&user, "user", "read").is_ok());
    assert!(check_permission_for_resource(&user, "user", "write").is_err());
}
