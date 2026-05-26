---
title: "feat: 入库库位占用校验"
type: feat
status: active
date: 2026-05-19
origin: docs/superpowers/specs/2026-05-19-stockin-location-occupancy-validation-design.md
---

# 入库库位占用校验

## Summary

在 `StockIn` 接口中增加库位占用校验：入库到某库位时，若该库位已有其他产品的库存（quantity > 0），则拒绝入库并返回包含产品名称和数量的详细错误信息。新增一个轻量 repository 方法查询库位占用者，复用已有的 `ServiceError::Conflict` 错误类型。

---

## Requirements

- R1. 一个库位同一时间只能存放一个产品（quantity > 0 时）
- R2. 库位库存为 0 时，允许其他产品入库
- R3. 同一产品入库到已占用的库位，正常累加
- R4. 库位被其他产品占用时，返回 `AlreadyExists` 错误，包含占用产品的名称和数量
- R5. 仅影响 `StockIn` 接口，`StockOut` 和 `Adjust` 不变

---

## Scope Boundaries

- 不修改 `StockOut` / `Adjust` 接口
- 不修改前端 `LocationSelect` 组件
- 不修改已有的 `get_location_occupants` 方法（导入流程使用）
- 不修改 proto 定义

---

## Context & Research

### Relevant Code and Patterns

- `abt/src/repositories/inventory_repo.rs` — 已有 `get_or_create_for_update` (行锁)、`get_location_occupants` (批量查询)
- `abt/src/implt/inventory_service_impl.rs` — `stock_in` 方法当前流程：验证库位 → 验证数量 → 获取/创建记录 → 更新 → 记录日志
- `common/src/error.rs` — 已有 `ServiceError::Conflict { resource, message }` 映射到 `Code::AlreadyExists`，支持 rich error details
- `abt/src/repositories/product_repo.rs` — `find_by_id` 方法返回 `Option<Product>`，包含 `pdt_name` 字段

### Patterns to follow

- Repository 方法使用 `Executor` 参数以支持事务内调用（与 `get_or_create_for_update` 一致）
- 服务层错误使用 `ServiceError` 枚举通过 `anyhow::Error::from()` 传播，gRPC handler 中 `err_to_status` 自动转换
- 现有 `stock_in` 已在事务中执行，新增查询使用同一个 `executor` 保持事务一致性

---

## Key Technical Decisions

- **新增专用 repository 方法而非复用 `get_location_occupants`**: `get_location_occupants` 是批量方法，返回 `HashMap<i64, Vec<i64>>`，不含 quantity，且接收 `PgPool` 而非 `Executor`，不适合事务内单次查询。新增 `find_occupant_by_location(executor, location_id)` 更轻量、与事务兼容。
- **使用 `ServiceError::Conflict` 返回错误**: 项目已有的 `ServiceError::Conflict` 映射到 gRPC `AlreadyExists`，支持 rich error details，与需求文档一致。

---

## Implementation Units

### U1. 新增 `find_occupant_by_location` repository 方法

**Goal:** 提供查询指定库位上占用产品的 repository 方法

**Requirements:** R1, R2

**Dependencies:** None

**Files:**
- Modify: `abt/src/repositories/inventory_repo.rs`

**Approach:**
- 在 `InventoryRepo` 中新增 `find_occupant_by_location(executor, location_id)` 方法
- SQL: `SELECT product_id, quantity FROM inventory WHERE location_id = $1 AND quantity > 0 LIMIT 1`
- 返回 `Result<Option<(i64, Decimal)>>` — 占用产品的 (product_id, quantity)，无占用则 None

**Patterns to follow:**
- 方法签名与 `get_or_create_for_update` 一致，使用 `Executor` 参数

**Test scenarios:**
- Happy path: 库位无库存记录 → 返回 None
- Happy path: 库位有产品 A 库存 (quantity > 0) → 返回 Some((A_id, qty))
- Edge case: 库位有产品 A 库存 (quantity = 0) → 返回 None
- Edge case: 库位有多条记录 (不同产品)，至少一条 quantity > 0 → 返回第一条

