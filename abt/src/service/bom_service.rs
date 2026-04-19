//! BOM 服务接口
//!
//! 定义 BOM 管理的业务逻辑接口。

use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;

use crate::models::{Bom, BomNode, BomQuery};
use crate::repositories::Executor;

/// BOM 服务接口
#[async_trait]
pub trait BomService: Send + Sync {
    /// 创建新 BOM
    async fn create(&self, name: &str, created_by: &str, bom_category_id: Option<i64>, executor: Executor<'_>) -> Result<i64>;

    /// 更新 BOM
    async fn update(&self, bom: Bom, executor: Executor<'_>) -> Result<()>;

    /// 更新 BOM 元数据（名称和分类，不涉及 bom_detail）
    async fn update_metadata(&self, bom_id: i64, name: &str, bom_category_id: Option<i64>, executor: Executor<'_>) -> Result<()>;

    /// 删除 BOM
    async fn delete(&self, bom_id: i64, executor: Executor<'_>) -> Result<()>;

    /// 根据 ID 查找 BOM
    async fn find(&self, bom_id: i64, executor: Executor<'_>) -> Result<Option<Bom>>;

    /// 查询 BOM 列表
    async fn query(&self, query: BomQuery) -> Result<(Vec<Bom>, i64)>;

    /// 添加 BOM 节点
    async fn add_node(&self, bom_id: i64, node: BomNode, executor: Executor<'_>) -> Result<i64>;

    /// 更新 BOM 节点
    async fn update_node(&self, bom_id: i64, node: BomNode, executor: Executor<'_>) -> Result<()>;

    /// 删除 BOM 节点（包含子节点）
    async fn delete_node(&self, bom_id: i64, node_id: i64, executor: Executor<'_>) -> Result<i64>;

    /// 交换节点位置
    async fn swap_node_position(
        &self,
        bom_id: i64,
        node_id1: i64,
        node_id2: i64,
        executor: Executor<'_>,
    ) -> Result<()>;

    /// 检查 BOM 名称是否存在
    async fn exists_name(&self, name: &str) -> Result<bool>;

    /// 导出 BOM 到 Excel 文件
    async fn export_to_excel(&self, bom_id: i64, path: &Path) -> Result<()>;

    /// 导出 BOM 到 Excel（返回字节数据和 BOM 名称，用于流式下载）
    async fn export_to_bytes(&self, bom_id: i64) -> Result<(Vec<u8>, String)>;

    /// 获取 BOM 叶子节点（用于出库）
    /// 只返回没有子节点的节点
    async fn get_leaf_nodes(&self, bom_id: i64, executor: Executor<'_>) -> Result<Vec<BomNode>>;

    /// 复制 BOM（另存为新 BOM）
    async fn save_as(
        &self,
        source_bom_id: i64,
        new_name: &str,
        created_by: &str,
        executor: Executor<'_>,
    ) -> Result<i64>;

    /// 获取 BOM 的产品编码（BOM 第一个节点的产品编码）
    async fn get_product_code(&self, bom_id: i64, executor: Executor<'_>) -> Result<Option<String>>;
}
