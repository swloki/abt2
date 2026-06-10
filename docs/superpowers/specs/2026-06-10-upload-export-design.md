# 上传导出功能设计文档

> **日期**：2026-06-10
> **分支**：`feat/upload-export`
> **状态**：待审核

---

## 1. 目标与范围

### 目标

在产品、BOM、工艺、仓库库位四个业务模块的列表页上，提供导入（Excel 上传）和导出（Excel 下载）功能。后端 Excel 框架已实现，本次只需补齐前端交互入口和 Web handler 层。

### 范围

| 功能 | 说明 |
|------|------|
| 导入 | 产品库存、工艺路线、仓库库位 — 三种导入 |
| 导出 | 产品（含库存/不含价格）、BOM（含/不含人工成本）、分类、工序字典、工艺路线、仓库库位 — 八种导出 |
| 模板下载 | 每种导入对应一个 Excel 模板文件，用户先下载模板填写后再上传 |
| 进度监控 | 导入/导出任务的实时进度反馈，完成后可下载结果 |

### 不在范围内

- 新增后端导入/导出能力（已有实现）
- 新增 Excel Importer/Exporter
- 数据库 schema 变更
- 权限控制（使用现有权限体系，按钮级别通过 `has_permission` 控制）

---

## 2. 方案选型

**选定方案：HTMX 原生方案（方案 A）**

完全遵循项目现有 HTMX + Maud + Surreal.js 架构，不引入额外前端依赖。

选择理由：
1. 项目已有成熟的 HTMX 模式和组件化规范
2. 导入/导出核心逻辑是服务端的（解析 Excel、生成文件），前端主要是触发和展示
3. 文件拖拽区域用纯 CSS + 少量 Surreal.js 即可实现
4. 保持架构一致性

---

## 3. 总体架构

### 3.1 组件层次

```
列表页（product_list / bom_list / labor_process_dict_list / category_list / ...）
  ├── page-actions 按钮区
  │   ├── 导出按钮 / 导出下拉菜单 → HTMX POST → 启动导出任务
  │   └── 导入按钮 → Surreal.js 打开导入 Modal
  │
  └── ImportModal（通用 Maud 组件，页面底部声明，通过 hsAdd/hsRemove 控制 is-open）
      ├── 步骤 1：模板下载链接 + 文件选择区
      ├── 步骤 2：上传中 — 进度条（HTMX 轮询 hx-trigger="every 1s"）
      └── 步骤 3：结果展示 — 成功数/失败数/行级错误详情
```

### 3.2 后端路由

所有导入导出的 Web handler 集中在 `abt-web/src/routes/excel.rs`，使用 TypedPath 强类型路由：

| TypedPath | 方法 | 功能 |
|-----------|------|------|
| `ExcelImportModalPath` | GET | 返回导入 Modal 的 HTML 片段（含模板列信息） |
| `ExcelImportUploadPath` | POST | 接收 multipart 文件上传，启动导入任务，返回进度 HTML |
| `ExcelImportProgressPath` | GET | 查询导入进度，返回进度条 HTML（HTMX 轮询目标） |
| `ExcelExportStartPath` | POST | 启动导出，同步生成文件，返回下载链接 HTML |
| `ExcelExportDownloadPath` | GET | 下载导出文件（二进制流） |
| `ExcelTemplatePath` | GET | 下载导入模板文件（二进制流） |

> **注意**：当前所有 Exporter 都是同步的（直接返回 `Vec<u8>`），导出不需要异步进度轮询。导出按钮点击后，HTMX POST 到 `ExcelExportStartPath`，handler 同步调用 Exporter 生成文件，直接返回含下载链接的 HTML 片段。

### 3.3 数据流

**导入流程：**

```
用户点击"导入"按钮
  → Surreal.js 给 #import-modal 添加 is-open class
  → Modal 显示（初始状态：文件选择区）
  用户选择文件并点击"开始导入"
  → HTMX hx-post=ExcelImportUploadPath (multipart/form-data)
  → Handler 调用 abt-core Importer，获得 task_id
  → 返回"进行中"HTML 片段（含进度条 + hx-trigger="every 1s" 轮询）
  HTMX 自动每秒轮询 ExcelImportProgressPath
  → Handler 查询进度
  → 未完成：返回更新后的进度条 HTML（hx-swap="outerHTML" 替换自身）
  → 已完成：返回结果 HTML（成功数、失败数、错误详情），同时停止轮询
```

