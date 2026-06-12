# 设计方案：销售订单确认后流转到采购/生产模块

> 日期：2026-06-12
> 状态：已确认
> 范围：P2（demands 事件驱动完善）+ P4（下游模块集成）
> 前置：P0（AcquireChannel 枚举化）+ P1（核心履约模型）已全部实现

## 1. 业务背景

销售订单确认后，系统已能完成库存预留、履行计划生成、demands 写入和 `DemandCreated` 事件发布。但下游模块（采购、MES）尚未接入事件消费链路，需求停留在"已发布但未处理"状态。

本次实现目标：
- **外购产品**：需求流转到采购模块 → 采购员查看需求池 → 合并创建采购订单草稿 → 审批执行
- **自制产品**：需求流转到生产模块 → 计划员查看需求池 → 合并创建生产计划草稿 → 释放工单

## 2. 设计决策记录

| 决策项 | 选择 | 理由 |
|--------|------|------|
| 实现范围 | P2 + P4 全做 | 用户要求完整实现 |
| 采购下游单据 | 采购订单（PO）草稿 | PurchaseOrderService 已支持 product_id，字段匹配度高 |
| 自制下游单据 | 生产计划 → 释放工单 | 符合 MES 现有流程，用户明确要求 |
| 事件处理模式 | 事件驱动 + API 混合 | Handler 做通知，API 做单据创建，保留人工合并控制 |
| 前端范围 | 只做后端 | 前端 UI 留到 P3 阶段 |
| Handler 通知数据来源 | 回查 demands 表 | 与 v5"Payload 精简，消费者回查"原则一致，通知始终反映真实状态 |
| 跨模块查询策略 | 数据库视图 | 单体 + 共享数据库架构下最轻量，封装 JOIN 逻辑避免直接依赖 |
| confirm 状态同步策略 | 同步更新 + Outbox 事件 | 调用方需要即时结果，Outbox 保证最终一致 |
| 供应商约束 | supplier_id 必填 | 一次创建只关联一个供应商，操作员自行决定合并或拆分 |
| 排程参数 | items 可选 + 默认值 | 初版支持批量创建后逐行修改，降低操作复杂度 |
| confirm 状态同步 | **异步事件驱动** | confirm 只更新 demands + Outbox 事件，Handler 异步更新 fulfillment/订单行，避免跨聚合死锁 |
| 并发控制 | **乐观锁（UPDATE WHERE status='Open'）** | 受影响行数校验，防止两个操作员同时抢占同一批需求 |
| 需求池查询维度 | **订单行 + 物料聚合双视图** | 物料维度是采购员/计划员主要操作入口，避免逐条勾选 |
| Handler 回查告警 | **warn! 日志** | Demand 不存在时非静默跳过，记录告警便于排查数据一致性 |
| 预留消耗策略 | **保持锁定 + 取消自动释放** | 部分发货后剩余预留保持锁定，取消时自动释放防止幽灵占用 |

## 3. 整体架构

```
销售订单 confirm()
      │
      ├── 1. 库存预留 + 履行计划 ✅（已实现）
      ├── 2. 缺货行写入 demands + publish(DemandCreated) ✅（已实现）
      │
      ▼ [本次新增]
      ┌─────────────────────────────────────────────────┐
      │          DomainEventBus (NOTIFY)                 │
      └──────────┬──────────────────┬───────────────────┘
                 │                  │
      ┌──────────▼──────┐  ┌───────▼──────────┐
      │ PurchaseDemand   │  │ MesDemandCreated  │
      │ CreatedHandler   │  │ Handler           │
      │ (acquire=外购)   │  │ (acquire=自制)     │
      └──────────┬──────┘  └───────┬──────────┘
                 │                  │
      ┌──────────▼──────┐  ┌───────▼──────────┐
      │ 发送通知         │  │ 发送通知           │
      │ "有新的外购需求"  │  │ "有新的生产需求"    │
      └─────────────────┘  └──────────────────┘
                 │                  │
      ┌──────────▼──────┐  ┌───────▼──────────┐
      │ 采购员查看需求池  │  │ 计划员查看需求池    │
      │ POST /purchase/  │  │ POST /mes/demands/ │
      │ demands/create-  │  │ create-plan        │
      │ order            │  │                    │
      └──────────┬──────┘  └───────┬──────────┘
                 │                  │
      ┌──────────▼──────┐  ┌───────▼──────────┐
      │ 创建 PO 草稿     │  │ 创建生产计划草稿   │
      │ (合并多条需求)   │  │ (合并多条需求)     │
      └──────────┬──────┘  └───────┬──────────┘
                 │                  │
      ┌──────────▼──────────────────▼───────────┐
      │ DemandService.confirm() ✅（已实现）      │
      │ → DemandConfirmed 事件                   │
      │ → 履行计划行 → Purchasing/Producing      │
      └──────────────────────────────────────────┘
```

