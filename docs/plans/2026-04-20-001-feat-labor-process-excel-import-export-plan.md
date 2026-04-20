---
title: "feat: Labor Process Excel Import/Export"
type: feat
status: active
date: 2026-04-20
origin: docs/superpowers/specs/2026-04-20-labor-process-excel-import-export-design.md
---

# feat: Labor Process Excel Import/Export

## Overview

为工序管理添加 Excel 批量导入/导出功能。用户可通过 Excel 文件批量创建和更新工序（名称、单价、备注），也可导出现有工序列表进行修改后重新导入。功能包括：中文名称规范化、Decimal 精度策略、ON CONFLICT 并发安全、逐行结果报告、可选的 dry-run 预览模式。

## Problem Frame

工序系统已完成三层模型重构（迁移 021），CRUD 功能已上线，但用户仍需手动逐条添加工序。对于需要管理几十到上百道工序的场景（如初始化或批量调价），Excel 导入是必需的效率工具。

## Requirements Trace

- R1. Excel 导入工序（upsert by name），支持名称规范化、精度验证、逐行错误报告
- R2. Excel 导出工序列表，格式与导入兼容（round-trip 保证）
- R3. Dry-run 预览模式，提交前查看影响范围
- R4. 并发安全（ON CONFLICT）
- R5. 独立 gRPC RPC（非共享端点字符串派发）

## Scope Boundaries

- 不涉及工序组（labor_process_group）的导入/导出
- 不涉及 BOM 劳务成本（bom_labor_cost）的导入/导出
- 不添加进度追踪（数据量小，不需要 AtomicUsize singleton）
- 不需要新的数据库迁移（无新表/列）
- 文件上传复用现有 UploadFile RPC（ImportLaborProcesses 只接收 file_path）

## Context & Research

### Relevant Code and Patterns

- **Excel 导入模式**: `abt/src/implt/product_excel_service_impl.rs` — calamine + RangeDeserializerBuilder + 中文表头反序列化
- **Excel 导出模式**: 同上 — rust_xlsxwriter Workbook + save_to_buffer()
- **gRPC 流式导出**: `abt-grpc/src/handlers/excel.rs` — ReceiverStream + tokio::mpsc + DownloadFileResponse
- **工序服务**: `abt/src/service/labor_process_service.rs` — async_trait, anyhow::Result
- **工序仓库**: `abt/src/repositories/labor_process_repo.rs` — sqlx::query!, sqlx::query_as, sqlx::QueryBuilder
- **工序 Handler**: `abt-grpc/src/handlers/labor_process.rs` — AppState, 事务, err_to_status
- **Proto 约定**: `proto/abt/v1/labor_process.proto` — Abt 前缀, PascalCase RPC, string 传 Decimal
- **错误处理**: `common/src/error.rs` — business_error() 用于验证, err_to_status() 用于基础设施错误

### Institutional Learnings

- **DB 并发安全** (docs/solutions/database-issues/): sqlx::QueryBuilder push_values 闭包内只有 push_bind，ON CONFLICT 需在 push_values 后用 builder.push() 追加
- **验证错误**: 使用 business_error() 避免日志噪音

## Key Technical Decisions

- **ON CONFLICT vs 先查后写**: 使用 INSERT ... ON CONFLICT (name) DO UPDATE，利用 UNIQUE 约束在 DB 层面保证并发安全（参见 origin: labor-process-database-concurrency-and-query-fixes-2026-04-19）
- **独立 RPC vs 共享端点**: 新建 ImportLaborProcesses / ExportLaborProcesses RPC，避免字符串派发的耦合（参见 origin: design spec §gRPC 接口设计）
- **无进度追踪**: 工序数据量小（几十到几百行），不需要 ProductExcelService 那样的 OnceLock + AtomicUsize 模式
- **列定义共享**: 定义常量数组 `LABOR_PROCESS_COLUMNS`，导出和导入共用，保证 round-trip 兼容

## Implementation Units

- [ ] **Unit 1: Proto 定义与代码生成**

**Goal:** 在 labor_process.proto 中添加 ImportLaborProcesses / ExportLaborProcesses RPC 和相关消息定义，运行 cargo build 生成 Rust 代码。

**Requirements:** R5

