# P0: AcquireChannel 枚举化

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将产品获取途径从 JSONB 字符串提升为强类型枚举 + 数据库独立列，为后续履行分流奠定基础。同时扩展 DomainEventType 支持需求事件。

**Architecture:** 新增 `AcquireChannel` 枚举（SelfProduced=1, Purchased=2, Outsourced=3, NonInventory=4, Legacy=9），添加到 `products` 表作为 `SMALLINT` 独立列 + CHECK 约束 + B-tree 索引。双写过渡期保留 JSONB 内原字段。

**Tech Stack:** Rust / sqlx / PostgreSQL

---

## 文件结构

| 操作 | 文件 |
|------|------|
| 创建 | `abt-core/migrations/032_acquire_channel_enum.sql` |
| 修改 | `abt-core/src/master_data/product/model.rs` |
| 修改 | `abt-core/src/master_data/product/repo.rs` |
| 修改 | `abt-core/src/master_data/product/implt.rs` |
| 修改 | `abt-core/src/master_data/product/mod.rs` |
| 修改 | `abt-core/src/shared/enums/event.rs` |

---

## Task 1: 数据库迁移

**Files:**
- 创建: `abt-core/migrations/032_acquire_channel_enum.sql`

- [ ] **Step 1: 编写迁移 SQL**

```sql
BEGIN;

-- 1. 添加 acquire_channel 独立列，DEFAULT 9 (Legacy) 确保现有行安全
ALTER TABLE products
  ADD COLUMN acquire_channel SMALLINT NOT NULL DEFAULT 9;

-- 2. CHECK 约束
ALTER TABLE products
  ADD CONSTRAINT chk_products_acquire_channel
  CHECK (acquire_channel IN (1, 2, 3, 4, 9));

-- 3. B-tree 索引（部分索引，仅活跃产品）
CREATE INDEX idx_products_acquire_channel
  ON products (acquire_channel)
  WHERE deleted_at IS NULL;

-- 4. 数据迁移：映射现有 JSONB 字符串值
UPDATE products
SET acquire_channel = CASE
    WHEN meta->>'acquire_channel' IN ('self-made', '自制', '自产') THEN 1
    WHEN meta->>'acquire_channel' IN ('purchase', '外购', '采购') THEN 2
    WHEN meta->>'acquire_channel' IN ('outsourced', '委外') THEN 3
    WHEN meta->>'acquire_channel' IN ('non-inventory', '费用', '服务') THEN 4
    ELSE 9  -- Legacy: 未确定，行为等同自制
END
WHERE deleted_at IS NULL;

COMMIT;
```

- [ ] **Step 2: 验证迁移**

运行: `cargo clippy -p abt-core`
预期: 编译通过（迁移文件不会被 clippy 检查，但 sqlx 宏会验证数据库 schema）

- [ ] **Step 3: 提交**

```bash
git add abt-core/migrations/032_acquire_channel_enum.sql
git commit -m "feat(master_data): add acquire_channel enum column to products table"
```

---

## Task 2: AcquireChannel 枚举定义

**Files:**
- 修改: `abt-core/src/master_data/product/model.rs`

- [ ] **Step 1: 在 `ProductStatus` 枚举之后添加 `AcquireChannel` 枚举**

在 `model.rs` 文件中，紧接 `ProductStatus` 的 serde 实现之后（约第 62 行），添加完整的枚举定义：

```rust
/// 产品获取途径
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum AcquireChannel {
    SelfProduced = 1,  // 自制
    Purchased = 2,     // 外购
    Outsourced = 3,    // 委外（预留）
    NonInventory = 4,  // 费用/服务/虚拟件（跳过库存校验和补货）
    Legacy = 9,        // 历史遗留（行为等同自制，日志驱动数据清洗）
}

impl AcquireChannel {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::SelfProduced),
            2 => Some(Self::Purchased),
            3 => Some(Self::Outsourced),
            4 => Some(Self::NonInventory),
            9 => Some(Self::Legacy),
            _ => None,
        }
    }

    pub fn as_i16(self) -> i16 {
        self as i16
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::SelfProduced => "SelfProduced",
            Self::Purchased => "Purchased",
            Self::Outsourced => "Outsourced",
            Self::NonInventory => "NonInventory",
            Self::Legacy => "Legacy",
        }
    }
}

impl sqlx::Type<sqlx::Postgres> for AcquireChannel {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <i16 as sqlx::Type<sqlx::Postgres>>::type_info()
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for AcquireChannel {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        <i16 as sqlx::Encode<'_, sqlx::Postgres>>::encode_by_ref(&self.as_i16(), buf)
    }
}

impl sqlx::Decode<'_, sqlx::Postgres> for AcquireChannel {
    fn decode(value: sqlx::postgres::PgValueRef<'_>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let v = <i16 as sqlx::Decode<'_, sqlx::Postgres>>::decode(value)?;
        Self::from_i16(v).ok_or_else(|| format!("unknown AcquireChannel: {v}").into())
    }
}

impl serde::Serialize for AcquireChannel {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_i16(self.as_i16())
    }
}

impl<'de> serde::Deserialize<'de> for AcquireChannel {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = i16::deserialize(d)?;
        Self::from_i16(v).ok_or_else(|| serde::de::Error::custom(format!("unknown AcquireChannel: {v}")))
    }
}
```

- [ ] **Step 2: 更新 `Product` 结构体**

在 `Product` struct 中添加 `acquire_channel` 字段，放在 `status` 之后：

```rust
pub struct Product {
    pub product_id: i64,
    pub pdt_name: String,
    pub product_code: String,
    pub unit: String,
    pub status: ProductStatus,
    pub acquire_channel: AcquireChannel,  // 新增
    pub external_code: Option<String>,
    pub owner_department_id: Option<i64>,
    pub meta: ProductMeta,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
}
```

