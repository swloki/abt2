use async_trait::async_trait;

use crate::qms::enums::*;
use crate::qms::inspection_result;
use crate::shared::types::{DomainError, PgExecutor, ServiceContext, Result};

/// 质量关卡服务 trait — 检查某个来源是否通过质量检验
#[async_trait]
pub trait QualityGateService: Send + Sync {
    async fn check_gate(
        &self,
        ctx: &ServiceContext, db: PgExecutor<'_>,
        source_type: i16,
        source_id: i64,
        inspection_type: i16,
    ) -> Result<QualityGateStatus>;
}

/// QualityGateServiceImpl — 质量关卡服务实现
///
/// 检查某个来源是否需要检验且已通过。
/// 语义: 在调用方事务内执行 (InCallerTx)，失败回滚主事务。
pub struct QualityGateServiceImpl;

impl Default for QualityGateServiceImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl QualityGateServiceImpl {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl QualityGateService for QualityGateServiceImpl {
    async fn check_gate(
        &self,
        _ctx: &ServiceContext, db: PgExecutor<'_>,
        source_type: i16,
        source_id: i64,
        inspection_type: i16,
    ) -> Result<QualityGateStatus> {
        let result = inspection_result::repo::find_by_source(
            db,
            source_type,
            source_id,
            inspection_type,
        )
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

        match result {
            Some(r) => {
                // Pending → 映射为 Failed（设计文档：待检 = 不放行）
                if r.status == InspectionStatus::Pending {
                    return Ok(QualityGateStatus::Failed);
                }
                match r.result {
                    InspectionResultType::Pass | InspectionResultType::Conditional => {
                        Ok(QualityGateStatus::Passed)
                    }
                    InspectionResultType::Fail => Ok(QualityGateStatus::Failed),
                }
            }
            None => Ok(QualityGateStatus::NotRequired),
        }
    }
}
