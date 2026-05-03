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

**阶段二：批量 upsert** — 在一个事务内逐行执行：

1. 按 `仓库编码` 查找仓库（`warehouse.code`）：
   - 已存在 → 更新 `warehouse_name`
   - 不存在 → 新建仓库（status = active）
2. 在该仓库下按 `库位编码` 查找库位（`location.code`，仓库内唯一）：
   - 已存在 → 更新 `location_name`, `capacity`
   - 不存在 → 新建库位

事务内的数据库错误会导致整批回滚。

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

## 新增文件

| 路径 | 说明 |
|------|------|
| `abt/src/implt/excel/warehouse_location_import.rs` | `WarehouseLocationImporter` — 实现 `ExcelImportService` |
| `abt/src/implt/excel/warehouse_location_export.rs` | `WarehouseLocationExporter` — 实现 `ExcelExportService` |

## 改动文件

| 文件 | 改动 |
|------|------|
| `proto/abt/v1/excel.proto` | `ImportExcelRequest` 增加 `string import_type = 3` |
| `abt/src/implt/excel/mod.rs` | 注册新模块，导出新类型 |
| `abt-grpc/src/handlers/excel.rs` | `import_excel` 增加 `warehouse_location` 分支；`download_export_file` 增加 `warehouse_location` 分支 |

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

## 错误处理

- **解析错误**（格式问题）：跳过问题行，记录错误信息，继续处理剩余行
- **数据库错误**：整批回滚，向上层返回错误
- 最终返回 `ImportResult`：成功行数 + 失败行数 + 错误详情列表
