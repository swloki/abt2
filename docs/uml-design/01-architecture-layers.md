# 分层架构设计 — abt-core Crate

> 日期：2026-05-22
> 状态：设计中
> 配套：[00-module-dependencies.html](./00-module-dependencies.html)（模块间接口依赖关系总览）

## 1. 背景与目标

当前 `abt` crate 内所有模块平铺在同一目录层级：

```
abt/src/
  models/        → 35+ 文件平铺
  service/       → 35+ 文件平铺
  repositories/  → 35+ 文件平铺
  implt/         → 35+ 文件平铺
```

随着 MES、WMS、SRM、FMS 等模块加入，文件数将突破 100，带来以下问题：

- **依赖方向不可见** — 无法从目录结构看出模块间的依赖关系
- **改一个模块要看四个目录** — Sales 的改动散落在 models/service/repositories/implt 四处
- **共享层无归属** — DocumentSequence、CostEntry 等共享服务没有明确的边界
- **未来扩展困难** — 新模块加入时不知道该依赖谁、被谁依赖

**目标**：创建 `abt-core` crate，按业务域组织模块，不动现有 `abt` 代码。

## 2. 设计原则

### 2.1 单向依赖

```
shared/ ← sales/ ← （禁止反向）
shared/ ← wms/   ← （禁止反向）
shared/ ← mes/   ← （禁止反向）
shared/ ← workflow/
```

业务模块只能依赖 `shared` 层和更底层的模块。Rust 模块系统天然阻止反向依赖——`shared/mod.rs` 中不会出现 `use crate::sales::*`。

### 2.2 三种事务模式（来自评审结论）

| 模式 | 适用场景 | 失败策略 |
|------|---------|---------|
| ① 同步强一致（主事务内） | 库存预留、质量关卡硬门 | 失败即回滚主事务 |
| ② 独立事务 | CostEntry 成本记录 | 主事务提交后开新事务，失败不影响主业务 |
| ③ 异步 Outbox | DocumentLink、Workflow 触发、通知 | 写 Outbox + NOTIFY，后台消费 |

### 2.3 状态机与流程分离

- **StateMachineService** — 纯状态转移规则（声明式、无外部依赖）
- **WorkflowService** — 审批、多步骤编排、Saga 补偿
- Workflow 通过调用 StateMachine 推进状态，禁止 StateMachine 自行推进需审批的状态

### 2.4 质量关卡硬门

业务层显式调用 `InspectionResultService.is_passed()` 检查，不通过则返回错误。

## 3. Crate 结构

```
abt-core/
  Cargo.toml                    # 依赖 common crate（PgExecutor）、sqlx、async-trait 等
  src/
    lib.rs                      # 声明所有模块，暴露工厂函数

    shared/                     # 共享基础设施层
      mod.rs
      document_sequence/        # 统一编号
        mod.rs
        model.rs                # DocumentSequence + DocumentType 枚举
        repo.rs
        service.rs              # async trait
        implt.rs                # 具体实现
      document_link/            # 文档关联图谱
        mod.rs, model.rs, repo.rs, service.rs, implt.rs
      inventory_reservation/    # 库存预留
        mod.rs, model.rs, repo.rs, service.rs, implt.rs
      cost_entry/               # 成本累积账本（独立事务）
        mod.rs, model.rs, repo.rs, service.rs, implt.rs
      event_bus/                # 领域事件总线
        mod.rs, model.rs, service.rs, implt.rs
      state_machine/            # 统一状态机
        mod.rs, model.rs, service.rs, implt.rs
      audit_log/                # 审计日志
        mod.rs, model.rs, repo.rs, service.rs, implt.rs
      idempotency/              # 幂等控制
        mod.rs, model.rs, repo.rs, service.rs, implt.rs

    sales/                      # 销售 CRM
      mod.rs
      quotation/
        mod.rs, model.rs, repo.rs, service.rs, implt.rs
      sales_order/
        mod.rs, model.rs, repo.rs, service.rs, implt.rs
      shipping_request/
        mod.rs, model.rs, repo.rs, service.rs, implt.rs
      sales_return/
        mod.rs, model.rs, repo.rs, service.rs, implt.rs
      reconciliation/
        mod.rs, model.rs, repo.rs, service.rs, implt.rs

    purchase/                   # 采购 SRM（未来）
      mod.rs

    wms/                        # 仓储 WMS（未来）
      mod.rs

    mes/                        # 生产 MES（未来）
      mod.rs

    om/                         # 委外管理（未来）
      mod.rs

    qms/                        # 质量管理（未来）
      mod.rs

    fms/                        # 财务管理（未来）
      mod.rs

    workflow/                   # 工作流引擎（从 abt 迁移）
      mod.rs
```

## 4. 模块内部结构约定

每个业务模块文件夹包含：

```
sales_order/
  mod.rs        # pub mod model/repo/service/implt; pub use 重新导出
  model.rs      # 数据结构（对应数据库行 + Proto 消息的中间态）
  repo.rs       # sqlx 原始 SQL 查询，返回 anyhow::Result
  service.rs    # async trait 定义业务接口
  implt.rs      # 基于 repo + shared service 的具体实现
```

