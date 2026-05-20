//! 采购对账单服务接口
//!
//! 定义采购对账单管理的业务逻辑接口。

use anyhow::Result;
use async_trait::async_trait;

use crate::models::{StatementDetail, StatementQuery, StatementWithItems};
use crate::repositories::{Executor, PaginatedResult};

/// 采购对账单服务接口
#[async_trait]
pub trait StatementService: Send + Sync {
    /// 自动生成对账单（基于指定供应商和期间的采购订单），返回 statement_id
    async fn generate(
        &self,
        supplier_id: i64,
        period_start: chrono::NaiveDate,
        period_end: chrono::NaiveDate,
        operator_id: Option<i64>,
        executor: Executor<'_>,
    ) -> Result<i64>;

    /// 根据 ID 获取对账单详情（含行项目）
    async fn get_by_id(&self, statement_id: i64) -> Result<Option<StatementWithItems>>;

    /// 分页查询对账单列表
    async fn list(&self, query: StatementQuery) -> Result<PaginatedResult<StatementDetail>>;

    /// 更新对账单状态
    async fn update_status(
        &self,
        statement_id: i64,
        status: i16,
        executor: Executor<'_>,
    ) -> Result<()>;
}
