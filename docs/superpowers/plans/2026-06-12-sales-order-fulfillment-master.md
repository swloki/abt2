# 销售订单履行流程 — 总体实现计划

> **Goal:** 实现销售订单确认后的智能库存校验、业务分流、履行计划跟踪与需求池事件驱动解耦

**Architecture:** 头行状态分离 + 四量模型 + 履行计划实体 + 需求池(demands)事件驱动解耦。销售模块不再直接调用 MES/Purchase，而是写入 demands 表 + 发布领域事件，由下游模块自主消费。

**Tech Stack:** Rust / sqlx / PostgreSQL / async-trait / DomainEventBus

**设计文档:**
- `docs/design-proposal-sales-order-fulfillment-flow.md` — 完整设计方案
- `docs/uml-design/01-sales.html` — 销售模块类图设计 v2
- `docs/uml-design/09-master-data.html` — 主数据模块类图设计 v7

---

## 分阶段计划文件

| 阶段 | 计划文件 | 核心交付 |
|------|----------|----------|
| P0 | [032-acquire-channel-enum.md](./032-acquire-channel-enum.md) | AcquireChannel 枚举化 + 产品表独立列 + DomainEventType 扩展 |
| P1 | [033-core-fulfillment-model.md](./033-core-fulfillment-model.md) | 头行状态分离 + 四量模型 + 履行计划 + confirm 重写 |
| P2 | [034-demands-event-driven.md](./034-demands-event-driven.md) | demands 实体 + DemandService + 事件处理器 |

## 依赖关系

```
P0 (AcquireChannel) ──→ P1 (Core Fulfillment) ──→ P2 (Demands + Events)
```

每个阶段完成后必须通过 `cargo clippy -p abt-core` 才能进入下一阶段。

## 涉及文件总览

### 新建文件
| 文件 | 阶段 | 用途 |
|------|------|------|
| `abt-core/migrations/032_acquire_channel_enum.sql` | P0 | 产品表 acquire_channel 列 |
| `abt-core/migrations/033_sales_order_fulfillment.sql` | P1 | cancelled_qty/line_status/version + fulfillment_plan_lines 表 |
| `abt-core/migrations/034_demands.sql` | P2 | demands 表 |

### 修改文件
| 文件 | 阶段 | 改动 |
|------|------|------|
| `abt-core/src/master_data/product/model.rs` | P0 | AcquireChannel 枚举 + Product/ProductMeta 更新 |
| `abt-core/src/master_data/product/repo.rs` | P0 | SQL 列更新 |
| `abt-core/src/master_data/product/implt.rs` | P0 | create/update 处理 acquire_channel |
| `abt-core/src/master_data/product/mod.rs` | P0 | 导出 AcquireChannel |
| `abt-core/src/shared/enums/event.rs` | P0 | DemandCreated/Confirmed/Rejected |
| `abt-core/src/sales/sales_order/model.rs` | P1 | 删除 InProduction + 新增枚举 + 更新 SalesOrderItem |
| `abt-core/src/sales/sales_order/service.rs` | P1+P2 | 新增方法 + DemandService trait |
| `abt-core/src/sales/sales_order/implt.rs` | P1+P2 | confirm 重写 + cancel_line + recalc 等 |
| `abt-core/src/sales/sales_order/repo.rs` | P1+P2 | 新增 repo + ITEM_COLUMNS 更新 |
| `abt-core/src/sales/sales_order/mod.rs` | P1+P2 | 导出新类型 |

## 验证方式

```bash
cargo clippy -p abt-core          # 每阶段主要验证手段
cargo test -p abt-core            # 运行单元测试
cargo build -p abt-core           # 编译验证
```
