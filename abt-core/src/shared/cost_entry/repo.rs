use sqlx::{FromRow, Row};
use crate::shared::types::Result;

use super::model::{CostEntry, EntryRequest};

pub struct CostEntryRepo;

impl CostEntryRepo {
    /// 在事务中批量 INSERT 成本分录（双层记账必须完整）
    pub async fn batch_insert(
        executor: &mut sqlx::postgres::PgConnection,
        entries: &[EntryRequest],
    ) -> Result<Vec<CostEntry>> {
        let mut results = Vec::with_capacity(entries.len());

        for entry in entries {
            let row = sqlx::query(
                r#"
                INSERT INTO cost_entries
                    (entity_type, entity_id, cost_type, debit_amount, credit_amount,
                     cost_center, profit_center, period, source_type, source_id)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                RETURNING id, entity_type, entity_id, cost_type, debit_amount, credit_amount,
                          cost_center, profit_center, period, source_type, source_id, created_at
                "#,
            )
            .bind(entry.entity_type)
            .bind(entry.entity_id)
            .bind(entry.cost_type)
            .bind(entry.debit_amount)
            .bind(entry.credit_amount)
            .bind(entry.cost_center)
            .bind(entry.profit_center)
            .bind(&entry.period)
            .bind(entry.source_type)
            .bind(entry.source_id)
            .fetch_one(&mut *executor)
            .await?;

            results.push(CostEntry::from_row(&row)?);
        }

        Ok(results)
    }

    /// 分页查询某实体的成本分录
    pub async fn find_by_entity(
        executor: &mut sqlx::postgres::PgConnection,
        entity_type_i16: i16,
        entity_id: i64,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<CostEntry>, u64)> {
        // Count
        let count_row = sqlx::query(
            r#"
            SELECT COUNT(*) AS cnt FROM cost_entries
            WHERE entity_type = $1 AND entity_id = $2
            "#,
        )
        .bind(entity_type_i16)
        .bind(entity_id)
        .fetch_one(&mut *executor)
        .await?;
        let total: i64 = count_row.try_get("cnt")?;

        // Data
        let rows = sqlx::query(
            r#"
            SELECT id, entity_type, entity_id, cost_type, debit_amount, credit_amount,
                   cost_center, profit_center, period, source_type, source_id, created_at
            FROM cost_entries
            WHERE entity_type = $1 AND entity_id = $2
            ORDER BY created_at DESC
            LIMIT $3 OFFSET $4
            "#,
        )
        .bind(entity_type_i16)
        .bind(entity_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&mut *executor)
        .await?;

        let items: Vec<CostEntry> = rows
            .iter()
            .filter_map(|row| CostEntry::from_row(row).ok())
            .collect();

        Ok((items, total as u64))
    }
}