### 3.1 关键原则

1. **EventHandler 只做通知，不自动创建单据** — 保留操作员合并需求的控制权
2. **下游单据由操作员通过 API 主动创建** — 支持多条需求合并为一张 PO/生产计划
3. **创建后调用 DemandService.confirm 关闭环** — 触发 DemandConfirmed 事件，同步履行计划行和订单行状态
4. **两个 Handler 注册在同一事件上** — 通过 acquire_channel 各自过滤，互不干扰

## 4. 采购模块集成

### 4.1 新增子模块结构

```
abt-core/src/purchase/demand_handler/
├── mod.rs           # 导出 + 工厂函数
├── handler.rs       # PurchaseDemandCreatedHandler
├── service.rs       # PurchaseDemandService trait + impl
├── model.rs         # 请求/响应模型
└── repo.rs          # demands 查询 + 关联操作
```

### 4.2 EventHandler — PurchaseDemandCreatedHandler

**原则**：通知内容**回查 demands 表获取真实数据**，不依赖 payload 快照。这与 v5 设计文档的"Payload 精简，消费者回查"原则一致，确保通知始终反映需求当前状态。

```rust
pub struct PurchaseDemandCreatedHandler {
    pool: PgPool,
}

#[async_trait]
impl EventHandler for PurchaseDemandCreatedHandler {
    async fn handle(&self, event: &DomainEvent) -> Result<()> {
        let payload = &event.payload;
        let acquire_channel = payload["acquire_channel"].as_i64();

        // 只处理外购需求
        if acquire_channel != Some(AcquireChannel::Purchased as i64) {
            return Ok(());
        }

        // 回查 demands 表获取真实数据（而非依赖 payload 快照）
        let demand_id = event.aggregate_id;
        let mut conn = self.pool.acquire().await
            .map_err(|e| DomainError::Internal(e.into()))?;
        let demand = match DemandRepo::find_by_id(&mut conn, demand_id).await? {
            Some(d) => d,
            None => {
                // 需求不存在（物理删除或归档）— 记录 Warning 以便排查数据一致性
                warn!(demand_id, "Demand not found for DemandCreated event, skipping notification");
                return Ok(());
            }
        };

        // 如果需求已被处理或取消，跳过通知（防御事件乱序）
        if demand.status != DemandStatus::Open {
            return Ok(());
        }

        // 查询产品名称（回查而非依赖 payload）
        let product = ProductRepo::find_by_id(&mut conn, demand.product_id).await?
            .ok_or_else(|| DomainError::not_found("Product"))?;
        let order_no = SalesOrderRepo::find_order_no_by_id(&mut conn, demand.order_id).await?;

        // 构造通知，内容来自真实数据
        let ctx = ServiceContext::system(event.operator_id);
        let notification_svc = new_notification_service(self.pool.clone());
        notification_svc.notify_by_role(&ctx, &mut conn, PURCHASE_ROLE_ID, BatchNotificationReq {
            notification_type: NotificationType::Business,
            title: "新的外购需求待处理".into(),
            content: Some(format!(
                "产品: {} ({}) × {}, 来源订单: {}",
                product.name, product.code, demand.quantity, order_no
            )),
            related_type: Some("demand".into()),
            related_id: demand_id,
        }).await?;

        Ok(())
    }

    fn name(&self) -> &str { "purchase_demand_created" }
}
```

**行为说明**：
- 收到 `DemandCreated` 事件后检查 `acquire_channel`，仅处理 `Purchased(2)`
- **回查 demands 表**获取需求数据，而非依赖 payload 快照（v5 原则：事件结构稳定，消费者回查）
- 再次校验 `demand.status == Open`，防御事件乱序（如需求已被手动关闭）
- 通过 `notify_by_role` 发送业务通知给采购角色
- **不创建任何下游单据**

### 4.3 PurchaseDemandService 接口

