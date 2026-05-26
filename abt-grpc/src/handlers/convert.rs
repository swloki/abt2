//! Type conversions between abt-core models and proto types

use crate::generated::abt::v1::{ProductMeta, ProductResponse};

// ========== Product conversions (abt-core) ==========

impl From<abt_core::master_data::product::Product> for ProductResponse {
    fn from(product: abt_core::master_data::product::Product) -> Self {
        ProductResponse {
            product_id: product.product_id,
            pdt_name: product.pdt_name,
            meta: Some(product.meta.into()),
            product_code: product.product_code,
            unit: product.unit,
            term_id: None, // abt-core 使用 product_categories 表，不再在 Product 上存储 term_id
        }
    }
}

impl From<abt_core::master_data::product::ProductMeta> for ProductMeta {
    fn from(meta: abt_core::master_data::product::ProductMeta) -> Self {
        ProductMeta {
            specification: meta.specification,
            acquire_channel: meta.acquire_channel,
            old_code: meta.old_code,
        }
    }
}

impl From<ProductMeta> for abt_core::master_data::product::ProductMeta {
    fn from(meta: ProductMeta) -> Self {
        abt_core::master_data::product::ProductMeta {
            specification: meta.specification,
            acquire_channel: meta.acquire_channel,
            old_code: meta.old_code,
        }
    }
}

// ========== BOM conversions (abt-core) ==========

use crate::generated::abt::v1::{BomDetailProto, BomNodeProto, BomResponse, BomNodeResponse};
use rust_decimal::prelude::ToPrimitive;

impl From<abt_core::master_data::bom::model::Bom> for BomResponse {
    fn from(bom: abt_core::master_data::bom::model::Bom) -> Self {
        use crate::generated::abt::v1::BomStatus as ProtoBomStatus;
        let status = match bom.status {
            abt_core::master_data::bom::model::BomStatus::Draft => ProtoBomStatus::Draft,
            abt_core::master_data::bom::model::BomStatus::Published => ProtoBomStatus::Published,
        };
        BomResponse {
            bom_id: bom.bom_id,
            name: bom.bom_name,
            created_by: bom.created_by.unwrap_or(0),
            created_at: bom.create_at.timestamp(),
            updated_at: bom.update_at.map(|t| t.timestamp()).unwrap_or(0),
            bom_detail: Some(bom.bom_detail.into()),
            bom_category_id: bom.bom_category_id,
            status: status.into(),
            published_at: bom.published_at.map(|t| t.timestamp()).unwrap_or(0),
        }
    }
}

impl From<abt_core::master_data::bom::model::BomNode> for BomNodeResponse {
    fn from(node: abt_core::master_data::bom::model::BomNode) -> Self {
        BomNodeResponse {
            node_id: node.id,
            bom_id: node.bom_id,
            parent_id: node.parent_id,
            product_id: node.product_id,
            product_name: node.product_code.clone().unwrap_or_default(),
            quantity: node.quantity.to_f64().unwrap_or(0.0),
            sort_order: node.order,
            product_code: node.product_code.unwrap_or_default(),
            loss_rate: node.loss_rate.to_f64().unwrap_or(0.0),
            unit: node.unit.unwrap_or_default(),
            remark: node.remark.unwrap_or_default(),
            position: node.position.unwrap_or_default(),
            work_center: node.work_center.unwrap_or_default(),
            properties: node.properties.unwrap_or_default(),
        }
    }
}

impl From<abt_core::master_data::bom::model::BomDetail> for BomDetailProto {
    fn from(detail: abt_core::master_data::bom::model::BomDetail) -> Self {
        BomDetailProto {
            nodes: detail.nodes.into_iter().map(|n| n.into()).collect(),
        }
    }
}

impl From<abt_core::master_data::bom::model::BomNode> for BomNodeProto {
    fn from(node: abt_core::master_data::bom::model::BomNode) -> Self {
        BomNodeProto {
            node_id: node.id,
            product_id: node.product_id,
            product_code: node.product_code.unwrap_or_default(),
            quantity: node.quantity.to_f64().unwrap_or(0.0),
            parent_id: node.parent_id,
            loss_rate: node.loss_rate.to_f64().unwrap_or(0.0),
            sort_order: node.order,
            unit: node.unit.unwrap_or_default(),
            remark: node.remark.unwrap_or_default(),
            position: node.position.unwrap_or_default(),
            work_center: node.work_center.unwrap_or_default(),
            properties: node.properties.unwrap_or_default(),
        }
    }
}

