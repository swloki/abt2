# ② 工单下达完善方案

> 对标 Odoo `action_confirm` + `button_plan` + `button_mark_done`
> 前置条件：migration 045（routing_steps 加字段）已执行

## 一、当前问题

### 问题1（P0 致命）：release 时工序属性全部丢失

**位置**：`work_order/implt.rs:162-178`

```rust
// 当前代码：从 routing_detail.steps 创建 WorkOrderRouting
// 但 RoutingStep 只有 process_code/step_order/is_required/remark
// 所有业务属性硬编码为 None/false
WorkOrderRouting {
    step_no: step.step_order,
    process_name: step.process_name.clone().unwrap_or_default(),
    work_center_id: None,      // ← 丢失
    standard_time: None,       // ← 丢失 → 无法算工时
    standard_cost: None,       // ← 丢失
    unit_price: None,          // ← 丢失 → 计件工资=0
    allowed_loss_rate: None,   // ← 丢失
    is_outsourced: false,      // ← 丢失 → 委外失效
    is_inspection_point: false,// ← 丢失 → IPQC不触发
}
```

**连锁后果**：
- `calculate_wage` 的 `unit_price.unwrap_or(Decimal::ZERO)` 永远返回 0 → 计件工资全部为 0
- `is_inspection_point` 永远 false → IPQC 自动报检不触发
- `is_outsourced` 永远 false → 委外分流失效

**根因**：`RoutingStep` model（`routing/model.rs:17-27`）只有 5 个字段，缺少工序业务属性。migration 045 已给 routing_steps 表加了字段，但 model 和 repo 还没适配。

### 问题5（P1）：物料预检从不执行

**位置**：`production_plan/implt.rs:175`

```rust
if let Some(snapshot_id) = item.bom_snapshot_id {
    // 物料预检代码...
}
```

但 `demand_handler/implt.rs:117` 和 `generate_work_orders` 创建计划项时 `bom_snapshot_id` 永远为 None → 预检分支永远不进入。

### 问题6（P1）：close 可绕过空批次校验

**位置**：`work_order/implt.rs:492-506`

```rust
let has_incomplete = batches.iter().any(|b| {
    b.status != BatchStatus::Completed && b.status != BatchStatus::Cancelled
});
if has_incomplete { return Err(...); }
```

batches 为空时 `has_incomplete = false`，未拆批的 Released 工单可直接关闭。

### 问题11（P2）：cancel 不取消关联领料单

**位置**：`work_order/implt.rs:537-601`

cancel 只做了库存预留释放 + 软删除，**没有取消领料单**（unrelease 有取消逻辑，cancel 没有）。

---

## 二、Odoo 参考实现

### Odoo 工序属性继承
```
mrp.bom → action_confirm() → 从 bom_id.operation_ids 创建 workorder
mrp.routing.workcenter 持有: name, workcenter_id, sequence, time_cycle_manual, cost_mode
mrp.workorder 从 routing.workcenter 完整继承所有属性（copy=True）
```

关键：Odoo 工序模板的属性在 MO 确认时一次性拷贝到工单工序上，之后不再查找源模板。

### Odoo button_mark_done 校验
```python
def button_mark_done(self):
    res = self.pre_button_mark_done()  # 校验消耗量、质检等
    if res is not True:
        return res
    # 校验 qty_producing > 0
    # 校验所有 quality checks 通过
```

---

## 三、完善方案

### 3.1 RoutingStep model 加字段

**文件**：`master_data/routing/model.rs`

```rust
pub struct RoutingStep {
    pub id: i64,
    pub routing_id: i64,
    pub process_code: String,
    pub step_order: i32,
    pub is_required: bool,
    pub remark: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    #[sqlx(default)]
    pub process_name: Option<String>,
    // ↓ migration 045 新增
    pub work_center_id: Option<i64>,
    pub standard_time: Option<Decimal>,       // 标准工时(分钟)
    pub standard_cost: Option<Decimal>,       // 标准成本(每小时)
    pub unit_price: Option<Decimal>,          // 计件单价
    pub allowed_loss_rate: Option<Decimal>,   // 允许损耗率
    pub is_outsourced: bool,
    pub is_inspection_point: bool,
}
```

同步更新 `RoutingStepInput`：
```rust
pub struct RoutingStepInput {
    pub process_code: String,
    pub step_order: i32,
    pub is_required: bool,
    pub remark: Option<String>,
    // ↓ 新增
    pub work_center_id: Option<i64>,
    pub standard_time: Option<Decimal>,
    pub standard_cost: Option<Decimal>,
    pub unit_price: Option<Decimal>,
    pub allowed_loss_rate: Option<Decimal>,
    pub is_outsourced: bool,
    pub is_inspection_point: bool,
}
```

### 3.2 routing repo 查询加字段

**文件**：`master_data/routing/repo.rs`

所有 SELECT 和 INSERT 语句加上新字段：
```sql
-- SELECT
SELECT id, routing_id, process_code, step_order, is_required, remark, created_at,
       work_center_id, standard_time, standard_cost, unit_price,
       allowed_loss_rate, is_outsourced, is_inspection_point
FROM routing_steps WHERE routing_id = $1

-- INSERT
INSERT INTO routing_steps (routing_id, process_code, step_order, is_required, remark,
    work_center_id, standard_time, standard_cost, unit_price,
    allowed_loss_rate, is_outsourced, is_inspection_point)
VALUES (...)
```

