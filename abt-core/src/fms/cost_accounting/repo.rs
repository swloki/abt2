use crate::shared::types::PgExecutor;
use rust_decimal::Decimal;
use crate::shared::types::Result;

use super::model::*;

/// 成本核算 Repo — 查询共享 cost_entries 表的聚合数据
pub struct CostAccountingRepo;

impl CostAccountingRepo {
    /// 查询指定产品在某期间的成本汇总
    pub async fn get_product_cost_by_period(
        executor: PgExecutor<'_>,
        product_id: i64,
        period: &str,
    ) -> Result<ProductCostSummary> {
        let rows = sqlx::query_as::<sqlx::Postgres, CostTypeRow>(
            r#"SELECT cost_type::smallint AS cost_type, COALESCE(SUM(debit_amount), 0) AS total
               FROM cost_entries
               WHERE entity_type = 1 AND entity_id = $1 AND period = $2
               GROUP BY cost_type"#,
        )
        .bind(product_id)
        .bind(period)
        .fetch_all(executor)
        .await?;

        let mut material = Decimal::ZERO;
        let mut labor = Decimal::ZERO;
        let mut overhead = Decimal::ZERO;
        let mut outsource = Decimal::ZERO;
        let mut rework = Decimal::ZERO;
        let mut scrap = Decimal::ZERO;

        for row in &rows {
            match row.cost_type {
                1 => material = row.total,
                2 => labor = row.total,
                3 => overhead = row.total,
                4 => outsource = row.total,
                5 => rework = row.total,
                6 => scrap = row.total,
                _ => {}
            }
        }

        let total_cost = material + labor + overhead + outsource + rework + scrap;

        Ok(ProductCostSummary {
            product_id,
            period: period.to_string(),
            material_cost: material,
            labor_cost: labor,
            overhead_cost: overhead,
            outsource_cost: outsource,
            rework_cost: rework,
            scrap_cost: scrap,
            total_cost,
        })
    }

    /// 查询指定工单的成本汇总
    pub async fn get_work_order_cost(
        executor: PgExecutor<'_>,
        work_order_id: i64,
    ) -> Result<WorkOrderCostSummary> {
        let rows = sqlx::query_as::<sqlx::Postgres, CostTypeRow>(
            r#"SELECT cost_type::smallint AS cost_type, COALESCE(SUM(debit_amount), 0) AS total
               FROM cost_entries
               WHERE entity_type = 2 AND entity_id = $1
               GROUP BY cost_type"#,
        )
        .bind(work_order_id)
        .fetch_all(executor)
        .await?;

        let mut material = Decimal::ZERO;
        let mut labor = Decimal::ZERO;
        let mut overhead = Decimal::ZERO;

        for row in &rows {
            match row.cost_type {
                1 => material = row.total,
                2 => labor = row.total,
                3 => overhead = row.total,
                _ => {}
            }
        }

        let total_cost = material + labor + overhead;

        Ok(WorkOrderCostSummary {
            work_order_id,
            material_cost: material,
            labor_cost: labor,
            overhead_cost: overhead,
            total_cost,
        })
    }

    /// 查询指定利润中心在时间范围内的汇总（分页）
    /// 返回 (数据列表, 总条数)
    #[allow(unused_assignments)]
    pub async fn get_profit_center_summary(
        executor: PgExecutor<'_>,
        profit_center_id: i64,
        from: &str,
        to: &str,
        page_size: u32,
        offset: u32,
    ) -> Result<(Vec<ProfitCenterSummary>, u64)> {
        // Count
        let total: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM (
                 SELECT 1 FROM cost_entries
                 WHERE profit_center = $1 AND period >= $2 AND period <= $3
                 GROUP BY period
               ) sub"#,
        )
        .bind(profit_center_id)
        .bind(from)
        .bind(to)
        .fetch_one(&mut *executor)
        .await?;

        // Data
        let rows = sqlx::query_as::<sqlx::Postgres, ProfitCenterRow>(
            r#"SELECT profit_center, period,
                      COALESCE(SUM(debit_amount), 0) AS total_debit,
                      COALESCE(SUM(credit_amount), 0) AS total_credit
               FROM cost_entries
               WHERE profit_center = $1 AND period >= $2 AND period <= $3
               GROUP BY profit_center, period
               ORDER BY period
               LIMIT $4 OFFSET $5"#,
        )
        .bind(profit_center_id)
        .bind(from)
        .bind(to)
        .bind(page_size as i64)
        .bind(offset as i64)
        .fetch_all(executor)
        .await?;

        let items = rows
            .into_iter()
            .map(|r| ProfitCenterSummary {
                profit_center_id: r.profit_center,
                period: r.period,
                net_amount: r.total_debit - r.total_credit,
                total_debit: r.total_debit,
                total_credit: r.total_credit,
            })
            .collect();

