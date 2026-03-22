//! BOM 人工工序服务接口

use anyhow::Result;
use async_trait::async_trait;

use crate::models::{BomLaborProcess, CreateLaborProcessRequest, ImportResult, ListLaborProcessRequest, UpdateLaborProcessRequest};
use crate::repositories::Executor;

/// 人工工序服务接口
#[async_trait]
pub trait LaborProcessService: Send + Sync {
    /// 创建人工工序
    async fn create(&self, req: CreateLaborProcessRequest, executor: Executor<'_>) -> Result<i64>;

    /// 更新人工工序
    async fn update(&self, req: UpdateLaborProcessRequest, executor: Executor<'_>) -> Result<()>;

    /// 删除人工工序
    async fn delete(&self, id: i64, product_code: &str, executor: Executor<'_>) -> Result<u64>;

    /// 查询人工工序列表
    async fn list(&self, req: ListLaborProcessRequest) -> Result<(Vec<BomLaborProcess>, i64)>;

    /// 批量导入人工工序（覆盖模式）
    /// 从 Excel 文件导入，先删除该产品编码的所有现有工序，再批量插入新工序
    /// Excel 格式：产品编码, 工序名称, 单价, 数量, 排序, 备注
    async fn import(&self, file_path: &str, executor: Executor<'_>) -> Result<ImportResult>;
}
