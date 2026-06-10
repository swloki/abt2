use std::collections::{HashMap, HashSet};

use rust_decimal::Decimal;

use super::model::*;
use super::repo::{InventoryCascadeRepo, BomRefRow, CascadeNodeFlat, ProductInfoRow};
use super::service::InventoryCascadeService;
use crate::shared::types::{PgExecutor, DomainError, ServiceContext, Result};

const MAX_BOM_REFS: i32 = 10;

#[derive(Default)]
pub struct InventoryCascadeServiceImpl;

impl InventoryCascadeServiceImpl {
    pub fn new() -> Self { Self }
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
    async fn cascade_inventory(&self, _ctx: &ServiceContext, db: PgExecutor<'_>, query: CascadeInventoryQuery) -> Result<CascadeInventoryResult> {
        if query.product_id.is_none() && query.product_code.is_none() {
            return Err(DomainError::validation("必须提供 product_id 或 product_code"));
        }

        // 1. 查找产品
        let product: Option<ProductInfoRow> = if let Some(id) = query.product_id {
            InventoryCascadeRepo::find_product_by_id(db, id).await?
        } else {
            InventoryCascadeRepo::find_product_by_code(db, query.product_code.as_deref().unwrap_or("")).await?
        };

        let product = product.ok_or_else(|| {
            DomainError::not_found("Product")
        })?;

        // 2. 查找最新 N 个 BOM 引用（产品作为子件被引用的节点）
        let bom_refs = InventoryCascadeRepo::find_bom_refs(db, product.product_id, MAX_BOM_REFS).await?;

        if bom_refs.is_empty() {
            // 即使没有 BOM，也查一下主产品库存
            let main_stock = InventoryCascadeRepo::query_stock_summary(db, &[product.product_id]).await
                .ok()
                .and_then(|s| s.into_iter().next())
                .map(|s| s.total_stock)
                .unwrap_or(Decimal::ZERO);
            return Ok(CascadeInventoryResult {
                product_id: product.product_id,
                product_code: product.product_code,
                product_name: product.pdt_name,
                total_quantity: main_stock,
                bom_groups: vec![],
            });
        }

        // 3. 加载这些 BOM 的全部节点
        let bom_ids: Vec<i64> = bom_refs.iter().map(|r: &BomRefRow| r.bom_id).collect();
        let all_nodes = InventoryCascadeRepo::find_bom_nodes(db, &bom_ids).await?;
        let all_product_ids: Vec<i64> = all_nodes
            .iter()
            .map(|n| n.product_id)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        let products = if all_product_ids.is_empty() {
            vec![]
        } else {
            InventoryCascadeRepo::find_products_by_ids(db, &all_product_ids).await?
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

        // 6. 批量查询库存（从 stock_ledger），包含主产品自身
        let mut all_query_ids: Vec<i64> = bom_groups
            .iter()
            .flat_map(|g| g.children.iter().map(|c| c.product_id))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        all_query_ids.push(product.product_id);

        let stock_map: HashMap<i64, Decimal> = if all_query_ids.is_empty() {
            HashMap::new()
        } else {
            let stocks = InventoryCascadeRepo::query_stock_summary(db, &all_query_ids).await?;
            stocks.into_iter().map(|s| (s.product_id, s.total_stock)).collect()
        };

        let total_quantity = stock_map.get(&product.product_id).copied().unwrap_or(Decimal::ZERO);

        // 7. 填充库存
        for group in &mut bom_groups {
            for child in &mut group.children {
                child.total_stock = stock_map.get(&child.product_id).copied().unwrap_or(Decimal::ZERO);
            }
        }

        // 计算产品总库存量
        let total_quantity: Decimal = stock_map.values().copied().sum();

        Ok(CascadeInventoryResult {
            product_id: product.product_id,
            product_code: product.product_code,
            product_name: product.pdt_name,
            total_quantity,
            bom_groups,
        })
    }
}
