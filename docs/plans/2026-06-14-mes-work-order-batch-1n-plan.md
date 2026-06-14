# 工单-批次 1:N 关联与三层工序模型 — 实现设计

**日期**：2026-06-14 | **范围**：MES 工单/批次/报工全链路 | **类型**：架构级重构

---

## 1. 背景与问题

### 1.1 现状

工单（WorkOrder）与生产批次（ProductionBatch）的数据模型从一开始就是 **1:N** 设计：

- `ProductionBatch.work_order_id: i64`（必填外键，`production_batch/model.rs:11`）
- `ProductionBatchService::split_work_order()` API 明确支持拆批（`service.rs:11`）
- `list_by_work_order()` 注定返回多批次（`service.rs:19`）

但实际落地不完整：

| 问题 | 位置 | 后果 |
|------|------|------|
| `release()` 硬编码只创建 1 个批次 | `work_order/implt.rs:194-220` | 1:N 落地为 1:1 |
| `release()` 用 `DocumentType::WorkOrder` 给批次编号 | `work_order/implt.rs:202-208` | 批次号 `WO-` 前缀与工单号混淆 |
| `WorkOrder` model **无** `completed_qty` 字段 | `work_order/model.rs:7-33` | 工单永远无法表达"已完工数量" |
| `ProductionBatch.completed_qty/scrap_qty` **从不 UPDATE** | 全仓库无 `UPDATE production_batches SET completed_qty` | UI 显示"完成量 0/0"（实测验证） |
| 真正累加数量的是工单级 `WorkOrderRouting` | `production_batch/repo.rs:333` | 多批次共享同一 routing 行，无法区分批次维度进度 |
| `split_work_order` **零校验** | `production_batch/implt.rs:71-101` | 可超 `planned_qty`、可在已取消工单上拆 |
| `unrelease` 物理删除 batches+routings | `work_order/implt.rs:404-415` | 审计断裂、报工孤儿 |

### 1.2 V2 设计文档方向错误

`docs/2026-06-13-mes-two-layer-architecture.md` "判断 2/3" 主张合并 `work_orders` 和 `production_batches`。该判断基于"当前没人拆批"的循环论证，方向错误。正确方向：**保持 1:N，补全落地**。

### 1.3 目标

- 让 1:N 拆批在数据层、Service 层、UI 层完整落地
- 建立三层工序模型（模板 → 快照 → 执行进度），消除"工单级 routing 共享累加"的语义错误
- 完成量在工单/批次/routing 三层事务内同步累加，UI 任何位置展示一致

---

## 2. 架构设计：三层工序模型

```
Layer 1  master_data.routings + routing_steps        (工序模板 — 已存在)
         · 产品级，BOM 关联（bom_routings）
         · step: process_code, step_order, is_required
         · 工艺员维护，跨工单共享

Layer 2  work_order_routings                         (工单工序快照 — 已存在，修正职责)
         · release() 时从 Layer 1 复制 step_no + process_name（快照）
         · 工单级实例化参数: planned_qty, work_center_id,
           standard_time, standard_cost, unit_price, allowed_loss_rate,
           is_inspection_point, is_outsourced
         · ❌ 移除: completed_qty, defect_qty, status（执行进度不属于定义层）
         · 保留意义: 工序快照（已下达工单不受主数据后续修改影响）

Layer 3  batch_routing_progress                      (批次执行进度 — 新增)
         · batch_id → work_order_routings.id (FK)
         · routing_id → work_order_routings.id (FK)
         · UNIQUE(batch_id, routing_id)
         · status: Pending/InProgress/Completed/Skipped
         · completed_qty, defect_qty, started_at, completed_at
         · 报工事务 confirm_routing_step 的写真相源
```

### 2.1 数量字段分布

| 实体 | 字段 | 来源 | 更新方式 |
|------|------|------|----------|
| `work_order_routings` | planned_qty | 工单 planned_qty | release() 时写入 |
| `batch_routing_progress` | completed_qty, defect_qty | 报工原子累加 | confirm_routing_step 行锁 `SET x = x + Δ` |
| `production_batches` | completed_qty, scrap_qty | `Σ(batch_routing_progress)` 冗余 | confirm 内同步累加 |
| `work_orders` | completed_qty, scrap_qty | `Σ(batches.completed_qty)` 冗余 | confirm 内同步累加 |

**冗余字段的存在理由**：列表筛选（如"近完成工单"）、列表渲染性能。不用于精确计算（精确计算走 `work_reports` SUM）。

### 2.2 状态机关系

```
WorkOrder:    Draft → Planned → Released → InProduction → Closed
                                              ↑                    ↑
                                    首次报工传播          所有批次 Completed/Cancelled

Batch:        Pending → InProgress → Suspended → PendingReceipt → Completed
                          ↑               ↓
                     首次报工         检验点挂起

RoutingProgress:  Pending → InProgress → Completed
                            ↑                ↑
                       首次报工         最后一道工序完成
```