**Dependencies:** None

**Files:**
- Modify: `proto/abt/v1/labor_process.proto`
- Generated: `abt-grpc/src/generated/abt.v1.rs`（cargo build 自动生成）

**Approach:**
- 在 AbtLaborProcessService 中添加两个 RPC
- ImportLaborProcesses 接收 file_path + 可选 dry_run 标志
- ImportLaborProcessesResponse 包含 success/failure/skip 计数 + 逐行结果列表 + affected_bom_count
- ExportLaborProcesses 返回 stream DownloadFileResponse（从 excel.proto 导入）
- 每个 ImportLaborProcessResult 包含 row_number, process_name, operation, error_message
- operation 为枚举字符串: "created", "updated", "unchanged", "error"

**Patterns to follow:**
- `proto/abt/v1/labor_process.proto` — 现有消息命名约定（Request/Response/Proto 后缀）
- `proto/abt/v1/excel.proto` — DownloadFileResponse 定义（用于流式导出）

**Test scenarios:**
- Test expectation: none — proto 变更通过 `cargo build` 验证编译通过

**Verification:**
- `cargo build` 成功，生成的 abt.v1.rs 包含新消息和 RPC

---

- [ ] **Unit 2: Repository 批量 Upsert**

**Goal:** 在 labor_process_repo.rs 中添加批量 upsert 方法（ON CONFLICT）和全量查询方法（用于导出）。

**Requirements:** R1, R4

**Dependencies:** None

**Files:**
- Modify: `abt/src/repositories/labor_process_repo.rs`

**Approach:**
- 添加 `batch_upsert_labor_processes(executor, items: &[(String, Decimal, Option<String>)])` 方法
  - 使用 sqlx::QueryBuilder 构造 `INSERT INTO labor_process (name, unit_price, remark) VALUES ... ON CONFLICT (name) DO UPDATE SET unit_price = EXCLUDED.unit_price, remark = EXCLUDED.remark, updated_at = NOW()`
  - push_values 后用 builder.push() 追加 ON CONFLICT 子句
  - 注意：sqlx::QueryBuilder push_values 闭包内只有 push_bind
- 添加 `list_all_labor_processes(pool) -> Result<Vec<LaborProcess>>` 方法
  - 查询所有未删除工序，按 name 排序，用于导出

**Patterns to follow:**
- `abt/src/repositories/labor_process_repo.rs` — batch_insert_bom_labor_cost 的 sqlx::QueryBuilder 模式
- `abt/src/repositories/labor_process_repo.rs` — query_as 动态查询模式

**Test scenarios:**
- Happy path: 3 条新工序批量插入，验证全部创建成功
- Happy path: 1 条已存在 + 2 条新建混合 upsert，验证已存在的被更新、新的被创建
- Edge case: 空列表传入，不应报错
- Edge case: 精确并发 upsert 同名工序（ON CONFLICT 不报错）
- Integration: upsert 后查询验证数据一致性

**Verification:**
- `cargo test -p abt` 相关测试通过
- 方法签名与现有 repo 方法一致（Executor 参数、anyhow::Result 返回）

---

- [ ] **Unit 3: Service Trait 扩展 + 导出实现**

**Goal:** 在 LaborProcessService trait 中添加 import 和 export 方法签名，实现导出功能。

**Requirements:** R2

**Dependencies:** Unit 2（list_all_labor_processes）

**Files:**
- Modify: `abt/src/service/labor_process_service.rs`
- Modify: `abt/src/implt/labor_process_service_impl.rs`

**Approach:**
- 在 trait 中添加：
  - `async fn import_processes_from_excel(&self, pool: &PgPool, file_path: &str, dry_run: bool) -> Result<ImportResult>`
  - `async fn export_processes_to_bytes(&self, pool: &PgPool) -> Result<Vec<u8>>`
- 定义共享常量 `LABOR_PROCESS_COLUMNS: &[&str] = &["工序名称", "单价", "备注"]`
- 定义 `ImportResult` 结构体（success_count, failure_count, skip_count, results: Vec<ImportRowResult>, affected_bom_count）
- 定义 `ImportRowResult` 结构体（row_number, process_name, operation, error_message）
- 实现导出：
  - 调用 repo 的 list_all_labor_processes
  - 使用 rust_xlsxwriter 创建 Workbook
  - 用 LABOR_PROCESS_COLUMNS 写入表头
  - 逐行写入数据（name → write_string, unit_price → write_number via to_f64, remark → write_string）
  - save_to_buffer() 返回