- [ ] **Step 3: 从 `ProductMeta` 移除 `acquire_channel`**

将 `ProductMeta` 改为：

```rust
pub struct ProductMeta {
    pub specification: String,
    // acquire_channel 已迁移为 Product 独立列
    pub old_code: Option<String>,
    #[serde(default)]
    pub remark: Option<String>,
}
```

注意：由于数据库 JSONB 中可能仍然存在 `acquire_channel` 键，`serde` 反序列化时默认忽略未知字段（`#[serde(deny_unknown_fields)]` 未使用），所以移除字段是安全的。

- [ ] **Step 4: 更新 `CreateProductReq` 和 `UpdateProductReq`**

```rust
pub struct CreateProductReq {
    pub name: String,
    pub unit: String,
    pub status: ProductStatus,
    pub acquire_channel: AcquireChannel,  // 新增
    pub external_code: Option<String>,
    pub owner_department_id: Option<i64>,
    pub meta: ProductMeta,
}

pub struct UpdateProductReq {
    pub name: Option<String>,
    pub unit: Option<String>,
    pub acquire_channel: Option<AcquireChannel>,  // 新增
    pub external_code: Option<String>,
    pub owner_department_id: Option<i64>,
    pub meta: Option<ProductMeta>,
}
```

- [ ] **Step 5: 验证编译**

运行: `cargo clippy -p abt-core`
预期: 报错（因为 repo 层还没更新列列表），记录这些错误用于下一步

- [ ] **Step 6: 提交**

```bash
git add abt-core/src/master_data/product/model.rs
git commit -m "feat(master_data): add AcquireChannel enum and update Product model"
```

---

## Task 3: 更新 Product Repo SQL

**Files:**
- 修改: `abt-core/src/master_data/product/repo.rs`

- [ ] **Step 1: 更新列常量**

找到 `PRODUCT_COLUMNS` 常量（或类似的列列表），在 `status` 之后添加 `acquire_channel`：

```rust
// 将
"... status, external_code, ..."
// 改为
"... status, acquire_channel, external_code, ..."
```

- [ ] **Step 2: 更新 INSERT 语句**

在 `create` 方法中添加 `acquire_channel` 绑定：
- INSERT 列列表添加 `acquire_channel`
- VALUES 添加对应的 `$N`
- `.bind(params.acquire_channel)`

- [ ] **Step 3: 更新 UPDATE 语句**

在 `update` 方法中，在 SET 子句中添加 `acquire_channel = $N` 条件更新：

```rust
// 在现有 SET 子句中添加
"acquire_channel = CASE WHEN $N::smallint IS NOT NULL THEN $N ELSE acquire_channel END"
```

或按现有 UPDATE 模式处理（始终更新）。

- [ ] **Step 4: 更新 `CreateProductParams`（如果存在）**

确保 repo 层的参数结构体包含 `acquire_channel` 字段。

- [ ] **Step 5: 验证编译**

运行: `cargo clippy -p abt-core`
预期: repo 编译通过

- [ ] **Step 6: 提交**

```bash
git add abt-core/src/master_data/product/repo.rs
git commit -m "feat(master_data): update product repo SQL for acquire_channel column"
```

---

## Task 4: 更新 Product Service 实现

**Files:**
- 修改: `abt-core/src/master_data/product/implt.rs`
- 修改: `abt-core/src/master_data/product/mod.rs`

- [ ] **Step 1: 更新 `create` 方法**

在 `create` 实现中，将 `req.acquire_channel` 传递给 repo 层参数。

- [ ] **Step 2: 更新 `update` 方法**

在 `update` 实现中，将 `req.acquire_channel` 传递给 repo 层参数。

- [ ] **Step 3: 更新 `mod.rs` 导出**

确保 `AcquireChannel` 被导出：

```rust
pub use model::AcquireChannel;
```

- [ ] **Step 4: 验证编译**

运行: `cargo clippy -p abt-core`
预期: master_data 模块编译通过

- [ ] **Step 5: 提交**

```bash
git add abt-core/src/master_data/product/
git commit -m "feat(master_data): integrate AcquireChannel into product service"
```

---

## Task 5: 扩展 DomainEventType

**Files:**
- 修改: `abt-core/src/shared/enums/event.rs`

- [ ] **Step 1: 添加三个新事件类型**

在 `DomainEventType` 枚举末尾（`PurchaseReturnCancelled = 63` 之后）添加：

```rust
    // Sales — Demand
    DemandCreated = 64,
    DemandConfirmed = 65,
    DemandRejected = 66,
```

- [ ] **Step 2: 更新 `from_i16` match**

在 `from_i16` 方法的 match 中添加：

```rust
            63 => Some(Self::PurchaseReturnCancelled),
            64 => Some(Self::DemandCreated),
            65 => Some(Self::DemandConfirmed),
            66 => Some(Self::DemandRejected),
            _ => None,
```

- [ ] **Step 3: 验证编译**

运行: `cargo clippy -p abt-core`
预期: 编译通过

- [ ] **Step 4: 提交**

```bash
git add abt-core/src/shared/enums/event.rs
git commit -m "feat(shared): add DemandCreated/Confirmed/Rejected event types"
```

---

## Task 6: P0 最终验证

- [ ] **Step 1: 全量编译检查**

运行: `cargo clippy -p abt-core`
预期: 零错误零警告

- [ ] **Step 2: 运行现有测试**

运行: `cargo test -p abt-core`
预期: 所有现有测试通过（新增字段有默认值，不会破坏现有逻辑）

- [ ] **Step 3: 合并提交（可选）**

如果需要 squash:
```bash
git rebase -i HEAD~5  # 交互式合并 P0 的 5 个提交
```