```rust
#[async_trait]
pub trait PurchaseDemandService: Send + Sync {
    /// 查询待处理的外购需求（订单行维度）
    /// 从 sales demands 表读取，按 acquire_channel = Purchased 过滤
    async fn list_pending_demands(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        query: DemandQuery,
    ) -> Result<PaginatedResult<DemandSummary>>;

    /// 按物料聚合查询外购需求（物料维度 — 采购员操作入口）
    /// 聚合结果：物料X，总需求100，涉及5个订单，净缺口70
    /// 这是采购员的**主要操作视图**，而非按订单行逐条展示
    async fn list_material_aggregated(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        query: MaterialAggQuery,
    ) -> Result<PaginatedResult<MaterialAggSummary>>;

    /// 从选中的需求批量创建采购订单草稿
    /// - 可合并多条需求为一张 PO（同供应商）
    /// - 使用乐观锁并发控制（见 4.5 步骤 1）
    async fn create_order_from_demands(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreateOrderFromDemandsReq,
    ) -> Result<i64>;
}
```

### 4.4 请求/响应模型

```rust
/// 需求查询参数（订单行维度）
pub struct DemandQuery {
    pub status: Option<DemandStatus>,   // 默认 Open
    pub product_id: Option<i64>,
    pub order_id: Option<i64>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

/// 需求摘要（订单行维度 — 展示给操作员）
pub struct DemandSummary {
    pub id: i64,
    pub order_id: i64,
    pub order_no: String,               // 来源订单号
    pub product_id: i64,
    pub product_name: String,           // 产品名称
    pub product_code: String,           // 产品编码
    pub quantity: Decimal,              // 需求数量
    pub required_date: Option<NaiveDate>,
    pub priority: i32,
    pub status: DemandStatus,
    pub created_at: NaiveDateTime,
}

/// 物料聚合查询参数
pub struct MaterialAggQuery {
    pub product_id: Option<i64>,        // 按产品筛选
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

/// 物料聚合摘要（物料维度 — 采购员主要操作视图）
pub struct MaterialAggSummary {
    pub product_id: i64,
    pub product_name: String,
    pub product_code: String,
    pub total_demand_qty: Decimal,      // 总需求量（SUM 所有 Open 需求）
    pub demand_count: i64,              // 涉及多少条需求
    pub earliest_required_date: Option<NaiveDate>, // 最早需求日期
    pub latest_required_date: Option<NaiveDate>,   // 最晚需求日期
    pub demand_ids: Vec<i64>,           // 包含的需求 ID 列表（前端展开用）
}

/// 从需求创建采购订单请求
pub struct CreateOrderFromDemandsReq {
    pub demand_ids: Vec<i64>,           // 选中的需求 ID 列表
    pub supplier_id: i64,               // 供应商 ID（操作员指定）
    pub expected_delivery_date: Option<NaiveDate>,
    pub remark: String,
}
```

**设计说明**：
- `list_material_aggregated` 是采购员的**主要操作视图**，避免采购员逐条勾选 500 条需求
- 前端展示：物料X，总需求100，涉及5个订单，最早需 7/15 → 操作员点击"创建PO"直接用所有 demand_ids
- `list_pending_demands` 仍保留，用于需要查看订单行明细的场景

### 4.5 create_order_from_demands 流程

1. **乐观锁抢占**（并发控制）：
   ```sql
   UPDATE demands SET status = 'Processing'
   WHERE id = ANY($1) AND status = 'Open' AND acquire_channel = 2 AND deleted_at IS NULL;
   ```
   - 检查受影响行数是否等于 `demand_ids.len()`，如果不等于说明部分需求已被他人处理
   - 返回 `OptimisticLockError("部分需求已被他人处理，请刷新重试")`
   - 这比先 SELECT 再 UPDATE 的两步模式更安全，避免了 TOCTOU 竞争
2. **供应商约束**：`CreateOrderFromDemandsReq.supplier_id` 为**必填**，操作员创建 PO 前必须指定供应商。**一次调用只创建一张 PO，只关联一个供应商**
3. **聚合**：按 `product_id` 聚合需求（多条需求同产品则合并数量）
4. **创建 PO**：调用 `PurchaseOrderService::create` 创建采购订单草稿
   - 每个 product_id 聚合后生成一个订单行
   - `line_no` 自动编号
   - `unit_price` 取产品默认采购价或 0（待采购员补充）
5. **关联需求**：更新每条 demand 的 `target_doc_id` = PO ID，发布 `DemandConfirmed` 事件（见 6.5 节异步策略）
6. **事务保证**：以上步骤在同一数据库事务中完成
7. **返回**：新建的 PO ID

### 4.6 Repo 层查询 — 跨模块数据访问策略

**核心问题**：采购/MES 模块需要 JOIN `products` 和 `sales_orders` 表来展示需求摘要，这产生跨模块耦合。

**决策：采用数据库视图封装 JOIN 逻辑**（短期方案）

