# ⑤ 完工入库完善方案

> 对标 Odoo `_cal_price` + `_post_inventory` + ERPNext `validate_inspection`
> 这是核心链路的最后一环，也是事务最复杂的地方

## 一、当前问题

### 问题2（P0 致命）：成本结转把数量当金额

**位置**：`production_receipt/implt.rs:182-194`

```rust
EntryRequest {
    cost_type: CostType::Material,
    debit_amount: receipt.received_qty,   // ← 数量！不是金额
    credit_amount: receipt.received_qty,   // ← 数量！
    period, ...
}
```

成本类型单一只有 Material，没区分料/工/费，没查产品标准成本。**财务数据完全错误**。

**Odoo 做法**：`_cal_price(consumed_moves)` 基于实际消耗 stock.move 的成本计算：
```
成品成本 = Σ(原材料消耗量 × 原材料单位成本)
         + 工时成本(workcenter.costs_hour × duration)
         + 副产品成本分摊
```

### 问题3（P0 致命）：FQC 门控形同虚设

**位置**：`production_receipt/implt.rs:121-153`

```rust
let fqc_results = inspection_result_svc.list_by_source(ctx, db,
    InspectionResultFilter {
        source_type: Some(InspectionSourceType::ArrivalNotice), // ← 到货检验，不是生产检验
        source_id: Some(id), ...
    }).await?;

let fqc_passed = fqc_results.items.is_empty()  // ← 没记录 = 通过
    || fqc_results.items.iter().all(|r| r.result == Pass);
```

查询条件用了错误的 source_type，且查不到记录时直接放行。

**ERPNext 做法**（`job_card.py:846-892`）：BOM 级 + 工序级双重门控，Rejected 有 Stop/Warning 可配置策略。

### 问题4（P1）：倒冲用独立连接破坏事务

**位置**：`production_receipt/implt.rs:200-228`

```rust
let backflush_result = {
    let mut bf_conn = self.pool.acquire().await?;  // ← 独立连接
    new_backflush_service(self.pool.clone())
        .execute(ctx, &mut bf_conn, ...)           // ← 不在调用方事务内
        .await
};
```

倒冲成功但后续步骤失败时，倒冲已生效无法回滚。

**Odoo 做法**：`_post_inventory` 的所有操作在同一个 cursor/transaction 内。

### 问题7（P1）：部分入库释放全部预留

**位置**：`production_receipt/implt.rs:230-237`

```rust
new_inventory_reservation_service(self.pool.clone())
    .cancel_by_source(ctx, db, DocumentType::WorkOrder, receipt.work_order_id)
    .await?;
```

每次入库都取消工单**全部**预留。部分入库（50/100）后剩余生产无预留。

### 问题8（P1）：PlanItem 无条件置 Completed

**位置**：`production_receipt/implt.rs:280-285`

```rust
ProductionPlanRepo::update_item_status_by_work_order(
    db, receipt.work_order_id, PlanItemStatus::Completed
).await?;
```

部分入库也会把计划项标记为"已完成"。

---

## 二、Odoo 成本结转详解

### Odoo _post_inventory（同一事务内）
```python
def _post_inventory(self, cancel_backorder=False):
    # 1. 原材料消耗 moves → _action_done()
    moves_to_do = self.move_raw_ids.filtered(lambda m: m.state != 'done')
    moves_to_do._action_done()

    # 2. 工序工时更新（用于成本计算）
    for workorder in order.workorder_ids:
        workorder.duration_expected = workorder._get_duration_expected()

    # 3. 成本计算
    order._cal_price(moves_to_do)  # 基于实际消耗 move 的成本

    # 4. 成品入库 moves → _action_done()
    moves_to_finish = self.move_finished_ids.filtered(...)
    moves_to_finish._action_done()
```

