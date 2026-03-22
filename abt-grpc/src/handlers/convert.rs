//! Type conversions between abt models and proto types

use crate::generated::abt::v1::{ProductMeta, ProductResponse};

impl From<abt::Product> for ProductResponse {
    fn from(product: abt::Product) -> Self {
        ProductResponse {
            product_id: product.product_id,
            pdt_name: product.pdt_name,
            meta: Some(product.meta.into()),
        }
    }
}

impl From<abt::ProductMeta> for ProductMeta {
    fn from(meta: abt::ProductMeta) -> Self {
        ProductMeta {
            category: meta.category,
            subcategory: meta.subcategory,
            product_code: meta.product_code,
            specification: meta.specification,
            unit: meta.unit,
            acquire_channel: meta.acquire_channel,
            loss_rate: meta.loss_rate,
            old_code: meta.old_code,
        }
    }
}

impl From<ProductMeta> for abt::ProductMeta {
    fn from(meta: ProductMeta) -> Self {
        abt::ProductMeta {
            category: meta.category,
            subcategory: meta.subcategory,
            product_code: meta.product_code,
            specification: meta.specification,
            unit: meta.unit,
            acquire_channel: meta.acquire_channel,
            loss_rate: meta.loss_rate,
            old_code: meta.old_code,
        }
    }
}

// ========== Term conversions ==========

use crate::generated::abt::v1::{TermMeta, TermResponse, TermTreeResponse};

impl From<abt::Term> for TermResponse {
    fn from(term: abt::Term) -> Self {
        TermResponse {
            term_id: term.term_id,
            term_name: term.term_name,
            term_parent: term.term_parent,
            taxonomy: term.taxonomy,
            term_meta: Some(TermMeta { count: term.term_meta.count }),
        }
    }
}

impl From<abt::TermTree> for TermTreeResponse {
    fn from(tree: abt::TermTree) -> Self {
        TermTreeResponse {
            term_id: tree.term_id,
            term_name: tree.term_name,
            term_parent: tree.term_parent,
            taxonomy: tree.taxonomy,
            term_meta: Some(TermMeta { count: tree.term_meta.count }),
            children: tree.children.into_iter().map(|c| c.into()).collect(),
        }
    }
}

// ========== BOM conversions ==========

use crate::generated::abt::v1::{BomDetailProto, BomNodeProto, BomResponse, BomNodeResponse};

impl From<abt::Bom> for BomResponse {
    fn from(bom: abt::Bom) -> Self {
        BomResponse {
            bom_id: bom.bom_id,
            name: bom.bom_name,
            created_by: bom.bom_detail.created_by.clone().unwrap_or_default(),
            created_at: bom.create_at.timestamp(),
            updated_at: bom.update_at.map(|t| t.timestamp()).unwrap_or(0),
            bom_detail: Some(bom.bom_detail.into()),
        }
    }
}

impl From<abt::BomNode> for BomNodeResponse {
    fn from(node: abt::BomNode) -> Self {
        BomNodeResponse {
            node_id: node.id,
            bom_id: 0, // bom_id not stored in BomNode, passed separately
            parent_id: node.parent_id,
            product_id: node.product_id,
            product_name: node.product_code.clone().unwrap_or_default(),
            quantity: node.quantity,
            sort_order: node.order,
            product_code: node.product_code.unwrap_or_default(),
            loss_rate: node.loss_rate,
            unit: node.unit.unwrap_or_default(),
            remark: node.remark.unwrap_or_default(),
            position: node.position.unwrap_or_default(),
            work_center: node.work_center.unwrap_or_default(),
            properties: node.properties.unwrap_or_default(),
        }
    }
}

impl From<abt::BomDetail> for BomDetailProto {
    fn from(detail: abt::BomDetail) -> Self {
        BomDetailProto {
            nodes: detail.nodes.into_iter().map(|n| n.into()).collect(),
            created_by: detail.created_by.unwrap_or_default(),
        }
    }
}

impl From<abt::BomNode> for BomNodeProto {
    fn from(node: abt::BomNode) -> Self {
        BomNodeProto {
            node_id: node.id,
            product_id: node.product_id,
            product_code: node.product_code.unwrap_or_default(),
            quantity: node.quantity,
            parent_id: node.parent_id,
            loss_rate: node.loss_rate,
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

impl From<abt::Warehouse> for WarehouseResponse {
    fn from(w: abt::Warehouse) -> Self {
        WarehouseResponse {
            warehouse_id: w.warehouse_id,
            warehouse_code: w.warehouse_code,
            warehouse_name: w.warehouse_name,
            address: String::new(), // field not in abt::Warehouse
            contact: String::new(), // field not in abt::Warehouse
            is_active: matches!(w.status, abt::WarehouseStatus::Active),
            created_at: w.created_at.timestamp(),
            updated_at: w.updated_at.map(|t| t.timestamp()).unwrap_or(0),
        }
    }
}

// ========== Location conversions ==========

use crate::generated::abt::v1::{LocationResponse, LocationWithWarehouseResponse};

impl From<abt::Location> for LocationResponse {
    fn from(l: abt::Location) -> Self {
        LocationResponse {
            location_id: l.location_id,
            warehouse_id: l.warehouse_id,
            location_code: l.location_code,
            location_name: l.location_name.unwrap_or_default(),
            location_type: String::new(), // field not in abt::Location
            is_active: l.deleted_at.is_none(),
            created_at: l.created_at.timestamp(),
            updated_at: l.created_at.timestamp(), // no updated_at, use created_at
        }
    }
}

impl From<abt::LocationWithWarehouse> for LocationWithWarehouseResponse {
    fn from(l: abt::LocationWithWarehouse) -> Self {
        LocationWithWarehouseResponse {
            location_id: l.location_id,
            warehouse_id: l.warehouse_id,
            warehouse_name: l.warehouse_name,
            location_code: l.location_code,
            location_name: l.location_name.unwrap_or_default(),
            location_type: String::new(), // field not in abt::LocationWithWarehouse
            is_active: true, // assume active
        }
    }
}

// ========== Inventory Stats conversions ==========

use crate::generated::abt::v1::{
    LocationInventoryStatsResponse, WarehouseInventoryStatsResponse,
};
use rust_decimal::prelude::ToPrimitive;

impl From<abt::WarehouseInventoryStats> for WarehouseInventoryStatsResponse {
    fn from(s: abt::WarehouseInventoryStats) -> Self {
        WarehouseInventoryStatsResponse {
            warehouse_id: s.warehouse_id,
            total_locations: s.location_count,
            total_products: s.product_count,
            total_quantity: s.total_quantity.to_f64().unwrap_or(0.0),
            total_value: 0.0,
        }
    }
}

impl From<abt::LocationInventoryStats> for LocationInventoryStatsResponse {
    fn from(s: abt::LocationInventoryStats) -> Self {
        LocationInventoryStatsResponse {
            location_id: s.location_id,
            total_products: s.product_count,
            total_quantity: s.total_quantity.to_f64().unwrap_or(0.0),
            total_value: 0.0,
        }
    }
}
