# 仓库库位 Excel 导入导出设计

## 概述

为仓库管理模块添加 Excel 导入导出功能，支持仓库及其库位的批量导入（按编码 upsert）和全量导出。

## Excel 格式

扁平结构，一行一个库位，仓库信息冗余：

| 仓库编码 | 仓库名称 | 库位编码 | 库位名称 | 容量 |
|---------|---------|---------|---------|------|
| WH-01 | 主仓库 | A-01 | 货架A区 | 100 |
| WH-01 | 主仓库 | A-02 | 货架B区 | 200 |

## 导入逻辑

两阶段处理：

**阶段一：解析与验证** — 遍历 Excel 行，将每行解析为结构化数据：
- 仓库编码为空 → 跳过该行，记录错误
- 库位编码为空 → 跳过该行，记录错误
- 容量非数字 → 跳过该行，记录错误
- 解析有效的行进入待处理列表
- **仓库名称一致性检查**：按 `warehouse_code` 分组，检查组内所有 `warehouse_name` 是否一致。若存在差异（如"主仓库"vs"主仓厍"），报阻塞性错误，防止静默覆盖
- **编码变更检测**：`warehouse_code` 未找到但 `warehouse_name` 匹配已有仓库，或 `location_code` 未找到但 `location_name` 匹配已有库位，报"疑似重命名"警告而非静默新建

**阶段二：批量 upsert** — 在一个事务内逐行执行：

1. 按 `仓库编码` 查找仓库（`warehouse.code`）：
   - 已存在 → 更新 `warehouse_name`
   - 不存在 → 新建仓库（status = active）
   - **软删除冲突处理**：若编码被软删除记录占用，将错误上报并停止导入，避免唯一约束冲突导致事务回滚
2. 在该仓库下按 `库位编码` 查找库位（`location.code`，仓库内唯一）：
   - 已存在 → 更新 `location_name`, `capacity`
   - 不存在 → 新建库位

事务内的数据库错误会导致整批回滚。

### 可选增强：Reconciliation 模式

通过 `import_type` 参数传递 `sync_mode` 标志（proto 字段 `optional bool sync_mode = 4`）。开启后，在 upsert 完成后，对文件中涉及的每仓库，额外执行：

```sql
-- 将文件中未出现的库位软删除
UPDATE location SET deleted_at = NOW()
WHERE warehouse_id IN (涉及的仓库列表)
  AND location_code NOT IN (文件中出现的库位编码列表)
  AND deleted_at IS NULL
```

安全措施：限制单次最多删除行数（如 20%），超限时报错而非静默执行。

## 导出逻辑

```sql
SELECT w.warehouse_code, w.warehouse_name,
       l.location_code, l.location_name, l.capacity
FROM warehouse w
LEFT JOIN location l ON w.warehouse_id = l.warehouse_id AND l.deleted_at IS NULL
WHERE w.deleted_at IS NULL
ORDER BY w.warehouse_code, l.location_code
```

## 接口复用

不新增 gRPC 方法，沿用现有 `AbtExcelService`：

- **导入**：上传文件 → `ImportExcel(import_type="warehouse_location")` → `WarehouseLocationImporter` 处理
- **导出**：`DownloadExportFile(export_type="warehouse_location")` → `WarehouseLocationExporter` 生成 → 流式下载

### 类型安全改进

将 `import_type` 和 `export_type` 从字符串改为 Rust `enum`：

```rust
#[non_exhaustive]
pub enum ImportType {
    ProductInventory,
    WarehouseLocation,
}

#[non_exhaustive]
pub enum ExportType {
    ProductsWithoutPrice,
    ProductAll,
    WarehouseLocation,
}
```

消除 key 碰撞 bug（两个并发导入用同一 type 字符串会覆盖彼此的进度追踪器），并提供编译时穷举检查。Handler 中的 match 语句变为穷举匹配，每次新增变体时编译器强制要求处理所有匹配位置。

## 新增文件

| 路径 | 说明 |
|------|------|
| `abt/src/implt/excel/warehouse_location_import.rs` | `WarehouseLocationImporter` — 实现 `ExcelImportService` |
| `abt/src/implt/excel/warehouse_location_export.rs` | `WarehouseLocationExporter` — 实现 `ExcelExportService` |

## 改动文件

| 文件 | 改动 |
|------|------|
| `proto/abt/v1/excel.proto` | `ImportExcelRequest` 增加 `string import_type = 3` 和 `optional bool sync_mode = 4`；新增 `RowError` message，`ImportResultResponse` 增加 `repeated RowError row_errors` |
| `abt/src/implt/excel/mod.rs` | 注册新模块，导出新类型 |
| `abt-grpc/src/handlers/excel.rs` | `import_excel` 增加 `warehouse_location` 分支并支持 sync_mode；`download_export_file` 增加 `warehouse_location` 分支；切换为类型化枚举调度 |
| `abt/src/service/excel_service.rs` | 新增 `ImportType` 和 `ExportType` 枚举定义；新增 `RowError` 结构体 |

## 代码结构

### WarehouseLocationImporter

```rust
pub struct WarehouseLocationImporter {
    pool: PgPool,
    tracker: Arc<ProgressTracker>,
}

impl ExcelImportService for WarehouseLocationImporter {
    async fn import(&self, source: ImportSource) -> Result<ImportResult>;
}

// Excel 行结构
struct ExcelRow {
    仓库编码: String,
    仓库名称: String,
    库位编码: String,
    库位名称: Option<String>,
    容量: Option<i32>,
}
```

### WarehouseLocationExporter

```rust
pub struct WarehouseLocationExporter {
    pool: PgPool,
}

impl ExcelExportService for WarehouseLocationExporter {
    type Params = ();

    async fn export(&self, req: ExportRequest<()>) -> Result<Vec<u8>>;
}
```

### 结构化错误类型

```rust
#[derive(Debug, Clone)]
pub struct RowError {
    pub row_index: usize,       // Excel 行号（1-based）
    pub column_name: String,    // 列名（如"仓库编码"、"容量"）
    pub reason: String,         // 错误原因
    pub raw_value: Option<String>, // 原始值
}
```

## 错误处理

- **解析错误**（格式问题）：跳过问题行，以结构化 `RowError`（含行号、列名、原始值）记录错误信息，继续处理剩余行
- **仓库名称不一致**：阻塞性错误，终止导入
- **数据库错误**：整批回滚，向上层返回错误
- **软删除编码冲突**：在 Phase 1 检查并报明确错误，避免 DB 阶段唯一约束冲突
- **编码变更检测**：报告"疑似重命名"警告，由用户决定是否继续
- 最终返回 `ImportResult`：成功行数 + 失败行数 + 错误详情列表（含结构化 `RowError`）