**导出流程（同步）：**

```
用户点击"导出"按钮（或下拉菜单中选择导出类型）
  → HTMX hx-post=ExcelExportStartPath
  → Handler 同步调用 abt-core Exporter 生成 Excel 字节数组
  → 将文件存入内存 ProgressStore，获得 task_id
  → 直接返回包含下载链接的 HTML 片段
  用户点击下载链接
  → GET ExcelExportDownloadPath
  → Handler 返回二进制文件流
```

> 导出是同步操作，不涉及进度轮询。对于数据量大的导出，可后续优化为异步。

---

## 4. 通用前端组件

### 4.1 ImportModalConfig 配置结构

```rust
/// 导入 Modal 的配置参数，各模块按需提供
pub struct ImportModalConfig {
    pub import_type: &'static str,          // 类型标识："product-inventory" / "labor-process" / "warehouse-location"
    pub title: &'static str,                // 标题："导入产品库存" / "导入工艺路线" / "导入仓库库位"
    pub template_columns: &'static str,     // 模板列说明，逗号分隔，显示在 Modal 中
}
```

### 4.2 import_modal 组件

通用导入 Modal 组件，遵循现有 `modal.rs` 模式：

```rust
pub fn import_modal(config: &ImportModalConfig) -> Markup
```

**Modal 内部结构（三个状态区域，通过 HTMX swap 切换）：**

```html
<div id="import-modal" class="modal-overlay">
  <div class="modal" style="max-width:560px">
    <!-- Head -->
    <div class="modal-head">
      <h2>导入产品库存</h2>
      <button onclick="hsRemoveClosest(this,'.modal-overlay','is-open')">×</button>
    </div>

    <!-- Body：包含一个可被 HTMX 替换的容器 -->
    <div class="modal-body">
      <div id="import-content" class="import-content">
        <!-- 初始状态：文件选择区 -->
        <div class="import-file-zone">
          <p class="import-cols">列格式：新编码, 旧编码, 物料名称, 库位编码, ...</p>
          <a href="/excel/template/product-inventory" class="btn btn-default">下载模板</a>
          <form hx-post="/excel/import/product-inventory"
                hx-target="#import-content"
                hx-swap="innerHTML"
                hx-encoding="multipart/form-data"
                hx-indicator="#import-content .htmx-indicator">
            <input type="file" name="file" accept=".xlsx,.xls" required>
            <button type="submit" class="btn btn-primary">开始导入</button>
            <div class="htmx-indicator">上传中...</div>
          </form>
        </div>
      </div>
    </div>
  </div>
</div>
```

**关键设计决策：**
- Modal 骨架（overlay + head）始终存在，只有 `#import-content` 内部被 HTMX 替换
- 文件选择区用纯 HTML `<input type="file">` + CSS 美化，不依赖 JS 拖拽库
- 进度条区域通过 `hx-trigger="every 1s"` 自动轮询，完成后返回不含轮询 trigger 的 HTML 自然停止

### 4.3 export_dropdown 组件

导出下拉按钮，适用于一个模块有多种导出类型的场景：

```rust
pub struct ExportItem {
    pub label: &'static str,       // "含库存产品" / "不含价格产品"
    pub export_type: &'static str, // "product-all" / "product-without-price"
}

pub fn export_dropdown(items: &[ExportItem]) -> Markup
```

**渲染效果：** 一个"导出"按钮，点击后展开下拉菜单，每项点击触发 HTMX POST 启动导出。

```html
<div class="export-dropdown">
  <button type="button" class="btn btn-default">
    <script>me().on('click', ev => { me(ev).nextElementSibling.classList.toggle('is-open') })</script>
    "导出"
  </button>
  <div class="export-dropdown-menu">
    <button type="button"
      hx-post="/excel/export/product-all"
      hx-target="#export-result"
      hx-swap="innerHTML">
      "含库存产品"
    </button>
    <button type="button"
      hx-post="/excel/export/product-without-price"
      hx-target="#export-result"
      hx-swap="innerHTML">
      "不含价格产品"
    </button>
  </div>
</div>
```