// ========== Warehouse conversions ==========

use crate::generated::abt::v1::WarehouseResponse;

impl From<abt_core::wms::warehouse::Warehouse> for WarehouseResponse {
    fn from(w: abt_core::wms::warehouse::Warehouse) -> Self {
        WarehouseResponse {
            warehouse_id: w.id,
            warehouse_code: w.code,
            warehouse_name: w.name,
            address: w.address.unwrap_or_default(),
            contact: String::new(),
            is_active: matches!(w.status, abt_core::wms::WarehouseStatus::Active),
            created_at: w.created_at.timestamp(),
            updated_at: w.updated_at.timestamp(),
        }
    }
}

// ========== Location conversions (abt-core Bin) ==========

use crate::generated::abt::v1::{LocationResponse, LocationWithWarehouseResponse};

impl From<abt_core::wms::warehouse::BinWithWarehouse> for LocationResponse {
    fn from(bw: abt_core::wms::warehouse::BinWithWarehouse) -> Self {
        let is_active = !matches!(bw.bin.status, abt_core::wms::BinStatus::Disabled);
        LocationResponse {
            location_id: bw.bin.id,
            warehouse_id: bw.warehouse_id,
            location_code: bw.bin.code,
            is_active,
            location_name: bw.bin.name,
            location_type: String::new(),
            created_at: bw.bin.created_at.timestamp(),
            updated_at: bw.bin.updated_at.timestamp(),
        }
    }
}

impl From<abt_core::wms::warehouse::BinWithWarehouse> for LocationWithWarehouseResponse {
    fn from(bw: abt_core::wms::warehouse::BinWithWarehouse) -> Self {
        let is_active = !matches!(bw.bin.status, abt_core::wms::BinStatus::Disabled);
        LocationWithWarehouseResponse {
            location_id: bw.bin.id,
            warehouse_id: bw.warehouse_id,
            warehouse_name: bw.warehouse_name,
            location_code: bw.bin.code,
            is_active,
            location_name: bw.bin.name,
            location_type: String::new(),
        }
    }
}

impl From<abt_core::wms::warehouse::Bin> for LocationResponse {
    fn from(b: abt_core::wms::warehouse::Bin) -> Self {
        let is_active = !matches!(b.status, abt_core::wms::BinStatus::Disabled);
        LocationResponse {
            location_id: b.id,
            warehouse_id: 0,
            location_code: b.code,
            is_active,
            location_name: b.name,
            location_type: String::new(),
            created_at: b.created_at.timestamp(),
            updated_at: b.updated_at.timestamp(),
        }
    }
}

// ========== Inventory Stats conversions ==========

use crate::generated::abt::v1::{
    LocationInventoryStatsResponse, WarehouseInventoryStatsResponse,
};

impl From<abt_core::wms::warehouse::WarehouseInventoryStats> for WarehouseInventoryStatsResponse {
    fn from(s: abt_core::wms::warehouse::WarehouseInventoryStats) -> Self {
        WarehouseInventoryStatsResponse {
            warehouse_id: s.warehouse_id,
            total_locations: s.bin_count,
            total_products: s.product_count,
            total_quantity: s.total_quantity.to_f64().unwrap_or(0.0),
            total_value: 0.0,
        }
    }
}

impl From<abt_core::wms::warehouse::BinInventoryStats> for LocationInventoryStatsResponse {
    fn from(s: abt_core::wms::warehouse::BinInventoryStats) -> Self {
        LocationInventoryStatsResponse {
            location_id: s.bin_id,
            total_products: s.product_count,
            total_quantity: s.total_quantity.to_f64().unwrap_or(0.0),
            total_value: 0.0,
        }
    }
}

// ========== Identity conversions (abt-core) ==========

use crate::generated::abt::v1::{
    DepartmentResponse as ProtoDepartmentResponse, RoleInfo as ProtoRoleInfo,
    RoleListItem as ProtoRoleListItem, RoleResponse as ProtoRoleResponse,
    UserResponse as ProtoUserResponse,
};

