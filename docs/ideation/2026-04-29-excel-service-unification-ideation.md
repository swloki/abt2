---
date: 2026-04-29
topic: excel-service-unification
focus: docs/superpowers/specs/2026-04-29-excel-service-unification-design.md
mode: repo-grounded
---

# Ideation: Excel 服务统一化设计评审

## Grounding Context

**Codebase**: Rust workspace, 6-layer architecture (proto→model→repo→service trait→service impl→gRPC handler), 18 domain modules. Excel logic currently scattered across 4 service traits + impl files. `OnceLock<ProductExcelServiceImpl>` is the ONLY service not using standard `Arc<Pool>` DI — a structural anomaly.

**External research**: Industry consensus favors separate import/export traits (vantage-dataset), trait-parameter progress reporting with NoopReporter null object (bdk/gifski), and channel-based progress mapping to gRPC (sitrep).

**Learnings**: `OnceLock` fail-open is warned against in past solutions; business vs infrastructure error separation belongs at handler layer; proc-macro patterns exist for cross-cutting handler concerns (`#[require_permission]`).

## Ranked Ideas

### 1. ProgressTracker 作为独立关注点（从 trait 中移除 progress）

**Description:** 将 `progress()` 从 `ExcelImportService` trait 中移除。改为独立的 `ProgressTracker` 结构体（持有 `Arc<AtomicUsize>` 的 current/total），导入器通过构造时接收 `Arc<ProgressTracker>` 来报告进度，handler 持有同一个 Arc 用于 `GetProgress` RPC。

**Rationale:** 当前设计存在两个进度真源（trait 的 `progress()` 方法 + 工厂返回的 Arc）。5 个不同领域的类比（物流追踪、打印机 spooler、空中交通管制、考古发掘、交响乐分谱）独立收敛到同一结论：进度追踪不是导入业务逻辑，是执行基础设施。这消除了代码库中 `OnceLock` 单例的最后一个使用场景。

**Downsides:** 工厂函数不再返回元组；handler 需要维护 `HashMap<String, Arc<ProgressTracker>>` 替代 `Mutex<Option<ImportTask>>`。

**Confidence:** 90%
**Complexity:** Medium
**Status:** Unexplored

### 2. Proto 枚举替代字符串 dispatch

**Description:** 将 `DownloadExportFileRequest.export_type: string` 替换为 proto enum，handler 用 `match` 做编译期穷尽检查。

**Rationale:** 当前 handler `excel.rs:170` 的 `match req.export_type.as_str() { "products_without_price" => ..., _ => ... }` 在拼写错误时静默 fallthrough 到默认分支。代码库已大量使用 `oneof`（`UploadFileRequest.data`、`DownloadFileResponse.data`），这是已建立的模式。

**Downsides:** proto 变更需要客户端重新生成；spec 中原计划不改 proto。

**Confidence:** 85%
**Complexity:** Low
**Status:** Unexplored

### 3. Trait 方法接受 Context 参数（修复构造时绑定矛盾）

**Description:** 将 `export_to_bytes(&self)` 改为接受一个泛型或具体的 context 参数，携带请求级数据（如 `bom_id`、`product_code`）。无参导出（如工序字典）传空 context。

**Rationale:** 当前设计中 `BomExporter` 需要 `bom_id` 来自请求——构造时绑定意味着每个请求都要 new 一个 BomExporter，此时 struct 退化为函数调用的包装器。`LaborProcessExporter` 同理。构造时绑定对无参数导出适用，但对请求参数化的导出产生无意义的对象创建开销。

**Downsides:** trait 方法不再完全无参；"无参数导出"原则需要加限定条件。

**Confidence:** 80%
**Complexity:** Low
**Status:** Unexplored

### 4. FileSource 抽象：导入接受 bytes 而非 Path

**Description:** `import_from_excel(&self, source: ImportSource) -> Result<ImportResult>`，其中 `ImportSource` 是 `enum { Path(PathBuf), Bytes(Vec<u8>) }`。calamine 的 XLSX 支持 `Cursor<Vec<u8>>`（`Read + Seek`）。handler 成为唯一的文件系统边界，导入逻辑可纯内存测试。

**Rationale:** 路径安全校验在 `excel.rs:114-121` 和 `labor_process_service_impl.rs:98-103` 中重复。接受 bytes 将 upload-then-import 的 temp 文件写入→重新读取变成内存内操作。测试不再依赖真实文件系统。

**Downsides:** calamine 的 `open_workbook` 只接受 Path，需改用 `Xlsx::new(BufReader::new(Cursor::new(bytes)))`，需验证兼容性。

**Confidence:** 75%
**Complexity:** Medium
**Status:** Unexplored

### 5. Schema-as-Code：导入导出共享列定义

**Description:** 在 trait 实现上定义 Excel 列 schema（名称、类型、顺序）作为关联常量。导入和导出均从同一 schema 读取列名，保证 round-trip 一致性。schema 也可驱动文档自动生成。

**Rationale:** 当前产品导入和"无价格产品导出"使用完全相同的 8 列 header，但定义在两个不同位置（impl line 103 和 line 359）。如果一方修改列名另一方不跟着改，导出的模板就无法重新导入。这是 2020 年 ideation 中以 95% 置信度标记但从未实现的想法。

**Downsides:** 为每个实现增加少量样板代码（定义 schema 常量）。对不同格式（导入列 vs 导出列不完全相同时）需要处理映射。

**Confidence:** 90%
**Complexity:** Low
**Status:** Unexplored

### 6. 注册表模式替代 handler 硬编码路由

**Description:** 每个导入/导出实现在构造时向全局注册表注册自己（操作名→工厂函数）。handler 的 `download_export_file` 变成一次注册表查找 + 调用，不再维护 `match export_type` 分支。新增操作只需创建 struct + trait impl + 一行注册，handler 代码不动。

**Rationale:** spec 规划了 10 个操作，后续还有库位等模块加入。每增加一个操作都要在 handler 中加 match 分支，这是 O(n) 的维护负担。注册表让新增操作变成 O(1)，且支持运行时自省（列出所有可用导出类型）。

**Downsides:** 引入全局注册表（类似 `OnceLock<HashMap>`），需处理重复注册、未注册类型的错误消息。组合 #2（proto enum）时注册 key 即为 enum variant。

**Confidence:** 80%
**Complexity:** Medium-High
**Status:** Unexplored

## Rejection Summary

41 ideas rejected from 47 raw candidates. Key rejection reasons:

| Category | Count | Examples |
|----------|-------|----------|
| Subsumed by stronger idea | 8 | Dual progress tracking, single Mutex bottleneck, medical lab catalog |
| Contradicts confirmed design | 6 | Domain-split traits, keep in domain services, drop traits for functions |
| Out of scope for unification | 5 | Dry-run combinator, FDW preview, batch partial commit |
| Premature optimization | 4 | Pull-based streaming, async spooler, resource-aware scheduling |
| Cosmetic / general hygiene | 4 | Arc tuple naming, macro factory functions, file-per-op convention |
| Too radical a departure | 5 | Closure registration, Excel as renderer, single-trait consolidation |
| Proto change required (for this round) | 1 | Streaming progress response |
| Weaker articulation of survivor | 8 | Various progress/file-source ideas absorbed into survivors |
