# Excel 导入导出服务统一设计

日期: 2026-04-29

## 背景

当前 Excel 导入导出逻辑分散在多个服务和实现文件中：

| 位置 | 功能 | 方式 |
|------|------|------|
| `ProductExcelService` | 产品库存/价格/安全库存导入 + 多种导出 | 独立 trait + 单例实现 |
| `LaborProcessService` | 工序导入/导出/无成本BOM导出 | trait 上附加 3 个 Excel 方法 |
| `LaborProcessDictService` | 工序字典导出 | trait 上附加 1 个 Excel 方法 |
| `BomService` | BOM 导出 | trait 上附加 2 个 Excel 方法 |

gRPC 层已有一个统一的 `AbtExcelService`（`proto/abt/v1/excel.proto`），但 handler 仅对接 `ProductExcelService`。

## 目标

- 抽取统一的 trait 抽象层，拆分导入/导出为两个独立 trait
- 每个具体操作为一个独立的轻量结构体（按操作拆分，而非按领域）
- 进度机制从全局单例改为 `Arc<AtomicUsize>`，handler 自行持有

## 核心 Trait

```rust
// abt/src/service/excel_service.rs

/// 统一导入结果
#[derive(Debug, Clone, Default)]
pub struct ImportResult {
    pub success_count: usize,
    pub failed_count: usize,
    pub errors: Vec<String>,
}

/// 导入进度，用于跨请求查询
#[derive(Debug, Clone, Default)]
pub struct ExcelProgress {
    pub current: usize,
    pub total: usize,
}

/// Excel 导入服务——每个实现对应一种导入操作
#[async_trait]
pub trait ExcelImportService: Send + Sync {
    async fn import_from_excel(&self, file_path: &str) -> Result<ImportResult>;
    fn progress(&self) -> ExcelProgress;
}

/// Excel 导出服务——每个实现对应一种导出操作
#[async_trait]
pub trait ExcelExportService: Send + Sync {
    async fn export_to_bytes(&self) -> Result<Vec<u8>>;
}
```

### 设计原则

- **导入/导出分离**：两种截然不同的操作，各自独立的 trait 避免强迫没有导入或导出功能的实现去填空方法
- **无参数导出**：`export_to_bytes()` 不接受参数——所有上下文（pool、product_code、bom_id 等）在构造时绑定
- **无默认实现**：`progress()` 必须显式提供，避免忘写进度导致客户端读取到假数据

## 实现映射

每个操作映射为一个独立结构体：

| 现有操作 | 新结构体 | 实现 trait |
|----------|----------|------------|
| 导入产品库存/价格 | `ProductInventoryImporter` | `ExcelImportService` |
| 导出全部产品 | `ProductAllExporter` | `ExcelExportService` |
| 导出无价格产品 | `ProductWithoutPriceExporter` | `ExcelExportService` |
| 导入工序 | `LaborProcessImporter` | `ExcelImportService` |
| 导出工序（按产品） | `LaborProcessExporter` | `ExcelExportService` |
| 导出无人工成本BOM | `BomsWithoutLaborCostExporter` | `ExcelExportService` |
| 导出工序字典 | `LaborProcessDictExporter` | `ExcelExportService` |
| 导出 BOM | `BomExporter` | `ExcelExportService` |
| 库位导入（后续） | `LocationImporter` | `ExcelImportService` |
| 库位导出（后续） | `LocationExporter` | `ExcelExportService` |

## 进度管理

不再使用全局单例，改为 handler 持有：

```rust
// handler 伪代码
struct ImportTask {
    current: Arc<AtomicUsize>,
    total: Arc<AtomicUsize>,
}

// handler 维护: Mutex<Option<ImportTask>>
// 导入开始前设置，结束后清空
// get_progress 读取该值
```

工厂函数返回实现 + 进度 handle：

```rust
// abt/src/lib.rs
pub fn get_product_inventory_importer(pool: &PgPool)
    -> (impl ExcelImportService, Arc<(AtomicUsize, AtomicUsize)>)
```

若后续需要支持多类型导入并发，可为 `GetProgressRequest` 添加 `import_type` 字段，不影响 trait 设计。

## 文件组织

```
abt/src/
  service/
    excel_service.rs           # trait + ImportResult + ExcelProgress（新建）
    product_excel_service.rs   # 删除（迁移到 excel/ 目录）
    labor_process_service.rs   # 移除 Excel 方法
    labor_process_dict_service.rs  # 移除 Excel 方法
    bom_service.rs             # 移除 Excel 方法

  implt/
    excel/
      mod.rs                   # re-export
      product_inventory_import.rs
      product_all_export.rs
      product_without_price_export.rs
      labor_process_import.rs
      labor_process_export.rs
      labor_process_dict_export.rs
      boms_no_labor_cost_export.rs
      bom_export.rs
    product_excel_service_impl.rs  # 删除
    labor_process_service_impl.rs  # 移除 Excel 方法，提取到 excel/
    labor_process_dict_service_impl.rs  # 移除 Excel 方法
    bom_service_impl.rs          # 移除 Excel 方法

  lib.rs                        # 移除 get_product_excel_service()，添加各 excel 工厂函数
```

## 迁移计划

### Phase 1 — 基础设施
1. 新建 `abt/src/service/excel_service.rs`，定义 trait、`ImportResult`、`ExcelProgress`
2. 更新 `abt/src/service/mod.rs`，导出新类型

### Phase 2 — 产品 Excel 拆分
3. 从 `product_excel_service_impl.rs` 提取三个实现到 `excel/` 目录
4. 更新 `lib.rs` 工厂函数，替换单例为 `Arc` 进度模式
5. 更新 gRPC handler `excel.rs` 适配新接口

### Phase 3 — 工序/BOM/字典 Excel 提取
6. 从各 service impl 中提取 Excel 逻辑到独立文件
7. 从各 trait 中移除 Excel 方法签名
8. 更新对应 gRPC handler 使用新的 Excel 实现

### Phase 4 — 清理
9. 删除旧文件 `product_excel_service_impl.rs`、`product_excel_service.rs`
10. 清理 `lib.rs` 中旧的全局单例

## 未决事项

- 库位 Excel 导入导出：有设计文档，后续按 Phase 2/3 模式添加即可
- gRPC proto 是否调整：当前不修改 proto，handler 内部通过 `export_type` 路由到不同实现