---

## 3. 数据模型变更

### 3.1 迁移 039：batch_routing_progress + 字段补充（加法迁移）

```sql
-- 新表
CREATE TABLE batch_routing_progress (
    id              BIGSERIAL   PRIMARY KEY,
    batch_id        BIGINT      NOT NULL REFERENCES production_batches(id),
    routing_id      BIGINT      NOT NULL REFERENCES work_order_routings(id),
    status          SMALLINT    NOT NULL DEFAULT 1,  -- 1=Pending, 2=InProgress, 3=Completed, 4=Skipped
    completed_qty   DECIMAL(18,6) NOT NULL DEFAULT 0,
    defect_qty      DECIMAL(18,6) NOT NULL DEFAULT 0,
    started_at      TIMESTAMPTZ,
    completed_at    TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (batch_id, routing_id)
);
CREATE INDEX idx_brp_batch   ON batch_routing_progress (batch_id);
CREATE INDEX idx_brp_routing ON batch_routing_progress (routing_id);

-- work_orders 加完成量
ALTER TABLE work_orders ADD COLUMN completed_qty DECIMAL(18,6) NOT NULL DEFAULT 0;
ALTER TABLE work_orders ADD COLUMN scrap_qty     DECIMAL(18,6) NOT NULL DEFAULT 0;

-- production_batches 补 deleted_at（cancel 软删需要）
ALTER TABLE production_batches ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ;

-- 数据回填（work_reports 是真相源）
-- 详见迁移文件 039_batch_routing_progress.sql
```

### 3.2 迁移 040：work_order_routings 删执行进度字段（减法迁移）

**⚠️ 必须在代码改完 + cargo clippy 通过后执行**

```sql
ALTER TABLE work_order_routings DROP COLUMN completed_qty;
ALTER TABLE work_order_routings DROP COLUMN defect_qty;
ALTER TABLE work_order_routings DROP COLUMN status;
DROP INDEX IF EXISTS idx_work_order_routings_status;
```

### 3.3 Rust 模型变更

**`work_order/model.rs` WorkOrder struct**：
```rust
pub struct WorkOrder {
    // ... 已有字段 ...
    pub completed_qty: Decimal,    // ← 新增
    pub scrap_qty: Decimal,        // ← 新增
}
```

**`production_batch/model.rs` WorkOrderRouting struct**：
```rust
pub struct WorkOrderRouting {
    pub id: i64,
    pub work_order_id: i64,
    pub step_no: i32,
    pub process_name: String,
    pub work_center_id: Option<i64>,
    pub standard_time: Option<Decimal>,
    pub standard_cost: Option<Decimal>,
    pub unit_price: Option<Decimal>,
    pub allowed_loss_rate: Option<Decimal>,
    pub planned_qty: Decimal,
    pub is_outsourced: bool,
    pub is_inspection_point: bool,
    // ❌ 删除: completed_qty, defect_qty, status
}
```

**`production_batch/model.rs` 新增 BatchRoutingProgress struct**：
```rust
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BatchRoutingProgress {
    pub id: i64,
    pub batch_id: i64,
    pub routing_id: i64,
    pub status: RoutingStatus,
    pub completed_qty: Decimal,
    pub defect_qty: Decimal,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>>,
}
```

---

## 4. Service Trait 变更

### 4.1 新增 BatchRoutingProgressRepo

```rust
// abt-core/src/mes/production_batch/repo.rs

pub struct BatchRoutingProgressRepo;

impl BatchRoutingProgressRepo {
    /// 查找或创建 (batch_id, routing_id) 记录（UPSERT），返回 id
    pub async fn upsert_and_get_id(db, batch_id, routing_id) -> Result<i64>

    /// 行锁原子累加 completed_qty / defect_qty
    pub async fn atomic_increment_qty(
        db, id, completed_delta: Decimal, defect_delta: Decimal
    ) -> Result<()>

    /// 更新状态
    pub async fn update_status(db, id, status: RoutingStatus) -> Result<()>

    /// 按批次查所有工序进度（批次详情工序流转用）
    pub async fn list_by_batch(db, batch_id) -> Result<Vec<BatchRoutingProgress>>

    /// 按 routing 查所有批次进度（工单详情工序矩阵用）
    pub async fn list_by_routing(db, routing_id) -> Result<Vec<BatchRoutingProgress>>

    /// 按 (batch_id, routing_id) 查单条
    pub async fn get_by_batch_and_routing(db, batch_id, routing_id) -> Result<Option<BatchRoutingProgress>>
}
```

### 4.2 WorkOrderRepo 新增方法

