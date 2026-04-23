---
date: 2026-04-22
topic: labor-process-routing
focus: Improving the labor process routing design spec
mode: repo-grounded
---

# Ideation: 劳务工序路线设计改进

## Grounding Context

ABT 是一个 Rust/gRPC/PostgreSQL BOM 管理系统。当前 `bom_labor_process` 表是扁平模型，没有校验基准。已设计路由系统（`labor_process_dict`、`routing`、`routing_step`、`bom_routing`）来防止工序缺失。

关键约束：避免数据库外键（应用层校验）；sqlx QueryBuilder 只有 push_bind；handler 使用三层错误处理。

## Ranked Ideas

### 1. Excel 导入时自动生成临时路线
**Description:** 当导入 Excel 时产品尚未绑定路线，自动创建一个包含 Excel 中所有工序的"临时路线"，标记为待审核。后续导入时用此路线做校验，人工审核后转为正式路线。
**Rationale:** 直接解决"先有路线才能导入"的鸡生蛋问题。让路线成为导入的副产品而非前提条件。
**Downsides:** 需要路线状态管理（临时 vs 正式），可能产生大量待审核路线。
**Confidence:** 90%
**Complexity:** Medium
**Status:** Explored

### 2. 自动从历史数据生成工艺路线
**Description:** 分析现有 `bom_labor_process` 记录，自动生成初始 routing 和 routing_step 模板。
**Rationale:** 解决冷启动问题——新系统上线时没有路线数据。
**Downsides:** 需要数据清洗逻辑，自动生成的路线需要人工确认。
**Confidence:** 85%
**Complexity:** Medium
**Status:** Unexplored

### 3. 导入前"关键暂停"检查清单
**Description:** Excel 导入确认前，显示摘要页面：必须工序✓、缺失工序✗、多余工序⚠。
**Rationale:** 防止用户跳过警告直接导入。
**Downsides:** 增加导入步骤。
**Confidence:** 85%
**Complexity:** Low
**Status:** Unexplored

### 4. 四表启动引导工具
**Description:** 提供引导式向导，从现有数据快速填充工序字典、创建路线、分配到产品。
**Rationale:** 减少初始配置摩擦。
**Downsides:** 需要额外 UI 设计。
**Confidence:** 80%
**Complexity:** Medium
**Status:** Unexplored

### 5. 路线差异与迁移工具
**Description:** 工序变更时自动传播到所有受影响的 routing 和 routing_step。
**Rationale:** 避免手动更新数百条路线。
**Downsides:** 增加系统复杂度。
**Confidence:** 75%
**Complexity:** High
**Status:** Unexplored

### 6. 路线模板继承
**Description:** 路线模板支持继承，基础路线可被子路线扩展。
**Rationale:** 避免路线模板组合爆炸。
**Downsides:** 增加路线解析复杂度。
**Confidence:** 70%
**Complexity:** High
**Status:** Unexplored

### 7. 工序字典作为通用参考
**Description:** `labor_process_dict` 作为全系统的工序标准参考，质量检验、成本估算等模块都可引用。
**Rationale:** 投资回报远超路线功能本身。
**Downsides:** 需要跨模块协调。
**Confidence:** 65%
**Complexity:** Low
**Status:** Unexplored

## Rejection Summary

| # | Idea | Reason Rejected |
|---|------|-----------------|
| F2.2 | Process Code Fuzzy Matching | 增加脆弱复杂性，要求正确编码更简单 |
| F2.3 | DB Trigger Shadow Validation | 与应用层校验模式矛盾 |
| F2.4 | Category-based Auto-Insertion | 无产品分类系统支撑 |
| F2.6 | Optimistic Locking for Routes | 对用户规模过度设计 |
| F2.7 | BOM Explosion Mining | 过于推测性 |
| F6.1 | Process Before Product | 对现有流程干扰太大 |
| F6.2 | Versioned Routing | 过度设计，核心问题是缺失而非演变 |
| F6.4 | Routing DSL | 极度过度设计 |
| F6.5 | Blocklist Routing | 不解决核心问题 |
| F6.7 | Anomaly Detection | 数据量不足支撑 |
| F6.8 | Per-Product Baselines | 已被当前方案覆盖 |
| F3.12 | JSONB on BOM | 与已批准设计矛盾 |
| F3.13 | Real-time Validation | 已在设计规范中 |
