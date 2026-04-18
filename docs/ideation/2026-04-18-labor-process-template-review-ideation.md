---
date: 2026-04-18
topic: labor-process-template-review
focus: 设计文档补充点审查
---

# Ideation: 工序模板设计文档补充点

## Codebase Context

ABT 是 Rust/gRPC/PostgreSQL BOM 管理系统。工序模板设计引入三层结构（组→分类→步骤）解耦人工成本管理。项目已有 DECIMAL(18,6) 精度约定、operator_id 审计约定、软删除模式、product_price_log 价格变更日志。权限系统已定义 `LABOR_PROCESS` 资源码。

## Ranked Ideas

### 1. DECIMAL(18,6) 替代 DECIMAL(12,2)
**Description:** 设计中的 DECIMAL(12,2) 与 migration 011 统一到 DECIMAL(18,6) 的约定不一致
**Rationale:** 精度不一致导致联合查询隐式类型转换；2位小数对按件计费可能不够
**Downsides:** 无
**Confidence:** 95%
**Complexity:** Low
**Status:** Explored — 已纳入设计文档

### 2. parent_id 用 NULL 替代 0 哨兵值
**Description:** parent_id = 0 无法建立外键约束，改为 NULL + REFERENCES 保证引用完整性
**Rationale:** 数据库层面保证步骤 parent_id 指向有效分类行，避免孤儿数据
**Downsides:** 查询条件从 parent_id = 0 变为 parent_id IS NULL
**Confidence:** 90%
**Complexity:** Low
**Status:** Explored — 已纳入设计文档

### 3. 分类删除必须检查子步骤引用
**Description:** 只有步骤被 bom_labor_process_ref 引用，删除分类时需递归检查所有子步骤
**Rationale:** 生产级数据损坏风险：直接删分类会留下孤儿步骤
**Downsides:** 无，必须修复
**Confidence:** 95%
**Complexity:** Low
**Status:** Explored — 已纳入设计文档

### 4. 添加 operator_id 审计追踪
**Description:** 三张新表均无 created_by/operator_id，与 CLAUDE.md 约定不一致
**Rationale:** 工序价格变更影响所有 BOM 成本，无审计无法追溯
**Downsides:** 需要价格变更日志机制（可后续添加）
**Confidence:** 85%
**Complexity:** Medium
**Status:** Explored — 已纳入设计文档（created_by 字段）

### 5. 明确 SetBomLaborProcess 切换语义 + step_id 校验
**Description:** 幂等替换语义 + repository 层校验 step_id 归属 group_id
**Rationale:** 防止不合法关联；切换工序组是真实业务场景
**Downsides:** 额外查询校验
**Confidence:** 90%
**Complexity:** Low
**Status:** Explored — 已纳入设计文档

### 6. BOM 列表页展示关联工序组信息
**Description:** BomResponse 增加 labor_process_group_id 和 labor_process_group_name 字段
**Rationale:** 前端列表页可直接看到工序配置状态，无需逐个查询
**Downsides:** 需修改现有 BOM proto 响应
**Confidence:** 85%
**Complexity:** Medium
**Status:** Explored — 已纳入设计文档

### 7. ListProcessGroups 搜索/过滤/分页
**Description:** ListProcessGroupsRequest 增加 keyword/page/page_size 参数
**Rationale:** proto 后改成本高；项目已有 ListBomsRequest 分页模式
**Downsides:** 无
**Confidence:** 80%
**Complexity:** Low
**Status:** Explored — 已纳入设计文档

## Rejection Summary

| # | Idea | Reason Rejected |
|---|------|-----------------|
| 1 | 软删除 deleted_at | 工序模板是管理级数据，被引用禁止删除已足够保护 |
| 2 | 并发竞态 FOR UPDATE | 重要但属实现细节，事务内先删后插本身是原子的 |
| 3 | 工序组重名约束 | 已通过 UNIQUE(name) 直接纳入 schema |
| 4 | 步骤跨分类移动 | YAGNI，未提出需求 |
| 5 | 批量创建 Item | 后续可加，不涉及 schema 变更 |
| 6 | 跨 Category 排序 Reorder | Swap 够用，Reorder 是优化 |
| 7 | 克隆工序组 | 低频操作，v1 不需要 |
| 8 | 错误信息设计 | 实现细节，遵循项目 gRPC 错误模式 |
| 9 | bom_labor_process_new 命名 | 已改名为 bom_labor_process_ref |
| 10 | 合并 Group/Item 为单一 Service | 架构偏好，两种都可 |
| 11 | 模板扩展到物料成本 | 过度设计，YAGNI |
| 12 | Department 作用域 | 无明确需求 |
| 13 | BOM 引用逻辑耦合 BomService | 可接受的耦合 |
| 14 | 索引策略补充 | 实现时按需添加 |
| 15 | 权限资源定义 | 实现时处理，LABOR_PROCESS 已定义 |

## Session Log
- 2026-04-18: Initial ideation — 29 candidates generated (3 agents), 7 survived, all adopted into design doc