```rust
// abt-core/src/mes/work_order/repo.rs

/// 行锁原子累加工单完成量（报工事务内调用）
pub async fn atomic_increment_completed_qty(
    db, id: i64, completed_delta: Decimal, scrap_delta: Decimal
) -> Result<()>
```

### 4.3 ProductionBatchRepo 新增方法

```rust
// abt-core/src/mes/production_batch/repo.rs

/// 行锁原子累加批次完成量（报工事务内调用）
pub async fn atomic_increment_qty(
    db, id: i64, completed_delta: Decimal, scrap_delta: Decimal
) -> Result<()>

/// 软删批次（替代物理 DELETE）
pub async fn soft_delete_by_work_order(db, work_order_id: i64) -> Result<()>
```

### 4.4 删除的方法

```rust
// ❌ 删除（执行进度已迁移到 batch_routing_progress）
WorkOrderRoutingRepo::atomic_increment_qty()    // production_batch/repo.rs:333
WorkOrderRoutingRepo::update_status()           // production_batch/repo.rs
```

---

## 5. Service 实现变更

### 5.1 confirm_routing_step 重构（核心）

**位置**：`abt-core/src/mes/production_batch/implt.rs:140-391`

**报工事务内四层同步累加**：

```
步骤 a-b: 获取批次 + 防跳序校验（不变）
步骤 c:   获取工序 work_order_routing（不变，但不再读 completed_qty/status）
步骤 d:   计算工资（不变）
步骤 e:   INSERT work_reports（幂等，不变）

--- 以下改为四层累加 ---
步骤 f1:  UPSERT batch_routing_progress (batch_id, routing_id) → 获取 brp_id
步骤 f2:  BatchRoutingProgressRepo::atomic_increment_qty(brp_id, Δcompleted, Δdefect)
          ← 替代旧的 WorkOrderRoutingRepo::atomic_increment_qty
步骤 f3:  ProductionBatchRepo::atomic_increment_qty(batch_id, Δcompleted, Δdefect)
          ← 新增，修复"批次完成量永不更新"bug
步骤 f4:  WorkOrderRepo::atomic_increment_completed_qty(work_order_id, Δcompleted, Δdefect)
          ← 新增，修复"工单无完成量"问题

--- 状态更新 ---
步骤 g1:  batch_routing_progress.status: Pending → InProgress（首次报工时）
步骤 g2:  batch.current_step 更新（不变）
步骤 g3:  batch.status 推进（不变：检验点 → Suspended，末道工序 → PendingReceipt）

--- 超额容差校验（改为基于批次累计）---
步骤 h:   从 batch_routing_progress 查本批次累计
          max_allowed = batch.batch_qty * (1 + tolerance)
          ← 修复"多批次共享 routing 累计导致误判"

--- 状态传播（不变）---
步骤 i:   首次报工 → WorkOrder.mark_in_production
```

### 5.2 release() 修改

**位置**：`abt-core/src/mes/work_order/implt.rs:86-311`

变更点：
1. **批次编号**（line 202-208）：`DocumentType::WorkOrder` → `DocumentType::ProductionBatch`
2. **批次创建**（line 194-220）：保持创建 1 个默认批次（`batch_qty = planned_qty`），但编号走 PB- 前缀
3. **routing 创建**（line 163-186）：不再写 `completed_qty/defect_qty/status`（字段已删除）

### 5.3 split_work_order 加校验

**位置**：`abt-core/src/mes/production_batch/implt.rs:71-101`

新增校验：
```rust
// 1. 工单必须处于 Released/InProduction 状态
if work_order.status != WorkOrderStatus::Released
    && work_order.status != WorkOrderStatus::InProduction {
    return Err(DomainError::BusinessRule("仅已下达/生产中工单可拆批"));
}

// 2. 拆分总量 + 已有批次总量 ≤ planned_qty × (1 + tolerance)
let existing_qty: Decimal = existing_batches.iter().map(|b| b.batch_qty).sum();
let split_qty: Decimal = splits.iter().map(|s| s.batch_qty).sum();
let tolerance = get_over_completion_tolerance(...).await?;
let max_allowed = work_order.planned_qty * (Decimal::ONE + tolerance);
if existing_qty + split_qty > max_allowed {
    return Err(DomainError::BusinessRule(format!(
        "拆分总量 {} + 已有 {} 超过计划量 {} 的容差上限",
        split_qty, existing_qty, max_allowed
    )));
}

// 3. 每个拆分项 batch_qty > 0
if splits.iter().any(|s| s.batch_qty <= Decimal::ZERO) {
    return Err(DomainError::validation("拆分量必须大于 0"));
}

// 4. 新批次自动创建 batch_routing_progress 记录（引用工单所有 routing）
for split in &splits {
    let batch_id = self.create(ctx, db, req).await?;
    // 为新批次初始化所有工序的 progress 记录
    let routings = WorkOrderRoutingRepo::get_by_work_order_id(db, work_order_id).await?;
    for r in &routings {
        BatchRoutingProgressRepo::upsert_and_get_id(db, batch_id, r.id).await?;
    }
}
```