**Verification:**
- `cargo clippy` 无错误
- 方法签名编译通过

---

### U2. 在 `stock_in` 中增加库位占用校验

**Goal:** 入库时校验库位是否被其他产品占用，占用则返回详细错误

**Requirements:** R1, R2, R3, R4, R5

**Dependencies:** U1

**Files:**
- Modify: `abt/src/implt/inventory_service_impl.rs`

**Approach:**
- 在 `stock_in` 方法中，验证数量为正数之后、`get_or_create_for_update` 之前，增加校验逻辑：
  1. 调用 `InventoryRepo::find_occupant_by_location(executor, req.location_id)`
  2. 若返回 `Some((occupant_product_id, qty))` 且 `occupant_product_id != req.product_id`
  3. 通过 `ProductRepo::find_by_id(&self.pool, occupant_product_id)` 查询产品名称
  4. 返回 `Err(anyhow::Error::from(ServiceError::Conflict { resource: "location".into(), message: format!("库位已被产品 {} 占用（库存: {}），请先清空该库位", pdt_name, qty) }))`
- 添加 `ProductRepo` 和 `ServiceError` 到 import 列表

**Patterns to follow:**
- `ServiceError` 使用方式参见其他 impl 文件中的 `NotFound` / `Conflict` 用法
- 错误通过 `anyhow::Error::from()` 传播，在 gRPC handler 层由 `err_to_status` 自动转换

**Test scenarios:**
- Happy path: 空库位入库新产品 → 正常入库
- Happy path: 库位已有同产品 (quantity > 0) → 正常累加入库
- Happy path: 库位已有同产品 (quantity = 0) → 正常入库
- Error path: 库位已有其他产品 (quantity > 0) → 返回 `AlreadyExists` 错误，包含产品名称和数量
- Happy path: 库位已有其他产品 (quantity = 0) → 允许入库

**Verification:**
- `cargo clippy` 无错误
- `cargo build` 编译通过

---

## System-Wide Impact

- **Interaction graph:** 仅影响 `StockIn` gRPC 调用路径。`StockOut`、`Adjust`、Excel 导入不受影响
- **Error propagation:** 新增的 `ServiceError::Conflict` 通过已有的 `err_to_status` 转换链传播到 gRPC 层，前端可通过 ConnectRPC `findDetails()` 提取结构化信息
- **State lifecycle risks:** 校验在事务内、`get_or_create_for_update` 之前执行。由于 `find_occupant_by_location` 使用同一 `executor`，在 `FOR UPDATE` 行锁获取前完成检查，不会产生竞态。后续 `get_or_create_for_update` 的行锁保证并发安全
- **Unchanged invariants:** Excel 导入流程的 `get_location_occupants` 不受影响，导入和手动入库使用独立的校验路径

---

## Risks & Dependencies

| Risk | Mitigation |
|------|------------|
| 并发入库到同一库位时，两个请求可能同时通过校验 | `get_or_create_for_update` 使用 `FOR UPDATE` 行锁，后续操作串行化。校验在行锁前执行，极端并发下可能有两个请求同时通过校验，但后执行的请求会在 `get_or_create_for_update` 处等待，此时已创建的记录不冲突（product_id 不同时先通过校验的请求已占住）。实际业务场景中，同时入库不同产品到同一库位极少发生 |

---

## Sources & References

- **Origin document:** [docs/superpowers/specs/2026-05-19-stockin-location-occupancy-validation-design.md](docs/superpowers/specs/2026-05-19-stockin-location-occupancy-validation-design.md)
- **前端需求文档:** `E:\work\front\abt_front\docs\api-requirement-stockin-location-validation.md`
- Related code: `abt/src/repositories/inventory_repo.rs`, `abt/src/implt/inventory_service_impl.rs`, `common/src/error.rs`
