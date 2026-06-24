# Issue #93 后续待办（Follow-up）

> 关联：[Issue #93](https://github.com/swloki/abt2/issues/93) / [PR #94](https://github.com/swloki/abt2/pull/94)
> 分支：`feat/issue-93-shipping-to-wms`
> 本文记录 #93 已完成内容 + 剩余待办，按优先级排列。

## 一、已完成（PR #94）

| Phase | 内容 | 关键 commit |
|---|---|---|
| **Phase 1 后端重构** | `shipping_request` 迁 `wms/outbound`；`ship()` 去跨域 repo 直访（AR/COGS 移交 `ShipmentShipped` 事件）；新增 `ShipmentShippedHandler`（5 trait 立账）；`SalesReturnReceivedHandler` 改 `post_entry`；fms `ArApService::post_entry` 立账接口；`SalesOrderService::record_shipment/delivery_status`；修复 `SalesOrderItemRepo` 跨域直访 | `ed46a720` |
| **Phase 2 前端归属** | 路由 `/admin/shipping/*`→`/admin/wms/shipping/*`（旧路径 308 重定向）；废弃 `/admin/wms/stock-out` 旁路；侧边栏销售组删「发货申请」，wms「出库管理」改指发货单 | `5bc00ce9` |
| **Phase 3 PickList 后端** | migration 071（pick_lists/pick_list_items 表）；`DocumentType::PickList=46`；`wms/pick_list` 模块（`PickListService`）；`outbound::pick()` 集成（MVP 自动满拣 + complete） | `7c02862a` |
| **WMS 作业中心** | `WorkCenterService.summary` 聚合 7 域待办（容错）；`/admin/wms/work-center` 页（对齐原型 `03-work-center.html`） | `64a23b94` `fea4911e` |

**架构达成**：跨域全部经 Service trait + Model，零 repo 直访；发货归仓库；仓库有「作业中心」待办看板（Odoo Operations 范式，非需求池）；#36 消解。

---

## 二、剩余待办

### P0 — 合并 / 环境相关（合并前）

1. **PR #94 review / merge** 到 master
2. **工作区 WIP 确认**：会话前遗留的未提交改动（`fms/adjustment/*`、`master_data/*`、`lib.rs`、`.sqlx/*` 删除、`.gitignore`）—— 非 #93 改动，本次未碰，需确认其归属（是别的在途工作还是可丢弃）
3. **migration 071 其他环境**：本地已 `psql -f` 跑过；staging/生产部署时需执行 `abt-core/migrations/071_create_pick_lists.sql`（项目无 migration runner，手动 psql）

### P1 — 功能完整性

4. **紧急 / 临期提醒区接真实数据**（作业中心底部）：当前为示意数据。需 `WorkCenterService` 加 urgent 计算：
   - 待收货逾期：`arrival_notice` 的 `expected_date < today` 且状态 Draft/Received
   - 发货临期：`outbound` 的 `expected_ship_date ≤ today+N` 且 Confirmed/Picking
   - 拣货超时：`pick_list` 的 `created_at < now-阈值` 且 Draft
   - 返回 `Vec<UrgentItem>`，前端替换示意数据
5. **PickList 前端拣货录入页**（`/admin/wms/shipping/{id}/pick`）：原型已就绪（`03-pick-list.html`），但 Maud SSR 页未实现。当前是 MVP 自动满拣（`pick()` 一步 generate+complete），若要**人工拣货**（部分拣 / 指定库位 / 手动 complete）：
   - 改 `outbound::pick()`：只 `generate_from_outbound`（留 Draft），不自动 complete
   - 新增前端页：录入 `picked_qty` / `bin_id` → `complete_pick`
   - 路由 + TypedPath + Maud 页（参照原型 + `page-creator` 流程）
6. **ship() 校验 PickList Picked**（可选）：当前 MVP 不强依赖（ship 基于 Picking 状态）。若要"必须拣货完成才能发货"，`ship()` 前置校验关联 pick_list 已 Picked

### P2 — 优化 / 技术债

7. **ShipmentShippedHandler 迁 fms**：设计文档原议放 fms（立账是 fms 职责），实际放 sales（与 `SalesReturnReceivedHandler` 对称、最小改动）。跨域已走 trait（合规），但语义归属可后续优化到 fms
8. **其他立账点统一 `post_entry`**（#93 范围外的同类违规）：以下业务点仍直访 `ArApLedgerRepo`（跨域），后续统一经 `ArApService::post_entry`：
   - 采购入库 `ArrivalAcceptedHandler`
   - 采购退货 `PurchaseReturnSettledHandler`
   - 委外收货 `OutsourcingOrder::receive`
   - 收付款 `CashJournal::confirm`
9. **UML HTML 同步**：`docs/uml-design/01-sales.html` / `03-wms.html` 是 Mermaid UML，需人工反映 shipping 从 sales 迁到 wms（markdown 设计文档 `wms-shipping-outbound.md` / `wms-work-center.md` 已同步）
10. **WorkCenter 紧急区后端测试**：urgent 计算逻辑（到期日/超时）需测试覆盖

---

## 三、不在 #93 范围（明确排除）

- 发货与开票的进一步解耦（已通过 `qty_delivered` 反写实现基础）
- 拣货策略优化（FIFO/FEFO/最短路径的库位建议，当前 pick_list 仅记 requested/picked_qty，bin_id 可选未用）
- 仓库作业中心的「最近动态 / 操作日志流」（原型未涉及，可作三期增强）
