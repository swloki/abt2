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

        // 触发通知：告诉采购员有新的外购需求待处理
        let notification_svc = new_notification_service(self.pool.clone());
        notification_svc.send(Notification {
            target_role: "purchase_staff",
            title: "新的外购需求待处理",
            body: format!("产品ID: {}, 来源订单ID: {}",
                payload["product_id"], payload["order_id"]),
            link: format!("/purchase/demands?status=Open"),
        }).await?;

        Ok(())
    }

    fn name(&self) -> &str { "PurchaseDemandCreatedHandler" }
}
```

**行为说明**：
- 收到 `DemandCreated` 事件后检查 `acquire_channel`
- 仅处理 `Purchased(2)` 类型的需求
- 通过通知服务发送提醒给采购角色
- **不创建任何下游单据**

### 4.3 PurchaseDemandService 接口

```rust
#[async_trait]
pub trait PurchaseDemandService: Send + Sync {
    /// 查询待处理的外购需求
    /// 从 sales demands 表读取，按 acquire_channel = Purchased 过滤
    async fn list_pending_demands(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        query: DemandQuery,
    ) -> Result<Vec<DemandSummary>>;

    /// 从选中的需求批量创建采购订单草稿
    /// - 可合并多条需求为一张 PO（同供应商）
    /// - 创建后调用 DemandService.confirm 关闭环
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
/// 需求查询参数
pub struct DemandQuery {
    pub status: Option<DemandStatus>,   // 默认 Open
    pub product_id: Option<i64>,
    pub order_id: Option<i64>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

/// 需求摘要（展示给操作员）
pub struct DemandSummary {
    pub id: i64,
    pub order_id: i64,
    pub order_no: String,               // 来源订单号
    pub product_id: i64,
    pub product_name: String,           // 产品名称（JOIN 查询）
    pub product_code: String,           // 产品编码
    pub quantity: Decimal,              // 需求数量
    pub required_date: Option<NaiveDate>,
    pub priority: i32,
    pub status: DemandStatus,
    pub created_at: NaiveDateTime,
}

/// 从需求创建采购订单请求
pub struct CreateOrderFromDemandsReq {
    pub demand_ids: Vec<i64>,           // 选中的需求 ID 列表
    pub supplier_id: i64,               // 供应商 ID（操作员指定）
    pub expected_delivery_date: Option<NaiveDate>,
    pub remark: String,
}
```

### 4.5 create_order_from_demands 流程

1. **校验**：读取选中的 demands，确认：
   - 状态必须为 `Open`
   - `acquire_channel` 必须为 `Purchased`
   - 不存在重复 ID
2. **聚合**：按 `product_id` 聚合需求（多条需求同产品则合并数量）
3. **创建 PO**：调用 `PurchaseOrderService::create` 创建采购订单草稿
   - 每个 product_id 聚合后生成一个订单行
   - `line_no` 自动编号
   - `unit_price` 取产品默认采购价或 0（待采购员补充）
4. **关联需求**：逐一调用 `DemandService::confirm`，传入 PO ID
5. **事务保证**：以上步骤在同一数据库事务中完成
6. **返回**：新建的 PO ID

### 4.6 Repo 层查询

```rust
/// 查询 demands 表（跨模块读取，只读）
/// 采购模块通过此 repo 读取 sales 模块的 demands 数据
pub struct PurchaseDemandRepo;

impl PurchaseDemandRepo {
    /// 按条件查询外购需求
    pub async fn find_demands(
        db: PgExecutor<'_>,
        query: &DemandQuery,
        page: u32,
        page_size: u32,
    ) -> Result<Vec<DemandSummary>> {
        // JOIN products 表获取产品名称和编码
        // JOIN sales_orders 表获取订单号
        // WHERE acquire_channel = 2 (Purchased)
        //   AND status = 'Open' (默认)
    }

