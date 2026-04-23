//! 工艺路线服务实现

use anyhow::Result;
use async_trait::async_trait;
use sqlx::PgPool;

use crate::models::*;
use crate::repositories::{Executor, RoutingRepo};
use crate::service::RoutingService;

pub struct RoutingServiceImpl {
    pool: PgPool,
}

impl RoutingServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl RoutingService for RoutingServiceImpl {
    // ========================================================================
    // 查询
    // ========================================================================

    async fn list(&self, query: ListRoutingQuery) -> Result<(Vec<Routing>, i64)> {
        let page = query.page.max(1);
        let page_size = query.page_size.clamp(1, 100);
        let kw = query.keyword.as_deref();
        let items = RoutingRepo::find_all(&self.pool, kw, page, page_size).await?;
        let total = RoutingRepo::count_all(&self.pool, kw).await?;
        Ok((items, total))
    }

    async fn get_detail(&self, id: i64) -> Result<(Routing, Vec<RoutingStep>)> {
        let routing = RoutingRepo::find_by_id(&self.pool, id)
            .await?
            .ok_or_else(|| common::error::ServiceError::NotFound {
                resource: "工艺路线".to_string(),
                id: id.to_string(),
            })?;

        let steps = RoutingRepo::find_steps_by_routing_id(&self.pool, id).await?;
        Ok((routing, steps))
    }

    // ========================================================================
    // 写入
    // ========================================================================

    async fn create(&self, req: CreateRoutingReq, executor: Executor<'_>) -> Result<i64> {
        // 创建路线
        let routing_id = RoutingRepo::insert_routing(
            executor,
            &req.name,
            req.description.as_deref(),
        )
        .await?;

        // 批量插入工序
        if !req.steps.is_empty() {
            RoutingRepo::batch_insert_steps(executor, routing_id, &req.steps).await?;
        }

        Ok(routing_id)
    }

    async fn update(&self, req: UpdateRoutingReq, executor: Executor<'_>) -> Result<()> {
        // 检查路线是否存在（使用 executor 确保在事务内查询）
        RoutingRepo::find_by_id_tx(executor, req.id)
            .await?
            .ok_or_else(|| common::error::ServiceError::NotFound {
                resource: "工艺路线".to_string(),
                id: req.id.to_string(),
            })?;

        // 更新路线基本信息
        RoutingRepo::update_routing(
            executor,
            req.id,
            &req.name,
            req.description.as_deref(),
        )
        .await?;

        // 删除旧的工序再重新插入
        RoutingRepo::delete_steps_by_routing_id(executor, req.id).await?;
        if !req.steps.is_empty() {
            RoutingRepo::batch_insert_steps(executor, req.id, &req.steps).await?;
        }

        Ok(())
    }

    async fn delete(&self, id: i64, executor: Executor<'_>) -> Result<u64> {
        // 检查路线是否存在（使用 executor 确保在事务内查询）
        RoutingRepo::find_by_id_tx(executor, id)
            .await?
            .ok_or_else(|| common::error::ServiceError::NotFound {
                resource: "工艺路线".to_string(),
                id: id.to_string(),
            })?;

        // 检查是否被 BOM 引用
        if RoutingRepo::exists_bom_routing_by_routing_id(&self.pool, id).await? {
            return Err(common::error::ServiceError::BusinessValidation {
                message: "该工艺路线已被产品绑定，无法删除".to_string(),
            }
            .into());
        }

        // 先删除工序，再删除路线
        RoutingRepo::delete_steps_by_routing_id(executor, id).await?;
        RoutingRepo::delete_routing(executor, id).await
    }

    // ========================================================================
    // 匹配查询
    // ========================================================================

    async fn find_matching_routing(&self, process_codes: &[String]) -> Result<Option<i64>> {
        RoutingRepo::find_matching_routing(&self.pool, process_codes).await
    }

    async fn find_matching_routing_tx(&self, process_codes: &[String], executor: Executor<'_>) -> Result<Option<i64>> {
        RoutingRepo::find_matching_routing_tx(executor, process_codes).await
    }

    async fn get_bom_routing_tx(
        &self,
        product_code: &str,
        executor: Executor<'_>,
    ) -> Result<Option<(i64, String, Vec<RoutingStep>)>> {
        let binding = RoutingRepo::find_bom_routing_tx(executor, product_code).await?;
        match binding {
            Some(b) => {
                let routing = RoutingRepo::find_by_id_tx(executor, b.routing_id).await?;
                match routing {
                    Some(r) => {
                        let steps =
                            RoutingRepo::find_steps_by_routing_id_tx(executor, b.routing_id).await?;
                        Ok(Some((r.id, r.name, steps)))
                    }
                    None => {
                        // 绑定的路线已被删除，清理孤儿绑定记录
                        RoutingRepo::delete_bom_routing(executor, product_code).await?;
                        Ok(None)
                    }
                }
            }
            None => Ok(None),
        }
    }

    async fn get_detail_tx(&self, id: i64, executor: Executor<'_>) -> Result<(Routing, Vec<RoutingStep>)> {
        let routing = RoutingRepo::find_by_id_tx(executor, id)
            .await?
            .ok_or_else(|| common::error::ServiceError::NotFound {
                resource: "工艺路线".to_string(),
                id: id.to_string(),
            })?;

        let steps = RoutingRepo::find_steps_by_routing_id_tx(executor, id).await?;
        Ok((routing, steps))
    }

    // ========================================================================
    // BOM 路线绑定
    // ========================================================================

    async fn set_bom_routing(
        &self,
        product_code: &str,
        routing_id: i64,
        executor: Executor<'_>,
    ) -> Result<()> {
        // 检查路线是否存在（必须用 executor 而非 pool，因为调用方可能在同一事务内刚创建了该路线）
        RoutingRepo::find_by_id_tx(executor, routing_id)
            .await?
            .ok_or_else(|| common::error::ServiceError::NotFound {
                resource: "工艺路线".to_string(),
                id: routing_id.to_string(),
            })?;

        RoutingRepo::set_bom_routing(executor, product_code, routing_id).await
    }

    async fn get_bom_routing(
        &self,
        product_code: &str,
    ) -> Result<Option<(i64, String, Vec<RoutingStep>)>> {
        let binding = RoutingRepo::find_bom_routing(&self.pool, product_code).await?;
        match binding {
            Some(b) => {
                let routing = RoutingRepo::find_by_id(&self.pool, b.routing_id).await?;
                match routing {
                    Some(r) => {
                        let steps =
                            RoutingRepo::find_steps_by_routing_id(&self.pool, b.routing_id).await?;
                        Ok(Some((r.id, r.name, steps)))
                    }
                    None => {
                        // 绑定的路线已被删除，清理孤儿绑定记录
                        RoutingRepo::delete_bom_routing(
                            &mut *self.pool.acquire().await?,
                            product_code,
                        )
                        .await?;
                        Ok(None)
                    }
                }
            }
            None => Ok(None),
        }
    }

    async fn list_boms_by_routing(
        &self,
        routing_id: i64,
        page: u32,
        page_size: u32,
    ) -> Result<(Vec<crate::repositories::BomBrief>, i64)> {
        RoutingRepo::find_boms_by_routing_id(&self.pool, routing_id, page, page_size).await
    }
}
