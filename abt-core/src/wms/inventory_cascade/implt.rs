use std::collections::{HashMap, HashSet};

use rust_decimal::Decimal;

use super::model::*;
use super::service::InventoryCascadeService;
use crate::shared::types::{DomainError, ServiceContext, Result};

const MAX_BOM_REFS: i32 = 10;

#[derive(Default)]
pub struct InventoryCascadeServiceImpl;

impl InventoryCascadeServiceImpl {
    pub fn new() -> Self { Self }
}

#[derive(Debug, sqlx::FromRow)]
struct ProductInfoRow {
    product_id: i64,
    product_code: String,
    pdt_name: String,
}

#[derive(Debug, sqlx::FromRow)]
struct BomRefRow {
    bom_id: i64,
    bom_name: String,
    entry_node_id: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct CascadeNodeFlat {
    node_id: i64,
    bom_id: i64,
    product_id: i64,
    product_code: Option<String>,
    quantity: Decimal,
    parent_id: Option<i64>,
    loss_rate: Decimal,
    order_num: i32,
    unit: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct StockSummaryRow {
    product_id: i64,
    total_stock: Decimal,
}

fn collect_descendants<'a>(
    root_id: i64,
    children_map: &'a HashMap<i64, Vec<&'a CascadeNodeFlat>>,
    valid_product_ids: &HashSet<i64>,
) -> Vec<&'a CascadeNodeFlat> {
    let mut result = Vec::with_capacity(64);
    let mut visited = HashSet::new();
    let mut stack = vec![root_id];
    while let Some(pid) = stack.pop() {
        if let Some(children) = children_map.get(&pid) {
            for child in children {
                if !valid_product_ids.contains(&child.product_id) || !visited.insert(child.node_id) {
                    continue;
                }
                result.push(*child);
                stack.push(child.node_id);
            }
        }
    }
    result
}

