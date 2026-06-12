# 设计方案：销售订单确认后的库存校验与业务分流

> 合并 Issue #12 + Issue #16，统一设计  
> v5 — cancelled_qty 防御部分取消 + 分配策略接口 + 状态对账 + DemandCreated 精简 Payload + 并发测试

## 1. 业务背景

销售订单确认后，系统需要根据每行产品的库存情况和产品属性进行智能分流：
- 库存足够 → 库存预留锁定，可立即发货或等齐一起发
- 库存不足 → 自制走生产工单，外购走采购申请，委外走委外订单
- 费用/服务类 → 跳过库存校验，直接走完成流程

## 2. 现状问题

### 2.1 头行状态耦合（根源问题）
- `InProduction` 状态在订单头级别，混合订单（自制+外购）时状态机崩溃
- 订单头到底算"生产中"还是"采购中"？无法表达
- `Only Shipped orders can be completed` 的报错正是这个设计缺陷的症状

### 2.2 缺少库存预留（Issue #12）
- 订单确认后没有库存校验和预留
- "查到库存够"≠"把库存锁定给这个订单"，存在超卖风险
- 没有为缺货产品触发补货流程

### 2.3 产品属性不是枚举（Issue #16）
- `ProductMeta.acquire_channel` 是自由文本 `String`（存在 JSONB meta 字段内），无法做类型安全判断，也无法加索引
- 前端按钮不区分产品类型，一律显示【创建发货申请 / 开始生产 / 取消订单】

### 2.4 模块强耦合
- 销售模块直接调用 MES/Purchase 创建单据，变成"上帝对象"
- 采购员无法合并多个订单的同种物料去谈折扣
- 生产计划员无法根据车间产能自主排程

## 3. 核心设计决策

### 3.1 头行状态分离（至关重要）

**订单头状态（Header Status）** — 只关注商务与整体履约进度：

```
Draft → Confirmed → PartiallyShipped → Shipped → Completed
         ↓
      Cancelled
```

- **删除 `InProduction` 状态**
- 订单头只反映**发货交付进度**
- 外购订单自然流转：`Confirmed → PartiallyShipped → Shipped → Completed`

**订单行状态（Line Status）** — 关注具体的物料履约动作：

```
Pending → Allocated(已分配) / Producing(生产中) / Purchasing(采购中) → Shipped(已发货) / Cancelled(已取消)
```

- `Allocated`：库存已预留，可直接发货
- `Producing`：已生成生产工单，等待完工入库
- `Purchasing`：已生成采购申请，等待到货入库
- `Cancelled`：行已取消（部分取消时拆行或调整 ordered_qty）

#### 3.1.1 头状态同步规则（幂等 + 可重入 + 防御部分取消）

订单头状态由**检查函数**驱动，不依赖事件链，避免事件丢失导致状态漂移：

```rust
/// 幂等的订单头状态计算 — 每次订单行变更后调用
/// 关键：cancelled_qty 不等于 shipped_qty，取消不是发货
fn recalc_order_header_status(items: &[SalesOrderItem]) -> SalesOrderStatus {
    let all_settled = items.iter().all(|i| {
        i.shipped_qty + i.cancelled_qty >= i.ordered_qty
    });
    let any_shipped = items.iter().any(|i| i.shipped_qty > Decimal::ZERO);
    let any_open = items.iter().any(|i| {
        i.shipped_qty + i.cancelled_qty < i.ordered_qty
    });

    if all_settled && any_shipped {
        SalesOrderStatus::Shipped           // 全部发完或取消完，且有实际发货
    } else if any_shipped && any_open {
        SalesOrderStatus::PartiallyShipped  // 有行已发，但还有未结清的行
    } else {
        SalesOrderStatus::Confirmed         // 都没发（纯取消的行单独处理）
    }
}
```

**四量模型**（防御"部分取消"的数学陷阱）：

