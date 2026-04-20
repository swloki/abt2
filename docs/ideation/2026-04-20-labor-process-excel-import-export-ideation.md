---
date: 2026-04-20
topic: labor-process-excel-import-export
focus: E:/work/abt/docs/superpowers/specs/2026-04-20-labor-process-excel-import-export-design.md
mode: repo-grounded
---

# Ideation: 工序 Excel 导入导出改进

## Grounding Context

ABT 是 Rust gRPC + PostgreSQL BOM 管理系统，工序系统刚完成三层模型重设计（labor_process → labor_process_group → bom_labor_cost）。当前设计是在 LaborProcessService 中添加 Excel 导入/导出，upsert by name，3 个字段（name, unit_price, remark）。历史经验：DB 并发竞态条件（SELECT FOR UPDATE 解决）、N+1 查询优化、business_error() 验证模式。

## Ranked Ideas

### 1. 结构化导入结果 + Dry-run 预览
**Description:** 导入完成后返回逐行结果（第 N 行：已创建 / 已更新 / 跳过），包含行号和字段级错误。添加可选的 `dry_run` 标志——为 true 时只验证和模拟，不写入数据库，返回预览报告。
**Rationale:** 用户最常见的痛点是导入失败后不知道哪里出了问题。dry-run 让用户在提交前看到影响范围，特别是在单价变更会级联影响 BOM 成本快照的场景下。
**Downsides:** 增加实现复杂度（两遍处理逻辑、结构化错误格式）。
**Confidence:** 85%
**Complexity:** Medium
**Status:** Unexplored

### 2. 中文名称规范化
**Description:** 导入解析后对"工序名称"执行规范化：去除首尾空白、全角空格→半角空格、全角括号→半角括号、零宽字符移除。规范化后再进行 upsert 匹配。
**Rationale:** product import 历史上已记录的痛点。中文 Excel 中的全半角差异是数据脏化的高频来源——"组装工艺(人工)"和"组装工艺（人工）"会创建幽灵重复记录。
**Downsides:** 规范化规则可能有边界情况。需要与数据库中已有数据的规范化保持一致。
**Confidence:** 90%
**Complexity:** Low
**Status:** Unexplored

### 3. 单价精度策略
**Description:** 明确 unit_price 的精度处理策略：Decimal(18,6) 存储，导入时对超出精度的值执行银行家舍入，对超出 6 位小数的值发出警告。拒绝负数和 NaN。
**Rationale:** 单价错误是财务级问题。BOM 系统中 labor cost 直接影响成本核算。Excel 的浮点表示与数据库的 Decimal 精度之间的不匹配是静默错误的来源。
**Downsides:** 需要在解析阶段明确处理，增加验证逻辑。
**Confidence:** 80%
**Complexity:** Low
**Status:** Unexplored

### 4. 导出-导入往返保证
**Description:** 确保导出和导入共享同一个 schema 定义（不是两个独立的硬编码列名列表）。导出的 Excel 可以直接重新导入而不需要用户修改格式。
**Rationale:** 用户最常见的使用模式是"导出 → 修改几个单价 → 重新导入"。如果 round-trip 不工作，功能价值大幅缩水。
**Downsides:** 无显著缺点。纯实现规范。
**Confidence:** 95%
**Complexity:** Low
**Status:** Unexplored

### 5. 并发导入安全（ON CONFLICT）
**Description:** 利用 labor_process 表 name 列的 UNIQUE 约束，使用 `INSERT ... ON CONFLICT (name) DO UPDATE SET ...` 进行批量 upsert。在事务内执行。
**Rationale:** 项目中已知的、已踩过的坑（price snapshot 竞态条件）。单事务 + ON CONFLICT 既解决并发问题又提供原子性。
**Downsides:** 不支持"部分成功"——任一行失败回滚全部。
**Confidence:** 88%
**Complexity:** Low
**Status:** Unexplored

### 6. 专用 Proto RPC vs 共享字符串派发
**Description:** 考虑为工序导入/导出创建独立的 ImportLaborProcesses / ExportLaborProcesses RPC 方法，而非通过 import_type 字符串在共享端点中派发。
**Rationale:** 字符串派发是耦合磁铁——每个新的导入类型都在已有代码中添加分支。专用 RPC 使功能自包含、独立测试。
**Downsides:** 增加 proto 定义量。如果未来只有 2-3 种导入类型，共享端点也可以接受。
**Confidence:** 70%
**Complexity:** Low
**Status:** Unexplored

## Rejection Summary

| # | Idea | Reason Rejected |
|---|------|-----------------|
| 1 | Memory spike / progress blackout | 工序数据量小，已明确不需要进度追踪 |
| 2 | Exported data lacks context | 用户已确认 3 字段范围 |
| 3 | Sync vs import 架构重构 | 超出当前需求范围 |
| 4 | 工序组才是真正的导入单元 | 用户已明确范围为 labor_process 表 |
| 5 | 服务器不应处理 Excel | 与现有代码库架构矛盾 |
| 6 | Remark 字段暗示数据模型不完整 | 推测性过强，不可操作 |
| 7 | 通用批量操作基础设施 | YAGNI — 过早抽象 |
| 8 | 导出不需要自己的 RPC | 与现有 DownloadExportFile 模式冲突 |
| 9 | Direct gRPC Stream Upsert | 用户明确要求 Excel 工作流 |
| 10 | Export-as-You-Edit | 过度设计 |
| 11 | Auto-Sync webhook | 无上游系统，超出范围 |
| 12 | Clipboard/Paste 导入 | 用户要求 Excel 工作流 |
| 13 | Undo via DB snapshot | 设计已使用单事务，已覆盖 |
| 14 | Schema 自动生成模板 | 3 个字段的过度工程 |
| 15 | Terraform plan-apply | 对 3 字段表过于复杂 |
| 16 | Content hashing lockfile | 过度工程 |
| 17 | Patch/Diff 格式 | 对 3 字段表过度复杂 |
| 18 | Idempotency key | 对此用例过度工程 |
| 19 | Merge strategy selection | 用户已确认 upsert 语义 |
| 20 | Name is not a natural key | 数据模型关注，不在导入导出范围 |
| 21 | No import deduplication | upsert by name 本身是幂等的 |
| 22 | No row-level error attribution | 已合并到幸存者 #1 |
| 23 | Fuzzy character mismatch | 已合并到幸存者 #2 |
| 24 | Concurrent import race condition | 已合并到幸存者 #5 |