**命名约定**：实现文件用 `implt`（与现有 `abt` crate 中 `abt/src/implt/` 保持一致）。

### 4.1 mod.rs 范例

```rust
// sales_order/mod.rs
pub mod model;
pub mod repo;
pub mod service;
pub mod implt;

// 重新导出主要类型，简化外部引用
pub use model::{SalesOrder, SalesOrderItem, SalesOrderStatus};
pub use service::SalesOrderService;
```

### 4.2 依赖声明范例

```rust
// sales_order/implt.rs
use crate::shared::document_sequence::DocumentSequenceService;
use crate::shared::inventory_reservation::InventoryReservationService;
use crate::shared::state_machine::StateMachineService;
use crate::shared::event_bus::DomainEventBus;

// 不依赖其他业务模块（如 wms、mes）
```

## 5. 与现有代码的关系

```
abt/           →  现有代码，继续运行，不改一行
abt-core/      →  新架构，按模块组织
abt-grpc/      →  gRPC handler 层，未来逐步切换到调用 abt-core
common/        →  PgExecutor 类型别名，abt 和 abt-core 共用
```

### 5.1 Workspace 变更

```toml
# Cargo.toml (workspace root)
[workspace]
members = ["common", "abt", "abt-grpc", "abt-macros", "abt-core"]
```

### 5.2 依赖关系

```
abt-core → common（PgExecutor）
abt-grpc → abt-core（新模块的工厂函数）
abt-grpc → abt     （旧模块的工厂函数，过渡期并存）
```

### 5.3 迁移策略

1. **Phase 1** — 创建 `abt-core` 骨架，实现 `shared/` 层（DocumentSequence、InventoryReservation、CostEntry）
2. **Phase 2** — 从 `abt` 迁移 Sales 模块到 `abt-core`，`abt-grpc` handler 逐步切换调用源
3. **Phase 3** — 新模块（WMS、MES、SRM）直接在 `abt-core` 中实现
4. **Phase 4** — `abt` crate 中剩余模块逐步迁移完毕后废弃

迁移过程中 `abt` 和 `abt-core` 并存，`abt-grpc` 可以同时调用两者。

## 6. CostEntry 独立事务实现模式

```rust
// 伪代码 — ShippingRequestServiceImpl
async fn confirm(&self, executor: &mut PgExecutor, request_id: Uuid) -> Result<()> {
    let mut tx = executor.begin().await?;

    // ① 主事务：质量关卡 + 状态推进 + 库存操作
    let inspection = InspectionResultService::is_passed(&mut tx, request_id).await?;
    if !inspection { return Err(anyhow!("OQC 检验未通过")); }

    StateMachineService::transition(&mut tx, request_id, "Shipped").await?;
    InventoryReservationService::fulfill(&mut tx, request_id).await?;
    InventoryTransactionService::record(&mut tx, ...).await?;

    tx.commit().await?;  // 主事务提交

    // ② 独立事务：成本记录
    let mut cost_tx = executor.begin().await?;
    CostEntryService::create(&mut cost_tx, ...).await?;
    cost_tx.commit().await?;  // 失败不影响主业务

    Ok(())
}
```

## 7. 文件命名与导出规范

| 文件 | 职责 | 导出 |
|------|------|------|
| `mod.rs` | 模块声明 + 重新导出 | `pub use` 主要类型 |
| `model.rs` | 结构体 + 枚举 | `pub struct` / `pub enum` |
| `repo.rs` | SQL 查询 | `pub async fn` |
| `service.rs` | 业务接口 | `#[async_trait] pub trait` |
| `implt.rs` | 具体实现 | `pub struct XxxServiceImpl; impl XxxService for XxxServiceImpl` |

### 工厂函数

`abt-core/src/lib.rs` 中为每个 service 暴露工厂函数：

```rust
pub fn get_sales_order_service(ctx: AppContext) -> impl SalesOrderService {
    SalesOrderServiceImpl::new(
        get_document_sequence_service(ctx.clone()),
        get_inventory_reservation_service(ctx.clone()),
        get_state_machine_service(ctx.clone()),
        get_event_bus(ctx),
    )
}
```

## 8. 审查决策记录

来自 UML 设计评审的采纳建议（2026-05-22）：

| # | 建议 | 决策 | 文档位置 |
|---|------|------|---------|
| 1 | 循环依赖风险 | 单向依赖，由模块结构强制 | 00-module-dependencies.html 原则② |
| 2 | 同步/异步边界模糊 | 三种事务模式明确划分 | 00-module-dependencies.html 事件驱动模式 |
| 3 | StateMachine vs Workflow | 分离职责 | 00-module-dependencies.html 原则④ |
| 4 | CostEntry 事务过长 | 独立事务 | 00-module-dependencies.html 原则⑥ + §6 |
| 5 | 质量硬门实现缺失 | 显式业务检查 | 00-module-dependencies.html 原则⑤ |
| 6 | DocumentType 枚举扩展性 | 不采纳，Rust 枚举优势 | — |
| 7 | 审计日志性能 | 原则认可，优先级低 | — |