### 5.4 unrelease 改软删

**位置**：`abt-core/src/mes/work_order/implt.rs:342-480`

变更：
```rust
// line 404-415 物理删除 → 软删除
// ProductionBatchRepo::soft_delete_by_work_order(db, id)  ← 替代 DELETE FROM production_batches
// WorkOrderRoutingRepo::soft_delete_by_work_order(db, id) ← 替代 DELETE FROM work_order_routings
// batch_routing_progress 也软删（或级联）

// 前置校验：无报工记录
let report_count: i64 = sqlx::query_scalar(
    "SELECT COUNT(*) FROM work_reports WHERE work_order_id = $1"
).fetch_one(...).await?;
if report_count > 0 {
    return Err(DomainError::BusinessRule("工单已有报工记录，无法反下达"));
}
```

---

## 6. UI 改造（P1，代码层完成后）

### 6.1 工单详情 tab 重组

**位置**：`abt-web/src/pages/mes_order_detail.rs:393-403`

| 现在 | 改为 |
|------|------|
| 工单信息 | 工单信息 |
| 工序明细 | 工艺路线（工序模板快照） |
| 关联单据（批次+报工平级） | **生产批次**（一级 tab，批次卡片含执行进度+报工折叠） |
| — | **报工记录**（聚合所有批次报工） |
| 操作日志 | 操作日志 |

### 6.2 工单列表加"批次执行"列

**位置**：`abt-web/src/pages/mes_order_list.rs:114-194`

```
列：批次执行 → "3 批 · 1 进行 · 45/100"  (completed_qty / planned_qty)
移除：车间列（全空）
```

### 6.3 批次详情加工单上下文条

**位置**：`abt-web/src/pages/mes_batch_detail.rs:170-210`

顶部 sticky 条：`工单 WO-xxx · 产品 · 计划量 · 交付日期`

### 6.4 拆批入口

工单详情"生产批次"tab 内加"追加批次"按钮 → 新建 `mes_batch_create.rs` 页面 + 路由。

---

## 7. 实施顺序与依赖链

```
Phase 1: 设计文档（本文档）→ 用户确认
    ↓
Phase 2: 数据层
    2a. 迁移 039（加法：建表+加列+回填）
    2b. 模型层 Rust struct 变更
    2c. Repo 层新增/修改方法
    ↓
Phase 3: Service 层
    3a. BatchRoutingProgressRepo 实现
    3b. confirm_routing_step 重构（四层累加）
    3c. release() 改编号
    3d. split_work_order 加校验
    3e. unrelease 改软删
    ↓
Phase 4: 编译验证
    4a. cargo clippy 全量通过
    4b. cargo build 通过
    ↓
Phase 5: 迁移 040（减法：删旧字段）
    ↓
Phase 6: UI 层
    6a. 工单详情 tab 重组
    6b. 工单列表批次执行列
    6c. 批次详情上下文条
    6d. 拆批入口
    ↓
Phase 7: 设计文档同步
    7a. 04-mes.html 工序三层模型
    7b. V2 两层架构文档修正
```

---

## 8. 风险与回滚

| 风险 | 缓解 |
|------|------|
| 迁移 039 数据回填不准 | 以 `work_reports` 为真相源（最可靠），回填后人工抽查 |
| 迁移 040 删列后代码遗漏引用 | cargo clippy 全量通过 + cargo build 后才执行 040 |
| 线上报工事务性能（四层累加） | 全部行锁 `SET x = x + Δ`，无 read-modify-write，单事务内 |
| 历史数据 `batch.completed_qty = 0` | 迁移 039 步骤 6 从 batch_routing_progress 回填 |

**回滚**：迁移 040 是唯一的不可逆操作（DROP COLUMN）。回滚需要从 batch_routing_progress 重建 work_order_routings.completed_qty。迁移 039 全部是加法，可安全回滚（DROP TABLE batch_routing_progress + DROP COLUMN completed_qty/scrap_qty）。

---

## 9. 验收标准

- [ ] `cargo clippy` 全量通过
- [ ] `cargo build` 通过
- [ ] 迁移 039 执行后，`batch_routing_progress` 数据与 `work_reports` SUM 一致
- [ ] `production_batches.completed_qty` > 0（之前永远是 0）
- [ ] `work_orders.completed_qty` > 0（之前字段不存在）
- [ ] 报工后工单列表"批次执行"列实时更新
- [ ] 批次详情"完成/报废"非 0
- [ ] 拆批表单校验生效（超量/非法状态拦截）
- [ ] unrelease 在有报工时拒绝执行
- [ ] 批次号 `PB-` 前缀（非 `WO-`）
