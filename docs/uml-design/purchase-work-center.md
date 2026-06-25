# 采购作业中心（WorkCenter）设计

> 关联：采购 SRM 各子域（需求池 / 报价 / 订单 / 对账 / 付款 / 退货 / 请购）已齐备，缺一个「采购员一进系统就知道先做什么」的聚合作业页。
> 参照：`wms-work-center.md`、`mes_work_center`（组件化单端点模式）。
> 现状：`purchase_dashboard`（`/admin/purchase`）是纯看板——stat card 只计数、待办不可操作、「最近活动」硬编码假数据。

## 1. 定位

采购是**计划 + 执行闭环最长**的业务域（需求 → 询价 → 下单 → 收货 → 对账 → 付款，逆向退货）。采购岗需要一个**作业中心**，把分散在各列表页的「待处理」状态聚合到一屏，就地审批 / 确认 / 发货，不跳详情页。

范式与 MES / WMS work_center 一致：**组件化单端点**（每个 card 一个 GET 端点 + `hx-select` 局部刷新）+ **HX-Trigger 事件联动**（写操作广播，相关 card 自刷新）+ **drawer 就地操作**。

## 2. PurchaseWorkCenterService 接口

```rust
#[async_trait]
pub trait PurchaseWorkCenterService: Send + Sync {
    /// 聚合各业务分组待办计数（首页锚点条 + 各 card 用）
    async fn summary(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
    ) -> Result<PurchaseWorkCenterSummary>;
}
```

**设计原则**（同 WMS）：WorkCenterService 只做**聚合计数**；各 card 列表**复用现有 service 的 `list`**（按状态过滤），不重复实现列表查询，不直访任何 repo。

## 3. PurchaseWorkCenterSummary model

```rust
pub struct PurchaseWorkCenterSummary {
    pub pending_demand: u64,            // 待处理外购需求（物料维度）
    pub pending_misc: u64,              // 待审批零星请购（Draft）
    pub po_pending_approval: u64,       // PO 待审批（PendingApproval）
    pub po_pending_receive: u64,        // PO 待收货（Confirmed）
    pub po_partial: u64,                // PO 部分收货（PartiallyReceived）
    pub recon_draft: u64,               // 草稿对账单（Draft）
    pub payment_pending_approval: u64,  // 付款申请待审批（Draft）
    pub return_pending_ship: u64,       // 采购退货待发货（Confirmed）
    pub return_shipped: u64,            // 采购退货已发出（Shipped）
    pub overdue_count: u64,             // 逾期：待收货订单期望交期早于今日
    pub soon_count: u64,                // 临期：待收货订单期望交期在 7 天内
}
```

`total()` = 前 9 项之和（不含 overdue/soon，避免与待收货计数重复）。

## 4. 各 card「待办」状态边界

| Card | tab / 数据源 | pending 状态（枚举值） | Service |
|---|---|---|---|
| ① 采购需求 | 物料汇总 / 请购明细 | `demand_status=1`（Pending） | `PurchaseDemandService::list_material_aggregated` / `list_pending_demands` |
| ① 采购需求 | 请购（misc） | `MiscRequestStatus::Draft` | `MiscellaneousRequestService::list` |
| ② 采购订单 | 待审批 | `PurchaseOrderStatus::PendingApproval` | `PurchaseOrderService::list` |
| ② 采购订单 | 待收货 | `PurchaseOrderStatus::Confirmed` | 同上 |
| ② 采购订单 | 部分收货 | `PurchaseOrderStatus::PartiallyReceived` | 同上 |
| ③ 对账付款 | 草稿对账单 | `PurchaseReconStatus::Draft` | `PurchaseReconciliationService::list` |
| ③ 对账付款 | 待审批付款 | `PaymentStatus::Draft` | `PaymentRequestService::list` |
| ④ 采购退货 | 待发货 | `PurchaseReturnStatus::Confirmed` | `PurchaseReturnService::list` |
| ④ 采购退货 | 已发出 | `PurchaseReturnStatus::Shipped` | 同上 |

## 5. 实现策略

`PurchaseWorkCenterServiceImpl::summary`（`abt-core/src/purchase/work_center/implt.rs`）：

- 按需工厂获取各 service（`new_purchase_order_service(self.pool.clone())` 等，struct 只持 `PgPool`）
- 每项计数调对应 `list(status=..., PageParams::new(1,1))` 取 `total`，经 `cnt()` helper **容错**：单项查询失败 `tracing::warn!` 后记 0，不连累整页（同 MES）
- **逾期 / 临期**：扫描待收货（Confirmed + PartiallyReceived）订单首页 500 条，按 `expected_delivery_date` 判定（`< today` 逾期，`<= today+7` 临期），近似统计

## 6. 前端（`abt-web`）

**路由**（`/admin/purchase/work-center`）：

| 路径 | 方法 | 说明 |
|---|---|---|
| `/admin/purchase/work-center` | GET | 主页（detail-header + 锚点条 + 4 card shell + 2 drawer overlay） |
| `/admin/purchase/work-center/demand` | GET | ① 需求 card（tab + 搜索 + `hx-select=#pc-demand-card`） |
| `/admin/purchase/work-center/orders` | GET | ② 订单 card |
| `/admin/purchase/work-center/settlement` | GET | ③ 对账付款 card |
| `/admin/purchase/work-center/returns` | GET | ④ 退货 card |
| `/orders/{id}/approve-drawer` | GET | 订单审批 drawer body |
| `/payments/{id}/approve-drawer` | GET | 付款审批 drawer body |
| `/orders/{id}/approve` | POST | 审批通过 → `HX-Trigger: poChanged` |
| `/orders/{id}/reject` | POST | 驳回 → `poChanged` |
| `/reconciliations/{id}/confirm` | POST | 对账确认 → `reconChanged` |
| `/payments/{id}/approve` | POST | 付款审批 → `reconChanged` |

**HTMX 契约**：
- card shell：`hx-trigger="load, poChanged from:body, reconChanged from:body, returnChanged from:body"`（懒加载 + 写操作后自刷新）
- card 内 tab/搜索：`hx-target="#pc-xxx-card" hx-select="#pc-xxx-card" hx-swap="outerHTML" hx-push-url="true"`
- 写操作：事务包裹（`state.pool.begin()...tx.commit()`），返回 `([("HX-Trigger", "poChanged")], Html::empty())`

**事件命名**：
- `poChanged` — 订单审批/驳回（影响订单 card + 锚点条）
- `reconChanged` — 对账确认/付款审批（影响对账付款 card）
- `returnChanged` — 预留（退货发货 Phase 2 就地化时启用）

**drawer**：复用 `render_drawer_overlay`（overlay + `open:` 变体 + Hyperscript 关闭），body 由 `hx-get` 填充。

## 7. Phase 划分

- **Phase 1（已实现）**：4 只读 card + 锚点条 + 订单审批/付款审批 drawer + 审批/驳回/对账确认/付款审批 4 写操作。转单 / 登记收货 / 退货发货 / 创建对账单 按钮暂跳转各详情页。
- **Phase 2（后续）**：上述复杂操作的就地 drawer（转单选供应商/报价、收货登记触发来料通知、退货发货物流录入、创建对账单期间选择）。
