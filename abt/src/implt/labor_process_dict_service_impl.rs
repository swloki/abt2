//! 工序字典服务实现

use anyhow::Result;
use async_trait::async_trait;
use sqlx::PgPool;

use crate::models::*;
use crate::repositories::{Executor, LaborProcessDictRepo};
use crate::service::LaborProcessDictService;

pub struct LaborProcessDictServiceImpl {
    pool: PgPool,
}

impl LaborProcessDictServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl LaborProcessDictService for LaborProcessDictServiceImpl {
    // ========================================================================
    // 查询
    // ========================================================================

    async fn list(&self, query: ListLaborProcessDictQuery) -> Result<(Vec<LaborProcessDict>, i64)> {
        let page = query.page.max(1);
        let page_size = query.page_size.clamp(1, 100);
        let kw = query.keyword.as_deref();
        let items = LaborProcessDictRepo::find_all(&self.pool, kw, page, page_size).await?;
        let total = LaborProcessDictRepo::count_all(&self.pool, kw).await?;
        Ok((items, total))
    }

    // ========================================================================
    // 写入
    // ========================================================================

    async fn create(&self, req: CreateLaborProcessDictReq, executor: Executor<'_>) -> Result<i64> {
        // 通过 nextval 生成编码，天然并发安全
        let cur: i64 = sqlx::query_scalar(
            "SELECT nextval('labor_process_dict_code_seq')"
        )
        .fetch_one(&self.pool)
        .await?;
        // 如果序列落后于表中已有最大 code，推进序列
        let max_code: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(code::bigint), 0) FROM labor_process_dict"
        )
        .fetch_one(&self.pool)
        .await?;
        let next_val = if cur <= max_code {
            sqlx::query(
                "SELECT setval('labor_process_dict_code_seq', $1)"
            )
            .bind(max_code)
            .execute(&self.pool)
            .await?;
            max_code + 1
        } else {
            cur
        };

        let code = format!("{:05}", next_val);

        LaborProcessDictRepo::insert(
            executor,
            &code,
            &req.name,
            req.description.as_deref(),
            req.sort_order,
        )
        .await
    }

    async fn update(&self, req: UpdateLaborProcessDictReq, executor: Executor<'_>) -> Result<()> {
        // 检查记录是否存在
        LaborProcessDictRepo::find_by_id(&self.pool, req.id)
            .await?
            .ok_or_else(|| common::error::ServiceError::NotFound {
                resource: "工序字典".to_string(),
                id: req.id.to_string(),
            })?;

        LaborProcessDictRepo::update(
            executor,
            req.id,
            &req.name,
            req.description.as_deref(),
            req.sort_order,
        )
        .await
    }

    async fn delete(&self, id: i64, executor: Executor<'_>) -> Result<u64> {
        // 检查记录是否存在并获取编码
        let existing = LaborProcessDictRepo::find_by_id(&self.pool, id)
            .await?
            .ok_or_else(|| common::error::ServiceError::NotFound {
                resource: "工序字典".to_string(),
                id: id.to_string(),
            })?;

        // 检查是否被 routing_step 引用
        if LaborProcessDictRepo::exists_by_process_code(&self.pool, &existing.code).await? {
            return Err(common::error::ServiceError::BusinessValidation {
                message: format!("工序编码 '{}' 已被工艺路线引用，无法删除", existing.code),
            }
            .into());
        }

        LaborProcessDictRepo::delete(executor, id).await
    }
}
