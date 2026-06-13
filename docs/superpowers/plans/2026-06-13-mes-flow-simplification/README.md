# MES 流程简化 — 实现计划总览

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 MES 从 6 步 4+ 次手动操作简化为 3 步 2 次操作，同时修复 13 个数据正确性问题

**Architecture:** 分 4 个阶段渐进交付，每阶段可独立上线。阶段 1 修复 P0 数据错误（止血），阶段 2 添加安全网（反下达）和 picking 模式，阶段 3 实现一键贯通，阶段 4 完成前端和排程。

**Tech Stack:** Rust (abt-core lib) + PostgreSQL + sqlx + async-trait

**Spec:** `docs/superpowers/specs/2026-06-13-mes-flow-simplification-design.md`

---

## 阶段计划文件

| 文件 | 阶段 | 目标 | 前置 |
|------|------|------|------|
| [phase-1-止血.md](./phase-1-止血.md) | 阶段 1：止血 | 修复 4 个 P0/P1 数据错误 | 无 |
| [phase-2-安全网.md](./phase-2-安全网.md) | 阶段 2：安全网 + picking | unrelease + material_consumption_mode | 阶段 1 |
| [phase-3-贯通.md](./phase-3-贯通.md) | 阶段 3：一键贯通 | 批量下达 + 预校验 + 容差 | 阶段 2 |
| [phase-4-前端.md](./phase-4-前端.md) | 阶段 4：前端 + 排程 | 前端页面 + 排程 V1 + 文档 | 阶段 3 |

## 当前代码关键文件

| 文件 | 职责 | 当前问题 |
|------|------|---------|
| `abt-core/src/mes/work_order/implt.rs` | 工单 Service 实现，含 `release()` | 工序从 BOM 叶子节点生成（错）、成品预留（错）、无 BOM 快照 |
| `abt-core/src/wms/backflush/implt.rs` | 倒冲执行，含 `execute()` | `warehouse_id: 0` 硬编码 |
| `abt-core/src/wms/material_requisition/implt.rs` | 领料单 Service，含 `create_for_work_order()` | 只创建单头无明细、用 work_center_id 当 warehouse_id |
| `abt-core/src/mes/production_plan/implt.rs` | 生产计划 Service，含 `release_to_work_orders()` | `sales_order_id: None`、只创建不 release |
| `abt-core/src/master_data/routing/` | 工艺路线 Service + Repo | `find_steps()` 已 JOIN labor_process_dicts，可直接获取 process_name |
| `abt-core/src/master_data/bom/` | BOM Service + 快照 | `BomSnapshotRepo.create()` 可创建快照；无 `find_by_product_code` 方法 |

## 共享约定

- **按需工厂模式**：struct 只持 `PgPool`，方法体通过 `new_xxx_service(pool.clone())` 获取接口实例
- **错误处理**：所有错误通过 `?` 传播或 `map_err` 转换为 `DomainError`
- **模块边界**：跨模块调用只通过 Service trait，禁止直接访问其他模块的 Repository
- **验证**：`cargo clippy` 通过即可，不用 `cargo run`

## 数据流图

```
release() 阶段 1 简化路径（backflush-only）:

  WorkOrder(Draft)
      │
      ├── 1. 乐观锁状态 → Released
      ├── 2. BOM 快照（查已发布 BOM → 创建/获取 snapshot → 写入 bom_snapshot_id）
      ├── 3. 工序创建（Routing steps → WorkOrderRouting / 虚拟默认工序）
      ├── 4. 创建 ProductionBatch
      ├── 5. 不预留（backflush 默认）
      ├── 6. 不创建领料单（backflush 默认）
      └── 7. 审计日志

  完工倒冲（backflush execute()）:
      从 BOM 快照获取叶子节点 → 计算 theoretical_qty
      → 4 级仓库策略确定 warehouse_id（不再硬编码 0）
      → InventoryTransaction 记录
```
