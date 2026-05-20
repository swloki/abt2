use std::sync::Arc;
use anyhow::Result;
use async_trait::async_trait;
use sqlx::PgPool;
use common::error::ServiceError;
use crate::models::{ReconciliationItem, ReconciliationQuery, ReconciliationStatement};
use crate::repositories::{DocumentSequenceRepo, Executor, PaginatedResult, PaginationParams, ReconciliationRepo};
use crate::service::ReconciliationService;

pub struct ReconciliationServiceImpl {
    pool: Arc<PgPool>,
}

impl ReconciliationServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

const STATUS_DRAFT: i16 = 1;
const STATUS_CONFIRMED: i16 = 2;
const STATUS_APPROVED: i16 = 3;

#[async_trait]
impl ReconciliationService for ReconciliationServiceImpl {
    async fn create(&self, operator_id: Option<i64>, mut stmt: ReconciliationStatement, executor: Executor<'_>) -> Result<i64> {
        // 1. 检查是否已存在该客户该月的对账单
        let existing = ReconciliationRepo::find_by_period(
            &self.pool, &stmt.customer_name, stmt.period_year, stmt.period_month,
        ).await?;

        if existing.is_some() {
            return Err(ServiceError::Conflict {
                resource: "ReconciliationStatement".to_string(),
                message: format!("客户 {} {}年{}月 对账单已存在", stmt.customer_name, stmt.period_year, stmt.period_month),
            }.into());
        }

        // 2. 查询该月发货明细和退货明细
        let mut shipping_items = ReconciliationRepo::query_shipping_items(
            &self.pool, &stmt.customer_name, stmt.period_year, stmt.period_month,
        ).await?;

        let mut return_items = ReconciliationRepo::query_return_items(
            &self.pool, &stmt.customer_name, stmt.period_year, stmt.period_month,
        ).await?;

        // 3. 生成编号
        let statement_no = DocumentSequenceRepo::next_number(&mut *executor, "RC").await?;
        stmt.statement_no = statement_no;
        stmt.operator_id = operator_id;
        stmt.status = STATUS_DRAFT;

        // 4. 插入主表
        let statement_id = ReconciliationRepo::insert(&mut *executor, &stmt).await?;

        // 5. 插入行项目
        for item in &mut shipping_items {
            item.statement_id = statement_id;
        }
        for item in &mut return_items {
            item.statement_id = statement_id;
        }
        ReconciliationRepo::insert_items(&mut *executor, &shipping_items).await?;
        ReconciliationRepo::insert_items(&mut *executor, &return_items).await?;

        // 6. 重算汇总金额
        ReconciliationRepo::update_totals(&mut *executor, statement_id).await?;

        Ok(statement_id)
    }

    async fn add_adjustments(&self, statement_id: i64, items: Vec<ReconciliationItem>, executor: Executor<'_>) -> Result<()> {
        let existing = ReconciliationRepo::find_by_id(&self.pool, statement_id).await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "ReconciliationStatement".to_string(),
                id: statement_id.to_string(),
            })?;

        if existing.status != STATUS_DRAFT {
            return Err(ServiceError::BusinessValidation {
                message: "只有草稿状态的对账单可以添加调整项".to_string(),
            }.into());
        }

        // 删除旧调整项，插入新调整项
        ReconciliationRepo::delete_by_statement(&mut *executor, statement_id).await?;

        let mut filled_items = Vec::new();
        for mut item in items {
            item.statement_id = statement_id;
            item.source_type = "adjustment".to_string();
            filled_items.push(item);
        }
        ReconciliationRepo::insert_items(&mut *executor, &filled_items).await?;

        // 重算汇总
        ReconciliationRepo::update_totals(&mut *executor, statement_id).await?;

        Ok(())
    }

    async fn update(&self, _operator_id: Option<i64>, stmt: ReconciliationStatement, executor: Executor<'_>) -> Result<()> {
        let existing = ReconciliationRepo::find_by_id(&self.pool, stmt.statement_id).await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "ReconciliationStatement".to_string(),
                id: stmt.statement_id.to_string(),
            })?;

        if existing.status != STATUS_DRAFT {
            return Err(ServiceError::BusinessValidation {
                message: "只有草稿状态的对账单可以编辑".to_string(),
            }.into());
        }

        ReconciliationRepo::update(&mut *executor, &stmt).await
    }

    async fn delete(&self, statement_id: i64, executor: Executor<'_>) -> Result<()> {
        let existing = ReconciliationRepo::find_by_id(&self.pool, statement_id).await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "ReconciliationStatement".to_string(),
                id: statement_id.to_string(),
            })?;

        if existing.status != STATUS_DRAFT {
            return Err(ServiceError::BusinessValidation {
                message: "只有草稿状态的对账单可以删除".to_string(),
            }.into());
        }

        ReconciliationRepo::soft_delete(executor, statement_id).await
    }

    async fn get_by_id(&self, statement_id: i64) -> Result<Option<ReconciliationStatement>> {
        let mut stmt = ReconciliationRepo::find_by_id(&self.pool, statement_id).await?;
        if let Some(ref mut s) = stmt {
            s.items = ReconciliationRepo::find_by_statement_id(&self.pool, statement_id).await?;
        }
        Ok(stmt)
    }

    async fn list(&self, query: ReconciliationQuery) -> Result<PaginatedResult<ReconciliationStatement>> {
        let page = query.page.unwrap_or(1).max(1) as u32;
        let page_size = query.page_size.unwrap_or(12).clamp(1, 100) as u32;
        let pagination = PaginationParams::new(page, page_size);

        let items = ReconciliationRepo::query(&self.pool, &query).await?;
        let total = ReconciliationRepo::query_count(&self.pool, &query).await?;

        let mut filled_items = Vec::new();
        for mut s in items {
            s.items = ReconciliationRepo::find_by_statement_id(&self.pool, s.statement_id).await?;
            filled_items.push(s);
        }

        Ok(PaginatedResult::new(filled_items, total as u64, &pagination))
    }

    async fn update_status(&self, statement_id: i64, status: i16, executor: Executor<'_>) -> Result<()> {
        let existing = ReconciliationRepo::find_by_id(&self.pool, statement_id).await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "ReconciliationStatement".to_string(),
                id: statement_id.to_string(),
            })?;

        let valid = matches!(
            (existing.status, status),
            (STATUS_DRAFT, STATUS_CONFIRMED)
                | (STATUS_CONFIRMED, STATUS_APPROVED)
        );

        if !valid {
            return Err(ServiceError::BusinessValidation {
                message: format!("不允许从状态 {} 转换到 {}", existing.status, status),
            }.into());
        }

        ReconciliationRepo::update_status(&mut *executor, statement_id, status).await
    }
}