| 字段 | 说明 |
|------|------|
| `ordered_qty` | 订单量（原始，不因取消而修改） |
| `shipped_qty` | 已发货量 |
| `cancelled_qty` | 已取消量（新增字段） |
| `open_qty` | 未交量 = ordered_qty - shipped_qty - cancelled_qty |

**模型层计算方法**（标准不变式）：

```rust
impl SalesOrderItem {
    fn open_qty(&self) -> Decimal {
        self.ordered_qty - self.shipped_qty - self.cancelled_qty
    }
    fn is_settled(&self) -> bool {
        self.shipped_qty + self.cancelled_qty >= self.ordered_qty
    }
}
```

**关键约束**：
- 取消不是发货：`cancelled_qty` 不参与 `shipped_qty` 的判定，但参与"是否结清"的判定
- `ordered_qty` **不被修改**，保持原始值，通过 `cancelled_qty` 标记取消量
- `PartiallyShipped` 判定：存在 `shipped_qty > 0` 且 `shipped_qty + cancelled_qty < ordered_qty` 的行
- `Shipped` 触发条件：所有行 `shipped_qty + cancelled_qty >= ordered_qty`（含容差），且有实际发货
- **DB CHECK 约束**：`CHECK(open_qty >= 0)`，防止数据层腐化
- **乐观锁**：订单行和履行计划行加 `version` 字段，防并发冲突
- **事务性保证**：订单行变更 + 头状态更新必须在同一数据库事务中完成
- **可重入**：即使调用多次，结果一致；事件丢失后重算也能恢复正确状态
- 行状态 `Cancelled` 的行不参与头状态计算（已被单独标记为取消）

### 3.2 引入库存预留（Allocation/Reservation）

订单确认时立即执行 Reserve（预占），不是只"看一眼 ATP"：

- **库存充足的行** → 硬预留（扣减 ATP），订单行状态变为 `Allocated`
- **库存部分满足** → 预占现有库存（Partial Allocation），剩余未满足量（Open Qty）触发补货
- **费用/服务类产品** → 跳过库存校验，直接标记为 `Allocated`

#### 3.2.1 预留的原子性保障

- **原子操作**：使用 `SELECT ... FOR UPDATE` 或数据库层面的原子扣减，确保 ATP 查询到预留创建之间无时间差
- **预留失败处理**：明确的错误码（如 `INSUFFICIENT_STOCK`），不静默创建负数库存；重试策略由调用方决定
- **预留粒度扩展点**：履行计划行预留 `reservation_details JSONB` 字段，当前存 `{"allocated_qty": x}`，后期扩展成 `{"warehouse_id":..., "batch":..., "allocated_qty":...}` 时无需改表

#### 3.2.2 补货入库后的分配策略（预留策略接口）

入库的库存如何分配给多个等待同一产品的履行计划行？需要一套分配规则：

```rust
/// 分配策略接口 — 当前 FIFO 按需求日期，未来可扩展
trait ReplenishmentAllocationStrategy {
    /// 查询 open_qty > 0 的履行计划行，按策略排序后逐行填充
    fn allocate(
        product_id: i64,
        available_qty: Decimal,
        candidates: &[FulfillmentPlanLine],
    ) -> Vec<AllocationResult>;
}

/// P5 先实现最简单的 FIFO
struct FifoByRequiredDate;
impl ReplenishmentAllocationStrategy for FifoByRequiredDate {
    fn allocate(...) -> Vec<AllocationResult> {
        // 按 required_date ASC 排序，逐行填充
    }
}
```

**实施策略**：
- P1 定义接口，P5 实现 FIFO 策略
- 策略做成独立函数/接口，预留扩展点（优先级加权、人工指定等）
- 入库事件携带产品ID、数量、仓库、批次；分配器查询待满足行后按策略分配
- `reservation_details` 当前存简单数量，未来扩展维度时分配逻辑无需重写

