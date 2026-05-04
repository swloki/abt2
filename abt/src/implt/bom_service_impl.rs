//! BOM 服务实现
//!
//! 实现 BOM 管理的业务逻辑。
//! 节点数据从 bom_nodes 表读写，不再操作 bom_detail JSONB。

use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::models::{f64_to_decimal, Bom, BomCostReport, BomDetail, BomLaborCostReport, BomNode, BomQuery, BomStatus, LaborCostItem, MaterialCostItem, NewBomNode};
use crate::repositories::{BomNodeRepo, BomRepo, Executor, LaborProcessRepo, ProductPriceRepo, ProductRepo};
use crate::service::{AttributeOverrides, BomService};

/// 收集所有后代节点 ID
fn collect_descendants(id: i64, children_map: &HashMap<i64, Vec<i64>>) -> Vec<i64> {
    let mut out = Vec::new();
    if let Some(children) = children_map.get(&id) {
        for &c in children {
            out.push(c);
            out.extend(collect_descendants(c, children_map));
        }
    }
    out
}

/// 节点树分析结果
struct NodeTreeAnalysis {
    nodes: Vec<BomNode>,
    parent_ids: HashSet<i64>,
    invalid_ids: HashSet<i64>,
    product_code_map: HashMap<i64, String>,
    name_map: HashMap<i64, String>,
}

/// BOM 服务实现
pub struct BomServiceImpl {
    pool: Arc<PgPool>,
}

impl BomServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    /// 从 bom_nodes 表加载节点并构建 BomDetail
    async fn build_bom_detail(&self, bom_id: i64) -> Result<BomDetail> {
        let nodes = BomNodeRepo::find_bom_nodes_by_bom_id(&self.pool, bom_id).await?;
        Ok(BomDetail { nodes })
    }

    /// 分析 BOM 节点树：验证产品、构建父子关系、标记无效节点
    async fn analyze_node_tree(&self, bom_id: i64) -> Result<NodeTreeAnalysis> {
        let nodes = BomNodeRepo::find_bom_nodes_by_bom_id(&self.pool, bom_id).await?;

        let product_ids: Vec<i64> = nodes.iter().map(|n| n.product_id).collect();
        let products = ProductRepo::find_by_ids(&self.pool, &product_ids).await?;
        let valid_product_ids: HashSet<i64> = products.iter().map(|p| p.product_id).collect();
        let product_code_map: HashMap<i64, String> = products.iter()
            .map(|p| (p.product_id, p.product_code.clone()))
            .collect();
        let name_map: HashMap<i64, String> = products.iter()
            .map(|p| (p.product_id, p.pdt_name.clone()))
            .collect();

        let parent_ids: HashSet<i64> = nodes.iter()
            .filter(|n| n.parent_id != 0)
            .map(|n| n.parent_id)
            .collect();

        let mut children_map: HashMap<i64, Vec<i64>> = HashMap::new();
        for node in &nodes {
            children_map.entry(node.parent_id).or_default().push(node.id);
        }

        let mut invalid_ids: HashSet<i64> = HashSet::new();
        for node in &nodes {
            if !valid_product_ids.contains(&node.product_id) {
                invalid_ids.insert(node.id);
                invalid_ids.extend(collect_descendants(node.id, &children_map));
            }
        }

        Ok(NodeTreeAnalysis {
            nodes,
            parent_ids,
            invalid_ids,
            product_code_map,
            name_map,
        })
    }
}

#[async_trait]
impl BomService for BomServiceImpl {
    async fn create(&self, name: &str, created_by: i64, bom_category_id: Option<i64>, executor: Executor<'_>) -> Result<i64> {
        let bom_id = BomRepo::insert(executor, name, bom_category_id, Some(created_by), BomStatus::Draft.as_str()).await?;
        Ok(bom_id)
    }

    async fn update(&self, bom: Bom, executor: Executor<'_>) -> Result<()> {
        BomRepo::update(executor, bom.bom_id, &bom.bom_name, bom.bom_category_id).await
    }

    async fn update_metadata(&self, bom_id: i64, name: &str, bom_category_id: Option<i64>, executor: Executor<'_>) -> Result<()> {
        BomRepo::update(executor, bom_id, name, bom_category_id).await
    }

