use async_trait::async_trait;
use chrono::{DateTime, Utc};

use super::model::*;
use crate::shared::types::{PgExecutor,PageParams, PaginatedResult, ServiceContext, Result};

/// BOM 展开采购需求项（对应 Odoo Procurement NamedTuple）
#[derive(Debug, Clone)]
pub struct ProcurementRequirement {
    /// 原材料 product_id
    pub product_id: i64,
    /// 净需求量（已含 loss_rate 损耗系数）
    pub required_qty: rust_decimal::Decimal,
    /// BOM 层级深度（0=成品直接子件，1=二级子件...）
    pub bom_level: u8,
}

#[async_trait]
pub trait BomQueryService: Send + Sync {
    async fn get(&self, ctx: &ServiceContext, db: PgExecutor<'_>, bom_id: i64) -> Result<Bom>;
    async fn list(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        query: BomQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<Bom>>;
    async fn get_leaf_nodes(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        bom_id: i64,
    ) -> Result<Vec<BomNode>>;
    /// 在指定 BOM 树中按 product_id 定位节点，取其**直接子级**（产出品的直接物料清单）。
    /// 用于工序级领料/齐套分析：产出品是成品 BOM 树的中间节点时取其下一级构成。
    async fn get_direct_children_by_product(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        bom_id: i64,
        product_id: i64,
    ) -> Result<Vec<BomNode>>;
    async fn get_snapshots(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        bom_id: i64,
        version: Option<i32>,
        limit: Option<i32>,
    ) -> Result<Vec<BomSnapshot>>;
    async fn exists_name(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        name: &str,
        caller_id: Option<i64>,
    ) -> Result<bool>;

    /// 查找产品关联的已发布 BOM，返回 bom_id
    async fn find_published_bom_by_product_code(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_code: &str,
    ) -> Result<Option<i64>>;

    /// 递归展开 BOM，返回所有需采购的原材料及其净需求量
    ///
    /// 参考 Odoo `_run_manufacture` 递归模式：
    /// 1. 查 product_code 的已发布 BOM
    /// 2. 按 acquire_channel 分流：Purchased→加入结果，SelfProduced→递归
    /// 3. 深度限制 10 层 + visited set 防循环
    async fn explode_for_procurement(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_code: &str,
        quantity: rust_decimal::Decimal,
    ) -> Result<Vec<ProcurementRequirement>>;

    /// 按 snapshot_id 加载快照
    async fn get_snapshot_by_id(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        snapshot_id: i64,
    ) -> Result<Option<BomSnapshot>>;

    /// 给定一组 product_code，返回这些产品已发布 BOM 树中【非叶子节点】的 product_id 集合（去重）。
    ///
    /// 用于工艺路线工序产出品候选集：产出品必须是关联产品 BOM 的成品/半成品（非原材料）。
    /// 非叶子 = 节点存在子节点（`EXISTS c.parent_id = bn.node_id`）。Issue #212。
    async fn list_non_leaf_product_ids_by_product_codes(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        product_codes: &[String],
    ) -> Result<Vec<i64>>;
}

#[async_trait]
pub trait BomCommandService: Send + Sync {
    async fn create(&self, ctx: &ServiceContext, db: PgExecutor<'_>, req: CreateBomReq) -> Result<i64>;
    async fn update(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        req: UpdateBomReq,
        expected_version: i32,
    ) -> Result<()>;
    async fn delete(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;
    async fn publish(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<i64>;
    async fn unpublish(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;
    async fn save_as(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        source_id: i64,
        new_name: String,
    ) -> Result<i64>;
    async fn substitute_product(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: SubstituteReq,
    ) -> Result<SubstitutionResult>;
    async fn validate_cycle(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        bom_id: i64,
    ) -> Result<()>;
}

#[async_trait]
pub trait BomNodeService: Send + Sync {
    async fn add_node(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        bom_id: i64,
        node: NewBomNode,
    ) -> Result<i64>;
    async fn update_node(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        bom_id: i64,
        node_id: i64,
        req: UpdateBomNodeReq,
        expected_version: i32,
    ) -> Result<()>;
    async fn delete_node(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        bom_id: i64,
        node_id: i64,
    ) -> Result<i64>;
    async fn move_node(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        bom_id: i64,
        node_id: i64,
        new_parent_id: i64,
        before_sibling_id: Option<i64>,
    ) -> Result<()>;
}

#[async_trait]
pub trait BomCostService: Send + Sync {
    async fn get_cost_report(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        bom_id: i64,
        as_of_date: Option<DateTime<Utc>>,
    ) -> Result<BomCostReport>;

    async fn get_labor_cost_report(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        bom_id: i64,
    ) -> Result<BomLaborCostReport>;
}

#[async_trait]
pub trait BomCategoryService: Send + Sync {
    async fn create(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        req: CreateBomCategoryReq,
    ) -> Result<i64>;
    async fn update(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        id: i64,
        req: UpdateBomCategoryReq,
    ) -> Result<()>;
    async fn delete(&self, ctx: &ServiceContext, db: PgExecutor<'_>, id: i64) -> Result<()>;
    async fn list(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        query: BomCategoryQuery,
        page: PageParams,
    ) -> Result<PaginatedResult<BomCategory>>;
}