### 4.4 export_button 组件

单导出类型时使用，直接一个按钮：

```rust
pub fn export_button(label: &str, export_type: &str) -> Markup
```

### 4.5 导出结果区域

各列表页底部放一个隐藏的结果区域，导出完成后显示下载链接：

```html
<div id="export-result"></div>
```

导出进行中时，HTMX 将此区域替换为进度条 + 轮询；完成后替换为下载链接。

---

## 5. 后端 Handler 设计

### 5.1 TypedPath 定义

```rust
// 导入 Modal HTML 片段
#[derive(TypedPath, Deserialize)]
#[typed_path("/excel/import/:import_type/modal")]
pub struct ExcelImportModalPath { pub import_type: String }

// 导入上传
#[derive(TypedPath, Deserialize)]
#[typed_path("/excel/import/:import_type")]
pub struct ExcelImportUploadPath { pub import_type: String }

// 导入进度轮询
#[derive(TypedPath, Deserialize)]
#[typed_path("/excel/import/:import_type/progress/:task_id")]
pub struct ExcelImportProgressPath { pub import_type: String, pub task_id: i64 }

// 导出启动（同步，直接返回下载链接）
#[derive(TypedPath, Deserialize)]
#[typed_path("/excel/export/:export_type")]
pub struct ExcelExportStartPath { pub export_type: String }

// 导出文件下载
#[derive(TypedPath, Deserialize)]
#[typed_path("/excel/export/download/:task_id")]
pub struct ExcelExportDownloadPath { pub task_id: i64 }

// 模板下载
#[derive(TypedPath, Deserialize)]
#[typed_path("/excel/template/:import_type")]
pub struct ExcelTemplatePath { pub import_type: String }
```

### 5.2 Handler 分发

`ExcelImportUploadPath` handler 根据 `import_type` 分发到不同的导入逻辑：

```rust
async fn post_import_upload(
    path: ExcelImportUploadPath,
    session: Session,
    state: State<AppState>,
    multipart: Multipart,
) -> Result<Html<String>> {
    let bytes = extract_file_bytes(multipart).await?;
    let ctx = ServiceContext::from(session);
    let mut tx = state.pool.begin().await?;

    let task_id = match path.import_type.as_str() {
        "product-inventory" => {
            let importer = ProductInventoryImporter::new(state.pool.clone(), tracker);
            importer.import(&ctx, &mut *tx, ImportSource::Bytes(bytes)).await?
        }
        "labor-process" => { /* ... */ }
        "warehouse-location" => { /* ... */ }
        _ => return Err(DomainError::NotFound("未知的导入类型".into())),
    };

    tx.commit().await?;
    // 返回进度轮询 HTML 片段
    Ok(Html(render_import_progress(&path.import_type, task_id, 0, 0)))
}
```

> **注意**：具体签名需根据 `abt-core/src/shared/excel/` 中现有 Importer/Exporter 的实际接口调整。当前 Importer（如 `ProductInventoryImporter`）的 `import()` 方法直接返回 `ImportResult`，不返回 task_id。进度跟踪可能需要在 Web 层用内存 HashMap + `ProgressTracker` 实现，或利用已有的 `ExcelImportService` trait。

### 5.3 进度轮询 Handler

```rust
async fn get_import_progress(
    path: ExcelImportProgressPath,
    state: State<AppState>,
) -> Result<Html<String>> {
    let progress = state.get_import_progress(path.task_id).await?;
    let html = render_import_progress(
        &path.import_type,
        path.task_id,
        progress.current,
        progress.total,
    );
    Ok(Html(html))
}
```

轮询 HTML 片段的两种状态：

**进行中：**
```html
<div id="import-content">
  <div class="import-progress">
    <div class="progress-bar" style="width: 45%"></div>
    <p>正在导入... 45/100</p>
  </div>
  <!-- hx-trigger="every 1s" 保持轮询 -->
  <div hx-get="/excel/import/product-inventory/progress/123"
       hx-trigger="every 1s"
       hx-target="#import-content"
       hx-swap="innerHTML"></div>
</div>
```