### 3.3 新增 OrderFulfillmentPlan（订单履行计划）实体

确认后生成履行计划，每行记录：

| 字段 | 说明 |
|------|------|
| 订单行ID | 关联 sales_order_items |
| 产品ID | 关联产品 |
| 需求数量 | 来自订单行 |
| 已预留数量 | 硬预留成功的量 |
| 缺货数量 | 需求 - 已预留 |
| 补充方式 | AcquireChannel |
| 关联补货单号 | 请购单ID / 生产工单ID / 委外单ID |
| 履行状态 | Pending / Allocated / Producing / Purchasing / Fulfilled |
| reservation_details | JSONB 预留扩展点（当前 `{"allocated_qty": x}`） |
| required_date | 需求日期（用于分配策略排序） |

**优势**：
- 订单头状态机不必膨胀
- "等齐一起发"或"部分发货"的判断基于计划完成度
- 补货完成后通过事件通知，无需监听底层库存表

### 3.4 需求池解耦（demands 实体 + DomainEventBus 事件驱动）

销售模块**不直接调用** MES/Purchase 创建单据，而是写入 `demands` 业务表 + 通过已有 `DomainEventBus` 发布需求事件：

```
销售模块 → 写入 demands 表 + DomainEventBus.publish(DemandCreated)
                │
                ├── 采购模块 EventProcessor 消费 → 读取 demands(Purchased, Open) → 勾选合并 → 生成 PR
                └── 生产模块 EventProcessor 消费 → 读取 demands(SelfProduced, Open) → 产能评估 → 生成工单草稿
```

**架构分层**：
- `demands` 表 = **业务数据层**（查询、展示、合并、状态管理）
- `DomainEventBus` = **通知机制**（触发下游模块处理，复用已有 Outbox + 异步分发）
- 两者各司其职，不重复

**demands 表设计**：

| 字段 | 说明 |
|------|------|
| id | 主键 |
| order_id | 来源销售订单ID |
| order_line_id | 来源订单行ID |
| product_id | 产品ID |
| quantity | 需求数量 |
| acquire_channel | AcquireChannel |
| status | Open / Processing / Done / Cancelled |
| target_doc_id | 下游单据ID（请购单/工单） |
| required_date | 需求日期（来自订单行交货日期） |
| priority | 优先级 |
| created_at | 创建时间 |

**关键原则**：
- 需求被下游**确认**后（如 PR 生成），下游发布 `DemandConfirmed` 事件，销售侧 EventProcessor 消费后将履行计划行状态变为 `Producing`/`Purchasing`
- 下游驳回/暂停时，发布 `DemandRejected` 事件，demand 状态回退为 `Open`，履行计划行回退到 `Pending`
- 避免"发出事件就以为在补货了"的假象 — 履行计划行状态变更由下游**确认事件**驱动，而非发布事件时立即变更
- 复用已有 `DomainEventBus` 基础设施，新增 `DomainEventType::DemandCreated / DemandConfirmed / DemandRejected` 枚举值

#### 3.4.1 状态对账机制（防脏读竞争）

demands 状态与履行计划行状态存在两个数据源指向同一业务含义，事件丢失或乱序可能导致不一致。**不依赖事件回写作为唯一同步机制**：

- **定时对账任务**：查询履行计划行状态为 `Producing`/`Purchasing` 但关联 demand 状态为 `Cancelled`/`Open` 的记录，自动回退
- **前端"刷新状态"按钮**：履约工作台提供手动触发对账，用户可一键同步
- **对账 SQL 示例**：
  ```sql
  SELECT fp.*
  FROM fulfillment_plan_lines fp
  JOIN demands d ON d.order_line_id = fp.order_line_id
  WHERE fp.status IN ('Producing', 'Purchasing')
    AND d.status IN ('Open', 'Cancelled')
    AND d.deleted_at IS NULL;
  ```

#### 3.4.2 事件 Payload 精简原则

