//! BOM 服务实现
//!
//! 实现 BOM 管理的业务逻辑。

use anyhow::Result;
use async_trait::async_trait;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::models::{Bom, BomDetail, BomNode, BomQuery};
use crate::repositories::{BomRepo, Executor, ProductRepo};
use crate::service::{AttributeOverrides, BomService};

/// BOM 服务实现
pub struct BomServiceImpl {
    pool: Arc<PgPool>,
}

impl BomServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl BomService for BomServiceImpl {
    async fn create(&self, name: &str, created_by: i64, bom_category_id: Option<i64>, executor: Executor<'_>) -> Result<i64> {
        // 检查名称是否已存在 - 这里需要 pool，暂时跳过
        let bom_detail = BomDetail {
            nodes: Vec::new(),
            created_by: Some(created_by),
        };

        let bom_id = BomRepo::insert(executor, name, &bom_detail, bom_category_id).await?;
        Ok(bom_id)
    }

    async fn update(&self, bom: Bom, executor: Executor<'_>) -> Result<()> {
        BomRepo::update(executor, bom.bom_id, &bom.bom_name, Some(&bom.bom_detail), bom.bom_category_id).await
    }

    async fn update_metadata(&self, bom_id: i64, name: &str, bom_category_id: Option<i64>, executor: Executor<'_>) -> Result<()> {
        BomRepo::update(executor, bom_id, name, None, bom_category_id).await
    }

    async fn delete(&self, bom_id: i64, executor: Executor<'_>) -> Result<()> {
        BomRepo::delete(executor, bom_id).await
    }

    async fn find(&self, bom_id: i64, executor: Executor<'_>) -> Result<Option<Bom>> {
        BomRepo::find_by_id(executor, bom_id).await
    }

    async fn query(&self, query: BomQuery) -> Result<(Vec<Bom>, i64)> {
        let list = BomRepo::query(&self.pool, &query).await?;
        let total = BomRepo::query_count(&self.pool, &query).await?;
        Ok((list, total))
    }

    async fn add_node(
        &self,
        bom_id: i64,
        mut node: BomNode,
        executor: Executor<'_>,
    ) -> Result<i64> {
        let mut bom = match BomRepo::find_by_id(executor, bom_id).await? {
            Some(bom) => bom,
            None => return Err(anyhow::anyhow!("BOM not found")),
        };

        // 生成新节点 ID
        let new_id = bom.bom_detail.nodes.iter().map(|n| n.id).max().unwrap_or(0) + 1;
        let count = bom.bom_detail.nodes.len() as i32;
        node.id = new_id;
        node.order = count;

        bom.bom_detail.nodes.push(node);

        BomRepo::update(executor, bom_id, &bom.bom_name, Some(&bom.bom_detail), bom.bom_category_id).await?;

        Ok(new_id)
    }

    async fn update_node(&self, bom_id: i64, node: BomNode, executor: Executor<'_>) -> Result<()> {
        let mut bom = match BomRepo::find_by_id(executor, bom_id).await? {
            Some(bom) => bom,
            None => return Err(anyhow::anyhow!("BOM not found")),
        };

        // 查找并更新节点（只更新可编辑字段，保留 product_id / parent_id / order / product_code）
        if let Some(existing_node) = bom.bom_detail.nodes.iter_mut().find(|n| n.id == node.id) {
            existing_node.quantity = node.quantity;
            existing_node.loss_rate = node.loss_rate;
            existing_node.unit = node.unit;
            existing_node.remark = node.remark;
            existing_node.position = node.position;
            existing_node.work_center = node.work_center;
            existing_node.properties = node.properties;
        }

        BomRepo::update(executor, bom_id, &bom.bom_name, Some(&bom.bom_detail), bom.bom_category_id).await
    }

