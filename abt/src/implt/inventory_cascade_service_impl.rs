//! 级联查询库存服务实现

use anyhow::Result;
use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;

use crate::models::{BomCascadeGroup, CascadeInventoryResult, ChildNodeInventory};
use crate::repositories::InventoryCascadeRepo;
use crate::service::InventoryCascadeService;

const MAX_RESULTS_LIMIT: i32 = 2000;

pub struct InventoryCascadeServiceImpl {
    pool: Arc<PgPool>,
}

impl InventoryCascadeServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl InventoryCascadeService for InventoryCascadeServiceImpl {
    async fn cascade_inventory(
        &self,
        product_id: Option<i64>,
        product_code: Option<String>,
        max_results: i32,
    ) -> Result<CascadeInventoryResult> {
        let max = max_results.clamp(1, MAX_RESULTS_LIMIT);

        let product_code_for_error = product_code.clone();
        let start = std::time::Instant::now();

        let rows = InventoryCascadeRepo::find_cascade_nodes(
            &self.pool,
            product_id,
            product_code,
            max,
        )
        .await?;

        tracing::info!(
            elapsed_ms = start.elapsed().as_millis() as u64,
            row_count = rows.len(),
            "cascade structure query completed"
        );

        if rows.is_empty() {
            anyhow::bail!(
                "产品不存在: {}",
                product_id
                    .map(|id| id.to_string())
                    .or(product_code_for_error)
                    .unwrap_or_default()
            );
        }

        let first = &rows[0];
        let result_product_id = first.root_product_id;
        let result_product_code = first.root_product_code.clone();
        let result_product_name = first.root_product_name.clone();

        let child_rows: Vec<_> = rows
            .into_iter()
            .filter(|r| r.bom_id.is_some() && r.node_id.is_some())
            .collect();

        let child_product_ids: Vec<i64> = child_rows
            .iter()
            .filter_map(|r| r.product_id)
            .collect();

        let stock_start = std::time::Instant::now();
        let stock_map = if child_product_ids.is_empty() {
            HashMap::new()
        } else {
            let stocks = InventoryCascadeRepo::find_stock_summary(
                &self.pool,
                &child_product_ids,
            )
            .await?;

            stocks
                .into_iter()
                .map(|s| (s.product_id, s.total_stock))
                .collect()
        };

        tracing::info!(
            elapsed_ms = stock_start.elapsed().as_millis() as u64,
            product_count = child_product_ids.len(),
            "cascade stock query completed"
        );

        let mut groups: HashMap<i64, BomCascadeGroup> = HashMap::new();

        for row in child_rows {
            let bom_id = match row.bom_id {
                Some(id) => id,
                None => continue,
            };
            let bom_name = row.bom_name.unwrap_or_default();

            let child = ChildNodeInventory {
                node_id: row.node_id.unwrap_or(0),
                product_id: row.product_id.unwrap_or(0),
                product_code: row.product_code.unwrap_or_default(),
                product_name: row.product_name.unwrap_or_default(),
                unit: row.unit,
                quantity: row.quantity.unwrap_or(Decimal::ZERO),
                total_stock: row
                    .product_id
                    .and_then(|pid| stock_map.get(&pid).copied())
                    .unwrap_or(Decimal::ZERO),
                loss_rate: row.loss_rate.unwrap_or(Decimal::ZERO),
                order: row.order.unwrap_or(0),
                parent_node_id: row.parent_node_id,
            };

            groups
                .entry(bom_id)
                .or_insert_with(|| BomCascadeGroup {
                    bom_id,
                    bom_name,
                    children: Vec::new(),
                })
                .children
                .push(child);
        }

        let mut bom_groups: Vec<BomCascadeGroup> = groups.into_values().collect();
        bom_groups.sort_by_key(|g| std::cmp::Reverse(g.bom_id));

        Ok(CascadeInventoryResult {
            product_id: result_product_id,
            product_code: result_product_code,
            product_name: result_product_name,
            bom_groups,
        })
    }
}
