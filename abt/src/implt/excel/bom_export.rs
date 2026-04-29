//! BOM Excel 导出实现

use std::collections::{HashMap, VecDeque};
use std::sync::OnceLock;

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use rust_xlsxwriter::{Color, Format, FormatAlign, FormatBorder, Workbook};
use sqlx::PgPool;

use crate::models::{Bom, BomNode, Product};
use crate::repositories::{BomRepo, ProductRepo};
use crate::service::{ExcelExportService, ExportRequest};

/// BOM 导出列定义（schema-as-code）
pub const BOM_EXPORT_HEADERS: [&str; 8] = [
    "序号", "阶层", "物料编码", "产品名称", "用量", "位置", "备注", "物料属性",
];
const _: () = assert!(BOM_EXPORT_HEADERS.len() == 8);

// ============================================================================
// 格式化函数
// ============================================================================

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

fn parent_format() -> &'static Format {
    static FORMAT: OnceLock<Format> = OnceLock::new();
    FORMAT.get_or_init(|| {
        Format::new()
            .set_background_color("#FFFF00")
            .set_border(FormatBorder::Thin)
            .set_align(FormatAlign::Left)
    })
}

fn normal_format() -> &'static Format {
    static FORMAT: OnceLock<Format> = OnceLock::new();
    FORMAT.get_or_init(|| {
        Format::new()
            .set_align(FormatAlign::Left)
            .set_border(FormatBorder::Thin)
    })
}

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

// ============================================================================
// 内部结构
// ============================================================================

struct NodeWithProduct {
    node: BomNode,
    product: Product,
}

struct ExportIndices {
    parent_children: HashMap<i64, Vec<usize>>,
    is_top_level: Vec<bool>,
    has_children: Vec<bool>,
    levels: Vec<usize>,
}

pub struct BomExporter {
    pool: PgPool,
}

impl BomExporter {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// 导出 BOM 并同时返回文件名（避免 handler 层 N+1 查询 BOM 名称）
    pub async fn export_with_name(&self, bom_id: i64) -> Result<(Vec<u8>, String)> {
        let (bom, list) = match self.load_bom_with_products(bom_id).await? {
            Some(data) => data,
            None => return Err(anyhow!("BOM not found")),
        };
        let bom_name = bom.bom_name;
        let bytes = Self::build_workbook(&list)?;
        Ok((bytes, bom_name))
    }

    async fn load_bom_with_products(
        &self,
        bom_id: i64,
    ) -> Result<Option<(Bom, Vec<NodeWithProduct>)>> {
        let bom = match BomRepo::find_by_id_pool(&self.pool, bom_id).await? {
            Some(bom) => bom,
            None => return Ok(None),
        };

        let product_ids: Vec<i64> = bom.bom_detail.nodes.iter().map(|n| n.product_id).collect();
        let products = ProductRepo::find_by_ids(&self.pool, &product_ids).await?;

        let product_map: HashMap<i64, &Product> =
            products.iter().map(|p| (p.product_id, p)).collect();

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

    fn build_export_indices(list: &[NodeWithProduct]) -> ExportIndices {
        let node_count = list.len();

        let mut id_to_index: HashMap<i64, usize> = HashMap::with_capacity(node_count);
        for (idx, n) in list.iter().enumerate() {
            id_to_index.insert(n.node.id, idx);
        }

        let mut parent_children: HashMap<i64, Vec<usize>> = HashMap::new();
        for (idx, n) in list.iter().enumerate() {
            parent_children
                .entry(n.node.parent_id)
                .or_default()
                .push(idx);
        }

        let mut is_top_level = vec![false; node_count];
        let top_children = parent_children.get(&0).map(|v| v.as_slice()).unwrap_or(&[]);
        for &idx in top_children {
            is_top_level[idx] = true;
        }

        let mut has_children = vec![false; node_count];
        for (idx, n) in list.iter().enumerate() {
            if parent_children.contains_key(&n.node.id) {
                has_children[idx] = true;
            }
        }

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

    fn sort_by_hierarchy_with_indices(
        list: &[NodeWithProduct],
        indices: &ExportIndices,
    ) -> Vec<usize> {
        let mut ordered_indices = Vec::with_capacity(list.len());

        let mut root_indices: Vec<usize> = indices
            .parent_children
            .get(&0)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
            .to_vec();
        root_indices.sort_by_key(|&idx| list[idx].node.order);

        let mut to_process: VecDeque<usize> = root_indices.into_iter().collect();

        while let Some(current_idx) = to_process.pop_front() {
            ordered_indices.push(current_idx);

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

    fn build_workbook(list: &[NodeWithProduct]) -> Result<Vec<u8>> {
        let indices = Self::build_export_indices(list);
        let ordered_indices = Self::sort_by_hierarchy_with_indices(list, &indices);

        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();

        const COLUMN_WIDTHS: [f64; 8] = [8.0, 8.0, 15.0, 25.0, 8.0, 10.0, 15.0, 15.0];
        for (col, &width) in COLUMN_WIDTHS.iter().enumerate() {
            worksheet.set_column_width(col as u16, width)?;
        }

        worksheet.set_row_height(0, 25.0)?;
        for (col, header) in BOM_EXPORT_HEADERS.iter().enumerate() {
            worksheet.write_string_with_format(0, col as u16, *header, header_format())?;
        }

        for (row_num, &idx) in ordered_indices.iter().enumerate() {
            let row = (row_num + 1) as u32;
            let node = &list[idx];

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
                row, 2, &node.product.meta.product_code, cell_format,
            )?;
            worksheet.write_string_with_format(row, 3, &node.product.pdt_name, cell_format)?;
            worksheet.write_number_with_format(row, 4, node.node.quantity, cell_format)?;

            write_optional_string(worksheet, row, 5, node.node.position.as_deref(), cell_format)?;
            write_optional_string(worksheet, row, 6, node.node.remark.as_deref(), cell_format)?;

            worksheet.write_string_with_format(
                row, 7, &node.product.meta.acquire_channel, cell_format,
            )?;
        }

        Ok(workbook.save_to_buffer()?)
    }
}

#[async_trait]
impl ExcelExportService for BomExporter {
    type Params = i64;

    async fn export(&self, req: ExportRequest<Self::Params>) -> Result<Vec<u8>> {
        let bom_id = req.params;
        let (_bom, list) = match self.load_bom_with_products(bom_id).await? {
            Some(data) => data,
            None => return Err(anyhow!("BOM not found")),
        };
        Self::build_workbook(&list)
    }
}