- 创建 `v_purchase_demands` 和 `v_production_demands` 视图，封装 demands + products + sales_orders 的 JOIN
- 下游模块只读视图，不直接依赖销售表结构
- 视图变更由数据库 migration 管理，如果销售表结构变更，只需更新视图定义
- 当前架构是单体 + 共享数据库，视图是最轻量、最务实的解法

```sql
-- v_purchase_demands 视图
CREATE VIEW v_purchase_demands AS
SELECT
    d.id, d.order_id, d.product_id, d.quantity,
    d.required_date, d.priority, d.status AS demand_status,
    d.acquire_channel, d.target_doc_id, d.created_at,
    p.name AS product_name, p.code AS product_code,
    so.order_no
FROM demands d
JOIN products p ON p.id = d.product_id
JOIN sales_orders so ON so.id = d.order_id
WHERE d.acquire_channel = 2    -- Purchased
  AND d.deleted_at IS NULL;
```

**Repo 层查询**：

```rust
/// 查询视图 v_purchase_demands（封装跨模块 JOIN）
pub struct PurchaseDemandRepo;

impl PurchaseDemandRepo {
    /// 按条件查询外购需求 — 读取视图而非原始表
    pub async fn find_demands(
        db: PgExecutor<'_>,
        query: &DemandQuery,
        page: u32,
        page_size: u32,
    ) -> Result<Vec<DemandSummary>> {
        // SELECT * FROM v_purchase_demands
        // WHERE status = 'Open' (默认) AND ...
        // ORDER BY required_date ASC, priority DESC
    }

    /// 批量读取指定 ID 的 demands（用于校验和创建 PO）
    /// 直接读 demands 原始表（需要写权限校验）
    pub async fn find_by_ids(
        db: PgExecutor<'_>,
        ids: &[i64],
    ) -> Result<Vec<Demand>> {
        // SELECT * FROM demands WHERE id = ANY($1) AND acquire_channel = 2
    }
}
```

**必要的索引**（性能保障）：
```sql
-- demands 表核心查询索引
CREATE INDEX idx_demands_channel_status ON demands (acquire_channel, status) WHERE deleted_at IS NULL;
CREATE INDEX idx_demands_product ON demands (product_id) WHERE deleted_at IS NULL;
```

**长期演进方向**（超出本次范围）：
- 如果未来拆分微服务，视图替换为服务间 API 调用
- 如果 demands 数据量 > 10 万行，引入读模型（CQRS）或物化视图

## 5. MES 模块集成

### 5.1 新增子模块结构

```
abt-core/src/mes/demand_handler/
├── mod.rs           # 导出 + 工厂函数
├── handler.rs       # MesDemandCreatedHandler
├── service.rs       # MesDemandService trait + impl
├── model.rs         # 请求/响应模型
└── repo.rs          # demands 查询 + 关联操作
```

### 5.2 EventHandler — MesDemandCreatedHandler

与采购 Handler 一致，**回查 demands 表获取真实数据**后构造通知。

```rust
pub struct MesDemandCreatedHandler {
    pool: PgPool,
}

#[async_trait]
impl EventHandler for MesDemandCreatedHandler {
    async fn handle(&self, event: &DomainEvent) -> Result<()> {
        let payload = &event.payload;
        let acquire_channel = payload["acquire_channel"].as_i64();

        // 只处理自制需求
        if acquire_channel != Some(AcquireChannel::SelfProduced as i64) {
            return Ok(());
        }

        // 回查 demands 表获取真实数据
        let demand_id = event.aggregate_id;
        let mut conn = self.pool.acquire().await
            .map_err(|e| DomainError::Internal(e.into()))?;
        let demand = match DemandRepo::find_by_id(&mut conn, demand_id).await? {
            Some(d) => d,
            None => {
                warn!(demand_id, "Demand not found for DemandCreated event, skipping notification");
                return Ok(());
            }
        };

        if demand.status != DemandStatus::Open {
            return Ok(());
        }

        let product = ProductRepo::find_by_id(&mut conn, demand.product_id).await?
            .ok_or_else(|| DomainError::not_found("Product"))?;
        let order_no = SalesOrderRepo::find_order_no_by_id(&mut conn, demand.order_id).await?;

        let ctx = ServiceContext::system(event.operator_id);
        let notification_svc = new_notification_service(self.pool.clone());
        notification_svc.notify_by_role(&ctx, &mut conn, PRODUCTION_ROLE_ID, BatchNotificationReq {
            notification_type: NotificationType::Business,
            title: "新的生产需求待处理".into(),
            content: Some(format!(
                "产品: {} ({}) × {}, 来源订单: {}",
                product.name, product.code, demand.quantity, order_no
            )),
            related_type: Some("demand".into()),
            related_id: demand_id,
        }).await?;

        Ok(())
    }

    fn name(&self) -> &str { "mes_demand_created" }
}
```