**已完成：**
```html
<div id="import-content">
  <div class="import-result">
    <div class="import-success">
      <p>✓ 导入完成</p>
      <p>成功：85 条 / 失败：2 条</p>
    </div>
    @if has_errors {
      <div class="import-errors">
        <p>错误详情：</p>
        <ul>
          <li>第 3 行，列"新编码"：值为空 (必填字段)</li>
          <li>第 7 行，列"库存数量"：非数字 "abc"</li>
        </ul>
      </div>
    }
  </div>
  <!-- 无 hx-trigger="every 1s"，轮询自然停止 -->
</div>
```

### 5.4 模板下载 Handler

```rust
async fn get_template(
    path: ExcelTemplatePath,
    state: State<AppState>,
) -> Result<impl IntoResponse> {
    let (bytes, filename) = match path.import_type.as_str() {
        "product-inventory" => generate_template(
            "产品导入模板",
            &["新编码", "旧编码", "物料名称", "库位编码", "库存数量", "价格", "安全库存", "分类ID"],
        ),
        "labor-process" => generate_template(
            "工艺路线导入模板",
            &["产品编码", "工序编码", "工序名称", "单价", "数量", "排序", "备注"],
        ),
        "warehouse-location" => generate_template(
            "仓库库位导入模板",
            &["仓库编码", "仓库名称", "库位编码", "库位名称", "容量"],
        ),
        _ => return Err(DomainError::NotFound("未知的导入类型".into())),
    };

    Ok((
        [(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}.xlsx\"", filename),
        )],
        bytes,
    ))
}

/// 使用 calamine/calamine-writer 生成模板 Excel（表头 + 示例行）
fn generate_template(sheet_name: &str, columns: &[&str]) -> (Vec<u8>, String) {
    // 利用 abt-core/src/shared/excel/helpers.rs 的 write_headers
    // 生成只有表头的空 Excel 文件
}
```

### 5.5 导出 Handler

导出为同步操作，handler 直接调用 Exporter 生成文件，返回下载链接。

```rust
async fn post_export_start(
    path: ExcelExportStartPath,
    session: Session,
    state: State<AppState>,
    Form(params): Form<ExportForm>,  // 可选参数（如 bom_id、product_code）
) -> Result<Html<String>> {
    let pool = state.pool.clone();

    let (bytes, filename) = match path.export_type.as_str() {
        "product-all" => {
            let exporter = ProductAllExporter::new(pool);
            (exporter.export()?, "产品库存导出".into())
        }
        "product-without-price" => {
            let exporter = ProductWithoutPriceExporter::new(pool);
            (exporter.export()?, "无价格产品".into())
        }
        "bom" => {
            let bom_id = params.bom_id.ok_or_else(|| DomainError::Validation("缺少 bom_id".into()))?;
            let exporter = BomExporter::new(pool, bom_id);
            let (data, name) = exporter.export_with_name()?;
            (data, name)
        }
        "bom-no-labor-cost" => {
            let exporter = BomsNoLaborCostExporter::new(pool);
            (exporter.export()?, "缺少人工成本BOM".into())
        }
        "category" => {
            let exporter = CategoryExporter::new(pool);
            (exporter.export()?, "分类导出".into())
        }
        "labor-process-dict" => {
            let exporter = LaborProcessDictExporter::new(pool);
            (exporter.export()?, "工序字典导出".into())
        }
        "labor-process" => {
            let code = params.product_code.ok_or_else(|| DomainError::Validation("缺少产品编码".into()))?;
            let exporter = LaborProcessExporter::new(pool, code);
            (exporter.export()?, "工艺路线导出".into())
        }
        "warehouse-location" => {
            let exporter = WarehouseLocationExporter::new(pool);
            (exporter.export()?, "仓库库位导出".into())
        }
        _ => return Err(DomainError::NotFound("未知的导出类型".into())),
    };

    let task_id = state.store_export_file(bytes, &filename).await;
    Ok(Html(render_export_result(task_id, &filename)))
}

#[derive(Deserialize)]
pub struct ExportForm {
    pub bom_id: Option<i64>,
    pub product_code: Option<String>,
}
```

