//! BOM 服务实现
//!
//! 实现 BOM 管理的业务逻辑。

use anyhow::Result;
use async_trait::async_trait;
use rust_xlsxwriter::{Color, Format, FormatAlign, FormatBorder, Workbook};
use sqlx::PgPool;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use std::sync::{Arc, OnceLock};

use crate::models::{Bom, BomDetail, BomNode, BomQuery, Product};
use crate::repositories::{BomRepo, Executor, ProductRepo};
use crate::service::{AttributeOverrides, BomService};

/// BOM 节点与产品的关联结构
struct NodeWithProduct {
    node: BomNode,
    product: Product,
}

/// Excel 表头格式
fn header_format() -> &'static Format {
    static FORMAT: OnceLock<Format> = OnceLock::new();
    FORMAT.get_or_init(|| {
        Format::new()
            .set_bold()
            .set_align(FormatAlign::Center)
            .set_align(FormatAlign::VerticalCenter)
            .set_background_color("#00B0F0")
            .set_font_color(Color::White)
            .set_border(FormatBorder::Thin)
    })
}

/// 顶层节点格式（紫色）
fn top_level_format() -> &'static Format {
    static FORMAT: OnceLock<Format> = OnceLock::new();
    FORMAT.get_or_init(|| {
        Format::new()
            .set_background_color("#7030A0")
            .set_font_color(Color::White)
            .set_bold()
            .set_border(FormatBorder::Thin)
            .set_align(FormatAlign::Left)
    })
}

/// 有子节点的格式（黄色）
fn parent_format() -> &'static Format {
    static FORMAT: OnceLock<Format> = OnceLock::new();
    FORMAT.get_or_init(|| {
        Format::new()
            .set_background_color("#FFFF00")
            .set_border(FormatBorder::Thin)
            .set_align(FormatAlign::Left)
    })
}

/// 普通单元格格式
fn normal_format() -> &'static Format {
    static FORMAT: OnceLock<Format> = OnceLock::new();
    FORMAT.get_or_init(|| {
        Format::new()
            .set_align(FormatAlign::Left)
            .set_border(FormatBorder::Thin)
    })
}

/// BOM 服务实现
pub struct BomServiceImpl {
    pool: Arc<PgPool>,
}

impl BomServiceImpl {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    /// 构建导出的 Excel 工作簿
    async fn build_export_workbook(&self, bom_id: i64) -> Result<Workbook> {
        // 加载 BOM 和产品数据
        let (_bom, list) = match self.load_bom_with_products(bom_id).await? {
            Some(data) => data,
            None => return Err(anyhow::anyhow!("BOM not found")),
        };

        // 预计算所有索引（一次性计算，O(n) 复杂度）
        let indices = Self::build_export_indices(&list);

        // 按层级排序（使用预计算的索引）
        let ordered_indices = Self::sort_by_hierarchy_with_indices(&list, &indices);

        // 创建 Excel 工作簿
        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();

        // 设置列宽
        const COLUMN_WIDTHS: [f64; 8] = [8.0, 8.0, 15.0, 25.0, 8.0, 10.0, 15.0, 15.0];
        for (col, &width) in COLUMN_WIDTHS.iter().enumerate() {
            worksheet.set_column_width(col as u16, width)?;
        }

        // 写入表头
        const HEADERS: [&str; 8] = [
            "序号",
            "阶层",
            "物料编码",
            "产品名称",
            "用量",
            "位置",
            "备注",
            "物料属性",
        ];
        worksheet.set_row_height(0, 25.0)?;
        for (col, header) in HEADERS.iter().enumerate() {
            worksheet.write_string_with_format(0, col as u16, *header, header_format())?;
        }

        // 写入数据行
        for (row_num, &idx) in ordered_indices.iter().enumerate() {
            let row = (row_num + 1) as u32;
            let node = &list[idx];

            // 使用预计算的值
            let cell_format = if indices.is_top_level[idx] {
                top_level_format()
            } else if indices.has_children[idx] {
                parent_format()
            } else {
                normal_format()
            };
            let level = indices.levels[idx];

            worksheet.set_row_height(row, 20.0)?;
            worksheet.write_number_with_format(row, 0, (row_num + 1) as f64, cell_format)?;
            worksheet.write_number_with_format(row, 1, level as f64, cell_format)?;
            worksheet.write_string_with_format(
                row,
                2,
                &node.product.meta.product_code,
                cell_format,
            )?;
            worksheet.write_string_with_format(row, 3, &node.product.pdt_name, cell_format)?;
            worksheet.write_number_with_format(row, 4, node.node.quantity, cell_format)?;

            // 位置（可选字段）
            write_optional_string(
                worksheet,
                row,
                5,
                node.node.position.as_deref(),
                cell_format,
            )?;

            // 备注（可选字段）
            write_optional_string(worksheet, row, 6, node.node.remark.as_deref(), cell_format)?;

            // 物料属性（获取途径）
            worksheet.write_string_with_format(
                row,
                7,
                &node.product.meta.acquire_channel,
                cell_format,
            )?;
        }

        Ok(workbook)
    }

