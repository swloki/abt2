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