### 3.3 release 完整映射工序属性

**文件**：`work_order/implt.rs` release 方法

```rust
for step in &routing_detail.steps {
    let routing = WorkOrderRouting {
        work_order_id: id,
        step_no: step.step_order,
        process_name: step.process_name.clone().unwrap_or_default(),
        // ↓ 从 RoutingStep 完整映射（对标 Odoo workorder 继承 routing.workcenter）
        work_center_id: step.work_center_id,        // 原来是 None
        standard_time: step.standard_time,           // 原来是 None
        standard_cost: step.standard_cost,           // 原来是 None
        unit_price: step.unit_price,                 // 原来是 None → 计件工资修复
        allowed_loss_rate: step.allowed_loss_rate,   // 原来是 None
        planned_qty: wo.planned_qty,
        is_outsourced: step.is_outsourced,           // 原来是 false
        is_inspection_point: step.is_inspection_point, // 原来是 false
    };
    ProductionBatchRepo::insert_routing(&mut *db, &routing).await?;
}
```

### 3.4 close 增加空批次和完工量校验

**文件**：`work_order/implt.rs` close 方法

```rust
// 校验1：必须有至少一个已完工批次
if batches.is_empty() {
    return Err(DomainError::BusinessRule(
        "工单无生产批次，不能直接关闭。请先拆批并完成生产。".into()
    ));
}

// 校验2：completed_qty 达到 planned_qty 的容差范围
let total_completed: Decimal = batches.iter()
    .filter(|b| b.status == BatchStatus::Completed)
    .map(|b| b.completed_qty)
    .sum();
let completion_rate = total_completed / work_order.planned_qty;
if completion_rate < Decimal::new(95, 2) {  // 95% 容差
    return Err(DomainError::BusinessRule(format!(
        "完工率 {:.0}% 低于 95%，无法关闭工单", completion_rate * 100
    )));
}
```

### 3.5 cancel 增加领料单取消

**文件**：`work_order/implt.rs` cancel 方法

在 cancel 方法中，复用 unrelease 的领料单取消逻辑：
```rust
// 取消关联领料单（与 unrelease 相同的逻辑）
let requisition_ids: Vec<i64> = sqlx::query_scalar(
    r#"SELECT source_id FROM document_links ... UNION SELECT target_id ..."
).bind(id)...fetch_all(&mut *db).await?;

for req_id in requisition_ids {
    // 领料单取消失败不阻断主流程，但记录审计
    if let Err(e) = new_material_requisition_service(self.pool.clone())
        .cancel(ctx, db, req_id).await
    {
        tracing::warn!(req_id, error = %e, "领料单取消失败");
    }
}

// 校验：如果有已确认的完工入库记录，拒绝取消
let receipt_count: i64 = sqlx::query_scalar(
    "SELECT COUNT(*) FROM production_receipts WHERE work_order_id = $1 AND status = 2"
).bind(id).fetch_one(&mut *db).await?;

if receipt_count > 0 {
    return Err(DomainError::BusinessRule(
        format!("工单已有 {} 张已确认的完工入库单，不能取消", receipt_count)
    ));
}
```

### 3.6 pre_validate 物料预检修复

**文件**：`production_plan/implt.rs` pre_validate

```rust
// 不再依赖 item.bom_snapshot_id（永远为 None）
// 改为动态查已发布 BOM
let bom_id = new_bom_query_service(self.pool.clone())
    .find_published_bom_by_product_code(ctx, db, &product.product_code)
    .await?;

if let Some(bom_id) = bom_id {
    // 直接从 BOM 展开做物料预检（不依赖 snapshot）
    let snapshot = new_bom_query_service(self.pool.clone())
        .get_snapshot_by_id(ctx, db, bom_id).await?;
    if let Some(snapshot) = snapshot {
        // 物料预检逻辑...
    }
}
```

---

## 四、实现步骤

1. 更新 `routing/model.rs`：RoutingStep + RoutingStepInput 加字段
2. 更新 `routing/repo.rs`：SELECT/INSERT 加字段
3. 更新 `routing/implt.rs`：create/update 传递新字段
4. 更新 `work_order/implt.rs` release：完整映射工序属性
5. 更新 `work_order/implt.rs` close：加空批次+完工量校验
6. 更新 `work_order/implt.rs` cancel：加领料单取消+入库校验
7. 更新 `production_plan/implt.rs` pre_validate：动态查 BOM 做预检
8. `cargo clippy` 验证

## 五、验收标准

- [ ] routing_steps 的工序属性（单价/工时/检验点/委外）能通过 API 设置
- [ ] release 后 work_order_routings 完整继承 routing_steps 的所有属性
- [ ] calculate_wage 能取到正确的 unit_price（非 0）
- [ ] is_inspection_point=true 的工序在报工时触发 IPQC
- [ ] close 拒绝无批次的工单
- [ ] close 校验完工率 >= 95%
- [ ] cancel 取消关联领料单
- [ ] cancel 拒绝有已确认入库单的工单
- [ ] pre_validate 动态查 BOM 做物料预检
