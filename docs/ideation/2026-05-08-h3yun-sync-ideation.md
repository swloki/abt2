---
date: 2026-05-08
topic: h3yun-sync
focus: docs/superpowers/specs/2026-05-08-h3yun-sync-design.md
mode: repo-grounded
---

# Ideation: H3Yun ERP Sync 改进想法

## Grounding Context

ABT 是 Rust BOM/库存管理系统（gRPC + PostgreSQL），正在构建 H3Yun ERP 单向同步功能（ABT → H3Yun）。设计文档已批准，包含产品和库存两类实体的同步，三种触发方式（5分钟定时、手动 gRPC、实时触发）。

关键发现：
- 项目已有 `ScheduledTaskService`（RunningGuard、60s 超时、优雅关机）
- 项目尚无 HTTP 客户端依赖（需新增 reqwest）
- 错误处理模式：基础设施错误 vs 业务错误分离
- TOCTOU/N+1 警告：外部 API 调用需避免逐记录查询模式
- Fail-open 危险：凭证缺失时不应静默继续
- 同步功能后端计划后续删除，因此尽量不污染现有表结构，可新建独立表

## Ranked Ideas

### 1. 独立 H3Yun 同步映射表 — 消除存在性查询 + 持久化水印

**Description:** 新建 `h3yun_sync_state` 表，存储 ABT 实体与 H3Yun ObjectId 的映射关系及同步水印：

```sql
CREATE TABLE h3yun_sync_state (
    id              SERIAL PRIMARY KEY,
    entity_type     VARCHAR(32) NOT NULL,  -- 'product' | 'inventory'
    entity_id       UUID NOT NULL,         -- ABT 中的 product_id / inventory_id
    h3yun_object_id VARCHAR(64),           -- H3Yun 返回的 ObjectId
    last_synced_at  TIMESTAMPTZ,           -- 上次成功同步时间
    content_hash    VARCHAR(64),           -- 上次同步的内容哈希（可选，用于去重）
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(entity_type, entity_id)
);
```

首次同步创建 H3Yun 记录后，将 ObjectId 写入此表。后续同步直接按 ObjectId 更新，跳过存在性查询。水印（`last_synced_at`）也在同一张表，重启不丢失。清理时直接 DROP TABLE 即可。

**Warrant:** `direct:` ProductMeta 是 JSONB 字段存储松散数据，但修改它属于"污染"现有表。设计中的 N+1 查询（500 产品 × 2 查询 × 300ms = 300 秒）根源在于每次都查询 H3Yun 存在性。
**Rationale:** 一张新表同时解决两个问题：消除存在性查询（ObjectId 映射）和持久化水印（last_synced_at）。不修改现有表结构。同步功能删除时直接删表，零残留。
**Downsides:** 需要在同步流程中维护映射表的写入。如果 ABT 实体被删除但映射表未清理，`h3yun_object_id` 成为孤儿记录（可通过定期清理解决）。
**Confidence:** 95%
**Complexity:** Low
**Status:** Unexplored

### 2. 统一事件管道 — tokio::sync::mpsc 合并三种触发器

**Description:** 用 `tokio::sync::mpsc` channel 替代设计中的三种独立触发路径。CRUD 操作发送 `SyncEvent { entity_type, entity_id, priority }` 到 channel；定时任务批量扫描变更实体发送低优先级事件；手动 gRPC 发送中优先级事件。单个 worker 消费 channel，处理去重（相同实体只保留最新），按优先级排序执行。

CRUD 响应不再等待 H3Yun（保持 ~5ms）。H3Yun 宕机时 ABT 产品操作毫秒级完成。Channel 提供背压（bounded capacity），天然去重定时+实时竞态。三种触发模式的错误处理、重试、去重逻辑统一到一处。

**Warrant:** `reasoned:` 设计已有三种代码路径调用相同同步逻辑，每个路径都必须处理错误、重试和去重。如果定时和实时同时触发同一产品，分散处理的竞争条件比统一处理更难调试。当前产品服务约 5ms 通过 PostgreSQL 完成，同步外部 HTTP 调用（200ms-5s）将 ABT 写入可用性与 H3Yun 可用性耦合。
**Rationale:** H3Yun 停机不应阻止在 ABT 中创建产品。Channel 还能去重定时 + 实时触发的竞态。
**Downsides:** 实时同步变为最终一致性（消费者排空 channel 前有毫秒级延迟）。轻微增加复杂度。
**Confidence:** 90%
**Complexity:** Medium
**Status:** Unexplored

### 3. 删除同步 — 设计中缺失的维度

**Description:** ABT `ProductRepo::delete` 执行硬删除（`DELETE FROM products WHERE product_id = $1`），但设计只定义了创建/更新流程，`RemoveBizObject` 从未被使用。ABT 删除的产品在 H3Yun 中成为幽灵记录，永远存在。

需要在删除流程中：
1. 删除前从 `h3yun_sync_state` 查询该实体的 `h3yun_object_id`
2. 调用 H3Yun `RemoveBizObject` 删除远程记录
3. 清理 `h3yun_sync_state` 中的映射行

如果实体未同步过（映射表中无记录），则跳过 H3Yun 删除操作。