        Ok((items, total as u64))
    }

    /// 查询指定销售订单的毛利分析
    /// estimated_cost 暂返回 ZERO（待后续实现从订单数据获取）
    pub async fn get_margin_analysis(
        executor: PgExecutor<'_>,
        order_id: i64,
    ) -> Result<MarginAnalysis> {
        let rows = sqlx::query_as::<sqlx::Postgres, CostTypeRow>(
            r#"SELECT cost_type::smallint AS cost_type, COALESCE(SUM(debit_amount), 0) AS total
               FROM cost_entries
               WHERE entity_type = 3 AND entity_id = $1
               GROUP BY cost_type"#,
        )
        .bind(order_id)
        .fetch_all(executor)
        .await?;

        let actual_cost: Decimal = rows.iter().map(|r| r.total).sum();
        let estimated_cost = Decimal::ZERO;
        let margin_amount = estimated_cost - actual_cost;
        let margin_rate = if estimated_cost > Decimal::ZERO {
            (margin_amount / estimated_cost * Decimal::from(100)).round_dp(2)
        } else {
            Decimal::ZERO
        };

        Ok(MarginAnalysis {
            order_id,
            estimated_cost,
            actual_cost,
            margin_amount,
            margin_rate,
        })
    }

    /// 查询指定期间所有产品的成本汇总（关联 products 表获取名称）
    pub async fn list_product_costs_by_period(
        executor: PgExecutor<'_>,
        period: &str,
    ) -> Result<Vec<ProductCostRow>> {
        let rows = sqlx::query_as::<sqlx::Postgres, ProductCostRow>(
            r#"SELECT ce.entity_id AS product_id,
                      p.product_code,
                      p.pdt_name AS product_name,
                      ce.cost_type::smallint AS cost_type,
                      COALESCE(SUM(ce.debit_amount), 0) AS total
               FROM cost_entries ce
               JOIN products p ON p.product_id = ce.entity_id
               WHERE ce.entity_type = 1 AND ce.period = $1
               GROUP BY ce.entity_id, p.product_code, p.pdt_name, ce.cost_type
               ORDER BY ce.entity_id, ce.cost_type"#,
        )
        .bind(period)
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }

    /// 查询所有工单的成本汇总（关联 work_orders + products）
    pub async fn list_work_order_costs(
        executor: PgExecutor<'_>,
    ) -> Result<Vec<WorkOrderCostRow>> {
        let rows = sqlx::query_as::<sqlx::Postgres, WorkOrderCostRow>(
            r#"SELECT ce.entity_id AS work_order_id,
                      wo.doc_number,
                      p.pdt_name AS product_name,
                      wo.planned_qty,
                      (SELECT COALESCE(SUM(wr.completed_qty), 0)
                       FROM work_reports wr WHERE wr.work_order_id = wo.id
                      ) AS completed_qty,
                      wo.status AS wo_status,
                      ce.cost_type::smallint AS cost_type,
                      COALESCE(SUM(ce.debit_amount), 0) AS total
               FROM cost_entries ce
               JOIN work_orders wo ON wo.id = ce.entity_id
               JOIN products p ON p.product_id = wo.product_id
               WHERE ce.entity_type = 2
               GROUP BY ce.entity_id, wo.doc_number, p.pdt_name, wo.planned_qty, wo.status, wo.id, ce.cost_type
               ORDER BY ce.entity_id, ce.cost_type"#,
        )
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }

    /// 查询所有利润中心 P&L（按 profit_center + period + cost_type 聚合）
    pub async fn list_profit_center_pl(
        executor: PgExecutor<'_>,
        period: &str,
    ) -> Result<Vec<ProfitCenterPLRow>> {
        let rows = sqlx::query_as::<sqlx::Postgres, ProfitCenterPLRow>(
            r#"SELECT profit_center, period,
                      cost_type::smallint AS cost_type,
                      COALESCE(SUM(debit_amount), 0) AS total_debit,
                      COALESCE(SUM(credit_amount), 0) AS total_credit
               FROM cost_entries
               WHERE profit_center IS NOT NULL AND period = $1
               GROUP BY profit_center, period, cost_type
               ORDER BY profit_center, cost_type"#,
        )
        .bind(period)
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }

    /// 查询所有销售订单的毛利数据（关联 sales_orders + customers）
    pub async fn list_margin_analysis(
        executor: PgExecutor<'_>,
    ) -> Result<Vec<MarginRow>> {
        let rows = sqlx::query_as::<sqlx::Postgres, MarginRow>(
            r#"SELECT ce.entity_id AS order_id,
                      so.doc_number,
                      COALESCE(c.customer_name, '') AS customer_name,
                      so.total_amount AS order_amount,
                      ce.cost_type::smallint AS cost_type,
                      COALESCE(SUM(ce.debit_amount), 0) AS total_cost
               FROM cost_entries ce
               JOIN sales_orders so ON so.id = ce.entity_id
               LEFT JOIN customers c ON c.customer_id = so.customer_id
               WHERE ce.entity_type = 3
               GROUP BY ce.entity_id, so.doc_number, c.customer_name, so.total_amount, ce.cost_type
               ORDER BY ce.entity_id, ce.cost_type"#,
        )
        .fetch_all(executor)
        .await?;
        Ok(rows)
    }
}
