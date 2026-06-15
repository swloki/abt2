# 采购订单确认 → 仓库收货完整流程设计

> **日期**: 2026-06-15
> **关联**: 设计文档 `docs/uml-design/02-purchase.html` L340, `03-wms.html` L532
> **状态**: 已确认，待实现（经 6 角色评审修订）

## 设计文档偏离声明

设计文档 `02-purchase.html` L340 原文：

> confirm → publish(PurchaseOrderConfirmed) → Outbox → WMS 异步创建 ArrivalNotice

本方案有意偏离：不自动创建来料通知，改为"手动创建 + 从 PO 导入"。理由：支持分批到货场景（一个 PO 可能分多次送货，自动创建全量来料通知不合理）。

**实现完成后必须同步更新** `docs/uml-design/02-purchase.html` L340 的 note，改为描述手动导入模式。

## 问题

采购订单确认后流程断裂：

- `PurchaseOrderConfirmed` 事件发布后无 handler 处理
- 来料通知（ArrivalNotice）需完全手动创建，无 PO 导入入口
- 来料检验通过后不回写 PO 的 `received_qty` 和状态
- PO 状态停留在 `Confirmed`，永远不会进入 `PartiallyReceived` / `Received`

## 目标流程

```
采购订单 (Draft)
    ↓ 用户确认
采购订单 (Confirmed)
    ↓ ──── 仓库收到货物（可能分批）────
    ↓ 手动创建来料通知（从 PO 导入，按实际到货数量调整）
来料通知 (Draft)
    ↓ 用户点"收货"，记录实收数量
来料通知 (Received)
    ↓ 用户填写检验结果，点"检验"
来料通知 (Accepted / PartiallyAccepted / Rejected)
    ↓ [事件] ArrivalInspected → handler 回写 PO received_qty + 状态
    ↓ PO: Confirmed → PartiallyReceived 或 Received
    ↓ ──── 仓库上架 ────
    ↓ 手动创建入库单（选来料通知来源，填库位）
入库单创建 → 库存增加 (InventoryTransaction)
```

**分批到货处理**：一个 PO 可以创建多张来料通知。每张记录实际到货数量。检验通过后 handler 重算所有关联来料通知的 accepted_qty 之和，更新 PO。

## 组件设计

### 组件 1：来料通知创建页面 — "从采购订单导入"

**层**: abt-web (`wms_arrival_create.rs` + `routes/wms_arrival.rs`)

**UI 变更**:

物料明细区域上方新增"从采购订单导入"按钮，紧邻现有"添加产品"按钮。点击弹出 PO 选择弹窗：

- 弹窗搜索框：按 PO 编号模糊搜索
- 列表查询：`PurchaseOrderService.list()` 过滤 `status = Confirmed`
- 每行显示：PO 编号、供应商名、订单日期、总金额
- **每行显示已关联来料通知数量**（"已创建 N 张"标签），通过查询 DocumentLink(ArrivalNotice → PO) 计数
- 选中 PO 后关闭弹窗，触发 HTMX 请求加载明细

**导入行为 — 追加而非替换**:

多次"从 PO 导入"是追加操作，不清空已有行。如果用户连续从 PO-A 和 PO-B 导入，明细行会合并。每行通过 `data-po-id` 属性标记来源 PO，便于追溯。

**新增 endpoint**:

`GET /admin/wms/arrivals/po-items/:po_id` (TypedPath: `ArrivalPoItemsPath`)

- 调用 `PurchaseOrderService.list_items()` 获取 PO 明细
- 调用 `ProductService.get_by_ids()` 获取产品编码和名称
- 渲染为 `<tr>` 行 HTML fragment，包含：
  - 隐藏字段 `data-product-id` + `data-po-id`
  - 隐藏字段 `name="order_item_id"`（关联 PO item ID，检验回写依赖）
  - 产品编码（只读显示）
  - 产品名称（只读显示）
  - 申报数量输入框（`name="declared_qty"`，默认值 = PO qty，可修改）
  - 批次号输入框（留空）
  - 删除行按钮
