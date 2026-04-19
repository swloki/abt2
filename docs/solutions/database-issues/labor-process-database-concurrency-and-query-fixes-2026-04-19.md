---
title: "劳务工序重设计：数据库并发安全与查询优化"
date: 2026-04-19
category: database-issues
module: abt
problem_type: database_issue
component: database
severity: high
symptoms:
  - "并发设置 BOM 劳务成本时价格快照与实际单价不一致"
  - "get_bom_labor_cost 执行 4 次数据库查询导致延迟"
  - "删除已被引用的工序后产生悬空引用"
  - "工序列表不支持按名称搜索"
root_cause: async_timing
resolution_type: code_fix
related_components:
  - labor_process_repo
  - labor_process_service
tags: [race-condition, n-plus-one-query, select-for-update, sqlx, validation, migration]
---

# 劳务工序重设计：数据库并发安全与查询优化

## Problem

劳务工序（labor process）功能在重设计过程中发现多个数据库交互层缺陷：价格快照存在竞态条件（并发读写无保护）、BOM 劳务成本查询存在 N+1 问题（4 次顺序查询）、删除工序缺少引用检查（导致悬空引用），以及列表查询不支持关键词搜索。

## Symptoms

- 并发创建 BOM 劳务成本记录时，工序单价可能在读取后被其他事务修改，导致快照价格与实际不一致
- `get_bom_labor_cost` 执行 4 次数据库查询（获取组 ID → 获取组详情 → 获取成员 → 获取成本项），响应延迟高
- 删除已被 BOM 劳务成本或工序组引用的工序后，产生悬空引用，后续查询报错
- 工序和工序组列表无法按名称搜索，用户只能翻页查找

## What Didn't Work

### 尝试1：sqlx::QueryBuilder 子查询方案

首次尝试修复价格快照竞态时，想在 `push_values` 闭包内嵌入 `SELECT` 子查询：

```rust
builder.push_values(items.iter(), |mut b, (process_id, quantity, remark)| {
    // 编译错误：Separated 没有 push_raw 方法
    b.push_raw(format!(
        "(SELECT unit_price FROM labor_process WHERE id = {})",
        process_id
    ));
});
```

**失败原因：** `sqlx::QueryBuilder::push_values` 闭包中的 `Separated` 结构体只有 `push_bind` 方法，没有 `push_raw`。无法在批量插入中内联子查询。

## Solution

### 修复1：价格快照竞态 — SELECT FOR UPDATE

将普通 `SELECT` 改为 `SELECT ... FOR UPDATE`，在事务内锁定工序行，防止并发修改价格：

```rust
// 新增 repo 方法
pub async fn lock_and_get_unit_prices(
    executor: Executor<'_>,
    process_ids: &[i64],
) -> Result<HashMap<i64, Decimal>> {
    let rows: Vec<(i64, Decimal)> = sqlx::query_as(
        "SELECT id, unit_price FROM labor_process WHERE id = ANY($1) FOR UPDATE"
    )
    .bind(process_ids)
    .fetch_all(executor)
    .await?;
    Ok(rows.into_iter().collect())
}

// service 层调用（在事务内）
let prices = LaborProcessRepo::lock_and_get_unit_prices(executor, &process_ids).await?;
// ... 随后在同一事务内用 prices 作为快照写入
```

### 修复2：N+1 查询优化 — 4 查询合并为 2 查询

新增 `get_bom_group_with_members`，通过 JOIN 一次获取组信息，消除旧的嵌套 `Option<Option<i64>>` 返回类型：

```rust
// Before: 4 次查询
let group_id = LaborProcessRepo::get_bom_process_group_id(&self.pool, bom_id).await?;  // 1
let group = LaborProcessRepo::get_group_by_id(&self.pool, group_id).await?;              // 2
let members = LaborProcessRepo::list_group_members(&self.pool, group_id).await?;         // 3
let items = LaborProcessRepo::get_bom_labor_cost(&self.pool, bom_id).await?;             // 4

// After: 2 次查询
let group_with_members = LaborProcessRepo::get_bom_group_with_members(&self.pool, bom_id).await?; // 1
let items = LaborProcessRepo::get_bom_labor_cost(&self.pool, bom_id).await?;                      // 2
```

JOIN 查询实现：