**Patterns to follow:**
- `abt/src/implt/product_excel_service_impl.rs` — export_boms_without_labor_cost_to_bytes 的 rust_xlsxwriter 模式
- `abt/src/service/labor_process_service.rs` — 现有 trait 方法定义

**Test scenarios:**
- Happy path: 导出包含 3 条工序的 Excel，验证字节流非空且可被 calamine 解析
- Happy path: 导出的 Excel 表头为 ["工序名称", "单价", "备注"]
- Edge case: 无工序时导出空 Excel（仅表头）
- Integration: 导出 → 解析验证列名和数据一致性

**Verification:**
- `cargo test -p abt` 相关测试通过
- 导出的 Excel 可被重新导入（round-trip 格式兼容）

---

- [ ] **Unit 4: 导入实现（解析、规范化、验证、Upsert、Dry-run）**

**Goal:** 实现完整的导入逻辑，包含 calamine 解析、名称规范化、数据验证、批量 upsert、dry-run 预览、逐行结果报告。

**Requirements:** R1, R3, R4

**Dependencies:** Unit 2（batch_upsert_labor_processes）, Unit 3（trait 和 ImportResult 定义）

**Files:**
- Modify: `abt/src/implt/labor_process_service_impl.rs`

**Approach:**
- 实现 import_processes_from_excel：
  1. **解析**: 使用 calamine::open_workbook + RangeDeserializerBuilder::with_headers(LABOR_PROCESS_COLUMNS)
     - 定义 Deserialize 结构体 `LaborProcessRow { name: String, unit_price: Decimal, remark: Option<String> }`
     - 使用 serde rename 映射中文列名
     - 自定义 Decimal 反序列化器处理空字符串和格式问题
  2. **规范化**: 对每个解析后的 name 执行：
     - trim()
     - 全角空格 → 半角空格
     - 全角括号 → 半角括号
     - 零宽字符移除
  3. **验证**: 收集所有错误（不中断）
     - 名称不能为空
     - 单价 >= 0
     - 单价超出 6 位小数时银行家舍入 + 标记
     - 重复名称检测（同文件内）
  4. **分类**: 将有效行分为 to_insert（名称在 DB 中不存在）和 to_update（已存在）
     - 批量查询现有名称判断
  5. **Dry-run**: 如果 dry_run=true，跳过数据库写入，直接返回预览结果
  6. **执行**: 事务内调用 repo 的 batch_upsert_labor_processes
  7. **统计受影响 BOM**: 查询单价变更的工序被多少 BOM 引用（复用现有 affected_bom_count 逻辑）
  8. **返回**: ImportResult 包含逐行结果

**Patterns to follow:**
- `abt/src/implt/product_excel_service_impl.rs` — import_quantity_from_excel 的完整流程（解析→批量查询→事务写入→结果返回）
- `abt/src/implt/product_excel_service_impl.rs` — calamine RangeDeserializerBuilder + 自定义 Deserialize

**Test scenarios:**
- Happy path: 3 条新工序导入成功，返回 created × 3
- Happy path: 1 条已存在（更新）+ 2 条新建，返回 updated × 1 + created × 2
- Happy path: dry_run=true 不写入数据库，返回预览报告
- Edge case: 名称含全角括号/全角空格，规范化后正确匹配
- Edge case: 单价超出精度，舍入并标记
- Edge case: Excel 中有重复名称，报告为错误
- Error path: 空文件（仅表头），返回 success_count = 0
- Error path: 名称列为空的行，收集为错误不中断
- Error path: 单价为负数，拒绝并报告

**Verification:**
- `cargo test -p abt` 相关测试通过
- 导入后数据库数据与 Excel 数据一致（名称已规范化）

---

- [ ] **Unit 5: gRPC Handler 集成**

**Goal:** 在 LaborProcessHandler 中添加导入和导出的 handler 方法，确保 server.rs 注册正确。

**Requirements:** R1, R2, R5

