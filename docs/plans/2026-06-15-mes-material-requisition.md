# ③ 领料完善方案

> 对标 Odoo `stock.move`（operation_id 关联）+ `_action_done`（事务内消费预留）
> 前置条件：migration 045（material_requisition_items 加 operation_id/batch_id）已执行

## 一、当前问题

领料模块位于 `wms/material_requisition/`，有基础骨架（Draft→Confirmed→Issued→Cancelled），但关键细节缺失。

### L1：领料不关联工序
`MaterialReqItem`（`model.rs:22-31`）没有 `operation_id` 字段。领料是工单级的，整单领全部物料。无法按工序分阶段领料。

**Odoo 做法**：`stock.move` 每条带 `operation_id`（来自 `bom_line.operation_id`），领料精确到工序。ERPNext 的 `job_card` 也支持工序级物料转移。

### L2：领料不关联批次
没有 `batch_id` 字段。工单拆多个流转卡后，所有批次共用一张领料单。

### L3：快照为空时生成空领料单
`implt.rs:68-87` — 如果 `wo.bom_snapshot_id` 为 None（release 时快照创建失败），领料明细为空，静默创建空单。

### L4：发料不消费库存预留（最严重）
`implt.rs:204-219` — issue 只记录 `MaterialIssue` 库存交易，**没有调用 inventory_reservation 消费预留**。release 时做的 HARD 预留一直挂着。

### L5：发料不带单位成本
`implt.rs:214` — `unit_cost: None`，库存出库无成本信息。

### L6：发料成本数量当金额
`implt.rs:237-254` — `debit_amount`/`credit_amount` 都是 `issued_qty`（数量），不是金额。

### L7：无退料功能
service trait 只有 create/confirm/issue/cancel，**没有 return**。

### L8：无法部分发料
issue 一次就把状态改为 Issued 终态，不支持多次发料。

---

## 二、Odoo 参考实现

### Odoo 领料 = stock.move
```
BOM 展开时每条 move 带:
  - operation_id (关联到工序)
  - product_uom_qty (需求量)
  - source_location_id (原料库)
  - dest_location_id (生产虚拟库)

发料 = move._action_done():
  - 自动消费预留（reservation）
  - 记录实际成本（基于产品 costing method: FIFO/AVCO/Standard）
  - 支持部分完成（partial_done）
  - 全部在同一事务
```

### Odoo 退料 = stock.move.reverse
```
退货时创建反向 move:
  - source/dest 对调
  - 关联原始 move（origin_returned_move_id）
  - 恢复库存
```

---

## 三、完善方案

### 3.1 MaterialReqItem model 加字段

**文件**：`wms/material_requisition/model.rs`

```rust
pub struct MaterialReqItem {
    pub id: i64,
    pub requisition_id: i64,
    pub product_id: i64,
    pub requested_qty: Decimal,
    pub issued_qty: Decimal,
    pub variance_qty: Decimal,
    pub bin_id: Option<i64>,
    // ↓ migration 045 新增
    pub operation_id: Option<i64>,  // 关联 work_order_routings.id
    pub batch_id: Option<i64>,      // 关联 production_batches.id
}
```

### 3.2 create_for_work_order 按工序展开领料明细

**文件**：`wms/material_requisition/implt.rs`

```rust
async fn create_for_work_order(&self, ctx, db, work_order_id: i64) -> Result<i64> {
    let wo = work_order_svc.find_by_id(ctx, db, work_order_id).await?;

    // 校验：BOM 快照必须存在
    let snapshot_id = wo.bom_snapshot_id.ok_or_else(|| DomainError::BusinessRule(
        "工单无 BOM 快照，请先确保 release 时 BOM 快照创建成功".into()
    ))?;

    let snapshot = bom_query_svc.get_snapshot_by_id(ctx, db, snapshot_id).await?
        .ok_or_else(|| DomainError::not_found("BomSnapshot"))?;

    // 从 work_order_routings 查工序列表（已有 operation_id 对应）
    let routings = WorkOrderRoutingRepo::get_by_work_order_id(db, work_order_id).await?;

    // BOM 展开：每个叶子组件关联到对应工序
    // 对标 Odoo _get_moves_raw_values: bom_line.operation_id
    let leaf_nodes = snapshot.bom_detail.leaf_nodes();
    for node in &leaf_nodes {
        let required_qty = node.quantity * wo.planned_qty;

        // 尝试匹配工序（通过 bom_node.work_center 或 product 匹配）
        let operation_id = routings.iter()
            .find(|r| r.step_no == node.order_num)
            .map(|r| r.id);

        MaterialRequisitionRepo::insert_item_with_operation(
            db, requisition.id, node.product_id, required_qty,
            operation_id, None,  // operation_id, batch_id
        ).await?;
    }
    // ...
}
```

### 3.3 issue 发料消费预留

**文件**：`wms/material_requisition/implt.rs` issue 方法

```rust
async fn issue(&self, ctx, db, req: IssueMaterialReq) -> Result<()> {
    // ... 现有校验 ...

    for item in &req.items {
        let found_item = existing_items.iter().find(|i| i.id == item.item_id).unwrap();

        // 1. 记录库存交易（现有逻辑）
        inventory_transaction_svc.record(ctx, db, RecordTransactionReq {
            quantity: -item.issued_qty,
            unit_cost: Some(unit_cost),  // ← 修复 L5：带单位成本
            // ...
        }).await?;

        // 2. 消费库存预留（新增，对标 Odoo move._action_done 消费 reservation）
        new_inventory_reservation_service(self.pool.clone())
            .consume(ctx, db, DocumentType::WorkOrder, requisition.work_order_id,
                     found_item.product_id, item.issued_qty)
            .await?;
    }

    // 3. 成本结转：查产品标准成本算金额（修复 L6）
    for item in &req.items {
        let product_cost = product_svc.get_cost(ctx, db, found_item.product_id).await?;
        let cost_amount = item.issued_qty * product_cost;

        cost_entry_svc.create_entries(ctx, db, vec![EntryRequest {
            debit_amount: cost_amount,   // ← 真实金额，不是数量
            credit_amount: cost_amount,
            // ...
        }]).await?;
    }
}
```

