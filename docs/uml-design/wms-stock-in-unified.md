# 统一采购入库入口（WMS Stock-In Unified）

> 记录库存入库页「选采购单入库」与「来料通知」两条路径如何汇聚到 `ArrivalAcceptedHandler`，保证 `received_qty + PO 状态 + 应付台账` 三者一致。

## 背景

采购入库历史上存在两条互不通的路径，路径B是断链 bug：

- **路径A（来料通知，闭环）**：`wms/arrival_notice` create→receive→inspect，inspect 发 `ArrivalInspected` 事件，`ArrivalAcceptedHandler`（`purchase/arrival_handler.rs`）异步回写 PO `received_qty`/状态 + 立应付台账。**来料通知流程本身不写库存。**
- **路径B（库存入库页，断链）**：`abt-web/pages/wms_stock_in_create.rs::create_stock_in` 在 `source_type=purchase` 时直接调 `InventoryTransactionService::record()` 只写库存，**不回写 PO、不发事件、不立台账、不建来料通知**。

后果：走路径B 的 PO 永远显示「待收货」（received_qty=0）、应付未立，但货已入库。事故案例：18 个 PO 断链（含 PO450，手工修复）。

## 治本方案：统一汇聚到 ArrivalAcceptedHandler

`create_stock_in` 的 `source_type=purchase` 分支不再直接 `record()`，而是在事务内编排来料通知闭环，复用现有回写链路。用户操作不变（仍在库存入库页选采购单 → 入库）。

```
create_stock_in(source_type=purchase)
  └─ 事务包裹（state.pool.begin）
       └─ 按 PO id 分组 web_items
            └─ 每个 PO：
                 1. po_svc.get / list_items → supplier_id + product_id→order_item_id 映射
                 2. arrival_svc.create（来料通知 Draft，明细 declared_qty=本次入库量）
                 3. arrival_svc.list_items → product_id→an_item_id
                 4. arrival_svc.receive（received_qty=入库量）
                 5. arrival_svc.inspect（accepted_qty=入库量）
                    └─ 内部 publish ArrivalInspected（事件行入 tx，NOTIFY commit 后生效）
                    └─ 内部立 CostEntry（材料成本）+ cancel InvRes（采购入库无预留，affected=0）
                 6. record_stock_in_item（source_type="arrival_notice", source_id=来料通知id）
       └─ tx.commit
            └─ EventProcessor（异步）→ ArrivalAcceptedHandler
                 └─ recompute_received_qty（SUM 来料通知 accepted_qty）
                 └─ batch_update_received_qty + PO 状态（Confirmed→PartiallyReceived→Received）
                 └─ 立应付台账（ar_ap_ledger Credit，source=来料通知，金额=Σ accepted×单价）
```

## 关键约束

- **默认免质检**：采购入库到货即入库。`inspect` 内部 `check_qms_gate` 在无 QMS 检验结果时自动通过（`arrival_notice/implt.rs:338-340`）。需质检的物料仍走来料通知模块手工检验。
- **事务一致性**：`create_stock_in` 全程事务包裹（修正历史 autocommit 违规，范本 `shipping_detail.rs::ship_shipping`）。事件 publish 在事务内落表，handler 由 `EventProcessor` 用独立连接异步处理（最终一致，秒级），与路径A 行为一致。
- **超收容差**：`ArrivalAcceptedHandler` 校验 `received_qty > quantity×(1+容差%)` 报错 → 整个事务回滚（含来料通知+库存）。
- **部分入库**：多次入库各建独立来料通知，`recompute_received_qty` 全量 SUM 累计，PO 状态自动流转。
- **库存来源关联**：库存流水 `source_type="arrival_notice"`、`source_id=来料通知id`（治本后），便于追溯。arrival/work_order/manual 来源保持原有直接 `record()` 逻辑（`handle_direct_stock_in`）。
- **库位占用校验（同物料合并放行，Issue #98）**：`InventoryTransactionService::record()` 入库前校验目标 bin 是否已被其他产品占用——目标产品**已在该 bin 有库存时放行**（同物料合并，即使 bin 同时混放其他产品），仅阻止"全新物料混入已占用库位"（`stock_ledger/repo.rs::has_stock_in_bin` + `find_other_occupant_in_bin`）。背景：历史默认库位 `DEFAULT-*` 多产品混放，入库页 `suggest_bins`「同物料合并」策略把混放库位置顶推荐，强制排他会阻断正常补货、导致整个入库事务回滚（来料通知+流水+台账全不写入，零痕迹）。
- **入库流水查询匹配来源单号（Issue #98）**：入库管理页搜索框同时匹配 `doc_number`（入库单号 RK-）和 `source_doc_number`（来源单号 PO-），`(doc_number ILIKE $n OR source_doc_number ILIKE $n)`（`inventory_transaction/repo.rs::query`）。原仅匹配 RK 号导致按 PO 号搜索查不到流水。

