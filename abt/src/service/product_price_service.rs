//! 产品价格服务接口
//!
//! 定义价格管理和历史记录的业务逻辑接口。

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::repositories::{Executor, PaginatedResult};

// ============================================================================
// 数据模型
// ============================================================================

/// 价格日志条目
#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct PriceLogEntry {
    pub log_id: i64,
    pub product_id: i64,
    pub new_price: Decimal,
    pub operator_id: Option<i64>,
    pub remark: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// 价格历史查询参数
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct PriceHistoryQuery {
    pub product_id: i64,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

/// 价格变更记录（包含产品信息）
#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct PriceLogWithProduct {
    pub log_id: i64,
    pub product_id: i64,
    pub product_name: String,
    pub product_code: Option<String>,
    pub new_price: Decimal,
    pub operator_id: Option<i64>,
    pub remark: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// 所有产品价格历史查询参数
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AllPriceHistoryQuery {
    pub product_id: Option<i64>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
    /// 按产品名称模糊搜索
    pub product_name: Option<String>,
    /// 按产品编码模糊搜索
    pub product_code: Option<String>,
}

// ============================================================================
// 服务接口
// ============================================================================

/// 产品价格服务接口
#[async_trait]
pub trait ProductPriceService: Send + Sync {
    /// 更新产品价格（自动记录历史）
    ///
    /// # 参数
    /// - `product_id`: 产品ID
    /// - `new_price`: 新价格
    /// - `operator_id`: 操作人用户ID（登录用户ID）
    /// - `remark`: 备注说明
    /// - `executor`: 数据库执行器（支持事务）
    ///
    /// # 事务说明
    /// 此方法在调用者提供的事务/连接中执行，
    /// 如果需要独立事务，调用者应先开启事务。
    async fn update_price(
        &self,
        product_id: i64,
        new_price: Decimal,
        operator_id: Option<i64>,
        remark: Option<&str>,
        executor: Executor<'_>,
    ) -> Result<()>;

    /// 获取产品价格历史（分页）
    async fn get_price_history(
        &self,
        query: PriceHistoryQuery,
        pool: &PgPool,
    ) -> Result<PaginatedResult<PriceLogEntry>>;

    /// 获取所有产品的价格历史（分页，可选按产品筛选）
    async fn list_all_price_history(
        &self,
        query: AllPriceHistoryQuery,
        pool: &PgPool,
    ) -> Result<PaginatedResult<PriceLogWithProduct>>;
}
