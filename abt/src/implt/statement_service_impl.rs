use anyhow::Result;
use async_trait::async_trait;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::sync::Arc;

use common::error::ServiceError;
use crate::models::{StatementDetail, StatementItem, StatementQuery, StatementWithItems};
use crate::repositories::{
    DocumentSequenceRepo, Executor, PaginatedResult, PaginationParams,
    StatementRepo,
};
use crate::service::StatementService;

/// 用于查询匹配采购订单的行项目
#[derive(Debug, sqlx::FromRow)]
struct PoItemRow {
    po_id: i64,
    po_no: String,
    product_id: i64,
    product_name: Option<String>,
    quantity: Decimal,
    unit_price: Decimal,
}

/// 用于查询匹配的采购订单 ID
#[derive(Debug, sqlx::FromRow)]
struct PoIdRow {
    po_id: i64,
}

pub struct StatementServiceImpl {
    pool: Arc<PgPool>,
}

impl StatementServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl StatementService for StatementServiceImpl {
    async fn generate(
        &self,
        supplier_id: i64,
        period_start: NaiveDate,
        period_end: NaiveDate,
        operator_id: Option<i64>,
        executor: Executor<'_>,
    ) -> Result<i64> {
        // 1. 生成对账单编号
        let statement_no = DocumentSequenceRepo::next_number(&mut *executor, "PS").await?;

        // 2. 查询匹配的采购订单（状态4/5，在期间内，未对账，未删除）
        let po_ids: Vec<PoIdRow> = sqlx::query_as::<_, PoIdRow>(
            r#"
            SELECT po.po_id
            FROM purchase_orders po
            WHERE po.supplier_id = $1
              AND po.status IN (4, 5)
              AND po.created_at >= $2::date
              AND po.created_at < ($3::date + interval '1 day')
              AND po.deleted_at IS NULL
              AND po.po_id NOT IN (SELECT psi.po_id FROM purchase_statement_items psi)
            ORDER BY po.po_id
            "#,
        )
        .bind(supplier_id)
        .bind(period_start)
        .bind(period_end)
        .fetch_all(&mut *executor)
        .await?;

        if po_ids.is_empty() {
            return Err(anyhow::Error::from(ServiceError::BusinessValidation {
                message: "指定供应商在指定期间内没有可对账的采购订单".to_string(),
            }));
        }

        let po_id_list: Vec<i64> = po_ids.iter().map(|r| r.po_id).collect();

        // 3. 查询这些订单的行项目
        let po_items: Vec<PoItemRow> = sqlx::query_as::<_, PoItemRow>(
            r#"
            SELECT poi.po_id, po.po_no, poi.product_id, poi.product_name,
                   poi.quantity, poi.unit_price
            FROM purchase_order_items poi
            INNER JOIN purchase_orders po ON poi.po_id = po.po_id
            WHERE poi.po_id = ANY($1)
            ORDER BY poi.po_id, poi.item_id
            "#,
        )
        .bind(&po_id_list)
        .fetch_all(&mut *executor)
        .await?;

        if po_items.is_empty() {
            return Err(anyhow::Error::from(ServiceError::BusinessValidation {
                message: "采购订单没有行项目，无法生成对账单".to_string(),
            }));
        }

        // 4. 计算总金额并构建行项目
        let mut total_amount = Decimal::ZERO;
        let mut statement_items: Vec<StatementItem> = Vec::with_capacity(po_items.len());

        for item in &po_items {
            let amount = item.quantity * item.unit_price;
            total_amount += amount;

            statement_items.push(StatementItem {
                item_id: 0,
                statement_id: 0, // 尚未生成
                po_id: item.po_id,
                po_no: Some(item.po_no.clone()),
                product_id: item.product_id,
                product_name: item.product_name.clone(),
                quantity: item.quantity,
                unit_price: item.unit_price,
                amount,
            });
        }

        // 5. 插入对账单
        let statement_id = StatementRepo::insert(
            executor,
            &statement_no,
            supplier_id,
            period_start,
            period_end,
            total_amount,
            operator_id,
        )
        .await?;

        // 6. 插入行项目
        for item in &mut statement_items {
            item.statement_id = statement_id;
        }
        StatementRepo::insert_items(executor, &statement_items).await?;

        crate::repositories::PurchaseOrderRepo::batch_update_status(executor, &po_id_list, 6).await?;

        Ok(statement_id)
    }

    /// 根据 ID 获取对账单详情（含行项目）
    async fn get_by_id(&self, statement_id: i64) -> Result<Option<StatementWithItems>> {
        let statement = match StatementRepo::find_by_id(&self.pool, statement_id).await? {
            Some(s) => s,
            None => return Ok(None),
        };

        let items = StatementRepo::find_items(&self.pool, statement_id).await?;

        Ok(Some(StatementWithItems { statement, items }))
    }

    /// 分页查询对账单列表
    async fn list(&self, query: StatementQuery) -> Result<PaginatedResult<StatementDetail>> {
        let page = query.page.unwrap_or(1).max(1) as u32;
        let page_size = query.page_size.unwrap_or(20).clamp(1, 100) as u32;

        let items = StatementRepo::query(&self.pool, &query).await?;
        let total = StatementRepo::query_count(&self.pool, &query).await?;

        let pagination = PaginationParams::new(page, page_size);
        Ok(PaginatedResult::new(items, total as u64, &pagination))
    }

    /// 更新对账单状态
    async fn update_status(
        &self,
        statement_id: i64,
        status: i16,
        executor: Executor<'_>,
    ) -> Result<()> {
        let current_status = StatementRepo::find_status(&self.pool, statement_id)
            .await?
            .ok_or_else(|| ServiceError::NotFound {
                resource: "PurchaseStatement".to_string(),
                id: statement_id.to_string(),
            })?;

        if !is_valid_statement_transition(current_status, status) {
            return Err(anyhow::Error::from(ServiceError::BusinessValidation {
                message: format!(
                    "不允许从状态【{}】变更为【{}】",
                    statement_status_label(current_status),
                    statement_status_label(status)
                ),
            }));
        }

        StatementRepo::update_status(executor, statement_id, status).await?;
        Ok(())
    }
}

/// 对账单状态转换白名单
fn is_valid_statement_transition(from: i16, to: i16) -> bool {
    matches!(
        (from, to),
        (1, 2) // 待确认 → 已确认
        | (1, 3) // 待确认 → 有异议
        | (3, 2) // 有异议 → 已确认
    )
}

/// 对账单状态标签
fn statement_status_label(status: i16) -> &'static str {
    match status {
        1 => "待确认",
        2 => "已确认",
        3 => "有异议",
        _ => "未知",
    }
}
