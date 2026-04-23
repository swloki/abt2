//! 工艺路线服务接口

use anyhow::Result;
use async_trait::async_trait;

use crate::models::*;
use crate::repositories::Executor;

/// 工艺路线服务接口
#[async_trait]
pub trait RoutingService: Send + Sync {
    /// 搜索工艺路线（分页）
    async fn list(&self, query: ListRoutingQuery) -> Result<(Vec<Routing>, i64)>;

    /// 获取路线详情（含工序列表）
    async fn get_detail(&self, id: i64) -> Result<(Routing, Vec<RoutingStep>)>;

    /// 创建工艺路线
    async fn create(&self, req: CreateRoutingReq, executor: Executor<'_>) -> Result<i64>;

    /// 更新工艺路线
    async fn update(&self, req: UpdateRoutingReq, executor: Executor<'_>) -> Result<()>;

    /// 删除工艺路线
    async fn delete(&self, id: i64, executor: Executor<'_>) -> Result<u64>;

    /// 根据工序编码集合查找匹配的路线 ID
    async fn find_matching_routing(&self, process_codes: &[String]) -> Result<Option<i64>>;

    /// 根据工序编码集合查找匹配的路线 ID（在事务内执行）
    async fn find_matching_routing_tx(&self, process_codes: &[String], executor: Executor<'_>) -> Result<Option<i64>>;

    /// 获取 BOM 路线绑定信息（在事务内执行）
    async fn get_bom_routing_tx(
        &self,
        product_code: &str,
        executor: Executor<'_>,
    ) -> Result<Option<(i64, String, Vec<RoutingStep>)>>;

    /// 获取路线详情（在事务内执行）
    async fn get_detail_tx(&self, id: i64, executor: Executor<'_>) -> Result<(Routing, Vec<RoutingStep>)>;

    /// 设置 BOM 路线绑定
    async fn set_bom_routing(
        &self,
        product_code: &str,
        routing_id: i64,
        executor: Executor<'_>,
    ) -> Result<()>;

    /// 获取 BOM 路线绑定信息（返回 routing_id, routing_name, steps）
    async fn get_bom_routing(
        &self,
        product_code: &str,
    ) -> Result<Option<(i64, String, Vec<RoutingStep>)>>;

    /// 查询引用指定路线的 BOM 列表（分页）
    async fn list_boms_by_routing(
        &self,
        routing_id: i64,
        page: u32,
        page_size: u32,
    ) -> Result<(Vec<crate::repositories::BomBrief>, i64)>;
}
