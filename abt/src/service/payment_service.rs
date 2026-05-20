//! 采购付款服务接口
//!
//! 定义采购付款管理的业务逻辑接口。

use anyhow::Result;
use async_trait::async_trait;
use rust_decimal::Decimal;

use crate::models::{PaymentDetail, PaymentQuery, PurchasePayment};
use crate::repositories::{Executor, PaginatedResult};

/// 采购付款服务接口
#[async_trait]
pub trait PaymentService: Send + Sync {
    /// 创建付款，返回 payment_id
    async fn create(
        &self,
        supplier_id: i64,
        invoice_id: Option<i64>,
        payment_amount: Decimal,
        payment_method: Option<String>,
        remark: Option<String>,
        operator_id: Option<i64>,
        executor: Executor<'_>,
    ) -> Result<i64>;

    /// 根据 ID 获取付款详情
    async fn get_by_id(&self, payment_id: i64) -> Result<Option<PurchasePayment>>;

    /// 分页查询付款列表
    async fn list(&self, query: PaymentQuery) -> Result<PaginatedResult<PaymentDetail>>;

    /// 更新付款状态
    async fn update_status(
        &self,
        payment_id: i64,
        status: i16,
        executor: Executor<'_>,
    ) -> Result<()>;
}
