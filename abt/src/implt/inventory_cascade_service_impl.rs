//! 级联查询库存服务实现

use anyhow::Result;
use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::models::{BomCascadeGroup, CascadeInventoryResult, ChildNodeInventory};
use crate::repositories::{CascadeNodeFlat, InventoryCascadeRepo};
use crate::service::InventoryCascadeService;

const MAX_BOM_REFS: i32 = 10;

pub struct InventoryCascadeServiceImpl {
    pool: Arc<PgPool>,
}

impl InventoryCascadeServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

/// 迭代 DFS 收集入口节点的所有后代，跳过无效产品，检测循环引用
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
                if !valid_product_ids.contains(&child.product_id) || !visited.insert(child.id) {
                    continue;
                }
                result.push(*child);
                stack.push(child.id);
            }
        }
    }
    result
}

#[async_trait]
impl InventoryCascadeService for InventoryCascadeServiceImpl {
    async fn cascade_inventory(
        &self,
        product_id: Option<i64>,
        product_code: Option<String>,
        _max_results: i32,
    ) -> Result<CascadeInventoryResult> {
        if product_id.is_none() && product_code.is_none() {
            anyhow::bail!("必须提供 product_id 或 product_code");
        }

        let product_code_for_error = product_code.clone();
        let start = std::time::Instant::now();

        // 1. 查找产品
        let product = InventoryCascadeRepo::find_product(&self.pool, product_id, product_code)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "产品不存在: {}",
                    product_id
                        .map(|id| id.to_string())
                        .or(product_code_for_error)
                        .unwrap_or_default()
                )
            })?;

        // 2. 查找最新 10 个已发布 BOM 引用
        let bom_refs =
            InventoryCascadeRepo::find_published_bom_refs(&self.pool, product.product_id, MAX_BOM_REFS)
                .await?;

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
        let all_nodes = InventoryCascadeRepo::find_nodes_by_bom_ids(&self.pool, &bom_ids).await?;

        tracing::info!(
            elapsed_ms = start.elapsed().as_millis() as u64,
            bom_count = bom_ids.len(),
            node_count = all_nodes.len(),
            "cascade nodes loaded"
        );

        // 4. 查询所有节点引用的产品（轻量查询，不加载 JSONB meta）
        let all_product_ids: Vec<i64> = all_nodes
            .iter()
            .map(|n| n.product_id)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        let products = InventoryCascadeRepo::find_products_by_ids(&self.pool, &all_product_ids).await?;
        let valid_product_ids: HashSet<i64> =
            products.iter().map(|p| p.product_id).collect();
        let product_name_map: HashMap<i64, String> =
            products.iter().map(|p| (p.product_id, p.pdt_name.clone())).collect();

        // 5. 按 bom_id 分组，一次性构建所有 children_map
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

            let descendants = collect_descendants(
                bom_ref.entry_node_id,
                cmap,
                &valid_product_ids,
            );

            if descendants.is_empty() {
                continue;
            }

            let children: Vec<ChildNodeInventory> = descendants
                .into_iter()
                .map(|node| ChildNodeInventory {
                    node_id: node.id,
                    product_id: node.product_id,
                    product_code: node.product_code.clone().unwrap_or_default(),
                    product_name: product_name_map
                        .get(&node.product_id)
                        .cloned()
                        .unwrap_or_default(),
                    unit: node.unit.clone(),
                    quantity: node.quantity,
                    total_stock: Decimal::ZERO,
                    loss_rate: node.loss_rate,
                    order: node.order,
                    parent_node_id: node.parent_id,
                })
                .collect();

            bom_groups.push(BomCascadeGroup {
                bom_id: bom_ref.bom_id,
                bom_name: bom_ref.bom_name.clone(),
                children,
            });
        }

        // 6. 批量查询库存
        let descendant_product_ids: Vec<i64> = bom_groups
            .iter()
            .flat_map(|g| g.children.iter().map(|c| c.product_id))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        let stock_map = if descendant_product_ids.is_empty() {
            HashMap::new()
        } else {
            let stocks = InventoryCascadeRepo::find_stock_summary(
                &self.pool,
                &descendant_product_ids,
            )
            .await?;
            stocks
                .into_iter()
                .map(|s| (s.product_id, s.total_stock))
                .collect()
        };

        // 7. 填充库存
        for group in &mut bom_groups {
            for child in &mut group.children {
                child.total_stock = stock_map
                    .get(&child.product_id)
                    .copied()
                    .unwrap_or(Decimal::ZERO);
            }
        }

        tracing::info!(
            elapsed_ms = start.elapsed().as_millis() as u64,
            bom_groups = bom_groups.len(),
            "cascade inventory completed"
        );

        Ok(CascadeInventoryResult {
            product_id: product.product_id,
            product_code: product.product_code,
            product_name: product.pdt_name,
            bom_groups,
        })
    }
}