- HTMX 将 fragment **追加**到 `#arrival-item-tbody`（不清空已有行）
- 导入的行必须与现有 `arrivalCollectItems()` JS 函数兼容

**自动填充供应商**:

选中 PO 后，JS 将供应商下拉框设为 PO 的供应商（`select.value = po.supplier_id`）。

**不变的部分**:

- `create_arrival` handler 保存逻辑不变
- `CreateArrivalNoticeReq.purchase_order_id` 已支持
- `ArrivalNoticeService.create()` 已自动创建 DocumentLink（ArrivalNotice → PurchaseOrder, Fulfills）

### 组件 2：ArrivalNotice.inspect() 发布事件

**层**: abt-core (`wms/arrival_notice/implt.rs`)

当前 `inspect()` 在确定 `final_status` 后执行 CostEntry 和 InventoryReservation。新增：在 `ArrivalRepo::update_status(final_status)` 之后，发布领域事件。

**新增事件类型**:

在 `abt-core/src/shared/enums/event.rs` 中新增 `ArrivalInspected = 27`（当前枚举到 26）。

> **不用 `ArrivalReceived`(=5)**：`ArrivalReceived` 语义对应 "实物收货"（Draft→Received），不是"检验通过"。`inspect()` 发布的是检验结果事件，应用新枚举 `ArrivalInspected` 避免语义混淆。

```rust
// inspect() 末尾，update_status(final_status) 之后
if matches!(final_status, ArrivalStatus::Accepted | ArrivalStatus::PartiallyAccepted) {
    new_domain_event_bus(self.pool.clone())
        .publish(ctx, db, EventPublishRequest {
            event_type: DomainEventType::ArrivalInspected,  // 新增枚举
            aggregate_type: "ArrivalNotice".to_string(),
            aggregate_id: req.id,
            payload: json!({
                "arrival_notice_id": req.id,
                "doc_number": notice.doc_number,
            }),
            idempotency_key: None,
        }).await?;
}
```

- `Rejected` 不发布事件（无合格品，不回写 PO）

### 组件 3：ArrivalAcceptedHandler — 回写 PO

**层**: abt-core（handler 结构体）+ abt-web（注册）

**新建文件**: `abt-core/src/purchase/order/arrival_handler.rs`

参照 `PurchaseDemandCreatedHandler`（`abt-core/src/purchase/demand_handler/handler.rs`）的位置模式 — handler 放在业务模块下而非 event_bus 目录下。

**注册**: `abt-web/src/state.rs` handler 注册区

```rust
registry.register(
    DomainEventType::ArrivalInspected,
    Arc::new(ArrivalAcceptedHandler::new(pool.clone())),
);
```

**Handler 逻辑**:

```
1. 从 event.payload 取 arrival_notice_id
2. 查来料通知明细（ArrivalNoticeRepo::get_items）
   → 取每行 order_item_id + accepted_qty
3. 查 DocumentLink: 使用 DocumentLinkService.find_linked(
       source_type: DocumentType::ArrivalNotice,
       source_id: arrival_notice_id,
       page: 1, page_size: 10
   )
   → 从返回的 DocumentLink 中找 target_type == PurchaseOrder 的 target_id
   → 若无关联 PO（手工来料通知）→ return Ok(())
4. 对每个有 order_item_id 的来料明细，调用 Repo 方法重算 received_qty：
   PurchaseOrderItemRepo::recompute_received_qty(db, po_id)
   → 内部执行: SELECT order_item_id, SUM(accepted_qty) 全量重算
   → 天然幂等（EventProcessor 重试不会重复计数）
5. 批量更新 PO items 的 received_qty
6. 读 PO 当前 status，防重入：
   - 若 PO 已是 Received → 跳过状态转换（只更新 received_qty）
   - 若 PO 是 Confirmed 或 PartiallyReceived → 继续判定
7. 判定目标状态：
   - 全部 items: received_qty >= quantity → Received
   - 部分 items: received_qty > 0 → PartiallyReceived
8. 状态机 transition（已验证存在于 state_transition_defs 表）:
   - Confirmed → PartiallyReceived ✓
   - Confirmed → Received ✓
   - PartiallyReceived → Received ✓
9. 更新 PO 实体表 status
10. 审计日志: AuditLogService.record(
        entity_type: "PurchaseOrder", entity_id: po_id,
        action: Transition,
        changes: { from: old_status, to: new_status, received_qty_updates: [...] }
    )
```