    async fn delete_node(&self, bom_id: i64, node_id: i64, executor: Executor<'_>) -> Result<i64> {
        let mut bom = match BomRepo::find_by_id(executor, bom_id).await? {
            Some(bom) => bom,
            None => return Err(anyhow::anyhow!("BOM not found")),
        };

        // 找出所有需要删除的节点 ID（包括子节点）
        let mut nodes_to_delete = Vec::new();
        let mut to_check = vec![node_id];

        while let Some(current_id) = to_check.pop() {
            nodes_to_delete.push(current_id);
            // 找出所有子节点
            let children: Vec<i64> = bom
                .bom_detail
                .nodes
                .iter()
                .filter(|n| n.parent_id == current_id)
                .map(|n| n.id)
                .collect();
            to_check.extend(children);
        }

        // 从 BOM 中移除这些节点
        bom.bom_detail
            .nodes
            .retain(|node| !nodes_to_delete.contains(&node.id));

        BomRepo::update(executor, bom_id, &bom.bom_name, Some(&bom.bom_detail), bom.bom_category_id).await?;

        Ok(node_id)
    }

    async fn swap_node_position(
        &self,
        bom_id: i64,
        node_id1: i64,
        node_id2: i64,
        executor: Executor<'_>,
    ) -> Result<()> {
        let mut bom = match BomRepo::find_by_id(executor, bom_id).await? {
            Some(bom) => bom,
            None => return Err(anyhow::anyhow!("BOM not found")),
        };

        // 获取两个节点的 order
        let mut order1 = 0;
        let mut order2 = 0;
        for node in bom.bom_detail.nodes.iter() {
            if node.id == node_id1 {
                order1 = node.order;
            }
            if node.id == node_id2 {
                order2 = node.order;
            }
        }

        // 交换两个节点的 order
        let nodes = &mut bom.bom_detail.nodes;
        for node in nodes.iter_mut() {
            if node.id == node_id1 {
                node.order = order2;
            }
            if node.id == node_id2 {
                node.order = order1;
            }
        }

        BomRepo::update(executor, bom_id, &bom.bom_name, Some(&bom.bom_detail), bom.bom_category_id).await
    }

    async fn exists_name(&self, name: &str) -> Result<bool> {
        BomRepo::exists_name(&self.pool, name).await
    }

    async fn get_leaf_nodes(&self, bom_id: i64, executor: Executor<'_>) -> Result<Vec<BomNode>> {
        // 1. 加载 BOM
        let bom = match BomRepo::find_by_id(executor, bom_id).await? {
            Some(bom) => bom,
            None => return Err(anyhow::anyhow!("BOM not found")),
        };

        // 2. 批量获取产品编码（避免 N+1）
        let product_ids: Vec<i64> = bom.bom_detail.nodes.iter().map(|n| n.product_id).collect();
        let products = ProductRepo::find_by_ids(&self.pool, &product_ids).await?;

        // 构建 product_id 存在的集合
        let valid_product_ids: HashSet<i64> = products.iter().map(|p| p.product_id).collect();

        // 构建 product_id -> product_code 映射
        let product_code_map: HashMap<i64, String> = products
            .iter()
            .map(|p| (p.product_id, p.meta.product_code.clone()))
            .collect();

        // 3. 构建 parent_id 集合，找出叶子节点
        let mut parent_ids: HashSet<i64> = HashSet::new();
        for node in &bom.bom_detail.nodes {
            if node.parent_id != 0 {
                parent_ids.insert(node.parent_id);
            }
        }

        // 4. 找出所有无效节点（产品不存在的节点）及其后代
        // 构建 parent_id -> children 映射
        let mut children_map: HashMap<i64, Vec<i64>> = HashMap::new();
        for node in &bom.bom_detail.nodes {
            children_map
                .entry(node.parent_id)
                .or_default()
                .push(node.id);
        }

        // 递归获取所有后代节点 ID
        fn get_all_descendants(node_id: i64, children_map: &HashMap<i64, Vec<i64>>) -> Vec<i64> {
            let mut descendants = Vec::new();
            if let Some(children) = children_map.get(&node_id) {
                for &child_id in children {
                    descendants.push(child_id);
                    descendants.extend(get_all_descendants(child_id, children_map));
                }
            }
            descendants
        }

        // 找出所有无效节点 ID（产品不存在 + 后代）
        let mut invalid_node_ids: HashSet<i64> = HashSet::new();
        for node in &bom.bom_detail.nodes {
            if !valid_product_ids.contains(&node.product_id) {
                invalid_node_ids.insert(node.id);
                // 添加所有后代
                for descendant in get_all_descendants(node.id, &children_map) {
                    invalid_node_ids.insert(descendant);
                }
            }
        }

        // 5. 过滤叶子节点（不在任何 parent_id 集合中，且不在无效节点集合中）
        let leaf_nodes: Vec<BomNode> = bom
            .bom_detail
            .nodes
            .into_iter()
            .filter(|node| !parent_ids.contains(&node.id))
            .filter(|node| !invalid_node_ids.contains(&node.id))
            .map(|mut node| {
                // 填充 product_code
                node.product_code = product_code_map.get(&node.product_id).cloned();
                node
            })
            .collect();

        // 按 order 排序
        let mut leaf_nodes = leaf_nodes;
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
        // 1. 获取源 BOM
        let source_bom = match BomRepo::find_by_id(executor, source_bom_id).await? {
            Some(bom) => bom,
            None => return Err(anyhow::anyhow!("源 BOM 不存在")),
        };

        // 2. 创建新的 BomDetail（复制节点）
        let new_detail = BomDetail {
            nodes: source_bom.bom_detail.nodes.clone(),
            created_by: Some(created_by),
        };

        // 3. 插入新 BOM（不复制分类，保持为空）
        let new_bom_id = BomRepo::insert(executor, new_name, &new_detail, None).await?;

        Ok(new_bom_id)
    }

