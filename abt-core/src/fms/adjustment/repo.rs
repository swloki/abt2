use rust_decimal::Decimal;

use super::model::*;
use crate::shared::types::{PageParams, PgExecutor, Result};

const ADJUSTMENT_COLUMNS: &str = "id, doc_number, party_type, party_id, direction, amount, \
    currency, exchange_rate, adjustment_date, period, int_order_no, ext_order_no, \
    description, ledger_id, operator_id, created_at";

/// 往来方名称子查询：party_type=1 取 customers.customer_name，2 取 suppliers.supplier_name
const PARTY_NAME_EXPR: &str = "CASE WHEN a.party_type = 1 \
    THEN (SELECT c.customer_name FROM customers c WHERE c.customer_id = a.party_id) \
    ELSE (SELECT s.supplier_name FROM suppliers s WHERE s.supplier_id = a.party_id) END";

pub struct AdjustmentRepo;

impl AdjustmentRepo {
    /// 插入调整单，返回 id
    pub async fn create(
        executor: PgExecutor<'_>,
        doc_number: &str,
        req: &CreateAdjustmentReq,
        currency: &str,
        exchange_rate: Decimal,
        operator_id: i64,
    ) -> Result<i64> {
        let id: i64 = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"INSERT INTO ar_ap_adjustments
               (doc_number, party_type, party_id, direction, amount, currency, exchange_rate,
                adjustment_date, period, int_order_no, ext_order_no, description, operator_id)
               VALUES ($1,$2,$3,$4,$5,$6,$7, $8,$9,$10,$11,$12,$13)
               RETURNING id"#,
        )
        .bind(doc_number)
        .bind(req.party_type)
        .bind(req.party_id)
        .bind(req.direction)
        .bind(req.amount)
        .bind(currency)
        .bind(exchange_rate)
        .bind(req.adjustment_date)
        .bind(&req.period)
        .bind(req.int_order_no.as_deref())
        .bind(req.ext_order_no.as_deref())
        .bind(&req.description)
        .bind(operator_id)
        .fetch_one(executor)
        .await?;
        Ok(id)
    }

    /// 回填过账生成的 ledger_id
    pub async fn update_ledger_id(
        executor: PgExecutor<'_>,
        id: i64,
        ledger_id: i64,
    ) -> Result<()> {
        sqlx::query("UPDATE ar_ap_adjustments SET ledger_id = $2 WHERE id = $1")
            .bind(id)
            .bind(ledger_id)
            .execute(executor)
            .await?;
        Ok(())
    }

    pub async fn get_by_id(executor: PgExecutor<'_>, id: i64) -> Result<Option<ArApAdjustment>> {
        let row = sqlx::query_as::<sqlx::Postgres, ArApAdjustment>(sqlx::AssertSqlSafe(
            format!("SELECT {ADJUSTMENT_COLUMNS} FROM ar_ap_adjustments WHERE id = $1"),
        ))
        .bind(id)
        .fetch_optional(executor)
        .await?;
        Ok(row)
    }

    /// 列表查询（子查询取往来方名称），返回 (rows, total)
    pub async fn query(
        executor: PgExecutor<'_>,
        filter: &AdjustmentFilter,
        page: &PageParams,
    ) -> Result<(Vec<AdjustmentRow>, u64)> {
        let mut conditions: Vec<String> = vec!["TRUE".to_string()];
        let mut param_idx = 0u32;

        let party_type_param = if let Some(pt) = filter.party_type {
            param_idx += 1;
            conditions.push(format!("a.party_type = ${param_idx}"));
            Some(pt)
        } else {
            None
        };

        let party_id_param = if let Some(pid) = filter.party_id {
            param_idx += 1;
            conditions.push(format!("a.party_id = ${param_idx}"));
            Some(pid)
        } else {
            None
        };

        let start_param = if let Some(d) = filter.start_date {
            param_idx += 1;
            conditions.push(format!("a.adjustment_date >= ${param_idx}"));
            Some(d)
        } else {
            None
        };

        let end_param = if let Some(d) = filter.end_date {
            param_idx += 1;
            conditions.push(format!("a.adjustment_date <= ${param_idx}"));
            Some(d)
        } else {
            None
        };

        // keyword: 往来方名称模糊搜（同一占位符在两个 EXISTS 子查询中复用）
        let keyword_param = if let Some(ref kw) = filter.keyword {
            if !kw.is_empty() {
                param_idx += 1;
                conditions.push(format!(
                    "(a.party_type = 1 AND EXISTS (SELECT 1 FROM customers c WHERE c.customer_id = a.party_id AND c.customer_name ILIKE ${param_idx})) \
                     OR (a.party_type = 2 AND EXISTS (SELECT 1 FROM suppliers s WHERE s.supplier_id = a.party_id AND s.supplier_name ILIKE ${param_idx}))"
                ));
                Some(format!("%{kw}%"))
            } else {
                None
            }
        } else {
            None
        };

        let where_clause = conditions.join(" AND ");

        // Count
        let count_sql = format!("SELECT COUNT(*) FROM ar_ap_adjustments a WHERE {where_clause}");
        let mut count_q =
            sqlx::query_scalar::<sqlx::Postgres, i64>(sqlx::AssertSqlSafe(count_sql));
        if let Some(v) = party_type_param {
            count_q = count_q.bind(v);
        }
        if let Some(v) = party_id_param {
            count_q = count_q.bind(v);
        }
        if let Some(v) = start_param {
            count_q = count_q.bind(v);
        }
        if let Some(v) = end_param {
            count_q = count_q.bind(v);
        }
        if let Some(ref v) = keyword_param {
            count_q = count_q.bind(v);
        }
        let total = count_q.fetch_one(&mut *executor).await? as u64;

        // Data
        param_idx += 1;
        let limit_idx = param_idx;
        param_idx += 1;
        let offset_idx = param_idx;
        let data_sql = format!(
            "SELECT a.id, a.doc_number, a.party_type, a.party_id, {PARTY_NAME_EXPR} AS party_name, \
             a.direction, a.amount, a.currency, a.adjustment_date, a.period, a.int_order_no, \
             a.ext_order_no, a.description, a.ledger_id, a.operator_id, a.created_at \
             FROM ar_ap_adjustments a WHERE {where_clause} \
             ORDER BY a.id DESC LIMIT ${limit_idx} OFFSET ${offset_idx}",
        );
        let mut data_q =
            sqlx::query_as::<sqlx::Postgres, AdjustmentRow>(sqlx::AssertSqlSafe(data_sql));
        if let Some(v) = party_type_param {
            data_q = data_q.bind(v);
        }
        if let Some(v) = party_id_param {
            data_q = data_q.bind(v);
        }
        if let Some(v) = start_param {
            data_q = data_q.bind(v);
        }
        if let Some(v) = end_param {
            data_q = data_q.bind(v);
        }
        if let Some(ref v) = keyword_param {
            data_q = data_q.bind(v);
        }
        data_q = data_q
            .bind(page.page_size as i64)
            .bind(page.offset() as i64);
        let items = data_q.fetch_all(executor).await?;

        Ok((items, total))
    }
}