`DemandCreated` 事件只放最小标识字段，消费者去 demands 表查详情：

```rust
EventPublishRequest {
    event_type: DomainEventType::DemandCreated,
    aggregate_type: "Demand".to_string(),
    aggregate_id: demand.id,       // demand 记录 ID
    payload: json!({
        "order_id": order_id,
        "product_id": product_id,
        "acquire_channel": acquire_channel.as_i16(),
    }),
    idempotency_key: None,
}
```

**好处**：事件结构稳定，不随 demands 字段膨胀而频繁改 schema。

**优势**：
- 采购员可在"待处理需求池"界面勾选多条需求合并生成一张 PR
- 生产计划员可自主排程
- 为未来引入 MRP 打下基础

### 3.5 补货流程：草稿 → 审批 → 执行

补货单据必须经过审批环节，不可全自动直达：
- **自制产品** → 生成生产工单**草稿**，MES 计划员确认排程后下达
- **外购产品** → 生成请购单（PR），采购审批通过后转为采购订单（PO）
- **委外产品** → 生成委外订单草稿，走委外审批流程

补货完成后触发**通知**（"订单X所有行已备齐，可创建发货申请"），而非静默创建发货单。

### 3.6 部分发货与"等齐一起发"

在履约工作台支持两种操作模式：
- **立即发货可发量**：从已预留库存创建发货单，消耗预留量，订单进入 `PartiallyShipped`
- **等待全部备齐**：履行计划所有行 `Fulfilled` 后，亮显【全部发货】按钮，可配置自动提醒

**约束**：
- 发货单创建时消耗预留，而非重新查库存
- 已发货行不可再取消或减少数量，只能取消未发货剩余量