    async fn delete(&self, bom_id: i64, executor: Executor<'_>) -> Result<()> {
        BomNodeRepo::delete_by_bom_id(&mut *executor, bom_id).await?;
        BomRepo::delete(executor, bom_id).await
    }

    async fn find(&self, bom_id: i64, executor: Executor<'_>) -> Result<Option<Bom>> {
        let Some(mut bom) = BomRepo::find_by_id(executor, bom_id).await? else {
            return Ok(None);
        };
        bom.bom_detail = self.build_bom_detail(bom_id).await?;
        Ok(Some(bom))
    }

    async fn query(&self, query: BomQuery) -> Result<(Vec<Bom>, i64)> {
        let mut list = BomRepo::query(&self.pool, &query).await?;
        let total = BomRepo::query_count(&self.pool, &query).await?;

        let bom_ids: Vec<i64> = list.iter().map(|b| b.bom_id).collect();
        let nodes_map = BomNodeRepo::find_by_bom_ids(&self.pool, &bom_ids).await?;
        for bom in &mut list {
            if let Some(rows) = nodes_map.get(&bom.bom_id) {
                let nodes: Vec<BomNode> = rows.iter().cloned().map(|r| r.into()).collect();
                bom.bom_detail.nodes = nodes;
            }
        }

        Ok((list, total))
    }

    async fn add_node(
        &self,
        bom_id: i64,
        node: BomNode,
        executor: Executor<'_>,
    ) -> Result<i64> {
        let existing = BomNodeRepo::find_by_bom_id_for_update(&mut *executor, bom_id).await?;
        let order = existing.len() as i32;

        let new_node = NewBomNode::from_node(bom_id, order, &node);
        let new_id = BomNodeRepo::insert(executor, &new_node).await?;
        Ok(new_id)
    }

    async fn update_node(&self, _bom_id: i64, node: BomNode, executor: Executor<'_>) -> Result<()> {
        BomNodeRepo::update(
            executor,
            node.id,
            f64_to_decimal(node.quantity),
            f64_to_decimal(node.loss_rate),
            node.unit.as_deref(),
            node.remark.as_deref(),
            node.position.as_deref(),
            node.work_center.as_deref(),
            node.properties.as_deref(),
        ).await
    }

    async fn delete_node(&self, _bom_id: i64, node_id: i64, executor: Executor<'_>) -> Result<i64> {
        BomNodeRepo::delete_with_descendants(executor, node_id).await?;
        Ok(node_id)
    }

    async fn swap_node_position(
        &self,
        bom_id: i64,
        node_id1: i64,
        node_id2: i64,
        executor: Executor<'_>,
    ) -> Result<()> {
        let nodes = BomNodeRepo::find_by_bom_id_for_update(&mut *executor, bom_id).await?;

        let n1 = nodes.iter().find(|n| n.id == node_id1);
        let n2 = nodes.iter().find(|n| n.id == node_id2);

        match (n1, n2) {
            (Some(a), Some(b)) => {
                BomNodeRepo::swap_order(executor, a.id, a.order, b.id, b.order).await?;
            }
            _ => return Err(anyhow::anyhow!("Node not found")),
        }

        Ok(())
    }

    async fn exists_name(&self, name: &str, caller_id: Option<i64>) -> Result<bool> {
        BomRepo::exists_name(&self.pool, name, caller_id).await
    }

    async fn publish(&self, bom_id: i64, operator_id: i64, executor: Executor<'_>) -> Result<Bom> {
        let bom = match BomRepo::find_by_id(&mut *executor, bom_id).await? {
            Some(bom) => bom,
            None => anyhow::bail!("BOM not found"),
        };

        bom.require_creator_or_published(operator_id, true)?;

        if bom.status == BomStatus::Published {
            let mut bom = bom;
            bom.bom_detail = self.build_bom_detail(bom_id).await?;
            return Ok(bom);
        }

        let mut published_bom = BomRepo::update_status(&mut *executor, bom_id, BomStatus::Published.as_str(), Some(Utc::now())).await?;
        published_bom.bom_detail = self.build_bom_detail(bom_id).await?;
        Ok(published_bom)
    }

