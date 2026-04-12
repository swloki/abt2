use crate::generated::abt::v1::{Action, Resource};

/// Trait for converting proto-generated permission enums to lowercase runtime strings.
///
/// The proto enums use SCREAMING_SNAKE_CASE (e.g., `WAREHOUSE`, `READ`), but the
/// runtime permission system uses lowercase strings (e.g., `"warehouse"`, `"read"`)
/// for JWT claims and `check_permission` matching. This trait bridges the two.
pub trait PermissionCode {
    fn code(&self) -> &'static str;
}

impl PermissionCode for Resource {
    fn code(&self) -> &'static str {
        match self {
            Self::Product => "product",
            Self::Term => "term",
            Self::Bom => "bom",
            Self::Warehouse => "warehouse",
            Self::Location => "location",
            Self::Inventory => "inventory",
            Self::Price => "price",
            Self::LaborProcess => "labor_process",
            Self::User => "user",
            Self::Role => "role",
            Self::Permission => "permission",
            Self::Department => "department",
            Self::Excel => "excel",
        }
    }
}

impl PermissionCode for Action {
    fn code(&self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Delete => "delete",
        }
    }
}

#[cfg(test)]
mod tests;
