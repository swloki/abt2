use crate::shared::types::Result;

use super::model::{CostEntry, EntryRequest};
use crate::shared::enums::{CostEntityType, CostType, DocumentType};

pub struct CostEntryRepo;

impl CostEntryRepo {
    /// 在事务中批量 INSERT 成本分录（双层记账必须完整）
    pub async fn batch_insert(
        executor: &mut sqlx::postgres::PgConnection,
        entries: &[EntryRequest],
    ) -> Result<Vec<CostEntry>> {
        let mut results = Vec::with_capacity(entries.len());

        for entry in entries {
            let row = sqlx::query!(
                r#"
                INSERT INTO cost_entries
                    (entity_type, entity_id, cost_type, debit_amount, credit_amount,
                     cost_center, profit_center, period, source_type, source_id)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                RETURNING id, entity_type as "entity_type: i16", entity_id, cost_type as "cost_type: i16", debit_amount, credit_amount,
                          cost_center, profit_center, period, source_type as "source_type: i16", source_id, created_at
                "#,
                entry.entity_type.as_i16(),
                entry.entity_id,
                entry.cost_type.as_i16(),
                entry.debit_amount,
                entry.credit_amount,
                entry.cost_center,
                entry.profit_center,
                &entry.period,
                entry.source_type.as_i16(),
                entry.source_id,
            )
            .fetch_one(&mut *executor)
            .await?;

            results.push(CostEntry {
                id: row.id,
                entity_type: CostEntityType::from_i16(row.entity_type).unwrap(),
                entity_id: row.entity_id,
                cost_type: CostType::from_i16(row.cost_type).unwrap(),
                debit_amount: row.debit_amount,
                credit_amount: row.credit_amount,
                cost_center: row.cost_center,
                profit_center: row.profit_center,
                period: row.period,
                source_type: DocumentType::from_i16(row.source_type).unwrap(),
                source_id: row.source_id,
                created_at: row.created_at,
            });
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
        let total: i64 = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) FROM cost_entries
            WHERE entity_type = $1 AND entity_id = $2
            "#,
            entity_type_i16,
            entity_id,
        )
        .fetch_one(&mut *executor)
        .await?
        .unwrap_or(0);

        // Data
        let rows = sqlx::query!(
            r#"
            SELECT id, entity_type as "entity_type: i16", entity_id, cost_type as "cost_type: i16", debit_amount, credit_amount,
                   cost_center, profit_center, period, source_type as "source_type: i16", source_id, created_at
            FROM cost_entries
            WHERE entity_type = $1 AND entity_id = $2
            ORDER BY created_at DESC
            LIMIT $3 OFFSET $4
            "#,
            entity_type_i16,
            entity_id,
            limit,
            offset,
        )
        .fetch_all(&mut *executor)
        .await?;

        let items: Vec<CostEntry> = rows
            .into_iter()
            .map(|r| CostEntry {
                id: r.id,
                entity_type: CostEntityType::from_i16(r.entity_type).unwrap(),
                entity_id: r.entity_id,
                cost_type: CostType::from_i16(r.cost_type).unwrap(),
                debit_amount: r.debit_amount,
                credit_amount: r.credit_amount,
                cost_center: r.cost_center,
                profit_center: r.profit_center,
                period: r.period,
                source_type: DocumentType::from_i16(r.source_type).unwrap(),
                source_id: r.source_id,
                created_at: r.created_at,
            })
            .collect();

        Ok((items, total as u64))
    }
}