    async fn unpublish(&self, bom_id: i64, operator_id: i64, executor: Executor<'_>) -> Result<Bom> {
        let bom = match BomRepo::find_by_id(&mut *executor, bom_id).await? {
            Some(bom) => bom,
            None => anyhow::bail!("BOM not found"),
        };

        if bom.created_by != Some(operator_id) {
            anyhow::bail!("Permission denied: only the creator can unpublish a BOM");
        }

        if bom.status == BomStatus::Draft {
            let mut bom = bom;
            bom.bom_detail = self.build_bom_detail(bom_id).await?;
            return Ok(bom);
        }

        let mut draft_bom = BomRepo::update_status(&mut *executor, bom_id, BomStatus::Draft.as_str(), None).await?;
        draft_bom.bom_detail = self.build_bom_detail(bom_id).await?;
        Ok(draft_bom)
    }

    async fn get_leaf_nodes(&self, bom_id: i64) -> Result<Vec<BomNode>> {
        let analysis = self.analyze_node_tree(bom_id).await?;

        if analysis.nodes.is_empty() {
            return Ok(Vec::new());
        }

        let mut leaf_nodes: Vec<BomNode> = analysis.nodes
            .into_iter()
            .filter(|n| !analysis.parent_ids.contains(&n.id) && !analysis.invalid_ids.contains(&n.id))
            .map(|mut n| {
                n.product_code = analysis.product_code_map.get(&n.product_id).cloned();
                n
            })
            .collect();

        leaf_nodes.sort_by_key(|n| n.order);
        Ok(leaf_nodes)
    }

    async fn save_as(
        &self,
        source_bom_id: i64,
        new_name: &str,
        created_by: i64,
        executor: Executor<'_>,
    ) -> Result<i64> {
        let _source_bom = BomRepo::find_by_id(&mut *executor, source_bom_id).await?
            .ok_or_else(|| anyhow::anyhow!("源 BOM 不存在"))?;

        let new_bom_id = BomRepo::insert(&mut *executor, new_name, None, Some(created_by), BomStatus::Draft.as_str()).await?;

        BomNodeRepo::copy_to_new_bom(executor, source_bom_id, new_bom_id).await?;

        Ok(new_bom_id)
    }

    async fn get_product_code(&self, bom_id: i64) -> Result<Option<String>> {
        let root = BomNodeRepo::find_root_by_bom_id(&self.pool, bom_id).await?;
        let Some(root) = root else {
            return Ok(None);
        };

        if let Some(ref code) = root.product_code {
            return Ok(Some(code.clone()));
        }

        if root.product_id > 0 {
            let products = ProductRepo::find_by_ids(&self.pool, &[root.product_id]).await?;
            if let Some(product) = products.first() {
                return Ok(Some(product.product_code.clone()));
            }
        }

        Ok(None)
    }

    async fn substitute_product(
        &self,
        old_product_id: i64,
        new_product_id: i64,
        bom_id: Option<i64>,
        overrides: AttributeOverrides,
        caller_id: i64,
        executor: Executor<'_>,
    ) -> Result<(i64, i64)> {
        if old_product_id == new_product_id {
            return Ok((0, 0));
        }

        let products = ProductRepo::find_by_ids(&self.pool, &[new_product_id]).await?;
        let new_product = products
            .first()
            .ok_or_else(|| anyhow::anyhow!("替换物料不存在: {}", new_product_id))?;
        let new_product_code = new_product.product_code.clone();

        let affected_boms: Vec<crate::models::Bom> = match bom_id {
            Some(id) => {
                let bom = BomRepo::find_by_id_for_update(&mut *executor, id).await?
                    .ok_or_else(|| anyhow::anyhow!("BOM not found"))?;
                bom.require_creator_or_published(caller_id, true)?;
                vec![bom]
            }
            None => {
                BomRepo::find_accessible_boms_by_product(&mut *executor, old_product_id, caller_id).await?
            }
        };

        let bom_ids: Vec<i64> = affected_boms.iter().map(|b| b.bom_id).collect();
        let nodes = BomNodeRepo::find_by_bom_ids_and_product(&mut *executor, &bom_ids, old_product_id).await?;

        let mut replaced_node_count: i64 = 0;
        let mut changed_bom_ids: HashSet<i64> = HashSet::new();

        for node in &nodes {
            let quantity = overrides.quantity
                .map(f64_to_decimal)
                .unwrap_or(node.quantity);
            let loss_rate = overrides.loss_rate
                .map(f64_to_decimal)
                .unwrap_or(node.loss_rate);
            let unit = overrides.unit.as_deref().or(node.unit.as_deref());
            let remark = overrides.remark.as_deref().or(node.remark.as_deref());
            let position = overrides.position.as_deref().or(node.position.as_deref());
            let work_center = overrides.work_center.as_deref().or(node.work_center.as_deref());
            let properties = overrides.properties.as_deref().or(node.properties.as_deref());

            BomNodeRepo::substitute_node_product(
                &mut *executor,
                node.id,
                new_product_id,
                Some(&new_product_code),
                quantity,
                loss_rate,
                unit,
                remark,
                position,
                work_center,
                properties,
            ).await?;

            replaced_node_count += 1;
            changed_bom_ids.insert(node.bom_id);
        }

        let affected_bom_count = changed_bom_ids.len() as i64;

        Ok((affected_bom_count, replaced_node_count))
    }

