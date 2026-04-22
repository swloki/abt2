//! 劳务工序服务接口

use anyhow::Result;
use async_trait::async_trait;

use crate::models::*;
use crate::repositories::Executor;

/// 劳务工序服务接口
#[async_trait]
pub trait LaborProcessService: Send + Sync {
    /// 搜索工序（按产品）
    async fn list(&self, query: ListLaborProcessQuery) -> Result<(Vec<BomLaborProcess>, i64)>;

    /// 创建工序
    async fn create(&self, req: CreateLaborProcessReq, executor: Executor<'_>) -> Result<i64>;

    /// 更新工序
    async fn update(&self, req: UpdateLaborProcessReq, executor: Executor<'_>) -> Result<()>;

    /// 删除工序
    async fn delete(&self, id: i64, product_code: &str, executor: Executor<'_>) -> Result<u64>;

    /// 从 Excel 导入工序（清除旧的，批量插入新的）
    async fn import_from_excel(&self, product_code: &str, file_path: &str) -> Result<LaborProcessImportResult>;

    /// 导出工序到 Excel 字节流
    async fn export_to_bytes(&self, product_code: &str) -> Result<Vec<u8>>;
}