### 5.3 MesDemandService 接口

```rust
#[async_trait]
pub trait MesDemandService: Send + Sync {
    /// 查询待处理的自制需求
    async fn list_pending_demands(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        query: DemandQuery,
    ) -> Result<Vec<DemandSummary>>;

    /// 从选中的需求创建生产计划草稿
    /// - 多条需求可合并为一张生产计划（同产品合并数量）
    /// - 创建后调用 DemandService.confirm 关闭环
    async fn create_plan_from_demands(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        req: CreatePlanFromDemandsReq,
    ) -> Result<i64>;
}
```

### 5.4 请求模型

```rust
pub struct CreatePlanFromDemandsReq {
    pub demand_ids: Vec<i64>,           // 选中的需求 ID 列表
    pub plan_type: PlanType,            // 计划类型
    pub plan_date: NaiveDate,           // 计划日期
    pub remark: Option<String>,
    /// 每条需求的排程参数 — 可选，不填则使用默认排程
    pub items: Option<Vec<PlanDemandItemReq>>,
    /// 默认排程参数（当 items 未提供时使用）
    pub default_scheduled_start: Option<NaiveDate>,  // 默认 = plan_date
    pub default_scheduled_end: Option<NaiveDate>,    // 默认 = plan_date + 7 天
}

pub struct PlanDemandItemReq {
    pub demand_id: i64,
    pub scheduled_start: NaiveDate,
    pub scheduled_end: NaiveDate,
    pub priority: i32,                  // 默认 0
}
```

**排程策略**：
- 如果提供了 `items`，使用每条需求各自的排程参数
- 如果未提供 `items`，所有需求统一使用 `default_scheduled_start/end`（默认 start = plan_date，end = plan_date + 7 天）
- 计划员可在前端逐步细化排程，初版支持批量创建后逐行修改

### 5.5 create_plan_from_demands 流程

1. **校验**：读取选中的 demands，确认：
   - 状态必须为 `Open`
   - `acquire_channel` 必须为 `SelfProduced`
   - 不存在重复 ID
2. **聚合**：按 `product_id` 聚合需求
3. **创建生产计划**：调用 `ProductionPlanService::create` 创建生产计划草稿
   - 利用已有的 `sales_order_id`、`sales_order_item_id` 关联字段
   - 每个 product_id 聚合后生成一个计划行
   - 排程日期、优先级从 `PlanDemandItemReq` 中取
4. **关联需求**：逐一调用 `DemandService::confirm`，传入生产计划 ID
5. **事务保证**：同一数据库事务
6. **返回**：新建的生产计划 ID

### 5.6 后续流程（已有实现）

- 计划员审核生产计划 → 调用 `ProductionPlanService::release`
- 释放生成工单 → `WorkOrderService::release`
- 工单完工 → `ProductionReceiptService` 入库
- 入库后 → `DemandService::fulfill` → 需求完成 → 履行计划行更新

## 6. EventHandler 注册与 EventProcessor 启动

### 6.1 前置问题：EventProcessor 尚未启动

**现状发现**：`EventProcessor` 基础设施完整（LISTEN/NOTIFY + 轮询 + 幂等 + 重试/死信），但 **从未在 `abt-web/src/main.rs` 中启动**。现有 `impl EventHandler` 实例（H3Yun handlers）也未注册。`handle_demand_confirmed`/`handle_demand_rejected` 是独立函数，不是 `impl EventHandler`。

**本次实施必须先完成**：
1. 在 `AppState` 或启动流程中创建 `EventProcessor`
2. 将已有的 `handle_demand_confirmed`/`handle_demand_rejected` 改造为 `impl EventHandler`（见 6.4 过渡策略）
3. 注册所有 Handler 并启动 `EventProcessor`

### 6.2 Handler 实现原则

EventHandler trait 签名为 `handle(&self, event: &DomainEvent) -> Result<()>`，Handler 持有自己的 `PgPool`，在 `handle` 方法中通过 `self.pool.acquire()` 获取连接（参考 `h3yun/handlers.rs`）。

**Handler 内部禁止直接调用需要 `ServiceContext` 的 Service trait 方法**。Handler 只做轻量操作（通知、状态标记），重操作（创建单据、预留库存）由 API 层的 Service 处理。

具体 Handler 实现代码见 4.2（采购）和 5.2（MES），均遵循**回查 demands 表**获取真实数据后构造通知的原则。

### 6.3 应用启动时的注册和启动

