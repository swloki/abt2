//! OM 跨模块依赖 Stub
//!
//! WMS 已集成真实 TransferService，不再使用 stub。
//! 待依赖模块（MES/QMS/Master Data）在 abt-core 中完善后替换剩余 stub。
//! 所有 stub 返回安全默认值，保证 OM 模块可独立编译运行。

use rust_decimal::Decimal;

use crate::shared::types::context::ServiceContext;
use crate::shared::types::error::DomainError;

// ---------------------------------------------------------------------------
// MES — 工单
// ---------------------------------------------------------------------------

/// 工单信息
#[derive(Debug, Clone)]
pub struct WorkOrderInfo {
    pub product_id: i64,
    pub warehouse_id: i64,
}

/// MES 工单 stub — 创建内部工单（转自制流程）
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

    /// 转自制时创建 MES 工单 — 返回虚拟工单 ID
    pub async fn create_from_outsourcing(
        _ctx: ServiceContext<'_>,
        _outsourcing_id: i64,
        _product_id: i64,
        _planned_qty: Decimal,
    ) -> Result<i64, DomainError> {
        // TODO: 待 MES.WorkOrderService 完善后替换
        Ok(0)
    }
}

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
// QMS — 检验结果
// ---------------------------------------------------------------------------

/// 检验结果 stub — 创建 IQC 检验记录
pub struct InspectionResultStub;

impl InspectionResultStub {
    /// 创建 IQC 检验结果 — 返回虚拟检验结果 ID
    pub async fn create_iqc(
        _ctx: ServiceContext<'_>,
        _source_type: &str,
        _source_id: i64,
        _sample_qty: Decimal,
        _passed_qty: Decimal,
    ) -> Result<i64, DomainError> {
        // TODO: 待 QMS.InspectionResultService 实现后替换
        Ok(0)
    }
}

// ---------------------------------------------------------------------------
// Master Data — 供应商
// ---------------------------------------------------------------------------

/// 供应商基本信息
#[derive(Debug, Clone)]
pub struct SupplierInfo {
    pub id: i64,
    pub code: String,
    pub name: String,
}

/// 供应商信息 stub — 默认返回空
pub struct SupplierStub;

impl SupplierStub {
    pub async fn get(
        _ctx: ServiceContext<'_>,
        supplier_id: i64,
    ) -> Result<SupplierInfo, DomainError> {
        Ok(SupplierInfo {
            id: supplier_id,
            code: String::new(),
            name: String::new(),
        })
    }
}