## 枚举值参考（实测）

| 字段 | 值 | 含义 |
|---|---|---|
| `arrival_notices.status` | 4 | Accepted |
| `purchase_orders.status` | 2→4 | Confirmed→Received（部分收→3） |
| `document_links.source_type` | 16 | ArrivalNotice |
| `document_links.target_type` | 7 | PurchaseOrder |
| `document_links.link_type` | 6 | Fulfills |
| `document_links.path` | `AN.{anid}.PO.{poid}` | |
| `ar_ap_ledger.party_type` | 2 | Supplier |
| `ar_ap_ledger.source_type` | 16 | ArrivalNotice |
| `ar_ap_ledger.direction` | 2 | Credit |

## 关联文件

- `abt-web/src/pages/wms_stock_in_create.rs::create_stock_in` — 唯一改动（事务包裹 + purchase 分支编排）
- `abt-core/src/wms/arrival_notice/implt.rs` — create/receive/inspect/list_items（零改动，编排依据）
- `abt-core/src/purchase/arrival_handler.rs` — ArrivalAcceptedHandler 回写逻辑（零改动）
- `scripts/fix-broken-po-arrival.sql` — 历史 17 个断链 PO 修复（received_qty=LEAST(入库量,订单量)，超收截断）
- `abt-web/tests/ar_ap_handler_e2e.rs::k6` — 端到端验证

## 幂等防护（防双击/网络重试）

- **前端**：表单加载时 Hyperscript `on load` 生成 `idempotency_key`（hidden input）；提交按钮 `hx-disabled-elt="#stockin-submit-btn"` 在请求期间禁用。
- **后端**：`create_stock_in` 事务内调 `IdempotencyService::try_claim(key)`（`shared/idempotency`）——纯 `INSERT ON CONFLICT DO NOTHING`、**不重置状态**（区别于事件处理幂等 `check_and_mark` 会重置 Processing 残留），重复 key 直接幂等返回（HX-Redirect）。记录带 `expires_at`（1h）由 `cleanup_expired` 清理。
- **并发安全**：PostgreSQL unique 约束 + 行锁保证——并发第二个请求阻塞至首个事务 commit 后 ON CONFLICT 跳过；首个事务 rollback 则第二个成为首次。适合 HTTP 并发幂等。
- 验证：`tests/ar_ap_handler_e2e.rs::k7`（同 key 提交两次，第二次跳过，来料通知不重复）。

## CostEntry 归属修正

`inspect` 立的材料成本 `entity_id` 已从来料通知 id 修正为 `notice.purchase_order_id.unwrap_or(req.id)`（PO id），匹配 `entity_type=CostEntityType::PurchaseOrder` 语义（`arrival_notice/implt.rs`）。当前 `fms/cost_accounting/repo.rs` 无 PurchaseOrder 维度成本查询消费，修正是预防性（将来加 PO 成本查询时 entity_id 正确）。

## Issue #98：库位占用校验细化 + 入库查询匹配来源单号

**现象**：合并销售单（多 Demand）生成的采购单（如 PO-2026-06-000455，由 Demand 390+391 合并下发）入库后，入库管理页查不到流水，PO `received_qty` 不变、零来料通知/库存流水痕迹。

**根因**：入库页 `suggest_bins`「同物料合并」策略把默认库位（`DEFAULT-*`，历史混放多产品）置顶推荐，用户选中后 `record()` 的「一库位一产品」硬校验 `find_other_occupant_in_bin` 命中其他产品 → `BusinessRule("库位已被其他产品占用…")` → HTTP 400 → 整个入库事务回滚 → 零痕迹。"仅合并 PO 特有"是归纳偏差（其物料恰在混放默认库位有大量库存被强推荐）；不选库位走 `resolve_default_bin` 解析到专属库位则正常。

**修复**：
1. **同物料合并放行**：`record()` 校验细化——目标产品已在该 bin 有库存（`StockLedgerRepo::has_stock_in_bin`）时放行，仅阻止"全新物料混入已占用库位"。改动：`inventory_transaction/implt.rs` + 新增 `stock_ledger/repo.rs::has_stock_in_bin`。
2. **查询匹配来源单号**：`inventory_transaction/repo.rs::query` 搜索条件由 `doc_number ILIKE` 改为 `(doc_number ILIKE OR source_doc_number ILIKE)`，PO 号可搜。

**回归测试**：`abt-web/tests/wms_stock_in_issue98.rs`——i1 同物料合并放行 / i2 新料混入拒绝（排他仍生效）/ i3 PO 号查询匹配。