    /// 批量读取指定 ID 的 demands（用于校验和创建 PO）
    pub async fn find_by_ids(
        db: PgExecutor<'_>,
        ids: &[i64],
    ) -> Result<Vec<Demand>>;
}
```

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

        // 触发通知
        let notification_svc = new_notification_service(self.pool.clone());
        notification_svc.send(Notification {
            target_role: "production_planner",
            title: "新的生产需求待处理",
            body: format!("产品ID: {}, 来源订单ID: {}",
                payload["product_id"], payload["order_id"]),
            link: format!("/mes/demands?status=Open"),
        }).await?;

        Ok(())
    }

    fn name(&self) -> &str { "MesDemandCreatedHandler" }
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
    /// 每条需求的排程参数
    pub items: Vec<PlanDemandItemReq>,
}

pub struct PlanDemandItemReq {
    pub demand_id: i64,
    pub scheduled_start: NaiveDate,
    pub scheduled_end: NaiveDate,
    pub priority: i32,                  // 默认 0
}
```

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
2. 将已有的 `handle_demand_confirmed`/`handle_demand_rejected` 改造为 `impl EventHandler`
3. 注册所有 Handler 并启动 `EventProcessor`

### 6.2 Handler 实现 — 遵循已有模式

EventHandler trait 签名为 `handle(&self, event: &DomainEvent) -> Result<()>`，Handler 持有自己的 `PgPool`，在 `handle` 方法中通过 `self.pool.acquire()` 获取连接（参考 `h3yun/handlers.rs`）。

```rust
// abt-core/src/purchase/demand_handler/handler.rs
pub struct PurchaseDemandCreatedHandler {
    pool: PgPool,
}

impl PurchaseDemandCreatedHandler {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EventHandler for PurchaseDemandCreatedHandler {
    async fn handle(&self, event: &DomainEvent) -> Result<()> {
        let payload = &event.payload;
        let acquire_channel = payload["acquire_channel"].as_i64();

        if acquire_channel != Some(AcquireChannel::Purchased as i64) {
            return Ok(());
        }

        // 获取连接，通过工厂函数创建通知服务
        let mut conn = self.pool.acquire().await
            .map_err(|e| DomainError::Internal(e.into()))?;
        let ctx = ServiceContext::system(event.operator_id);
        let notification_svc = new_notification_service(self.pool.clone());
        notification_svc.notify_by_role(&ctx, &mut conn, purchase_role_id, BatchNotificationReq {
            notification_type: NotificationType::Business,
            title: "新的外购需求待处理".into(),
            content: Some(format!("产品ID: {}, 来源订单ID: {}",
                payload["product_id"].as_i64().unwrap_or(0),
                payload["order_id"].as_i64().unwrap_or(0))),
            related_type: Some("demand".into()),
            related_id: event.aggregate_id,
        }).await?;

        Ok(())
    }

    fn name(&self) -> &str { "purchase_demand_created" }
}
```

```rust
// abt-core/src/mes/demand_handler/handler.rs
pub struct MesDemandCreatedHandler {
    pool: PgPool,
}

impl MesDemandCreatedHandler {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EventHandler for MesDemandCreatedHandler {
    async fn handle(&self, event: &DomainEvent) -> Result<()> {
        let payload = &event.payload;
        let acquire_channel = payload["acquire_channel"].as_i64();

        if acquire_channel != Some(AcquireChannel::SelfProduced as i64) {
            return Ok(());
        }

        let mut conn = self.pool.acquire().await
            .map_err(|e| DomainError::Internal(e.into()))?;
        let ctx = ServiceContext::system(event.operator_id);
        let notification_svc = new_notification_service(self.pool.clone());
        notification_svc.notify_by_role(&ctx, &mut conn, production_role_id, BatchNotificationReq {
            notification_type: NotificationType::Business,
            title: "新的生产需求待处理".into(),
            content: Some(format!("产品ID: {}, 来源订单ID: {}",
                payload["product_id"].as_i64().unwrap_or(0),
                payload["order_id"].as_i64().unwrap_or(0))),
            related_type: Some("demand".into()),
            related_id: event.aggregate_id,
        }).await?;

        Ok(())
    }

    fn name(&self) -> &str { "mes_demand_created" }
}
```

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

## 7. API 路由设计

```
# 采购模块 — 需求处理
GET    /purchase/demands                — 查询待处理外购需求
       ?status=Open&product_id=xxx&page=1&page_size=20
POST   /purchase/demands/create-order   — 从需求创建采购订单草稿

# MES 模块 — 需求处理
GET    /mes/demands                     — 查询待处理自制需求
       ?status=Open&product_id=xxx&page=1&page_size=20
POST   /mes/demands/create-plan         — 从需求创建生产计划草稿
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
| `src/purchase/demand_handler/handler.rs` | PurchaseDemandCreatedHandler |
| `src/purchase/demand_handler/service.rs` | PurchaseDemandService trait + impl |
| `src/purchase/demand_handler/model.rs` | DemandQuery、DemandSummary、CreateOrderFromDemandsReq |
| `src/purchase/demand_handler/repo.rs` | demands 查询 + 聚合操作 |
| `src/purchase/mod.rs` | 导出 demand_handler 子模块 |
| `src/mes/demand_handler/mod.rs` | 新增子模块导出 + 工厂函数 |
| `src/mes/demand_handler/handler.rs` | MesDemandCreatedHandler |
| `src/mes/demand_handler/service.rs` | MesDemandService trait + impl |
| `src/mes/demand_handler/model.rs` | DemandQuery、DemandSummary、CreatePlanFromDemandsReq |
| `src/mes/demand_handler/repo.rs` | demands 查询 + 聚合操作 |
| `src/mes/mod.rs` | 导出 demand_handler 子模块 |

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
| 需求已被其他操作员处理 | 乐观锁或状态校验，返回 "demand already processing" 错误 |
| 多条需求的产品无供应商 | 返回明确错误，列出无供应商的产品 |
| 生产计划创建失败 | 事务回滚，demand 状态保持 Open |
| 通知服务不可用 | EventHandler 失败后进入重试队列，不影响需求创建 |
| 同一 demand 被两个模块处理 | acquire_channel 强过滤，不可能交叉 |
| 需求取消后下游单据已创建 | 需要手动取消下游单据，DemandService.reject 回退 |
| PO 被取消 | 采购模块发布事件 → DemandService.reject → demand 回退到 Open |

## 11. 与已有设计文档的关系

本方案是 `docs/design-proposal-sales-order-fulfillment-flow.md` 的 P2+P4 实现延续：
- P0（AcquireChannel 枚举化）✅ 已完成
- P1（核心履约模型）✅ 已完成
- P2（demands + 事件驱动）⚠️ 部分完成（创建和发布已实现，下游消费未实现）→ 本次补全
- P3（前端 UI）— 不在本次范围
- P4（下游模块集成）❌ 未实现 → 本次实现
- P5（补货完成闭环）— 后续实现