在 `AppState::new` 或 `main.rs` 启动流程中：

```rust
use abt_core::shared::event_bus::{
    EventHandlerRegistryImpl, EventProcessor, DeadLetterServiceImpl,
};

// 创建注册表
let registry = Arc::new(EventHandlerRegistryImpl::new());

// 注册所有 Handler
registry.register(
    DomainEventType::DemandCreated,
    Arc::new(PurchaseDemandCreatedHandler::new(pool.clone())),
);
registry.register(
    DomainEventType::DemandCreated,
    Arc::new(MesDemandCreatedHandler::new(pool.clone())),
);
// 未来可注册更多 Handler：
// registry.register(DomainEventType::DemandConfirmed, Arc::new(SalesDemandConfirmedHandler::new(pool.clone())));
// registry.register(DomainEventType::ProductStatusChanged, Arc::new(ProductSyncHandler::new(pool.clone())));

// 创建并启动 EventProcessor
let dead_letter = Arc::new(DeadLetterServiceImpl::new(pool.clone()));
let processor = EventProcessor::new(
    Arc::new(pool.clone()),
    registry,
    dead_letter,
    5, // max_retries
);
processor.start();
```

两个 Handler 注册在同一 `DemandCreated` 事件类型上，`EventHandlerRegistry` 支持一对多，EventProcessor 会逐一调用。每个 Handler 通过 `acquire_channel` 过滤，互不干扰。

### 6.4 handle_demand_confirmed / handle_demand_rejected 改造过渡策略

**现状**：`sales_order/implt.rs` 中的 `handle_demand_confirmed` 和 `handle_demand_rejected` 是独立 `pub async fn`，接受 `(PgPool, &ServiceContext, PgExecutor, &DomainEvent)` 参数。经搜索确认，这两个函数**当前无任何调用点**（仅在定义处出现），因此改造不存在重复执行风险。

**改造方案**：
1. **新建** `SalesDemandConfirmedHandler` 和 `SalesDemandRejectedHandler`，各自 `impl EventHandler`，内部调用原函数逻辑
2. **保留**原独立函数但不导出（`pub(crate)`），作为 Handler 内部的实现复用
3. **注册**到 `DemandConfirmed(65)` 和 `DemandRejected(66)` 事件类型
4. 如果未来有其他模块也需要消费这两个事件，Handler 通过过滤各自处理

```rust
// abt-core/src/sales/sales_order/event_handlers.rs（新文件）
pub struct SalesDemandConfirmedHandler { pool: PgPool }

#[async_trait]
impl EventHandler for SalesDemandConfirmedHandler {
    async fn handle(&self, event: &DomainEvent) -> Result<()> {
        let mut conn = self.pool.acquire().await
            .map_err(|e| DomainError::Internal(e.into()))?;
        let ctx = ServiceContext::system(event.operator_id);
        // 复用已有逻辑
        super::implt::handle_demand_confirmed(
            self.pool.clone(), &ctx, &mut conn, event
        ).await
    }
    fn name(&self) -> &str { "sales_demand_confirmed" }
}
```

### 6.5 DemandService.confirm 的实现策略（异步事件驱动，避免跨聚合死锁）

**核心问题**：`confirm` 方法是在同一事务内同步更新 demands + fulfillment_plan_lines + sales_order_items 三张表，还是只更新 demands 然后通过事件异步更新其余两张？

**决策**：`confirm` 方法**只更新 demands 表 + 发布 DemandConfirmed 事件**，由 `SalesDemandConfirmedHandler` 异步更新 fulfillment_plan_lines 和 sales_order_items。

**理由**：
1. **跨聚合死锁风险**：多个采购员同时确认不同需求但涉及同一销售订单的多行时，同一事务跨三张表的 UPDATE 极易引发行锁竞争甚至死锁
2. **事务持有锁时间短**：confirm 事务只锁 demands 行，迅速释放；fulfillment/订单行更新由 Handler 独立事务完成
3. **EventProcessor 通常在秒级内消费事件**，用户体感上接近即时
4. **幂等设计保证最终一致**：Handler 先检查状态再更新，重复消费不会出错

**具体实现**：
```
confirm() 事务内（只涉及 demands 表）：
  1. UPDATE demands SET status = 'Processing', target_doc_id = ?, target_doc_type = ?
  2. INSERT INTO domain_events (DemandConfirmed) -- Outbox
  3. NOTIFY domain_event

SalesDemandConfirmedHandler（异步，独立事务）：
  1. 从 event payload 获取 order_line_id、acquire_channel、target_doc_id
  2. 查询 fulfillment_plan_line，幂等检查：如果 status 已经是 Producing/Purchasing，跳过
  3. UPDATE fulfillment_plan_lines SET status = Producing/Purchasing
  4. UPDATE sales_order_items SET line_status = Producing/Purchasing
```