    async fn get_product_code(&self, bom_id: i64, executor: Executor<'_>) -> Result<Option<String>> {
        // 1. 获取 BOM
        let bom = match BomRepo::find_by_id(executor, bom_id).await? {
            Some(bom) => bom,
            None => return Ok(None),
        };

        // 2. 获取第一个节点（顶层节点，通常是产品）
        let first_node = bom.bom_detail.nodes.first();

        if let Some(node) = first_node {
            // 3. 如果节点有 product_code，直接返回
            if let Some(ref code) = node.product_code {
                return Ok(Some(code.clone()));
            }
            // 4. 否则尝试从产品表获取
            if node.product_id > 0 {
                let products = ProductRepo::find_by_ids(&self.pool, &[node.product_id]).await?;
                if let Some(product) = products.first() {
                    return Ok(Some(product.meta.product_code.clone()));
                }
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
        executor: Executor<'_>,
    ) -> Result<(i64, i64)> {
        if old_product_id == new_product_id {
            return Ok((0, 0));
        }

        let mut boms = match bom_id {
            Some(id) => {
                let bom = BomRepo::find_by_id_for_update(executor, id).await?;
                match bom {
                    Some(b) => vec![b],
                    None => return Err(anyhow::anyhow!("BOM not found")),
                }
            }
            None => BomRepo::find_all_boms_using_product(executor, old_product_id).await?,
        };

        let products = ProductRepo::find_by_ids(&self.pool, &[new_product_id]).await?;
        let new_product = products
            .first()
            .ok_or_else(|| anyhow::anyhow!("替换物料不存在: {}", new_product_id))?;
        let new_product_code = new_product.meta.product_code.clone();

        let mut affected_bom_count: i64 = 0;
        let mut replaced_node_count: i64 = 0;

        for bom in &mut boms {
            let mut bom_changed = false;

            for node in &mut bom.bom_detail.nodes {
                if node.product_id == old_product_id {
                    node.product_id = new_product_id;
                    node.product_code = Some(new_product_code.clone());

                    if let Some(q) = overrides.quantity {
                        node.quantity = q;
                    }
                    if let Some(lr) = overrides.loss_rate {
                        node.loss_rate = lr;
                    }
                    if let Some(ref u) = overrides.unit {
                        node.unit = Some(u.clone());
                    }
                    if let Some(ref r) = overrides.remark {
                        node.remark = Some(r.clone());
                    }
                    if let Some(ref p) = overrides.position {
                        node.position = Some(p.clone());
                    }
                    if let Some(ref w) = overrides.work_center {
                        node.work_center = Some(w.clone());
                    }
                    if let Some(ref p) = overrides.properties {
                        node.properties = Some(p.clone());
                    }

                    replaced_node_count += 1;
                    bom_changed = true;
                }
            }

            if bom_changed {
                BomRepo::update(
                    executor,
                    bom.bom_id,
                    &bom.bom_name,
                    Some(&bom.bom_detail),
                    bom.bom_category_id,
                )
                .await?;
                affected_bom_count += 1;
            }
        }

        Ok((affected_bom_count, replaced_node_count))
    }
}
