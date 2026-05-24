//! WMS 跨模块依赖 Stub
//!
//! 待依赖模块（QMS/MES/Master Data/FMS/Shared）在 abt-core 中实现后替换。
//! 所有 stub 返回安全默认值，保证 WMS 模块可独立编译运行。

use rust_decimal::Decimal;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;

// ---------------------------------------------------------------------------
// QMS — 质量门禁
// ---------------------------------------------------------------------------

/// QMS 质量门禁 stub — 默认检验通过
pub struct QualityGateStub;

impl QualityGateStub {
    /// IQC 硬门禁：返回该来源单据是否通过质量检验
    pub async fn is_passed(
        _ctx: ServiceContext<'_>,
        _source_type: &str,
        _source_id: i64,
    ) -> Result<bool, DomainError> {
        Ok(true)
    }
}

// ---------------------------------------------------------------------------
// MES — 工单 / BOM
// ---------------------------------------------------------------------------

/// BOM 组件
#[derive(Debug, Clone)]
pub struct BomComponent {
    pub product_id: i64,
    pub required_qty: Decimal,
}

/// 工单基本信息
#[derive(Debug, Clone)]
pub struct WorkOrderInfo {
    pub product_id: i64,
    pub warehouse_id: i64,
}

/// MES 工单 stub — 默认返回空 BOM
pub struct WorkOrderStub;

impl WorkOrderStub {
    pub async fn get_info(
        _ctx: ServiceContext<'_>,
        _work_order_id: i64,
    ) -> Result<WorkOrderInfo, DomainError> {
        Ok(WorkOrderInfo {
            product_id: 0,
            warehouse_id: 0,
        })
    }

    pub async fn get_bom_components(
        _ctx: ServiceContext<'_>,
        _work_order_id: i64,
    ) -> Result<Vec<BomComponent>, DomainError> {
        Ok(vec![])
    }
}

// ---------------------------------------------------------------------------
// Master Data — 产品
// ---------------------------------------------------------------------------

/// 产品基本信息
#[derive(Debug, Clone)]
pub struct ProductInfo {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub unit: String,
}

/// 产品信息 stub — 默认返回空
pub struct ProductStub;

impl ProductStub {
    pub async fn get(
        _ctx: ServiceContext<'_>,
        product_id: i64,
    ) -> Result<ProductInfo, DomainError> {
        Ok(ProductInfo {
            id: product_id,
            code: String::new(),
            name: String::new(),
            unit: String::new(),
        })
    }
}

// ---------------------------------------------------------------------------
// Shared — 单据序列号
// ---------------------------------------------------------------------------

/// 单据序列号 stub — 简单前缀 + 时间戳
pub struct DocumentSequenceStub;

impl DocumentSequenceStub {
    pub async fn next_number(
        _ctx: ServiceContext<'_>,
        prefix: &str,
    ) -> Result<String, DomainError> {
        let now = chrono::Utc::now();
        let suffix = now.format("%Y%m%d%H%M%S%3f");
        Ok(format!("{prefix}{suffix}"))
    }
}

// ---------------------------------------------------------------------------
// Shared — 成本分录
// ---------------------------------------------------------------------------

/// 成本分录请求
#[derive(Debug, Clone)]
pub struct CostEntryReq {
    pub cost_type: String,
    pub debit_account: String,
    pub credit_account: String,
    pub amount: Decimal,
    pub source_type: String,
    pub source_id: i64,
}

/// 成本分录 stub — 记录但不实际执行
pub struct CostEntryStub;

impl CostEntryStub {
    /// 独立事务模式：主事务提交后开新事务
    pub async fn record(
        _ctx: ServiceContext<'_>,
        _req: CostEntryReq,
    ) -> Result<(), DomainError> {
        // TODO: 待 FMS 模块实现后替换为 CostEntryService.record()
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Shared — 库存预留
// ---------------------------------------------------------------------------

/// 预留类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReservationType {
    Hard,
    Soft,
}

/// 库存预留 stub — 占位
pub struct InventoryReservationStub;

impl InventoryReservationStub {
    /// 消耗预留（领料 HARD / 发货 SOFT）
    pub async fn fulfill(
        _ctx: ServiceContext<'_>,
        _product_id: i64,
        _warehouse_id: i64,
        _qty: Decimal,
        _reservation_type: ReservationType,
    ) -> Result<(), DomainError> {
        // TODO: 待共享层 InventoryReservationService 实现后替换
        Ok(())
    }

    /// 释放预留（取消/完成时释放安全库存）
    pub async fn release(
        _ctx: ServiceContext<'_>,
        _source_type: &str,
        _source_id: i64,
    ) -> Result<(), DomainError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Shared — 单据关联
// ---------------------------------------------------------------------------

/// 单据关联 stub
pub struct DocumentLinkStub;

impl DocumentLinkStub {
    /// 创建单据关联（异步 Outbox 模式）
    pub async fn link(
        _ctx: ServiceContext<'_>,
        _from_type: &str,
        _from_id: i64,
        _to_type: &str,
        _to_id: i64,
    ) -> Result<(), DomainError> {
        // TODO: 待共享层 DocumentLinkService 实现后替换
        Ok(())
    }
}