**Dependencies:** Unit 1（proto 定义）, Unit 3（trait + 导出实现）, Unit 4（导入实现）

**Files:**
- Modify: `abt-grpc/src/handlers/labor_process.rs`
- Modify: `abt-grpc/src/server.rs`（如果需要更新注册，实际上 RPC 添加到现有 service 不需要新注册）

**Approach:**
- 导入 handler (import_labor_processes):
  - request.into_inner() 获取 file_path + dry_run
  - AppState::get().await → labor_process_service()
  - 调用 import_processes_from_excel(file_path, dry_run)
  - 将 ImportResult 转换为 proto ImportLaborProcessesResponse
  - 逐行结果映射为 repeated ImportLaborProcessResult
  - 错误映射: .map_err(error::err_to_status)
- 导出 handler (export_labor_processes):
  - AppState::get().await → labor_process_service()
  - 调用 export_processes_to_bytes()
  - 使用 ReceiverStream 模式流式返回（复用 excel.rs 的 chunk 分割模式）
  - 第一个消息发 metadata（文件名 + 大小 + MIME type）
  - 后续消息发 64KB chunks
- 权限装饰器: 使用 `#[require_permission(Resource::LaborProcess, Action::Write)]` 控制导入，`Action::Read` 控制导出

**Patterns to follow:**
- `abt-grpc/src/handlers/labor_process.rs` — 现有 handler 方法结构
- `abt-grpc/src/handlers/excel.rs` — DownloadExportFile 的 ReceiverStream 流式导出模式
- `abt-grpc/src/handlers/mod.rs` — EXCEL_MIME_TYPE 和 STREAM_CHUNK_SIZE 常量

**Test scenarios:**
- Happy path: 导入请求返回 ImportLaborProcessesResponse，包含正确的计数和逐行结果
- Happy path: 导出请求返回 stream，第一个消息为 metadata，后续为 chunks
- Integration: 完整的导入→导出 round-trip，数据一致
- Error path: 无效 file_path 返回 gRPC error status
- Error path: 验证失败的业务错误使用 business_error 映射

**Verification:**
- `cargo build` 编译通过
- `cargo test -p abt-grpc` 相关测试通过
- gRPC reflection 可发现新的 RPC 方法

## System-Wide Impact

- **Interaction graph**: 无回调、无中间件影响。新功能纯粹是新增 RPC，不影响现有工序 CRUD
- **Error propagation**: service 层 anyhow::Result → handler 层 err_to_status 映射，验证错误用 business_error
- **State lifecycle risks**: 单事务保证原子性，dry-run 不写入数据库。并发安全由 ON CONFLICT 保证
- **API surface parity**: 独立 RPC，不影响现有 AbtExcelService
- **Unchanged invariants**: 现有 ListLaborProcesses / CreateLaborProcess / UpdateLaborProcess 不受影响

## Risks & Dependencies

| Risk | Mitigation |
|------|------------|
| ON CONFLICT 与 sqlx::QueryBuilder 兼容性 | push_values 后用 push() 追加 ON CONFLICT 子句（已验证可行）|
| calamine 中文表头匹配失败 | 使用 RangeDeserializerBuilder::with_headers 精确匹配列名 |
| Excel 浮点精度丢失 | 自定义 Decimal 反序列化器 + 银行家舍入策略 |
| 大文件内存占用 | 工序数据量小（<1000 行），calamine 全量加载可接受 |

## Sources & References

- **Origin document:** [docs/superpowers/specs/2026-04-20-labor-process-excel-import-export-design.md](docs/superpowers/specs/2026-04-20-labor-process-excel-import-export-design.md)
- **Ideation:** [docs/ideation/2026-04-20-labor-process-excel-import-export-ideation.md](docs/ideation/2026-04-20-labor-process-excel-import-export-ideation.md)
- Related code: `abt/src/implt/product_excel_service_impl.rs` (Excel 模式参考)
- Related code: `abt/src/repositories/labor_process_repo.rs` (Repo 模式参考)
- Past learning: `docs/solutions/database-issues/labor-process-database-concurrency-and-query-fixes-2026-04-19.md`
- Past learning: `docs/solutions/developer-experience/silent-business-error-helper-2026-04-19.md`
