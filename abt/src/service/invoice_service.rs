//! 采购发票服务接口
//!
//! 定义采购发票管理的业务逻辑接口。

use anyhow::Result;
use async_trait::async_trait;
use chrono::NaiveDate;
use rust_decimal::Decimal;

use crate::models::{InvoiceDetail, InvoiceQuery};
use crate::repositories::{Executor, PaginatedResult};

/// 采购发票服务接口
#[async_trait]
pub trait InvoiceService: Send + Sync {
    /// 创建发票，返回 invoice_id
    async fn create(
        &self,
        invoice_no: String,
        supplier_id: i64,
        statement_id: Option<i64>,
        invoice_amount: Decimal,
        invoice_date: NaiveDate,
        remark: Option<String>,
        operator_id: Option<i64>,
        executor: Executor<'_>,
    ) -> Result<i64>;

    /// 分页查询发票列表
    async fn list(&self, query: InvoiceQuery) -> Result<PaginatedResult<InvoiceDetail>>;

    /// 更新发票状态
    async fn update_status(
        &self,
        invoice_id: i64,
        status: i16,
        executor: Executor<'_>,
    ) -> Result<()>;
}