    /// 加载 BOM 数据并关联产品信息
    async fn load_bom_with_products(
        &self,
        bom_id: i64,
    ) -> Result<Option<(Bom, Vec<NodeWithProduct>)>> {
        // 获取 BOM（使用 pool 版本）
        let bom = match BomRepo::find_by_id_pool(&self.pool, bom_id).await? {
            Some(bom) => bom,
            None => return Ok(None),
        };

        // 获取所有产品 ID
        let product_ids: Vec<i64> = bom.bom_detail.nodes.iter().map(|n| n.product_id).collect();
        let products = ProductRepo::find_by_ids(&self.pool, &product_ids).await?;

        // 使用 HashMap 优化 O(n*m) 到 O(n+m)
        let product_map: HashMap<i64, &Product> =
            products.iter().map(|p| (p.product_id, p)).collect();

        // 关联节点和产品
        let list: Vec<NodeWithProduct> = bom
            .bom_detail
            .nodes
            .iter()
            .filter_map(|node| {
                product_map
                    .get(&node.product_id)
                    .map(|product| NodeWithProduct {
                        node: node.clone(),
                        product: (*product).clone(),
                    })
            })
            .collect();

        Ok(Some((bom, list)))
    }

    /// 构建导出所需的预计算索引
    fn build_export_indices(list: &[NodeWithProduct]) -> ExportIndices {
        let node_count = list.len();

        // 构建 ID -> 节点索引的映射（用于计算层级）
        let mut id_to_index: HashMap<i64, usize> = HashMap::with_capacity(node_count);
        for (idx, n) in list.iter().enumerate() {
            id_to_index.insert(n.node.id, idx);
        }

        // 构建父节点 -> 子节点列表的映射
        let mut parent_children: HashMap<i64, Vec<usize>> = HashMap::new();
        for (idx, n) in list.iter().enumerate() {
            parent_children
                .entry(n.node.parent_id)
                .or_default()
                .push(idx);
        }

        // 预计算是否为顶层节点
        let mut is_top_level = vec![false; node_count];
        let top_children = parent_children.get(&0).map(|v| v.as_slice()).unwrap_or(&[]);
        for &idx in top_children {
            is_top_level[idx] = true;
        }

        // 预计算是否有子节点
        let mut has_children = vec![false; node_count];
        for (idx, n) in list.iter().enumerate() {
            if parent_children.contains_key(&n.node.id) {
                has_children[idx] = true;
            }
        }

        // 预计算层级深度
        let mut levels = vec![1usize; node_count];
        for idx in 0..node_count {
            let mut level = 1;
            let mut current_id = list[idx].node.parent_id;
            while current_id != 0 {
                if let Some(&parent_idx) = id_to_index.get(&current_id) {
                    level += 1;
                    current_id = list[parent_idx].node.parent_id;
                } else {
                    break;
                }
            }
            levels[idx] = level;
        }

        ExportIndices {
            parent_children,
            is_top_level,
            has_children,
            levels,
        }
    }

    /// 按层级排序节点（广度优先遍历，使用预计算索引）
    fn sort_by_hierarchy_with_indices(
        list: &[NodeWithProduct],
        indices: &ExportIndices,
    ) -> Vec<usize> {
        let mut ordered_indices = Vec::with_capacity(list.len());

        // 获取顶层节点并按 order 排序
        let mut root_indices: Vec<usize> = indices
            .parent_children
            .get(&0)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
            .to_vec();
        root_indices.sort_by_key(|&idx| list[idx].node.order);

        // BFS 遍历
        let mut to_process: VecDeque<usize> = root_indices.into_iter().collect();

        while let Some(current_idx) = to_process.pop_front() {
            ordered_indices.push(current_idx);

            // 获取子节点并按 order 排序
            let current_id = list[current_idx].node.id;
            let mut children: Vec<usize> = indices
                .parent_children
                .get(&current_id)
                .map(|v| v.as_slice())
                .unwrap_or(&[])
                .to_vec();
            children.sort_by_key(|&idx| list[idx].node.order);
            to_process.extend(children);
        }

        ordered_indices
    }
}

/// 导出预计算的索引结构
struct ExportIndices {
    /// 父节点 ID -> 子节点索引列表
    parent_children: HashMap<i64, Vec<usize>>,
    /// 是否为顶层节点（按索引）
    is_top_level: Vec<bool>,
    /// 是否有子节点（按索引）
    has_children: Vec<bool>,
    /// 层级深度（按索引）
    levels: Vec<usize>,
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

        // 查找并更新节点
        if let Some(existing_node) = bom.bom_detail.nodes.iter_mut().find(|n| n.id == node.id) {
            *existing_node = node;
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

    async fn export_to_excel(&self, bom_id: i64, path: &Path) -> Result<()> {
        let mut workbook = self.build_export_workbook(bom_id).await?;
        workbook.save(path)?;
        Ok(())
    }

    async fn export_to_bytes(&self, bom_id: i64) -> Result<(Vec<u8>, String)> {
        // 加载 BOM 数据获取名称
        let (bom, _) = match self.load_bom_with_products(bom_id).await? {
            Some(data) => data,
            None => return Err(anyhow::anyhow!("BOM not found")),
        };
        let bom_name = bom.bom_name.clone();

        // 构建工作簿并导出
        let mut workbook = self.build_export_workbook(bom_id).await?;
        let bytes = workbook.save_to_buffer()?;
        Ok((bytes, bom_name))
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

/// 写入可选字符串到 Excel 单元格
fn write_optional_string(
    worksheet: &mut rust_xlsxwriter::Worksheet,
    row: u32,
    col: u16,
    value: Option<&str>,
    format: &Format,
) -> Result<()> {
    match value {
        Some(text) => {
            worksheet.write_string_with_format(row, col, text, format)?;
        }
        None => {
            worksheet.write_blank(row, col, format)?;
        }
    }
    Ok(())
}