### Odoo _cal_price 算法
```python
def _cal_price(self, consumed_moves):
    # 成品单位成本 = (材料成本 + 工时成本 + 副产品分摊) / 完工量
    for order in self:
        workcenter_cost = 0
        for workorder in order.workorder_ids:
            duration = workorder.duration / 60  # 小时
            workcenter_cost += duration * workorder.workcenter_id.costs_hour

        material_cost = sum(move._compute_value() for move in consumed_moves)

        # 成品库存估值 = 材料成本 + 工时成本
        finished_move.value = material_cost + workcenter_cost
```

---

## 三、完善方案

### 3.1 成本结转修正（问题2）

**文件**：`production_receipt/implt.rs` confirm 方法

```rust
async fn confirm(&self, ctx, db, id: i64) -> Result<()> {
    // ... 现有校验 ...

    // 修复：成本结转 — 查产品标准成本算真实金额
    let product = product_svc.get(ctx, db, receipt.product_id).await?;
    let unit_cost = product.standard_cost;  // 产品标准成本

    // 区分料/工/费三类成本（对标 Odoo material_cost + workcenter_cost）
    let material_cost = unit_cost * receipt.received_qty * Decimal::new(60, 2);  // 60%材料
    let labor_cost = unit_cost * receipt.received_qty * Decimal::new(25, 2);     // 25%人工
    let overhead_cost = unit_cost * receipt.received_qty * Decimal::new(15, 2);  // 15%制造费用

    // 或更精确：从领料实际成本汇总材料 + 从报工工时算人工
    // let material_cost = sum_actual_material_cost(work_order_id);
    // let labor_cost = sum_actual_labor_cost(work_order_id);

    let period = chrono::Local::now().format("%Y-%m").to_string();
    cost_entry_svc.create_entries(ctx, db, vec![
        EntryRequest {
            entity_type: CostEntityType::WorkOrder,
            entity_id: receipt.work_order_id,
            cost_type: CostType::Material,
            debit_amount: material_cost,    // ← 真实金额
            credit_amount: material_cost,
            period: period.clone(),
            source_type: DocumentType::ProductionReceipt,
            source_id: id,
        },
        EntryRequest {
            cost_type: CostType::Labor,     // ← 人工成本
            debit_amount: labor_cost,
            credit_amount: labor_cost,
            ..同上
        },
        EntryRequest {
            cost_type: CostType::Overhead,  // ← 制造费用
            debit_amount: overhead_cost,
            credit_amount: overhead_cost,
            ..同上
        },
    ]).await?;
}
```

### 3.2 FQC 门控修正（问题3）

**文件**：`production_receipt/implt.rs` confirm 方法

```rust
// 修复1：用正确的 source_type（生产完工检验，不是到货检验）
let fqc_results = inspection_result_svc.list_by_source(ctx, db,
    InspectionResultFilter {
        source_type: Some(InspectionSourceType::ProductionReceipt), // ← 修正
        source_id: Some(id),
        ..Default::default()
    },
    PageParams { page: 1, page_size: 100 },
).await?;

// 修复2：查不到记录时报错，不放行（对标 ERPNext Stop 策略）
if fqc_results.items.is_empty() {
    return Err(DomainError::BusinessRule(
        "完工入库前必须完成 FQC 质检（无检验记录）".into()
    ));
}

// 修复3：有 Rejected 结果时阻断
let has_rejected = fqc_results.items.iter()
    .any(|r| r.result == InspectionResultType::Reject);
if has_rejected {
    return Err(DomainError::BusinessRule(
        "FQC 质检存在不合格结果，不允许入库。请先处理不合格品".into()
    ));
}

let fqc_passed = fqc_results.items.iter().all(|r| {
    r.status == InspectionStatus::Completed && r.result == InspectionResultType::Pass
});
if !fqc_passed {
    return Err(DomainError::BusinessRule(
        "FQC 质检未全部通过（有未完成的检验）".into()
    ));
}
```