### 3.4 发料带单位成本

发料时查产品成本：
```rust
// 查产品标准成本（或移动加权平均）
let unit_cost: Decimal = sqlx::query_scalar(
    "SELECT standard_cost FROM products WHERE product_id = $1"
).bind(found_item.product_id).fetch_one(&mut *db).await
 .unwrap_or(Decimal::ZERO);

// 传入库存交易
RecordTransactionReq {
    quantity: -item.issued_qty,
    unit_cost: Some(unit_cost),  // ← 不再是 None
    // ...
}
```

### 3.5 新增退料功能

**文件**：`material_requisition/service.rs` 加 trait 方法

```rust
/// 退料：Issued → PartiallyReturned / FullyReturned
/// 对标 Odoo stock.move.reverse（反向 move）
async fn return_materials(
    &self,
    ctx: &ServiceContext, db: PgExecutor<'_>,
    req: ReturnMaterialReq,
) -> Result<()>;
```

**model.rs** 加请求结构：
```rust
pub struct ReturnMaterialReq {
    pub requisition_id: i64,
    pub items: Vec<ReturnItemReq>,
    pub reason: String,
}

pub struct ReturnItemReq {
    pub item_id: i64,
    pub return_qty: Decimal,
    pub bin_id: Option<i64>,
}
```

**implt.rs** 实现：
```rust
async fn return_materials(&self, ctx, db, req: ReturnMaterialReq) -> Result<()> {
    let requisition = get_by_id(db, req.requisition_id).await?;

    // 校验：领料单必须已发料
    if requisition.status != RequisitionStatus::Issued {
        return Err(DomainError::InvalidStateTransition { ... });
    }

    for item in &req.items {
        let orig = get_item(db, item.item_id).await?;

        // 校验：退料量 <= 已发料量
        if item.return_qty > orig.issued_qty {
            return Err(DomainError::Validation(format!(
                "退料量 {} 超过已发料量 {}", item.return_qty, orig.issued_qty
            )));
        }

        // 库存交易：退料入库（正数）
        inventory_transaction_svc.record(ctx, db, RecordTransactionReq {
            quantity: item.return_qty,  // ← 正数 = 入库
            transaction_type: TransactionType::MaterialReturn,
            unit_cost: Some(unit_cost),
            // ...
        }).await?;

        // 更新 issued_qty
        update_item_issued(db, item.item_id,
            orig.issued_qty - item.return_qty, ...).await?;
    }

    // 审计：记录退料原因
    audit_log_svc.record(ctx, db, RecordAuditLogReq {
        entity_type: "MaterialRequisition",
        entity_id: req.requisition_id,
        action: AuditAction::Update,
        changes: Some(json!({ "return_reason": req.reason })),
        ..Default::default()
    }).await?;
}
```

### 3.6 支持部分发料

**文件**：`material_requisition/implt.rs` + `enums.rs`

```rust
// RequisitionStatus 加 PartiallyIssued
pub enum RequisitionStatus {
    Draft = 1,
    Confirmed = 2,
    PartiallyIssued = 3,  // ← 新增：部分发料
    Issued = 4,           // 全部发料完成
    Cancelled = 5,
}

// issue 方法：检查是否全部发完
let all_issued = req.items.iter().all(|item| {
    let orig = existing_items.iter().find(|i| i.id == item.item_id).unwrap();
    item.issued_qty >= orig.requested_qty
});

let new_status = if all_issued { RequisitionStatus::Issued }
                 else { RequisitionStatus::PartiallyIssued };

// PartiallyIssued 状态允许再次调用 issue（继续发料）
```

---

## 四、实现步骤

1. 更新 `wms/material_requisition/model.rs`：MaterialReqItem 加 operation_id/batch_id
2. 更新 `wms/material_requisition/repo.rs`：SELECT/INSERT 加字段 + 加 insert_item_with_operation
3. 更新 `wms/enums.rs`：RequisitionStatus 加 PartiallyIssued + TransactionType 加 MaterialReturn
4. 更新 `implt.rs` create_for_work_order：按工序展开 + 校验快照
5. 更新 `implt.rs` issue：消费预留 + 带成本 + 真实金额成本结转
6. 新增 `implt.rs` return_materials：退料入库
7. 更新 issue 支持部分发料（PartiallyIssued 状态）
8. 新增 TransactionType::MaterialReturn migration（如需要）
9. `cargo clippy` 验证

## 五、验收标准

- [ ] 领料明细行带 operation_id（关联到工序）
- [ ] 无 BOM 快照时报错而非静默创建空单
- [ ] 发料时消费对应库存预留
- [ ] 发料库存交易带 unit_cost
- [ ] 成本结转用金额（数量 × 单价），不是数量
- [ ] 退料功能可用（退料入库 + 恢复库存）
- [ ] 退料记录原因到审计日志
- [ ] 部分发料后状态为 PartiallyIssued，可继续发料
- [ ] 全部发完后状态为 Issued