### 3.7 AcquireChannel 枚举化

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum AcquireChannel {
    SelfProduced = 1,  // 自制
    Purchased = 2,     // 外购
    Outsourced = 3,    // 委外（预留）
    NonInventory = 4,  // 费用/服务/虚拟件（跳过库存校验和补货）
    Legacy = 9,        // 历史遗留（行为等同 SelfProduced，后台日志驱动数据清洗）
}
```

**迁移策略**：
- DB 层用 `SMALLINT` + `CHECK(acquire_channel IN (1,2,3,4,9))`
- 转换脚本：映射现有值（"自制"→1, "外购"/"采购"→2），**不留 Unknown/0**
- **宁可拍脑袋映射，也不留生产数据中的阻断值**
- 无法确定的归为 `Legacy(9)`：允许确认（行为等同自制），但后台记录日志，驱动数据清洗任务
- 这样不阻断业务，但有追责途径
- 预留 `Outsourced = 3` 和 `NonInventory = 4`，避免未来再次改表

### 3.8 前端 UI：履约工作台

订单详情页不直接堆叠履行计划（50 行产品会极其臃肿），采用分层展示：

**订单详情页（Header）**：
- 整体进度条（加权计算：`sum(shipped_qty) / sum(ordered_qty)`，而非简单行数比例）
- 订单级操作按钮（根据履行计划聚合）

**履约计划工作台（Fulfillment Tab/抽屉）**：
- 独立 Tab 或右侧抽屉，展开后显示详细履行计划
- 表格视图：产品名 | 需求量 | 可用库存 | 缺口 | 建议动作 | 操作
- 批量操作按钮：
  - 【一键生成采购申请】— 仅本订单外购缺货行（跨订单合并在远期规划）
  - 【一键生成生产工单】— 仅本订单自制缺货行
  - 【一键创建发货单】— 仅已 Allocated 的行
- 视觉引导：库存充足绿色高亮，不足橙色/红色高亮
- 补货单创建后提供**跳转链接**到下游单据详情（PR 编号/工单编号），方便追踪
- **【刷新状态】按钮**：手动触发 demands ↔ 履行计划行状态对账

**订单级按钮规则**：

| 条件 | 显示按钮 |
|------|----------|
| 至少一行 Allocated 且未发货 | 【创建发货申请】 |
| 存在自制缺货行且未生成工单 | 【生成生产工单】 |
| 存在外购缺货行且未生成请购单 | 【生成采购申请】 |
| 存在委外缺货行且未生成委外单 | 【生成委外订单】（预留） |
| 所有行 shipped_qty + cancelled_qty ≥ ordered_qty（含容差） | 【完成订单】 |
| 存在 open_qty > 0 的行 | 【取消订单】 |

## 4. 完整业务流程

```
销售订单确认（Confirmed）
  │
  ├─ 1. 原子性库存预留 + 生成 OrderFulfillmentPlan（同一事务）
  │     逐行：需求数量 vs ATP 可用量（SELECT ... FOR UPDATE）
  │     ├── 库存充足（含 NonInventory）→ 硬预留，行状态 = Allocated
  │     └── 库存不足 → 部分预留，行状态 = Pending
  │
  ├─ 2. 缺货行写入 demands 表 + DomainEventBus.publish(DemandCreated)
  │     Payload 精简：demand_id, order_id, product_id, acquire_channel
  │     ├── SelfProduced → demand.acquire_channel = 1
  │     ├── Purchased    → demand.acquire_channel = 2
  │     └── Outsourced   → demand.acquire_channel = 3
  │
  ├─ 3. 履约工作台展示
  │     订单头：加权进度条（shipped/ordered 比例）
  │     履约 Tab：逐行状态 + 批量操作 + 下游单据跳转链接
  │     【刷新状态】按钮：手动触发对账
  │
  ├─ 4. 用户操作
  │     │
  │     ├─ Allocated 行 → 立即发货 / 等齐一起发
  │     │
  │     └─ Pending 行 → 批量生成补货单（草稿）
  │          ├── 自制 → 生产工单草稿 → 计划员审批 → 下达执行
  │          ├── 外购 → 请购单 PR → 采购审批 → 转采购订单 PO
  │          └── 委外 → 委外订单草稿 → 审批 → 执行
  │          （补货单确认后 → DemandConfirmed 事件 → demand → Processing
  │            → 履行计划行 → Producing/Purchasing）
  │
  ├─ 5. 补货执行（入库后）
  │     库存补充 → ReplenishmentAllocationStrategy 分配
  │     → 匹配待满足履行计划行 → 原子性重新预占
  │     → 履行计划行 → Allocated/Fulfilled
  │     → 通知："订单X所有行已备齐"
  │
  └─ 6. 全部备齐 → 创建发货申请 → 发货消耗预留
           → recalc_order_header_status（含 cancelled_qty 防御）
           → Shipped → Completed

  ── 后台 ──
  定时对账任务：检测 demands ↔ 履行计划行状态不一致，自动修复