```rust
pub async fn get_bom_group_with_members(pool: &PgPool, bom_id: i64) -> Result<Option<LaborProcessGroupWithMembers>> {
    let row = sqlx::query_as!(LaborProcessGroup,
        "SELECT g.id, g.name, g.remark, g.created_at, g.updated_at
         FROM bom b JOIN labor_process_group g ON g.id = b.process_group_id
         WHERE b.bom_id = $1", bom_id)
        .fetch_optional(pool).await?;
    let group = match row { Some(g) => g, None => return Ok(None) };
    let members = Self::list_group_members(pool, group.id).await?;
    Ok(Some(LaborProcessGroupWithMembers { group, members }))
}
```

### 修复3：删除引用检查

```rust
pub async fn is_process_referenced(pool: &PgPool, process_id: i64) -> Result<bool> {
    let in_group: bool = sqlx::query_scalar!(
        "SELECT EXISTS(SELECT 1 FROM labor_process_group_member WHERE process_id = $1)", process_id
    ).fetch_one(pool).await?.unwrap_or(false);
    if in_group { return Ok(true); }
    let in_cost: bool = sqlx::query_scalar!(
        "SELECT EXISTS(SELECT 1 FROM bom_labor_cost WHERE process_id = $1)", process_id
    ).fetch_one(pool).await?.unwrap_or(false);
    Ok(in_cost)
}
```

### 修复4：搜索查询支持

将 service trait 从固定分页参数改为查询结构体：

```rust
pub struct LaborProcessQuery {
    pub keyword: Option<String>,
    pub page: u32,
    pub page_size: u32,
}

// service trait
async fn list_processes(&self, query: LaborProcessQuery) -> Result<(Vec<LaborProcess>, i64)>;

// repo 使用 ILIKE 动态查询
let pattern = format!("%{kw}%");
sqlx::query_as("SELECT ... FROM labor_process WHERE name ILIKE $1 ORDER BY id ASC LIMIT $2 OFFSET $3")
    .bind(&pattern).bind(page_size).bind(offset).fetch_all(pool).await?
```

### 修复5：迁移与回滚

- 移除所有 `REFERENCES` 外键约束（按用户要求，外键在微服务架构中引入跨表耦合）
- 在 `bom_labor_cost(bom_id)` 和 `bom_labor_cost(process_id)` 上添加索引
- 创建回滚迁移 `022_rollback_labor_process_redesign.sql`

## Why This Works

1. **SELECT FOR UPDATE**：PostgreSQL 的 `FOR UPDATE` 在事务提交前锁定选中行，阻止其他事务修改 `unit_price`。这是标准的"先读后写"竞态条件解决方案。由于 `set_bom_labor_cost` 的 clear + insert 已在同一事务内，加上行锁保护，保证了原子性。

2. **JOIN 合并查询**：`get_bom_group_with_members` 通过 `bom JOIN labor_process_group` 一次获取组信息，在数据库引擎内部执行连接，减少 3 次网络往返为 1 次。同时消除了 `get_bom_process_group_id` 返回的 `Option<Option<i64>>` 嵌套类型。

3. **应用层引用检查**：比数据库外键更灵活（可返回友好的中文错误消息），同时避免外键带来的迁移耦合和级联锁定问题。

4. **动态 SQL + ILIKE**：keyword 为空时走简单查询路径，非空时才添加 ILIKE 过滤，避免对全表扫描场景的性能影响。

## Prevention

- **竞态审计**：任何"先读后写"模式必须在事务内使用 `SELECT ... FOR UPDATE`。代码审查时重点检查此类模式。
- **N+1 检测**：服务层方法调用多个 repo 方法获取关联数据时，考虑合并为 JOIN 查询。避免循环内调用异步查询。
- **删除标准流程**：所有删除操作必须先检查引用关系，再执行删除。
- **sqlx 限制**：`push_values` 闭包中只有 `push_bind`，需要原始 SQL 时改用 `query_as` 手动构建。
- **迁移规范**：始终编写对应回滚迁移；优先用索引保证查询性能；外键约束应谨慎使用。

## Related

- [权限缓存与迁移数据丢失](../security-issues/permission-cache-fail-open-and-migration-data-loss-2026-04-17.md) — 迁移安全模式（归档替代删除、回滚迁移）与本次修复同一类问题
- [require-permission 宏](../developer-experience/require-permission-macro-async-trait-2026-04-05.md) — 劳务工序 handler 使用的权限宏模式