**导出结果 HTML 片段：**
```html
<div id="export-result" class="export-result">
  <p>✓ 导出完成</p>
  <a href="/excel/export/product-all/download/123"
     class="btn btn-primary">
    下载 "产品库存导出.xlsx"
  </a>
</div>
```

---

## 6. 各模块集成方案

### 6.1 产品模块（product_list.rs）

**位置**：`page-actions` 区域

```rust
div class="page-actions" {
    // 导入按钮
    button type="button" class="btn btn-default"
        onclick="hsAdd(null,'#import-modal','is-open')" {
        (icon::upload_icon("w-4 h-4"))
        "导入"
    }
    // 导出下拉菜单
    (export_dropdown(&[
        ExportItem { label: "含库存产品", export_type: "product-all" },
        ExportItem { label: "不含价格产品", export_type: "product-without-price" },
    ]))
    // 原有的"新建产品"按钮
    a href=(ProductCreatePath::PATH) class="btn btn-primary" { "新建产品" }
}

// 页面底部：导入 Modal + 导出结果区域
(import_modal(&ImportModalConfig {
    import_type: "product-inventory",
    title: "导入产品库存",
    template_columns: "新编码, 旧编码, 物料名称, 库位编码, 库存数量, 价格, 安全库存, 分类ID",
}))
div id="export-result" {}
```

### 6.2 BOM 模块（bom_list.rs）

**位置**：`page-actions` 区域

BOM 导出较特殊 — `BomExporter` 需要指定 `bom_id`（单条导出），而 `BomsNoLaborCostExporter` 是批量导出。

- **列表页**：导出下拉菜单提供"缺少人工成本的BOM"批量导出
- **详情页**：单条 BOM 导出按钮（已有 `BomDetailPath`）

BOM 单条导出需要 `bom_id` 参数，通过 `hx-vals='{"bom_id": 123}'` 传入请求。对应 `ExcelExportStartPath` handler 从表单数据中读取 `bom_id`。批量导出（缺少人工成本）不需要额外参数。

```rust
// bom_list.rs — 列表页
div class="page-actions" {
    (export_dropdown(&[
        ExportItem { label: "缺少人工成本BOM", export_type: "bom-no-labor-cost" },
    ]))
    a href=(BomCreatePath::PATH) class="btn btn-primary" { "新建BOM" }
}

// bom_detail.rs — 详情页
// 单条 BOM 导出按钮
(export_button("导出 BOM", &format!("bom?bom_id={}", bom_id)))
```

### 6.3 工艺模块

工艺模块涉及两个页面：工序字典（`labor_process_dict_list.rs`）和工艺路线（列表页）。

**工序字典页面**：
```rust
div class="page-actions" {
    (export_button("导出工序字典", "labor-process-dict"))
}
```

**工艺路线页面**：
```rust
div class="page-actions" {
    // 导入按钮
    button type="button" class="btn btn-default"
        onclick="hsAdd(null,'#import-modal','is-open')" {
        (icon::upload_icon("w-4 h-4"))
        "导入"
    }
    // 导出按钮（需要选择产品编码）
    (export_button("导出工艺路线", "labor-process"))
}

(import_modal(&ImportModalConfig {
    import_type: "labor-process",
    title: "导入工艺路线",
    template_columns: "产品编码, 工序编码, 工序名称, 单价, 数量, 排序, 备注",
}))
```

### 6.4 仓库库位模块

需要先确认列表页位置。仓库库位的导出可在仓库管理页面提供。

```rust
div class="page-actions" {
    button type="button" class="btn btn-default"
        onclick="hsAdd(null,'#import-modal','is-open')" {
        (icon::upload_icon("w-4 h-4"))
        "导入"
    }
    (export_button("导出库位", "warehouse-location"))
}

(import_modal(&ImportModalConfig {
    import_type: "warehouse-location",
    title: "导入仓库库位",
    template_columns: "仓库编码, 仓库名称, 库位编码, 库位名称, 容量",
}))
```

---

## 7. 文件结构

### 7.1 新增文件

