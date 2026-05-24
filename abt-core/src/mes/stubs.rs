//! MES 跨模块依赖 Stub
//!
//! 待依赖模块（QMS/WMS/FMS/Master Data/Shared）在 abt-core 中实现后替换。
//! 所有 stub 返回安全默认值，保证 MES 模块可独立编译运行。
//!
//! 关键：MES stubs 使用本地独立类型，不引用 WMS 模块类型（遵循 CLAUDE.md 模块依赖规则）。

use rust_decimal::Decimal;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;

// ---------------------------------------------------------------------------
// Shared — 单据序列号
// ---------------------------------------------------------------------------

/// 单据序列号 stub
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
// Shared — 单据关联
// ---------------------------------------------------------------------------

/// 单据关联 stub
pub struct DocumentLinkStub;

impl DocumentLinkStub {
    pub async fn link(
        _ctx: ServiceContext<'_>,
        _from_type: &str,
        _from_id: i64,
        _to_type: &str,
        _to_id: i64,
    ) -> Result<(), DomainError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Shared — 库存预留
// ---------------------------------------------------------------------------

/// 预留类型（MES 本地定义，不引用 WMS 模块）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReservationType {
    Hard,
    Soft,
}

/// 库存预留 stub
pub struct InventoryReservationStub;

impl InventoryReservationStub {
    /// 预留库存（工单下达时 HARD 预留）
    pub async fn reserve(
        _ctx: ServiceContext<'_>,
        _product_id: i64,
        _warehouse_id: i64,
        _qty: Decimal,
        _reservation_type: ReservationType,
    ) -> Result<(), DomainError> {
        Ok(())
    }

    /// 消耗预留（完工入库时 fulfill）
    pub async fn fulfill(
        _ctx: ServiceContext<'_>,
        _product_id: i64,
        _warehouse_id: i64,
        _qty: Decimal,
        _reservation_type: ReservationType,
    ) -> Result<(), DomainError> {
        Ok(())
    }

    /// 释放预留（取消/关闭时释放）
    pub async fn release(
        _ctx: ServiceContext<'_>,
        _source_type: &str,
        _source_id: i64,
    ) -> Result<(), DomainError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Shared — 审计日志
// ---------------------------------------------------------------------------

/// 审计日志 stub
pub struct AuditLogStub;

impl AuditLogStub {
    pub async fn record(
        _ctx: ServiceContext<'_>,
        _action: &str,
        _entity_type: &str,
        _entity_id: i64,
        _detail: &str,
    ) -> Result<(), DomainError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// QMS — 质量门禁
// ---------------------------------------------------------------------------

/// QMS 质检 stub
pub struct QmsInspectionStub;

impl QmsInspectionStub {
    /// FQC 硬门禁：返回该来源单据是否通过质量检验
    pub async fn is_passed(
        _ctx: ServiceContext<'_>,
        _source_type: &str,
        _source_id: i64,
    ) -> Result<bool, DomainError> {
        Ok(true)
    }

    /// IPQC 创建
    pub async fn create_inspection(
        _ctx: ServiceContext<'_>,
        _inspection_type: &str,
        _source_type: &str,
        _source_id: i64,
    ) -> Result<i64, DomainError> {
        Ok(0)
    }
}

// ---------------------------------------------------------------------------
// WMS — 库存事务（MES 本地类型）
// ---------------------------------------------------------------------------

/// WMS 库存事务类型（MES 本地定义，不引用 WMS 模块）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WmsTransactionType {
    ProductionReceipt,
    MaterialIssue,
    Backflush,
}

/// WMS 库存事务 stub
pub struct WmsInventoryTransactionStub;

impl WmsInventoryTransactionStub {
    pub async fn record(
        _ctx: ServiceContext<'_>,
        _txn_type: WmsTransactionType,
        _product_id: i64,
        _warehouse_id: i64,
        _qty: Decimal,
        _source_type: &str,
        _source_id: i64,
    ) -> Result<(), DomainError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// WMS — 领料单
// ---------------------------------------------------------------------------

/// 领料单状态（MES 本地定义）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WmsRequisitionStatus {
    Draft,
    Confirmed,
}

/// WMS 领料单 stub
pub struct WmsMaterialRequisitionStub;

impl WmsMaterialRequisitionStub {
    /// 工单下达时创建领料单
    pub async fn create_for_work_order(
        _ctx: ServiceContext<'_>,
        _work_order_id: i64,
        _product_id: i64,
        _warehouse_id: i64,
        _qty: Decimal,
    ) -> Result<i64, DomainError> {
        Ok(0)
    }
}

// ---------------------------------------------------------------------------
// FMS — 成本
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

/// 成本分录 stub
pub struct CostEntryStub;

impl CostEntryStub {
    pub async fn record(
        _ctx: ServiceContext<'_>,
        _req: CostEntryReq,
    ) -> Result<(), DomainError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 倒冲
// ---------------------------------------------------------------------------

/// 倒冲 stub
pub struct BackflushStub;

impl BackflushStub {
    /// 执行倒冲。失败时返回 Err，调用方可选择不阻断入库
    pub async fn execute(
        _ctx: ServiceContext<'_>,
        _work_order_id: i64,
        _product_id: i64,
        _completed_qty: Decimal,
    ) -> Result<(), DomainError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Master Data — BOM
// ---------------------------------------------------------------------------

/// BOM 工序步骤
#[derive(Debug, Clone)]
pub struct BomRoutingStep {
    pub step_no: i32,
    pub process_name: String,
    pub work_center_id: Option<i64>,
    pub standard_time: Option<Decimal>,
    pub standard_cost: Option<Decimal>,
    pub unit_price: Option<Decimal>,
    pub allowed_loss_rate: Option<Decimal>,
    pub planned_qty: Decimal,
    pub is_outsourced: bool,
    pub is_inspection_point: bool,
}

/// BOM 组件
#[derive(Debug, Clone)]
pub struct BomComponent {
    pub product_id: i64,
    pub required_qty: Decimal,
}

/// BOM 展开结果
#[derive(Debug, Clone)]
pub struct BomSnapshot {
    pub routing_steps: Vec<BomRoutingStep>,
    pub components: Vec<BomComponent>,
}

/// BOM 服务 stub
pub struct BomServiceStub;

impl BomServiceStub {
    /// 获取产品的 BOM 展开（工序 + 组件）
    pub async fn get_bom_snapshot(
        _ctx: ServiceContext<'_>,
        _product_id: i64,
    ) -> Result<BomSnapshot, DomainError> {
        Ok(BomSnapshot {
            routing_steps: vec![],
            components: vec![],
        })
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
    pub warehouse_id: i64,
}

/// 产品信息 stub
pub struct ProductServiceStub;

impl ProductServiceStub {
    pub async fn get(
        _ctx: ServiceContext<'_>,
        product_id: i64,
    ) -> Result<ProductInfo, DomainError> {
        Ok(ProductInfo {
            id: product_id,
            code: String::new(),
            name: String::new(),
            unit: String::new(),
            warehouse_id: 0,
        })
    }
}
