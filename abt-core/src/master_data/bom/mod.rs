pub mod model;
pub mod repo;
pub mod service;
pub mod implt;

pub use model::*;
pub use service::{BomCategoryService, BomCommandService, BomCostService, BomNodeService, BomQueryService};

use sqlx::PgPool;

pub fn new_bom_query_service(pool: PgPool) -> impl BomQueryService {
    implt::BomQueryServiceImpl::new(pool)
}

pub fn new_bom_command_service(pool: PgPool) -> impl BomCommandService {
    implt::BomCommandServiceImpl::new(pool)
}

pub fn new_bom_node_service(pool: PgPool) -> impl BomNodeService {
    implt::BomNodeServiceImpl::new(pool)
}

pub fn new_bom_cost_service(pool: PgPool) -> impl BomCostService {
    implt::BomCostServiceImpl::new(pool)
}

pub fn new_bom_category_service(pool: PgPool) -> impl BomCategoryService {
    implt::BomCategoryServiceImpl::new(pool)
}