```
abt-web/src/
├── routes/
│   └── excel.rs                          # 导入导出路由定义 + Handler
├── components/
│   ├── import_modal.rs                   # 通用导入 Modal 组件
│   └── export_button.rs                  # 通用导出按钮/下拉菜单组件
```

### 7.2 修改文件

```
abt-web/src/
├── components/mod.rs                     # 新增 pub mod import_modal; pub mod export_button;
├── routes/mod.rs                         # 新增 excel 路由注册
├── pages/
│   ├── product_list.rs                   # page-actions 添加导入/导出按钮 + Modal
│   ├── bom_list.rs                       # page-actions 添加导出下拉菜单
│   ├── bom_detail.rs                     # 添加单条 BOM 导出按钮
│   ├── labor_process_dict_list.rs        # page-actions 添加导出按钮
│   └── （工艺路线列表页）                # page-actions 添加导入/导出按钮 + Modal
```

---

## 8. 进度跟踪实现方案

现有 Importer（如 `ProductInventoryImporter`）直接返回 `ImportResult`，不支持异步进度。需要增加进度跟踪能力。

### 方案：内存 ProgressStore

在 `AppState` 中增加一个 `Arc<DashMap<i64, ImportProgress>>`，Web handler 在调用 Importer 前注册一个 task_id，Importer 处理过程中通过 `ProgressTracker` 更新进度，轮询 Handler 查询 DashMap 返回当前进度。

```rust
// state.rs 新增
pub struct AppState {
    // ... 现有字段
    pub import_progress: Arc<DashMap<i64, ImportTaskState>>,
    pub export_files: Arc<DashMap<i64, ExportFileInfo>>,
}

pub struct ImportTaskState {
    pub status: TaskStatus,    // Running / Completed / Failed
    pub current: usize,
    pub total: usize,
    pub result: Option<ImportResult>,
}

pub struct ExportFileInfo {
    pub filename: String,
    pub bytes: Vec<u8>,
    pub created_at: DateTime<Utc>,
}
```

**同步导入的处理方式：**
由于现有 Importer 是同步执行的（`import()` 调用直到完成才返回），进度跟踪的粒度有限。两种处理策略：

1. **简单方案**：导入请求用 `tokio::spawn_blocking` 在后台线程执行，前端显示"处理中"状态，完成后显示结果。不支持实时进度百分比。
2. **完整方案**：改造 Importer 使其接受 `ProgressTracker` 回调，每处理一行更新进度。当前 `ProductInventoryImporter` 已接受 `ProgressTracker` 参数，可以直接使用。

**选择完整方案**，因为 `ProgressTracker` 已有基础设施。

---

## 9. 错误处理

| 场景 | 处理方式 |
|------|----------|
| 文件格式错误（非 Excel） | Modal 内显示错误提示，不关闭 Modal |
| 导入数据校验失败（必填字段为空） | 结果中显示行级错误详情 |
| 导入部分成功 | 显示成功数 + 失败数 + 失败行详情 |
| 导出无数据 | 提示"无数据可导出" |
| 文件过大（>10MB） | 前端限制 + 后端校验，返回错误提示 |
| 网络中断 | HTMX 自动重试或显示错误状态 |

---

## 10. 实现顺序

1. **通用组件**：`import_modal.rs` + `export_button.rs` + CSS 样式
2. **后端路由**：`routes/excel.rs` — TypedPath + Handler 骨架 + 模板下载
3. **进度跟踪**：AppState 增加 ProgressStore，对接现有 ProgressTracker
4. **产品模块集成**：product_list.rs 添加按钮 + Modal，验证完整流程
5. **BOM 模块集成**：bom_list.rs + bom_detail.rs
6. **工艺模块集成**：labor_process_dict_list.rs + 工艺路线列表页
7. **仓库库位集成**：仓库管理页面
8. **设计文档同步**：更新 `docs/uml-design/` 中的 Excel 服务设计

---

## 11. 设计文档同步

实现完成后需更新以下设计文档：

- `docs/uml-design/00-shared-infrastructure.html` — 补充 Web handler 层的接口说明
- 如需新增 `abt-core` 接口，同步更新对应设计文档
