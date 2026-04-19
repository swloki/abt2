---
date: 2026-04-18
topic: labor-process-redesign
focus: labor process redesign — replacing bom_labor_process with three-layer model
mode: repo-grounded
---

# Ideation: 劳务工序重设计改进建议

## Grounding Context

**Codebase Context:** Rust BOM/Inventory 管理系统，gRPC API + PostgreSQL/sqlx，分层架构（proto → model → repository → service → impl → handler）。当前 `bom_labor_process` 是扁平结构，每个 BOM 单独配置劳务工序，价格变更需要手动更新所有 BOM。

**New Design:** 三层模型 — `labor_process`（工序主表）→ `labor_process_group`（工序组，JSONB process_ids）→ `bom_labor_cost`（per-BOM 工时/数量）。BOM 表增加 `process_group_id` 外键。价格从 master 实时获取，不在 bom_labor_cost 存储。

**Past Learnings:** 迁移安全模式（归档而非删除表，INSERT ON CONFLICT 代替 TRUNCATE）；Proto 枚举规范（proto 为单一事实来源）。无 JSONB 存储模式的历史文档。

**External Context:** 三层模型是 ERP 标准模式（Acumatica, Prism, SAP）；JSONB vs 连接表在数据完整性驱动决策时倾向连接表；实时取价 vs 快照是真实权衡；递归 BOM 成本卷积是图问题。

## Ranked Ideas

### 1. JSONB → 连接表 + 外键 + 排序

**Description:** 将 `labor_process_group.process_ids JSONB` 替换为 `labor_process_group_member (group_id BIGINT REFERENCES labor_process_group(id), process_id BIGINT REFERENCES labor_process(id), sort_order INT NOT NULL)` 连接表。PostgreSQL 外键自动保证引用完整性，删除工序时数据库级 RESTRICT 代替应用层扫描 JSONB。sort_order 让工序顺序成为一级公民。

**Rationale:** 多个 ideation frame 独立收敛到同一结论。JSONB 的灵活性在工序-组关系这个高度结构化的多对多场景中是不必要的。删除校验、孤儿检测等补偿逻辑的复杂度远超一张三列表。连接表还让"查询哪些组包含工序 X"这类反向查询变为标准 SQL。

**Downsides:** 增加一张表和一个 repository；查询从 JSONB 包含操作变为 JOIN。两者在数据量小的情况下性能差异可忽略。需要调整 proto 中 process_ids 的表示方式（repeated int64 → repeated ProcessGroupMember messages）。

**Confidence:** 95%
**Complexity:** 低
**Status:** Unexplored

### 2. bom_labor_cost 增加价格快照

**Description:** 在 `bom_labor_cost` 表中增加 `unit_price_snapshot DECIMAL(12,6)` 字段。在 SetBomLaborCost 时冻结当时的 master 单价到快照字段。GetBomLaborCost 同时返回快照价格和当前 master 价格，前端可标记差异（如颜色高亮 drift）。

**Rationale:** "永远不存储价格"是简化决策，但牺牲了审计能力。财务对账需要知道"当时的成本是多少"；报价历史对比需要可追溯的价格记录。这在订单系统（订单行 unit_price）中是标准做法。成本极低（一列），价值极高（历史可追溯）。

**Downsides:** 快照可能过时；前端需清晰区分"快照价"和"当前价"。需要在 GetBomLaborCost 的 proto response 中增加两个字段（snapshot_price, current_price）。

**Confidence:** 90%
**Complexity:** 低
**Status:** Unexplored

### 3. 价格变更时显示影响范围计数器

**Description:** 修改 `UpdateLaborProcess` 流程：当 `unit_price` 发生变更时，在事务内查询受影响的 BOM 数量和 bom_labor_cost 条目数量，随响应返回。操作员在提交价格变更前可看到影响范围（如"此变更将影响 47 个 BOM"）。

**Rationale:** 集中定价创造新的操作员焦虑："我改了什么？" 扁平模型中改一个 BOM 的价格影响有限且可见；三层模型中改 master 价格的 blast radius 不可预测。简单计数器几乎不增加实现成本，但极大降低操作员心理负担。

**Downsides:** 需要在 UPDATE 事务中做一次聚合查询（通过 bom_labor_cost JOIN labor_process_group_member）；计数可能因并发而不精确（可接受，因为是 advisory 而非 transactional）。

**Confidence:** 90%
**Complexity:** 低
**Status:** Unexplored

## Rejection Summary

| # | Idea | Reason Rejected |
|---|------|-----------------|
| 1 | Per-BOM price override (CSS cascade) | 用户评估后认为不需要，master 统一定价即可 |
| 2 | cost_source enum (standard/actual/estimated) | 用户评估后认为当前阶段不需要 |
| 3 | Generalize to manufacturing_resource | 用户评估后认为当前阶段保持 labor 专注即可 |
| 4 | Category-level default labor configuration | 用户评估后认为当前阶段不需要 |
| 5 | Versioned pricing history | #2 快照方案的过度工程版本 |
| 6 | Pluggable cost model / price formulas | 太投机，超出当前需求 |
| 7 | Event sourcing audit | 当前需求下过度工程 |
| 8 | Versioned diff updates | 小数据集 clear-then-insert 已足够 |
| 9 | Multiple groups per BOM | 改变核心数据模型太多 |
| 10 | Process group versioning | 增加显著复杂度 |
| 11 | Multi-dimensional pricing | 太投机 |
| 12 | Process group inheritance / skill tree | 复杂度过高 |
| 13 | Recursive cost rollup API | 功能请求，非设计改进 |
| 14 | Auto-group / auto-migration generation | 不在焦点范围内 |
| 15 | Process DAG | 有序列表已足够 |
| 16 | Template BOMs replacing process groups | 与已确定的设计方向矛盾 |