> 注意：需要确认 `InspectionSourceType` 枚举是否有 `ProductionReceipt` 变体。如没有，需要新增。
> 同时需要在创建完工检验记录时使用正确的 source_type。

### 3.3 倒冲纳入同一事务（问题4）

**文件**：`production_receipt/implt.rs` confirm 方法

```rust
// 修复：倒冲使用传入的 db（同一事务），不再 acquire 独立连接
let backflush_result = new_backflush_service(self.pool.clone())
    .execute(ctx, db, receipt.work_order_id, receipt.received_qty)
    .await;

match backflush_result {
    Ok(()) => {
        ProductionReceiptRepo::set_backflush_triggered(db, id, true).await?;
    }
    Err(e) => {
        // 倒冲失败：整个事务回滚（包括入库和成本结转）
        // 不再"best-effort"，因为库存一致性优先
        return Err(DomainError::BusinessRule(format!(
            "倒冲失败，入库已回滚: {:?}", e
        )));
    }
}
```

### 3.4 预留按入库比例释放（问题7）

**文件**：`production_receipt/implt.rs` confirm 方法

```rust
// 修复：按入库比例释放预留，不是全部取消
let wo = work_order_svc.find_by_id(ctx, db, receipt.work_order_id).await?;
let completion_ratio = receipt.received_qty / wo.planned_qty;

if completion_ratio >= Decimal::ONE {
    // 全部入库 → 释放全部预留
    inventory_reservation_svc
        .cancel_by_source(ctx, db, DocumentType::WorkOrder, receipt.work_order_id)
        .await?;
} else {
    // 部分入库 → 按比例释放（需要 reservation service 支持 reduce_by_quantity）
    inventory_reservation_svc
        .reduce_by_source_and_ratio(
            ctx, db, DocumentType::WorkOrder, receipt.work_order_id, completion_ratio
        ).await?;
}
```

> 需要在 `inventory_reservation/service.rs` 加 `reduce_by_source_and_ratio` 方法。

### 3.5 PlanItem 条件 Completed（问题8）

**文件**：`production_receipt/implt.rs` confirm 方法

```rust
// 修复：仅在所有批次终态时设 PlanItem Completed
let all_batches = ProductionBatchRepo::list_by_work_order(
    db, receipt.work_order_id
).await?;

let has_active_batch = all_batches.iter().any(|b| {
    b.status != BatchStatus::Completed && b.status != BatchStatus::Cancelled
});

if !has_active_batch {
    // 所有批次终态 → PlanItem 设为 Completed
    ProductionPlanRepo::update_item_status_by_work_order(
        db, receipt.work_order_id, PlanItemStatus::Completed
    ).await?;
}
// 否则保持 InProduction 状态（部分入库场景）
```

---

## 四、实现步骤

1. 确认/新增 `InspectionSourceType::ProductionReceipt` 枚举变体
2. 新增 `inventory_reservation/service.rs` 的 `reduce_by_source_and_ratio` 方法
3. 重写 `production_receipt/implt.rs` confirm 方法：
   - 成本结转用真实金额（料/工/费分类）
   - FQC 门控修正（正确 source_type + 空记录阻断 + Rejected 阻断）
   - 倒冲用传入 db（同一事务）
   - 预留按比例释放
   - PlanItem 条件 Completed
4. `cargo clippy` 验证

## 五、验收标准

- [ ] 成本结转 debit/credit 是金额（数量 × 单价），不是数量
- [ ] 成本分料/工/费三类
- [ ] FQC 无检验记录时报错（不放行）
- [ ] FQC 有 Rejected 结果时报错
- [ ] 倒冲失败时整个事务回滚（入库和成本一起回滚）
- [ ] 部分入库（50/100）时只释放 50% 预留
- [ ] 全部入库时释放全部预留
- [ ] 部分入库时 PlanItem 保持 InProduction
- [ ] 全部入库（所有批次终态）时 PlanItem 设为 Completed