```

## 5. 异常与边界情况

| 场景 | 处理方式 |
|------|----------|
| 补货单据中途取消 | `DemandRejected` 事件 → demand 回退到 Open → 履行计划行回退到 Pending |
| 部分已发货后取消剩余 | `cancelled_qty` 增加，`ordered_qty` 不变，头状态通过 `recalc` 重新计算 |
| 部分已发货后取消整单 | 走退货退款流程，订单变为 Cancelled 但保留已发货记录，财务介入 |
| 库存充足但用户想走自制（批次要求） | 后期在履行计划行上允许人工覆盖补充方式 |
| 已发货行不可再取消或减少 | 只能取消未发货剩余量（增加 `cancelled_qty`） |
| 费用/服务类产品 | 跳过库存校验和补货，直接标记 Allocated |
| acquire_channel = Legacy(9) | 允许确认（行为等同自制），后台记录日志驱动清洗 |
| 预留失败（ATP 被抢） | 明确错误码 `INSUFFICIENT_STOCK`，不静默创建负数库存 |
| 下游驳回/暂停补货 | `DemandRejected` → demand 回退到 Open → 对账任务兜底 |
| 头状态同步事件丢失 | 幂等检查函数可重入，任何时候调用都能恢复正确状态 |
| demands ↔ 履行计划行状态不一致 | 定时对账任务 + 前端【刷新状态】按钮 |
| 入库量 < 所有缺货总量 | `ReplenishmentAllocationStrategy`（FIFO 按需求日期）按优先级分配 |
| 行被全部取消（shipped=0, cancelled=ordered） | 行状态 → Cancelled，不参与头状态计算 |

## 6. 涉及的模块和文件

### 6.1 abt-core（模型 + 服务层）

| 文件 | 改动内容 |
|------|----------|
| `src/master_data/product/model.rs` | 新增 `AcquireChannel` 枚举（含 Legacy），`acquire_channel` 从 `ProductMeta` JSONB 提升为 `Product` 独立列 + B-tree 索引 |
| `src/sales/sales_order/model.rs` | 头行状态分离：订单头删除 `InProduction`，新增 `SalesOrderLineStatus` 枚举 + 四量模型（含 `cancelled_qty`） |
| `src/sales/sales_order/service.rs` | 新增履行计划接口 + 确认时预留 + recalc_header_status + 对账接口 |
| `src/sales/sales_order/implt.rs` | 确认时原子性预留 + 生成履行计划 + 写入 demands + 幂等头状态同步 |
| `src/sales/sales_order/repo.rs` | 履行计划 CRUD + demands 表操作 + 对账查询 |
| `src/sales/sales_order/mod.rs` | 导出新类型 |
| `src/shared/types/` | `DomainEventType` 新增 `DemandCreated / DemandConfirmed / DemandRejected` |
| `migrations/` | acquire_channel 枚举约束 + 履行计划表 + demands 表 + 订单行 cancelled_qty + 状态机调整 |

### 6.2 abt-web（前端）

| 文件 | 改动内容 |
|------|----------|
| `src/pages/sales_order_detail.rs` | 加权进度条 + 履约工作台 Tab + 动态按钮 + 补货单跳转链接 + 刷新状态按钮 |
| `src/routes/order.rs` | 新增路由（生成工单草稿 / 生成请购单 / 刷新状态） |

### 6.3 设计文档

| 文件 | 改动内容 |
|------|----------|
| `docs/uml-design/09-master-data.html` | `AcquireChannel` 枚举、`acquire_channel` 从 `ProductMeta` 提升为 `Product` 独立列 |
| `docs/uml-design/01-sales.html` | 头行状态分离、四量模型、`OrderFulfillmentPlan` 实体、demands 表、分配策略接口、对账机制 |

## 7. 实施阶段

### P0：AcquireChannel 枚举化（前置）
- 新增 `AcquireChannel` 枚举（含 Legacy = 9）
- 数据迁移脚本：现有值映射到确定值，不留 Unknown
- `acquire_channel` 从 `ProductMeta` JSONB 提升为 `Product` 独立列 + CHECK 约束 + B-tree 索引
- 双写过渡期：新列和 JSONB 并存，过渡期后清理 JSONB 内的 acquire_channel 字段
- `DomainEventType` 新增 `DemandCreated / DemandConfirmed / DemandRejected`

### P1：核心履约模型（原 P1+P2 合并，强耦合不可拆分）
交付物：
- 订单头状态机重构（删除 `InProduction`）
- 订单行状态枚举 `SalesOrderLineStatus`（含 `Cancelled`）
- **四量模型**（ordered_qty / shipped_qty / cancelled_qty / open_qty）
- `OrderFulfillmentPlan` 实体 + 履行计划表
- 确认时原子性库存预留 + 履行计划生成
- 幂等头状态同步函数 `recalc_order_header_status`（含 cancelled_qty 防御）
- `ReplenishmentAllocationStrategy` 接口定义（P5 实现 FIFO）
- **此阶段暂不做 UI，但 API 必须能跑通集成测试**
- **并发压测**：多个事务同时确认订单抢库存，验证 `FOR UPDATE` 和预留逻辑

### P2：demands 实体 + 事件驱动（销售模块解耦）
- `demands` 业务表 + CRUD
- 确认时缺货行自动写入 demands + `DomainEventBus.publish(DemandCreated)`（Payload 精简）
- demand 状态生命周期管理（Open / Processing / Done / Cancelled）
- 下游模块 EventProcessor 消费 `DemandCreated` 事件，读取 demands 表处理
- 下游确认/驳回时发布 `DemandConfirmed` / `DemandRejected` 事件回写

### P3：履约工作台 UI
- 加权进度条 + 履约工作台 Tab/抽屉
- 动态按钮渲染 + 批量操作
- 补货单创建后跳转链接
- **【刷新状态】按钮**：手动触发 demands ↔ 履行计划行对账
- **原型阶段让真实用户（销售内勤、采购员）走一遍操作流程**，确认按钮文案、跳转逻辑、状态刷新频率
- 验收确认："一键生成采购申请"仅限本订单是否可接受？如果不可接受，提前暴露跨订单合并 UI 入口

### P4：下游模块集成
- 采购模块：查询 demands(Purchased, Open) → 合并生成 PR
- 生产模块：查询 demands(SelfProduced, Open) → 生成工单草稿
- 下游确认后 → `DemandConfirmed` 事件 → demand → Processing → 履行计划行 → Producing/Purchasing

### P5：补货完成闭环
- 实现 `FifoByRequiredDate` 分配策略
- 入库后匹配待满足履行计划行 → 原子性重新预占
- 通知："订单X所有行已备齐"
- 履行计划行全 Fulfilled 后触发发货
- **定时对账任务**：检测并修复 demands ↔ 履行计划行状态不一致

### 远期规划（不在本次范围）

| 项目 | 说明 | 触发条件 |
|------|------|----------|
| MRP（物料需求计划） | 独立规划模块，基于 demands 表做合并采购、产能评估 | 订单量 > 100/天 |
| WebSocket 实时推送 | 替代当前通知轮询，入库后实时更新履行计划 | 用户反馈刷新不及时 |
| 人工覆盖补充方式 | 允许用户在履行计划行上手动选择补货路径 | 业务提出批次/渠道切换需求 |
| 多订单合并采购 | 采购模块跨订单合并同类需求，与供应商谈折扣 | 采购员反馈重复操作 |
| 分配策略扩展 | 优先级加权、人工指定、仓库/批次维度 | 多仓库/批次管理上线 |
| Reservation 独立实体 | 超时释放、手动干预、预留生命周期管理 | 出现预留长期占用不释放的问题 |
| Saga 补偿机制 | 跨模块长流程编排（生产驳回→释放预留→重排） | 下游模块 > 3 且驳回场景复杂 |
| 读写分离/需求池读模型 | 采购/生产计划员专用查询视图 | demands 数据量 > 10万 |
| InventoryType 与 AcquireChannel 分离 | 非库存物品仍需采购跟踪的场景 | 出现"服务类需要采购跟踪"需求 |
| 产品主通道+备选通道 | 一个产品配置 fallback 补货路径 | 单一渠道频繁缺货 |
| 模拟预留按钮 | 不实际扣减，仅供计划员评估 | 计划员提出预评估需求 |
| 等齐发货组（Fulfillment Group） | 波次拣货、成套发货 | 仓储管理精细化 |
| 大订单异步分批处理 | >100 行时预留和状态重算异步化 | 单订单行数 > 100 |
| 审计轨迹（quantity_history） | 独立记录每次 shipped_qty/cancelled_qty 变更 | 审计要求升级，AuditLogService 不够用 |
| 对账告警体系 | 对账不一致时自动告警 | 出现状态漂移导致业务损失 |