**Warrant:** `direct:` `product_repo.rs` 的 `DELETE FROM products WHERE product_id = $1` — 硬删除。设计列出了 `RemoveBizObject` 操作但同步流程未使用它。H3Yun 中的幽灵产品会在库存报表、采购订单中被引用。
**Rationale:** ERP 中的幽灵产品会在库存报表、采购订单中被引用，导致实际业务错误。这不是优化，是数据正确性问题。
**Downsides:** 需要在删除前查询映射表。如果 ObjectId 过期（H3Yun 侧已删除），RemoveBizObject 可能失败——应作为 warn 处理而非 error。
**Confidence:** 85%
**Complexity:** Low
**Status:** Unexplored

### 4. 逐记录错误隔离 + SyncError 分类体系

**Description:** 创建 `SyncError` enum 层次结构：

```rust
enum SyncError {
    Transient { backoff_hint: Duration },           // 网络超时、429 rate limit
    ValidationError { record_id: String, fields: Vec<String> },  // 数据被 H3Yun 拒绝
    FatalError { reason: String },                  // 认证失败、schema 不匹配
}
```

每条记录的同步包裹在独立错误边界中，收集成功/失败。通过 `TaskRunResult { processed, succeeded, message }` 上报。Transient 错误自动重试（带退避），ValidationError 记录后跳过，FatalError 中止整个同步批次。

**Warrant:** `direct:` 项目文档化的三层错误处理（infrastructure / validation / business_error）。H3Yun HTTP 失败是基础设施错误（err_to_status），记录验证失败是业务结果（business_error）。`reasoned:` H3Yun 会拒绝某些记录（非法字符、缺失字段），所有记录共享的 `?` 传播意味着第 47 条失败会阻塞第 48-200 条。
**Rationale:** 没有 per-record 隔离，一条坏数据成为整个批次的阻塞点。结构化错误类型让重试策略、告警策略从单一来源派生。
**Downsides:** 轻微增加错误收集逻辑复杂度。需要区分哪些错误是 Transient（可重试）vs ValidationError（不可重试）。
**Confidence:** 95%
**Complexity:** Low
**Status:** Unexplored

### 5. 读回对账 — 金融结算模式验证同步正确性

**Description:** 定期（每小时或每天）从 H3Yun 读取所有记录，与 ABT 本地状态对比。仅标记漂移（记录缺失、字段不同、幽灵记录），不自动修复。漂移报告写入日志或作为 gRPC 查询结果暴露。

对账使用 H3Yun 的 `LoadBizObjects`（读取 API），不消耗写入配额。对比逻辑：对每个 H3Yun 记录，查 `h3yun_sync_state` 找到对应 ABT 实体，比较关键字段。对于 ABT 中存在但 H3Yun 中缺失的记录，标记为"同步丢失"。对于 H3Yun 中存在但 ABT 已删除的，标记为"幽灵记录"。

**Warrant:** `reasoned:` 金融系统从不信任交易执行而不做结算验证（T+1/T+2 模式）。当前设计是 fire-and-forget。H3Yun 可能静默拒绝记录、截断字段、或在升级时改变 schema。没有对账，同步故障是静默的——直到有人在 H3Yun 中发现错误数据。
**Rationale:** 成本极低（每个 schema 一个 LoadBizObjects 调用），但能捕获整个类别的 bug：H3Yun 静默拒绝、字段截断、schema 变更。首次部署后运行对账可快速验证映射正确性。
**Downsides:** 需要一个读取路径。对账本身是 advisory（建议性），不做自动修复。对比逻辑需要处理字段映射的复杂性。
**Confidence:** 80%
**Complexity:** Low
**Status:** Unexplored

## Rejection Summary

| # | Idea | Reason Rejected |
|---|------|-----------------|
| 1 | last_synced_at 列加在实体行上 | 同步功能会删除，不应污染现有表；被独立映射表（Idea 1）覆盖 |
| 2 | Dry-run 模式 | 有价值但是运营工具，非设计级改进 |
| 3 | Generic SyncPipeline 抽象 | v1 只有一个目标，过早抽象 |
| 4 | 双向同步契约 | 设计明确单向，below ambition floor |
| 5 | Feature-flagged 同步 | 过度工程，无多目标证据 |
| 6 | 反转同步方向（拉取代替推送） | 主体替换——改变根本架构 |
| 7 | 移除定时器 | 过于极端；定时同步提供安全网 |
| 8 | 事件溯源重定义 | 过于昂贵的架构重设计 |
| 9 | 凭证配置蔓延 | fail-open 已是已知教训，战术层面 |
| 10 | Schema 漂移炸弹 | 被 read-back reconciliation（Idea 5）覆盖 |
| 11 | 蚂蚁信息素自适应频率 | below ambition floor，nice-to-have |
| 12 | 多目标路由器 | 只有一个目标，过早抽象 |
| 13 | WebSocket 流式同步 | 主体替换，H3Yun API 不支持 |
| 14 | CRDT 双向同步 | 主体替换，scope 远超设计 |
| 15 | Wire logging + correlation ID | 强想法但 v2 级别，reconciliation 覆盖核心需求 |
| 16 | Sync metrics emitter trait | v2 级别 observability |
| 17 | Git fast-forward 分歧检测 | 前提是 H3Yun 中有人直接编辑——v1 未验证 |
| 18 | 字段映射配置化 | 先前 ideation 已拒绝，v1 hardcoded 足够 |
| 19 | JSONB sync_pending 字段 | 被 event pipeline（Idea 2）覆盖 |
| 20 | 内容哈希去重 | 有价值但优先级低于映射表 |
