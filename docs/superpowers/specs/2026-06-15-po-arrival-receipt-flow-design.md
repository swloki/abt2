# 采购订单确认 → 仓库收货完整流程设计

> **日期**: 2026-06-15
> **关联**: 设计文档 `docs/uml-design/02-purchase.html` L340
> **状态**: 已确认，待实现

## 问题

采购订单确认后流程断裂。设计文档规定：

> confirm → publish(PurchaseOrderConfirmed) → Outbox → WMS 异步创建 ArrivalNotice

但代码中：
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
    ↓ [事件] ArrivalReceived → handler 回写 PO received_qty + 状态
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
- 选中 PO 后关闭弹窗，触发 HTMX 请求加载明细

**新增 endpoint**:

`GET /admin/wms/arrivals/po-items/:po_id` (TypedPath: `ArrivalPoItemsPath`)

- 调用 `PurchaseOrderService.list_items()` 获取 PO 明细
- 调用 `ProductService.get_by_ids()` 获取产品编码和名称
- 渲染为 `<tr>` 行 HTML fragment，包含：
  - 隐藏字段 `data-product-id`
  - 产品编码（只读显示）
  - 产品名称（只读显示）
  - 申报数量输入框（`name="declared_qty"`，默认值 = PO qty，可修改）
  - 批次号输入框（留空）
  - 删除行按钮
- HTMX 将 fragment 插入 `#arrival-item-tbody`

**自动填充供应商**:

选中 PO 后，JS 将供应商下拉框设为 PO 的供应商（`select.value = po.supplier_id`）。

**不变的部分**:

- `create_arrival` handler 保存逻辑不变
- `CreateArrivalNoticeReq.purchase_order_id` 已支持
- `ArrivalNoticeService.create()` 已自动创建 DocumentLink（ArrivalNotice → PurchaseOrder, Fulfills）

### 组件 2：ArrivalNotice.inspect() 发布事件

**层**: abt-core (`wms/arrival_notice/implt.rs`)

当前 `inspect()` 在确定 `final_status` 后执行 CostEntry 和 InventoryReservation。新增：在 `ArrivalRepo::update_status(final_status)` 之后，发布领域事件。

```rust
// inspect() 末尾
if matches!(final_status, ArrivalStatus::Accepted | ArrivalStatus::PartiallyAccepted) {
    new_domain_event_bus(self.pool.clone())
        .publish(ctx, db, EventPublishRequest {
            event_type: DomainEventType::ArrivalReceived,
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
- `DomainEventType::ArrivalReceived = 5`，枚举值已存在

### 组件 3：ArrivalAcceptedHandler — 回写 PO

**层**: abt-core（handler 结构体）+ abt-web（注册）

**新建文件**: `abt-core/src/shared/event_bus/handlers/arrival_accepted_handler.rs`

或在现有 handler 位置放置（参照 `PurchaseDemandCreatedHandler` 的位置模式）。

**注册**: `abt-web/src/state.rs` handler 注册区

```rust
registry.register(
    DomainEventType::ArrivalReceived,
    Arc::new(ArrivalAcceptedHandler::new(pool.clone())),
);
```

**Handler 逻辑**:

```
1. 从 event.payload 取 arrival_notice_id
2. 查来料通知明细（ArrivalNoticeRepo::get_items）
   → 取每行 order_item_id + accepted_qty
3. 查 DocumentLink(ArrivalNotice → PurchaseOrder, Fulfills)
   → 找到关联 PO ID
   → 若无关联 PO（手工来料通知）→ return Ok(())
4. 对每个有 order_item_id 的来料明细，重算 PO item 的 received_qty：
   SELECT COALESCE(SUM(ani.accepted_qty), 0)
   FROM arrival_notice_items ani
   JOIN arrival_notices an ON ani.notice_id = an.id
   WHERE ani.order_item_id = $1
     AND an.status IN (Accepted, PartiallyAccepted)
     AND an.deleted_at IS NULL
   重算而非累加 → 天然幂等（EventProcessor 重试不会重复计数）
5. 批量更新 PO items 的 received_qty
6. 判定 PO 状态转换：
   - 全部 items: received_qty >= quantity → Confirmed/PartiallyReceived → Received
   - 部分 items: received_qty > 0 → Confirmed → PartiallyReceived
7. 状态机 transition（确保转换已定义）
8. 更新 PO 实体表 status
```

**状态机转换**（已验证存在于 `state_transition_defs` 表）:

- `Confirmed → PartiallyReceived` ✓
- `Confirmed → Received` ✓
- `PartiallyReceived → Received` ✓

无需新增转换定义。

**PurchaseOrderItemRepo 新增方法**:

```rust
/// 批量更新多个明细行的 received_qty
async fn batch_update_received_qty(db: &mut PgConnection, updates: &[(i64, Decimal)]) -> Result<()>
```

**DocumentLinkService 查询**:

使用 `DocumentLinkService.list_links()` 查询 source = ArrivalNotice, link_type = Fulfills 的链接，获取 target PurchaseOrder ID。

### 组件 4：入库来源选择过滤

**层**: abt-web (`wms_stock_in_create.rs`)

当前 `get_source_pick` handler 查询来料通知时无状态过滤。修改为只返回 `Accepted` 和 `PartiallyAccepted` 状态的来料通知：

```rust
let filter = ArrivalNoticeFilter {
    doc_number: ...,
    status: None,  // 改为下面逻辑
    ..
};
// 修改：查全部后在代码中过滤，或扩展 ArrivalNoticeFilter 支持多状态
```

最简改法：查询后在代码中过滤 `n.status == Accepted || n.status == PartiallyAccepted`。

## 边界与约束

1. **手工来料通知**：无关联 PO 的来料通知（`purchase_order_id = None`），handler 直接跳过 PO 回写。
2. **多次检验**：同一来料通知不会重复检验（状态机保证 `Accepted` 不可再 `inspect`）。
3. **超收处理**：accepted_qty > PO quantity 的情况，PO received_qty 按实际累加，不做拦截（仓库已实物收货，不能假装没收到）。
4. **退货冲减**：PurchaseReturn 的 returned_qty 不在此流程处理。后续退货流程单独处理。
5. **幂等性**：handler 使用重算策略（全量 SUM），EventProcessor 重试安全。
6. **DocumentLink**：来料通知创建时已自动建立 ArrivalNotice → PO 的 Fulfills 链接，handler 查询依赖此链接。

## 不做的事

- 不自动创建来料通知（PO 确认后不自动生成，支持分批到货场景）
- 不自动入库（检验通过后不自动创建 InventoryTransaction，仓库需手动指定库位上架）
- 不改 `PurchaseOrderConfirmed` 事件（设计文档提到的"异步创建 ArrivalNotice"调整为手动导入模式）
- 不处理 PurchaseReturn / PurchaseReconciliation / Payment 流程（本次只到入库）