impl From<abt_core::shared::identity::RoleInfo> for ProtoRoleInfo {
    fn from(role: abt_core::shared::identity::RoleInfo) -> Self {
        ProtoRoleInfo {
            role_id: role.role_id,
            role_name: role.role_name,
            role_code: role.role_code,
        }
    }
}

impl From<abt_core::shared::identity::UserWithRoles> for ProtoUserResponse {
    fn from(u: abt_core::shared::identity::UserWithRoles) -> Self {
        ProtoUserResponse {
            user_id: u.user.user_id,
            username: u.user.username,
            display_name: u.user.display_name.unwrap_or_default(),
            is_active: u.user.is_active,
            is_super_admin: u.user.is_super_admin,
            roles: u.roles.into_iter().map(|r| r.into()).collect(),
            created_at: u.user.created_at.timestamp(),
        }
    }
}

impl From<abt_core::shared::identity::Role> for ProtoRoleListItem {
    fn from(role: abt_core::shared::identity::Role) -> Self {
        ProtoRoleListItem {
            role_id: role.role_id,
            role_name: role.role_name,
            role_code: role.role_code,
            is_system_role: role.is_system_role,
            description: role.description.unwrap_or_default(),
        }
    }
}

impl From<abt_core::shared::identity::RoleWithPermissions> for ProtoRoleResponse {
    fn from(r: abt_core::shared::identity::RoleWithPermissions) -> Self {
        ProtoRoleResponse {
            role_id: r.role.role_id,
            role_name: r.role.role_name,
            role_code: r.role.role_code,
            is_system_role: r.role.is_system_role,
            description: r.role.description.unwrap_or_default(),
            permission_codes: r.permissions,
        }
    }
}

impl From<abt_core::shared::identity::Department> for ProtoDepartmentResponse {
    fn from(d: abt_core::shared::identity::Department) -> Self {
        ProtoDepartmentResponse {
            department_id: d.department_id,
            department_name: d.department_name,
            department_code: d.department_code,
            description: d.description.unwrap_or_default(),
            is_active: d.is_active,
            is_default: d.is_default,
        }
    }
}

// ========== BomCategory conversions ==========

use crate::generated::abt::v1::BomCategoryResponse as ProtoBomCategoryResponse;

impl From<abt_core::master_data::bom::model::BomCategory> for ProtoBomCategoryResponse {
    fn from(c: abt_core::master_data::bom::model::BomCategory) -> Self {
        ProtoBomCategoryResponse {
            bom_category_id: c.bom_category_id,
            bom_category_name: c.bom_category_name,
            created_at: c.created_at.timestamp(),
        }
    }
}

// ========== BOM Cost Report conversions (abt-core) ==========

use crate::generated::abt::v1::{
    BomCostReportResponse, LaborCostItem as ProtoLaborCostItem,
    MaterialCostItem as ProtoMaterialCostItem,
};

impl From<abt_core::master_data::bom::model::BomCostReport> for BomCostReportResponse {
    fn from(report: abt_core::master_data::bom::model::BomCostReport) -> Self {
        BomCostReportResponse {
            bom_id: report.bom_id,
            bom_name: report.bom_name,
            product_code: report.product_code,
            material_costs: report.material_costs.into_iter().map(|m| m.into()).collect(),
            labor_costs: report.labor_costs.into_iter().map(|l| l.into()).collect(),
            warnings: report.warnings,
        }
    }
}

impl From<abt_core::master_data::bom::model::MaterialCostItem> for ProtoMaterialCostItem {
    fn from(item: abt_core::master_data::bom::model::MaterialCostItem) -> Self {
        ProtoMaterialCostItem {
            node_id: item.node_id,
            product_id: item.product_id,
            product_name: item.product_name,
            product_code: item.product_code,
            quantity: item.quantity.to_f64().unwrap_or(0.0),
            unit_price: item.unit_price.map(|p| p.to_string()),
        }
    }
}

impl From<abt_core::master_data::bom::model::LaborCostItem> for ProtoLaborCostItem {
    fn from(item: abt_core::master_data::bom::model::LaborCostItem) -> Self {
        ProtoLaborCostItem {
            id: item.id,
            name: item.name,
            unit_price: item.unit_price.to_string(),
            quantity: item.quantity.to_string(),
            sort_order: item.sort_order,
            remark: item.remark,
        }
    }
}

