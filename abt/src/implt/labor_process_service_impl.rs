//! 劳务工序服务实现

use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::models::*;
use crate::repositories::{Executor, LaborProcessRepo};
use crate::service::LaborProcessService;

pub struct LaborProcessServiceImpl {
    pool: PgPool,
}

impl LaborProcessServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl LaborProcessService for LaborProcessServiceImpl {
    // ========================================================================
    // 工序 CRUD
    // ========================================================================

    async fn list_processes(&self, query: LaborProcessQuery) -> Result<(Vec<LaborProcess>, i64)> {
        let page = query.page.max(1);
        let page_size = query.page_size.clamp(1, 100);
        let kw = query.keyword.as_deref();
        let items = LaborProcessRepo::list(&self.pool, page, page_size, kw).await?;
        let total = LaborProcessRepo::count(&self.pool, kw).await?;
        Ok((items, total))
    }

    async fn create_process(&self, req: CreateLaborProcessReq, executor: Executor<'_>) -> Result<i64> {
        LaborProcessRepo::insert(executor, &req.name, req.unit_price, req.remark.as_deref()).await
    }

    async fn update_process(
        &self,
        req: UpdateLaborProcessReq,
        executor: Executor<'_>,
    ) -> Result<Option<PriceChangeImpact>> {
        let old_price = LaborProcessRepo::get_unit_price(&self.pool, req.id).await?;
        let price_changed = old_price.is_some_and(|p| p != req.unit_price);

        LaborProcessRepo::update(executor, req.id, &req.name, req.unit_price, req.remark.as_deref()).await?;

        if price_changed {
            let (affected_bom_count, affected_item_count) =
                LaborProcessRepo::price_change_impact(&self.pool, req.id).await?;
            Ok(Some(PriceChangeImpact {
                affected_bom_count,
                affected_item_count,
            }))
        } else {
            Ok(None)
        }
    }

    async fn delete_process(&self, id: i64, executor: Executor<'_>) -> Result<u64> {
        let referenced = LaborProcessRepo::is_process_referenced(&self.pool, id).await?;
        if referenced {
            anyhow::bail!("工序被引用，无法删除");
        }
        LaborProcessRepo::delete(executor, id).await
    }

    // ========================================================================
    // 工序组 CRUD
    // ========================================================================

    async fn list_groups(&self, query: LaborProcessGroupQuery) -> Result<(Vec<LaborProcessGroupWithMembers>, i64)> {
        let page = query.page.max(1);
        let page_size = query.page_size.clamp(1, 100);
        let kw = query.keyword.as_deref();

        let groups = LaborProcessRepo::list_groups(&self.pool, page, page_size, kw).await?;
        let total = LaborProcessRepo::count_groups(&self.pool, kw).await?;

        let group_ids: Vec<i64> = groups.iter().map(|g| g.id).collect();
        let all_members = LaborProcessRepo::list_group_members_batch(&self.pool, &group_ids).await?;

        let mut members_map: HashMap<i64, Vec<LaborProcessGroupMember>> = HashMap::new();
        for member in all_members {
            members_map.entry(member.group_id).or_default().push(member);
        }

        let result = groups
            .into_iter()
            .map(|group| {
                let members = members_map.remove(&group.id).unwrap_or_default();
                LaborProcessGroupWithMembers { group, members }
            })
            .collect();

        Ok((result, total))
    }

    async fn create_group(&self, req: CreateLaborProcessGroupReq, executor: Executor<'_>) -> Result<i64> {
        if req.members.is_empty() {
            anyhow::bail!("工序组至少需要一个成员");
        }

        let group_id = LaborProcessRepo::insert_group(
            executor,
            &req.name,
            req.remark.as_deref(),
        )
        .await?;

        let members = to_member_tuples(&req.members);
        LaborProcessRepo::set_group_members(executor, group_id, &members).await?;

        Ok(group_id)
    }

    async fn update_group(&self, req: UpdateLaborProcessGroupReq, executor: Executor<'_>) -> Result<()> {
        LaborProcessRepo::update_group(executor, req.id, &req.name, req.remark.as_deref()).await?;

        let members = to_member_tuples(&req.members);
        LaborProcessRepo::set_group_members(executor, req.id, &members).await?;

        Ok(())
    }

    async fn delete_group(&self, id: i64, executor: Executor<'_>) -> Result<u64> {
        let referenced = LaborProcessRepo::is_group_referenced_by_bom(&self.pool, id).await?;
        if referenced {
            anyhow::bail!("工序组被 BOM 引用，无法删除");
        }
        LaborProcessRepo::delete_group(executor, id).await
    }

    // ========================================================================
    // BOM 劳务成本
    // ========================================================================

    async fn set_bom_labor_cost(&self, req: SetBomLaborCostReq, executor: Executor<'_>) -> Result<()> {
        for item in &req.items {
            if item.quantity.is_zero() && item.remark.as_ref().is_none_or(|r| r.is_empty()) {
                anyhow::bail!("工序 {} 的数量为 0，备注不能为空", item.process_id);
            }
        }

        // 锁定工序行防止并发修改价格，然后读取当前单价作为快照
        let process_ids: Vec<i64> = req.items.iter().map(|i| i.process_id).collect();
        let prices = LaborProcessRepo::lock_and_get_unit_prices(executor, &process_ids).await?;

        let cost_items: Vec<(i64, Decimal, Option<Decimal>, Option<&str>)> = req
            .items
            .iter()
            .map(|item| {
                let snapshot = prices.get(&item.process_id).copied();
                (item.process_id, item.quantity, snapshot, item.remark.as_deref())
            })
            .collect();

        LaborProcessRepo::clear_bom_labor_cost(executor, req.bom_id).await?;
        LaborProcessRepo::batch_insert_bom_labor_cost(executor, req.bom_id, &cost_items).await?;
        LaborProcessRepo::set_bom_process_group(executor, req.bom_id, req.process_group_id).await?;

        Ok(())
    }

    async fn get_bom_labor_cost(&self, bom_id: i64) -> Result<Option<(LaborProcessGroupWithMembers, Vec<BomLaborCostItem>)>> {
        let group_with_members = LaborProcessRepo::get_bom_group_with_members(&self.pool, bom_id).await?;
        if group_with_members.is_none() {
            return Ok(None);
        }

        let items = LaborProcessRepo::get_bom_labor_cost(&self.pool, bom_id).await?;

        Ok(Some((group_with_members.unwrap(), items)))
    }
}

fn to_member_tuples(members: &[LaborProcessGroupMemberInput]) -> Vec<(i64, i32)> {
    members.iter().map(|m| (m.process_id, m.sort_order)).collect()
}