#[async_trait::async_trait]
impl InventoryCascadeService for InventoryCascadeServiceImpl {
    async fn cascade_inventory(&self, ctx: ServiceContext<'_>, query: CascadeInventoryQuery) -> Result<CascadeInventoryResult> {
        if query.product_id.is_none() && query.product_code.is_none() {
            return Err(DomainError::validation("必须提供 product_id 或 product_code"));
        }

        // 1. 查找产品
        let product: Option<ProductInfoRow> = if let Some(id) = query.product_id {
            sqlx::query_as::<sqlx::Postgres, ProductInfoRow>(
                "SELECT product_id, product_code, pdt_name FROM products WHERE product_id = $1 LIMIT 1",
            )
            .bind(id)
            .fetch_optional(&mut *ctx.executor)
            .await.map_err(|e| DomainError::Internal(e.into()))?
        } else {
            sqlx::query_as::<sqlx::Postgres, ProductInfoRow>(
                "SELECT product_id, product_code, pdt_name FROM products WHERE product_code = $1 LIMIT 1",
            )
            .bind(&query.product_code)
            .fetch_optional(&mut *ctx.executor)
            .await.map_err(|e| DomainError::Internal(e.into()))?
        };

        let product = product.ok_or_else(|| {
            DomainError::not_found("Product")
        })?;

        // 2. 查找最新 N 个 BOM 引用（产品作为子件被引用的节点）
        let bom_refs = sqlx::query_as::<sqlx::Postgres, BomRefRow>(
            r#"SELECT DISTINCT ON (bn.bom_id) bn.bom_id, b.bom_name, bn.node_id AS entry_node_id
               FROM bom_nodes bn
               JOIN boms b ON b.bom_id = bn.bom_id AND b.deleted_at IS NULL
               WHERE bn.product_id = $1
               ORDER BY bn.bom_id DESC, bn.node_id ASC
               LIMIT $2"#,
        )
        .bind(product.product_id)
        .bind(MAX_BOM_REFS)
        .fetch_all(&mut *ctx.executor)
        .await.map_err(|e| DomainError::Internal(e.into()))?;

        if bom_refs.is_empty() {
            return Ok(CascadeInventoryResult {
                product_id: product.product_id,
                product_code: product.product_code,
                product_name: product.pdt_name,
                bom_groups: vec![],
            });
        }

        // 3. 加载这些 BOM 的全部节点
        let bom_ids: Vec<i64> = bom_refs.iter().map(|r| r.bom_id).collect();
        let all_nodes = sqlx::query_as::<sqlx::Postgres, CascadeNodeFlat>(
            r#"SELECT node_id, bom_id, product_id, product_code, quantity, parent_id, loss_rate, order_num, unit
               FROM bom_nodes
               WHERE bom_id = ANY($1)
               ORDER BY order_num"#,
        )
        .bind(&bom_ids)
        .fetch_all(&mut *ctx.executor)
        .await.map_err(|e| DomainError::Internal(e.into()))?;
        let all_product_ids: Vec<i64> = all_nodes
            .iter()
            .map(|n| n.product_id)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        let products = if all_product_ids.is_empty() {
            vec![]
        } else {
            sqlx::query_as::<sqlx::Postgres, ProductInfoRow>(
                "SELECT product_id, product_code, pdt_name FROM products WHERE product_id = ANY($1)",
            )
            .bind(&all_product_ids)
            .fetch_all(&mut *ctx.executor)
            .await.map_err(|e| DomainError::Internal(e.into()))?
        };

        let valid_product_ids: HashSet<i64> = products.iter().map(|p| p.product_id).collect();
        let product_name_map: HashMap<i64, String> = products.iter().map(|p| (p.product_id, p.pdt_name.clone())).collect();

        // 5. 按 bom_id 分组构建 children_map
        let mut children_by_bom: HashMap<i64, HashMap<i64, Vec<&CascadeNodeFlat>>> = HashMap::new();
        for node in &all_nodes {
            if let Some(pid) = node.parent_id {
                children_by_bom
                    .entry(node.bom_id)
                    .or_default()
                    .entry(pid)
                    .or_default()
                    .push(node);
            }
        }

        let mut bom_groups: Vec<BomCascadeGroup> = Vec::new();

        for bom_ref in &bom_refs {
            let cmap = match children_by_bom.get(&bom_ref.bom_id) {
                Some(m) => m,
                None => continue,
            };

            let descendants = collect_descendants(bom_ref.entry_node_id, cmap, &valid_product_ids);
            if descendants.is_empty() {
                continue;
            }

            let children: Vec<ChildNodeInventory> = descendants
                .into_iter()
                .map(|node| ChildNodeInventory {
                    node_id: node.node_id,
                    product_id: node.product_id,
                    product_code: node.product_code.clone().unwrap_or_default(),
                    product_name: product_name_map.get(&node.product_id).cloned().unwrap_or_default(),
                    unit: node.unit.clone(),
                    quantity: node.quantity,
                    total_stock: Decimal::ZERO,
                    loss_rate: node.loss_rate,
                    order: node.order_num,
                    parent_node_id: node.parent_id,
                })
                .collect();

            bom_groups.push(BomCascadeGroup {
                bom_id: bom_ref.bom_id,
                bom_name: bom_ref.bom_name.clone(),
                children,
            });
        }

        // 6. 批量查询库存（从 stock_ledger）
        let descendant_product_ids: Vec<i64> = bom_groups
            .iter()
            .flat_map(|g| g.children.iter().map(|c| c.product_id))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        let stock_map: HashMap<i64, Decimal> = if descendant_product_ids.is_empty() {
            HashMap::new()
        } else {
            let stocks = sqlx::query_as::<sqlx::Postgres, StockSummaryRow>(
                "SELECT product_id, SUM(quantity) AS total_stock FROM stock_ledger WHERE product_id = ANY($1) GROUP BY product_id",
            )
            .bind(&descendant_product_ids)
            .fetch_all(&mut *ctx.executor)
            .await.map_err(|e| DomainError::Internal(e.into()))?;

            stocks.into_iter().map(|s| (s.product_id, s.total_stock)).collect()
        };

        // 7. 填充库存
        for group in &mut bom_groups {
            for child in &mut group.children {
                child.total_stock = stock_map.get(&child.product_id).copied().unwrap_or(Decimal::ZERO);
            }
        }

        Ok(CascadeInventoryResult {
            product_id: product.product_id,
            product_code: product.product_code,
            product_name: product.pdt_name,
            bom_groups,
        })
    }
}