**执行顺序保证**：received_qty 更新 → 状态转换 → 审计日志。任何步骤失败，EventProcessor 重试时从第 1 步重新执行（幂等）。

**事务边界**：handler 在 EventProcessor 的独立连接中执行（`processor.rs:262`），不在 inspect() 的事务内。handler 失败不影响检验结果，通过重试保证最终一致性。

**状态机转换**（已验证存在于 `state_transition_defs` 表）:

- `Confirmed → PartiallyReceived` ✓
- `Confirmed → Received` ✓
- `PartiallyReceived → Received` ✓

无需新增转换定义。

**PurchaseOrderItemRepo 新增方法**:

```rust
/// 重算指定 PO 所有明细的 received_qty（基于关联来料通知的 accepted_qty 求和）
/// 幂等：每次执行全量重算，不累加
pub async fn recompute_received_qty(
    executor: &mut sqlx::postgres::PgConnection,
    po_id: i64,
) -> Result<Vec<(i64, Decimal)>>  // returns [(order_item_id, new_received_qty)]

/// 批量更新明细行的 received_qty 字段
pub async fn batch_update_received_qty(
    executor: &mut sqlx::postgres::PgConnection,
    updates: &[(i64, Decimal)],  // [(item_id, received_qty)]
) -> Result<()>
```

### 组件 4：入库来源选择过滤

**层**: abt-web (`wms_stock_in_create.rs`)

**4a. 来料通知过滤**:

当前 `get_source_pick` handler（`:146-166`）查询来料通知时无状态过滤。修改为只返回 `Accepted` 和 `PartiallyAccepted` 状态的来料通知。

最简改法：查询后在代码中过滤 `n.status == Accepted || n.status == PartiallyAccepted`。

**4b. 采购订单过滤**:

当前 `get_source_pick` handler（`:168-183`）查询 PO 时无状态过滤（`PurchaseOrderQuery::default()`）。修改为只返回 `PartiallyReceived` 和 `Received` 状态的 PO（已部分或全部到货，可直接入库的场景）。

## 边界与约束

1. **手工来料通知**：无关联 PO 的来料通知（`purchase_order_id = None`），handler 直接跳过 PO 回写。
2. **多次检验**：同一来料通知不会重复检验（状态机保证 `Accepted` 不可再 `inspect`）。
3. **超收处理**：accepted_qty > PO quantity 的情况，PO received_qty 按实际累加，不做拦截（仓库已实物收货，不能假装没收到）。审计日志的 `changes` 中记录超收比例。
4. **退货冲减**：PurchaseReturn 的 returned_qty 不在此流程处理。后续退货流程单独处理。
5. **幂等性**：handler 使用重算策略（全量 SUM），EventProcessor 重试安全。
6. **DocumentLink**：来料通知创建时已自动建立 ArrivalNotice → PO 的 Fulfills 链接，handler 查询依赖此链接。
7. **PO 状态防重入**：handler 在状态转换前检查 PO 当前 status，若已 `Received` 则只更新 received_qty 不触发状态转换。

## 后续追踪项（不在本次实现）

- **来料通知 Rejected 引导**：Rejected 状态下来料通知详情页显示"不合格品处理"提示或 PurchaseReturn 快捷入口
- **一键收货+检验**：来料通知详情页支持跳过质检的快捷操作（适用于免检物料）
- **PurchaseReturn / PurchaseReconciliation / Payment 流程**：本次只到入库，后续单独设计

## 不做的事

- 不自动创建来料通知（PO 确认后不自动生成，支持分批到货场景）
- 不自动入库（检验通过后不自动创建 InventoryTransaction，仓库需手动指定库位上架）
- 不改 `PurchaseOrderConfirmed` 事件（设计文档提到的"异步创建 ArrivalNotice"调整为手动导入模式）
- 不处理 PurchaseReturn / PurchaseReconciliation / Payment 流程（本次只到入库）
