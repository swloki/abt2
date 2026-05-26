---
name: reconciliation
description: 对账单模块设计，含月度自动汇总发货/退货明细、调整项管理、净额计算和状态流转
---

# Reconciliation Module Design

Date: 2026-05-21

## Overview

对账单用于月末与客户核对应收账款。创建时自动汇总指定客户当月的发货和退货明细，支持手工调整项，最终生成净额供双方确认。

## Scope

**包含：**
- 按客户 + 年月创建对账单（唯一约束）
- 自动汇总已完成的发货明细和退货明细
- 调整项管理（增/删/改，仅 Draft 状态）
- 净额自动计算（shipping_total - return_total + adjustment_total）
- 状态流转（Draft → Confirmed → Approved）

**不包含：**
- 应收账款自动生成（后续财务模块）
- 对账单导出/打印
- 差异自动标记

## Data Model

### reconciliation_statements

```sql
CREATE TABLE reconciliation_statements (
    statement_id     BIGSERIAL PRIMARY KEY,
    statement_no     VARCHAR(32) NOT NULL UNIQUE,    -- RC-YYYY-MM-NNNNN
    customer_name    VARCHAR(200) NOT NULL,
    period_year      SMALLINT NOT NULL,
    period_month     SMALLINT NOT NULL,
    shipping_total   DECIMAL(14,2) NOT NULL DEFAULT 0,
    return_total     DECIMAL(14,2) NOT NULL DEFAULT 0,
    adjustment_total DECIMAL(14,2) NOT NULL DEFAULT 0,
    net_amount       DECIMAL(14,2) NOT NULL DEFAULT 0,
    status           SMALLINT NOT NULL DEFAULT 1,    -- 1=草稿,2=已确认,3=已审批
    remark           TEXT,
    operator_id      BIGINT,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at       TIMESTAMPTZ
);

-- 同一客户同月只允许一条有效对账单
CREATE UNIQUE INDEX idx_reconciliation_period
    ON reconciliation_statements(customer_name, period_year, period_month)
    WHERE deleted_at IS NULL;
```

### reconciliation_items

```sql
CREATE TABLE reconciliation_items (
    item_id       BIGSERIAL PRIMARY KEY,
    statement_id  BIGINT NOT NULL REFERENCES reconciliation_statements(statement_id),
    source_type   VARCHAR(20) NOT NULL,              -- shipping / return / adjustment
    source_id     BIGINT,                            -- 发货行ID 或 退货行ID
    product_id    BIGINT,
    product_code  VARCHAR(100),
    product_name  VARCHAR(200),
    unit          VARCHAR(20),
    quantity      DECIMAL(14,6) NOT NULL,
    unit_price    DECIMAL(14,6) NOT NULL,
    amount        DECIMAL(14,2) NOT NULL,             -- 正数=发货, 负数=退货, 可正可负=调整
    remark        TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

Key decisions:
- `source_type` 区分行项目来源：shipping（发货）、return（退货）、adjustment（调整）
- 退货行的 `amount` 为负数
- `adjustment` 行的 `source_id` 为 NULL
- 汇总金额由 SQL 聚合自动计算：`net_amount = SUM(amount)`

## Proto Definition

File: `proto/abt/v1/reconciliation.proto`

7 个 RPC：
- `CreateReconciliation` — 创建对账单（自动汇总发货/退货明细）
- `AddReconciliationAdjustment` — 添加/替换调整项（Draft 状态）
- `UpdateReconciliation` — 更新备注（Draft 状态）
- `DeleteReconciliation` — 软删除（仅 Draft）
- `GetReconciliation` — 按ID查询（含行项目）
- `ListReconciliations` — 分页查询（支持 status/period_year/period_month 过滤）
- `UpdateReconciliationStatus` — 状态变更

Key decisions:
- `CreateReconciliationRequest` 仅需 customer_name + period_year + period_month，行项目自动汇总
- `AddReconciliationAdjustment` 整体替换调整项（先删后插），然后自动重算汇总
- 不提供单独的行项目 CRUD，行项目由系统管理

## Business Logic

### create
1. 校验该客户该月不存在已创建的对账单（唯一约束）
2. 查询已发货（status=3）的发货明细：按 shipped_at 落在当月范围内
3. 查询已完成（status=4）的退货明细：按 created_at 落在当月范围内
4. 调用 `DocumentSequenceRepo::next_number(executor, "RC")` 生成编号
5. 插入主表 + 发货行项目 + 退货行项目
6. 调用 `ReconciliationRepo::update_totals` 重算汇总金额

### add_adjustments
1. 校验状态为 Draft(1)
2. 删除旧调整项（`source_type = 'adjustment'`）
3. 插入新调整项
4. 重算汇总（SQL 聚合：shipping_total / return_total / adjustment_total / net_amount）

### update
- 仅 Draft(1) 状态可修改备注

### delete
- 仅 Draft(1) 状态可删除

### update_status

| From → To | 含义 |
|-----------|------|
| Draft(1) → Confirmed(2) | 确认对账 |
| Confirmed(2) → Approved(3) | 审批通过 |

## Status Enum Mapping

| DB (i16) | Proto Enum | 含义 |
|----------|-----------|------|
| 1 | RECONCILIATION_STATUS_DRAFT | 草稿 |
| 2 | RECONCILIATION_STATUS_CONFIRMED | 已确认 |
| 3 | RECONCILIATION_STATUS_APPROVED | 已审批 |

## File List

| Layer | Files |
|-------|-------|
| Proto | `proto/abt/v1/reconciliation.proto` |
| Migration | `abt/migrations/050_create_reconciliation.sql` |
| Model | `abt/src/models/reconciliation.rs` |
| Repository | `abt/src/repositories/reconciliation_repo.rs` |
| Service | `abt/src/service/reconciliation_service.rs` |
| Impl | `abt/src/implt/reconciliation_service_impl.rs` |
| Handler | `abt-grpc/src/handlers/reconciliation.rs` |
