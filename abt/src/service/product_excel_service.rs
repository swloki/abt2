//! 产品 Excel 服务接口
//!
//! 定义产品 Excel 导入导出的业务逻辑接口。

use anyhow::Result;
use async_trait::async_trait;
use sqlx::PgPool;
use std::path::Path;

/// Excel 导入结果
#[derive(Debug, Clone, Default)]
pub struct ImportResult {
    /// 成功导入数量
    pub success_count: usize,
    /// 失败数量
    pub failed_count: usize,
    /// 错误信息
    pub errors: Vec<String>,
}

/// Excel 处理进度
#[derive(Debug, Clone, Default)]
pub struct ExcelProgress {
    /// 当前处理数量
    pub current: usize,
    /// 总数量
    pub total: usize,
}

/// 产品 Excel 服务接口
#[async_trait]
pub trait ProductExcelService: Send + Sync {
    /// 从 Excel 导入库存、价格和安全库存数据
    ///
    /// # 参数
    /// - `pool`: 数据库连接池
    /// - `path`: Excel 文件路径
    /// - `operator_id`: 操作人用户ID（用于记录操作人）
    ///
    /// # Excel 格式要求
    /// - 新编码: 产品新编码
    /// - 旧编码: 产品旧编码（可选）
    /// - 物料名称: 产品名称
    /// - 仓库名称: 仓库名称
    /// - 库位名称: 库位名称（可选）
    /// - 库存数量: 盘点数量（直接设置为该值）
    /// - 价格: 单价（可选，填写则更新）
    /// - 安全库存: 安全库存数量（可选，填写则更新）
    async fn import_quantity_from_excel(
        &self,
        pool: &PgPool,
        path: &Path,
        operator_id: Option<i64>,
    ) -> Result<ImportResult>;

    /// 导出产品到 Excel（详细格式，每行一个库位）
    async fn export_products_to_excel(&self, pool: &PgPool, path: &Path) -> Result<()>;

    /// 导出产品到 Excel（返回字节数据，用于流式下载）
    async fn export_products_to_bytes(&self, pool: &PgPool) -> Result<Vec<u8>>;

    /// 获取处理进度
    fn get_progress(&self) -> ExcelProgress;
}