**幂等保证**：
- Handler 先 SELECT 检查状态，已更新则跳过
- EventProcessor 的 IdempotencyRepo.check_and_mark 提供第一层去重
- 即使事件被重复消费，结果一致（天然幂等）

## 7. API 路由设计

```
# 采购模块 — 需求处理
GET    /purchase/demands                      — 查询待处理外购需求（订单行维度）
       ?status=Open&product_id=xxx&page=1&page_size=20
GET    /purchase/demands/material-aggregated  — 按物料聚合查询（物料维度，主要操作入口）
       ?product_id=xxx&page=1&page_size=20
POST   /purchase/demands/create-order         — 从需求创建采购订单草稿

# MES 模块 — 需求处理
GET    /mes/demands                           — 查询待处理自制需求（订单行维度）
       ?status=Open&product_id=xxx&page=1&page_size=20
GET    /mes/demands/material-aggregated       — 按物料聚合查询（物料维度，主要操作入口）
       ?product_id=xxx&page=1&page_size=20
POST   /mes/demands/create-plan               — 从需求创建生产计划草稿
```

## 8. 完整闭环时序

```
销售确认 → 预留 → demands写入 → DemandCreated事件
                                        │
                    ┌───────────────────┼───────────────────┐
                    ▼                                       ▼
            PurchaseDemandHandler                   MesDemandHandler
            (通知采购员)                             (通知计划员)
                    │                                       │
                    ▼                                       ▼
            采购员查看需求池                          计划员查看需求池
            合并创建 PO 草稿                          合并创建生产计划草稿
                    │                                       │
                    ▼                                       ▼
            DemandService.confirm()               DemandService.confirm()
            → DemandConfirmed                     → DemandConfirmed
                    │                                       │
                    ▼                                       ▼
            履行计划行 → Purchasing              履行计划行 → Producing
            订单行 → Purchasing                  订单行 → Producing
                    │                                       │
                    ▼                                       ▼
            PO 审批 → 到货入库                    计划释放 → 工单执行 → 完工入库
                    │                                       │
                    └───────────────┬───────────────────────┘
                                    ▼
                        DemandService.fulfill()
                        → demand → Done
                        → 履行计划行 → Allocated/Fulfilled
                        → 库存重新预留
                        → 通知："订单X所有行已备齐"
```

## 9. 涉及的文件清单

### 9.1 abt-core

| 文件 | 改动 |
|------|------|
| `src/purchase/demand_handler/mod.rs` | 新增子模块导出 + 工厂函数 |
| `src/purchase/demand_handler/handler.rs` | PurchaseDemandCreatedHandler（回查 demands 表） |
| `src/purchase/demand_handler/service.rs` | PurchaseDemandService trait + impl |
| `src/purchase/demand_handler/model.rs` | DemandQuery、DemandSummary、CreateOrderFromDemandsReq |
| `src/purchase/demand_handler/repo.rs` | 视图查询 + demands 批量读取 |
| `src/purchase/mod.rs` | 导出 demand_handler 子模块 |
| `src/mes/demand_handler/mod.rs` | 新增子模块导出 + 工厂函数 |
| `src/mes/demand_handler/handler.rs` | MesDemandCreatedHandler（回查 demands 表） |
| `src/mes/demand_handler/service.rs` | MesDemandService trait + impl |
| `src/mes/demand_handler/model.rs` | DemandQuery、DemandSummary、CreatePlanFromDemandsReq |
| `src/mes/demand_handler/repo.rs` | 视图查询 + demands 批量读取 |
| `src/mes/mod.rs` | 导出 demand_handler 子模块 |
| `src/sales/sales_order/event_handlers.rs` | 新增：SalesDemandConfirmedHandler + SalesDemandRejectedHandler（impl EventHandler） |
| `src/sales/sales_order/mod.rs` | 导出 event_handlers |
| `migrations/` | 新增：v_purchase_demands + v_production_demands 视图 + 索引 |

### 9.2 abt-web

| 文件 | 改动 |
|------|------|
| `src/routes/purchase.rs` | 新增 GET /purchase/demands + POST /purchase/demands/create-order |
| `src/routes/mes.rs` | 新增 GET /mes/demands + POST /mes/demands/create-plan |
| `src/state.rs` 或 `src/main.rs` | 创建 EventHandlerRegistry + 注册所有 Handler + 启动 EventProcessor |

### 9.3 设计文档