    async fn get_bom_cost_report(&self, bom_id: i64, executor: Executor<'_>) -> Result<BomCostReport> {
        let bom = BomRepo::find_by_id(executor, bom_id).await?
            .ok_or_else(|| anyhow::anyhow!("BOM not found"))?;

        let analysis = self.analyze_node_tree(bom_id).await?;

        let root_node = analysis.nodes.first()
            .ok_or_else(|| anyhow::anyhow!("BOM has no nodes"))?;
        let product_code = if let Some(ref code) = root_node.product_code {
            code.clone()
        } else {
            analysis.product_code_map.get(&root_node.product_id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("Root product not found"))?
        };

        let mut leaf_nodes: Vec<&BomNode> = analysis.nodes.iter()
            .filter(|n| !analysis.parent_ids.contains(&n.id) && !analysis.invalid_ids.contains(&n.id))
            .collect();
        leaf_nodes.sort_by_key(|n| n.order);

        let leaf_product_ids: Vec<i64> = leaf_nodes.iter().map(|n| n.product_id).collect();

        let (prices_result, labor_result) = tokio::join!(
            ProductPriceRepo::get_prices_by_ids(&self.pool, &leaf_product_ids),
            LaborProcessRepo::list_all_by_product_code(&self.pool, &product_code),
        );
        let prices = prices_result?;
        let labor_processes = labor_result?;

        let material_costs: Vec<MaterialCostItem> = leaf_nodes.iter().map(|node| {
            MaterialCostItem {
                node_id: node.id,
                product_id: node.product_id,
                product_name: analysis.name_map.get(&node.product_id).cloned().unwrap_or_default(),
                product_code: analysis.product_code_map.get(&node.product_id).cloned().unwrap_or_default(),
                quantity: node.quantity,
                unit_price: prices.get(&node.product_id).map(|p| p.to_string()),
            }
        }).collect();

        let warnings: Vec<String> = material_costs.iter()
            .filter(|m| m.unit_price.is_none())
            .map(|m| m.product_name.clone())
            .collect();

        let labor_costs: Vec<LaborCostItem> = labor_processes.iter().map(LaborCostItem::from).collect();

        Ok(BomCostReport {
            bom_id,
            bom_name: bom.bom_name,
            product_code,
            material_costs,
            labor_costs,
            warnings,
        })
    }

    async fn get_bom_labor_cost(&self, bom_id: i64) -> Result<BomLaborCostReport> {
        let bom = BomRepo::find_by_id_pool(&self.pool, bom_id).await?
            .ok_or_else(|| anyhow::anyhow!("BOM not found"))?;

        let root = BomNodeRepo::find_root_by_bom_id(&self.pool, bom_id).await?
            .ok_or_else(|| anyhow::anyhow!("BOM has no nodes"))?;

        let product_code = if let Some(ref code) = root.product_code {
            code.clone()
        } else if root.product_id > 0 {
            let products = ProductRepo::find_by_ids(&self.pool, &[root.product_id]).await?;
            products.first()
                .map(|p| p.product_code.clone())
                .ok_or_else(|| anyhow::anyhow!("Root product not found"))?
        } else {
            anyhow::bail!("Root product not found");
        };

        let labor_processes = LaborProcessRepo::list_all_by_product_code(&self.pool, &product_code).await?;

        let labor_costs: Vec<LaborCostItem> = labor_processes.iter().map(LaborCostItem::from).collect();

        Ok(BomLaborCostReport {
            bom_id,
            bom_name: bom.bom_name,
            product_code,
            labor_costs,
            warnings: Vec::new(),
        })
    }
}