| 文件 | 改动 |
|------|------|
| `docs/uml-design/06-purchase.html` | 新增 PurchaseDemandService 接口和需求池查询 |
| `docs/uml-design/03-mes.html` | 新增 MesDemandService 接口和需求池查询 |

## 10. 异常与边界情况

| 场景 | 处理方式 |
|------|----------|
| **并发抢占同一批需求** | 乐观锁：`UPDATE demands SET status='Processing' WHERE id=ANY($1) AND status='Open'`，受影响行数 ≠ 期望数时返回 `OptimisticLockError`，提示"部分需求已被他人处理，请刷新" |
| 多条需求的产品无供应商 | 返回明确错误，列出无供应商的产品 |
| 生产计划创建失败 | 事务回滚，demand 状态保持 Open |
| 通知服务不可用 | EventHandler 失败后进入重试队列，不影响需求创建 |
| 同一 demand 被两个模块处理 | acquire_channel 强过滤，不可能交叉 |
| 需求取消后下游单据已创建 | 需要手动取消下游单据，DemandService.reject 回退 |
| PO 被取消 | 采购模块发布事件 → DemandService.reject → demand 回退到 Open |
| **生产计划被取消** | MES 模块在 `ProductionPlanService::cancel` 中调用 `DemandService.reject` → demand 回退到 Open → 履行计划行回退到 Pending。**必须在生产计划模块的取消逻辑中集成此调用** |
| **EventHandler 重复消费** | EventProcessor 幂等检查（IdempotencyRepo）+ Handler 内部先检查状态再更新，双重保障 |
| **事件乱序** | Handler 内部回查 demands 表验证当前状态，status ≠ Open 则跳过通知 |
| **Demand 回查不存在** | Handler 记录 `warn!` 日志（非静默跳过），便于排查数据一致性 |
| **部分发货后预留消耗** | 订单行需求 100，预留 100。第一次发货 60 → 消耗 60 预留，剩余 40 预留**保持锁定**（不释放回公共池），直到订单行关闭或取消 |
| **取消订单释放预留** | 取消未发货剩余量（增加 cancelled_qty）→ 触发**自动释放预留**（Release Allocation），释放量 = 取消量。否则导致库存"幽灵占用" |

## 11. 实施风险与缓解

| 风险 | 等级 | 缓解措施 |
|------|------|----------|
| EventProcessor 未启动是最大风险 | **高** | 本次实施第一步解决，所有后续工作依赖此 |
| 跨模块 JOIN 性能 | 中 | 创建 `idx_demands_channel_status` 复合索引，视图中不涉及复杂计算 |
| 采购员/计划员无 UI 操作入口 | 中 | 后端 API 先行，P3 阶段尽早安排前端实现 |
| demand 状态不一致 | 低 | 本次无自动对账（P5），风险可接受。标注：当前只能靠手动排查 |
| 库存重新预留不在本次范围 | 低 | 正确：预留重算在补货入库后（P5），当前只创建补货单据 |

## 12. 与已有设计文档的关系

本方案是 `docs/design-proposal-sales-order-fulfillment-flow.md` 的 P2+P4 实现延续：
- P0（AcquireChannel 枚举化）✅ 已完成
- P1（核心履约模型）✅ 已完成
- P2（demands + 事件驱动）⚠️ 部分完成（创建和发布已实现，下游消费未实现）→ 本次补全
- P3（前端 UI）— 不在本次范围
- P4（下游模块集成）❌ 未实现 → 本次实现
- P5（补货完成闭环）— 后续实现

## 13. 远期规划（未纳入本次范围的建议）

以下建议来自专家审查，架构价值高但超出 P2+P4 范围，记录以备后续迭代：

| 建议 | 价值 | 触发条件 |
|------|------|----------|
| **交期反向同步（CTP/ATP）** | PO 预计到货日 / 工单预计完工日 → 回写 `sales_order_items.estimated_ready_date` | P5 补货闭环阶段一起实现 |
| **变更级联评估（Impact Analysis）** | 销售取消/减量前评估下游单据状态，阻断或提示先作废下游 | 业务提出"取消操作不安全"反馈 |
| **acquire_channel 柔性路由** | 允许计划员在 demand/履行计划行上人工覆盖补货通道 | 出现"产能满载需临时外购"场景 |
| **CQRS 读模型宽表** | 用事件驱动的宽表 `purchase_demand_pool` 替换视图 | demands 数据量 > 5 万行或需要数据权限控制 |
| **需求认领/锁定机制** | 采购员勾选时锁定 demand（assigned_to + 5分钟超时） | 多采购员并发操作频繁冲突 |
