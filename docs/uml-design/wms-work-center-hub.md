# WMS 作业中心 Hub 设计（WorkCenter Hub）

> 关联：#93 已上线 WorkCenter（dashboard 式计数墙）→ 升级为「就地可执行工作台」
> 参照：`03-shipping-hub.html`（Doc Hub 范式，本设计同款）；[`wms-doc-hub.md`](wms-doc-hub.md)（Doc Hub 契约，互为镜像）
> 配套原型：`03-work-center-hub.html`
> 状态：**Phase A/B 已上线，Phase C（就地操作）已落地**（5 drawer + 2 跳转 + 全域深链，见 §3/§8）
> 现行实现：`abt-web/src/pages/wms_work_center.rs`（摘要带 + 7 disclosure 懒加载 + 共享 `#wc-drawer-overlay` 就地操作 + taskDone 联动）；`WorkCenterService::{summary,list_pending,urgent_summary}`

## 1. 背景与定位

### 1.1 问题：当前 WorkCenter 是 dashboard，不是工作台

`wms_work_center.rs` 现状是「计数墙 + 跳转」：

- 7 张卡片只显示待办计数，点击 → 跳各域列表页 → 再进详情 → 才能操作，**两段断点**
- 底部「紧急/临期」区是**示意假数据**（#93 followup P1 item 4 至今未接）
- htmx 几乎没用：整页一次 `summary()` SSR；原型里 `setInterval` 局部刷新注释掉没做
- 仓库员「一个页面完成更多工作」做不到——看一眼 7 个数字就跳走

### 1.2 目标：就地可执行工作台

采用 `03-shipping-hub.html` 同款「少即是多」范式（摘要带 + disclosure 折叠 + drawer 就地操作）重做，**摒弃卡片网格**：

- **摘要带**：总待办 / 紧急 / 逾期，替代「您有 N 项待办」
- **7 个 disclosure 分区**（替代卡片）：展开是该环节的待办单据队列（按紧急度排序），**不是跳转**
- **队列行就地 drawer 操作**：拣货 / 收货 / 确认发出，复用 Doc Hub 的 drawer 组件，不离开本页
- 仓库员进 WorkCenter 即可逐项处理日常作业，**一页完成**

### 1.3 与 Doc Hub 的镜像关系

两者 disclosure 结构对称，组成 WMS 前端统一范式：

| | WorkCenter Hub（本设计） | Doc Hub（`wms-doc-hub.md`） |
|---|---|---|
| 聚合维度 | 按**环节**聚合多张单据 | 按**单据**拆解多个子域 |
| disclosure 内容 | 该环节 top N 待办单据队列 | 该单据的明细/拣货/事务/日志 |
| Header | 摘要带（无状态步骤条/来源链） | 单号 + 状态步骤条 + 来源链 + 摘要带 |
| 共享 | **drawer 组件**（拣货 / 收货 / 确认发出） | |
| 跳转 | 队列行点单据号 → Doc Hub 看全生命周期 | — |

## 2. 页面骨架（对齐 shipping-hub，无卡片）

### 2.1 Header 层
- 标题：仓库作业中心 + 当班员 + 日期
- **待办通知锚点条** `todo-nav`（sticky 吸顶）：左侧总待办数 + 横向环节 chip 列表。**只显示 `count > 0` 的环节**（无待办的环节不露脸，少即是多）；每个 chip 带计数 + 异常染色（逾期红 / 临期橙）+ 脉冲点。点击 chip → `toggleAndScroll` 平滑滚动并展开对应 disclosure。替代原「您有 N 项待办」单一数字
- 就地操作后（`taskDone` 事件）整条重拉：计数下降、处理完毕的 chip 自动消失
- **无状态步骤条、无来源链**（WorkCenter 不是单据，是聚合视图）

### 2.2 Disclosure 层（7 环节，懒加载队列）
每个环节一个折叠卡片（替代原卡片网格的一格）：
- header：图标（角标红点 if 该环节有逾期/紧急）+ 环节名 + 摘要（"3 笔 · 1 逾期 · 最久 2h"）+ chevron
- **默认折叠**：仅 header + 计数摘要，不查明细
- 展开时 `hx-get` 拉该环节 top N 待办队列（懒加载）
- 有逾期/紧急的环节默认展开 + 红点驱动

### 2.3 Drawer 层（就地操作，复用 Doc Hub drawer）
队列每行带快操作按钮，点击弹出 drawer（与 shipping-hub 同套 drawer 组件）。

### 2.4 整页
- 首屏 `GET /admin/wms/work-center` 只查 `summary()` + `urgent_summary()`，渲染 Header + 7 个 disclosure header（折叠态）
- 展开某环节才 `list_pending` 查明细；操作后 htmx 片段级刷新

## 3. 7 环节映射 + 就地操作范围

| 环节 | pending 状态（对齐 summary） | 就地操作（drawer） | 复杂操作（跳 Doc Hub） |
|---|---|---|---|
| 待收货 | `ArrivalStatus::Draft` | 收货核对 | 多行大批量收货 |
| 待质检 | `ArrivalStatus::Inspecting` | 跳 QMS 录结果 | — |
| 待拣货 | `PickListStatus::Draft` | **拣货录入**（复用） | 部分拣 / 多库位 |
| 待发货 | `ShippingStatus::Confirmed` / `Picking` | **确认发出**（复用） | — |
| 待领料 | `RequisitionStatus::Confirmed` / `PartiallyIssued` | 发料 | 退料 |
| 待调拨 | `TransferStatus::Draft` / `InTransit` | 确认收货 | — |
| 待盘点 | `CycleCountStatus::Draft` / `Counting` / `PendingReview` | — | 盘点录入（跳 Doc Hub） |

**就地操作范围克制**：只有「快操作」（单一动作）就地；复杂操作（盘点录多库位、形态转换）跳 Doc Hub。点单据号始终可跳 Doc Hub 看全貌。

### 3.1 落地说明（Phase C · PR1-5 已实现）

所有就地操作统一进**单一共享 drawer** `#wc-drawer-overlay`（各域 GET 端点填 `#wc-drawer-body`），提交后广播 `HX-Trigger: {"taskDone":"","closeWcDrawer":""}` 联动刷新 disclosure 队列 + todo-nav 计数、关闭 drawer（不整页刷新）。深链（单据号→各域详情页）在 abt-web 前端按 `domain + doc_id` 计算（`domain_detail_url`），**不入 `PendingTask` model**（分层：abt-core 不硬编码前端 URL）。

| 环节 | drawer 类型 | service 调用 | 备注 |
|---|---|---|---|
| 待收货 | 行级收货量 + 批次 | `arrival_notice.receive` | 顶部警示「收货后待质检才入库」；默认实收=申报 |
| 待质检 | 跳转（无 drawer） | — | 跳**来料通知详情**（inspect 5 步联动在该页触发，非独立 QMS 页） |
| 待拣货 | 行级拣货量 | `pick_list.record_pick_items + complete_pick` | 已迁共享 drawer |
| 待发货 | 确认型·状态分流 | `outbound.ship` | GET 查状态：Picking→确认发出；Confirmed→「需先拣货」跳详情（ship 前必须 pick） |
| 待领料 | 全量发料（Confirmed） | `material_requisition.issue` | PartiallyIssued→跳详情：issue 记库存事务用绝对量，就地重复发料会重复扣库存 |
| 待调拨 | 确认型·状态分流 | `transfer.dispatch` / `complete` | GET 查状态：Draft→调出；InTransit→到货确认；POST 带 `action` 字段分发 |
| 待盘点 | 跳转（无 drawer） | — | 多状态多动作（start/count/complete/adjust/approve），跳盘点详情 |

权限：GET `INVENTORY/read`，POST 对齐各域（发货 `SHIPPING/update`，其余 `INVENTORY/update`）。错误处理复用全局 toast（`static/app.js` 兜底 htmx 4xx/5xx + 必填校验）。

## 4. 接口设计（WorkCenterService 扩展）

### 4.1 trait 扩展
```rust
#[async_trait]
pub trait WorkCenterService: Send + Sync {
    /// 聚合各环节待办计数（保留；摘要带 + disclosure header 用）
    async fn summary(&self, ctx: &ServiceContext, db: PgExecutor<'_>) -> Result<WorkCenterSummary>;

    /// 新增：某环节的待办单据队列（disclosure 展开懒加载）
    async fn list_pending(
        &self, ctx: &ServiceContext, db: PgExecutor<'_>,
        domain: WorkCenterDomain, page: PageParams,
    ) -> Result<PaginatedResult<PendingTask>>;

    /// 新增：紧急/逾期汇总（摘要带的紧急/逾期数字；消化 #93 P1 item 4）
    async fn urgent_summary(&self, ctx: &ServiceContext, db: PgExecutor<'_>) -> Result<UrgentSummary>;
}
```

### 4.2 model 新增
```rust
pub enum WorkCenterDomain {
    Arrival, Inspection, Pick, Outbound, Requisition, Transfer, CycleCount,
}

pub enum Urgency { Normal, Soon, Overdue }

/// 跨域统一待办视图（各域实体在 WorkCenterService 内映射成此结构）
pub struct PendingTask {
    pub doc_id: i64,
    pub doc_number: String,
    pub domain: WorkCenterDomain,
    pub counterparty: String,           // 客户 / 供应商
    pub summary: String,                // "3 项 · 320 件"
    pub expected_at: Option<DateTime<Utc>>,
    pub urgency: Urgency,
    // 注：单据号深链（→ 各域详情页）在 abt-web 前端按 domain + doc_id 计算（domain_detail_url），
    // 不入 model——分层约定：abt-core 不硬编码前端 URL。
}

pub struct UrgentSummary {
    pub overdue_count: u64,             // 逾期
    pub soon_count: u64,                // today+N 内临期
}
```

### 4.3 紧急度口径（消化 #93 followup P1 item 4）
| 环节 | 逾期 `Overdue` | 临期 `Soon` |
|---|---|---|
| 待收货 | `arrival_notice.expected_date < today` 且 Draft | `≤ today+N` |
| 待发货 | `outbound.expected_ship_date < today` 且 Confirmed/Picking | `≤ today+N` |
| 待拣货 | `pick_list.created_at < now − 阈值` 且 Draft（无到期日，用创建时长判超时） | — |
| 其他 | 按各自 `expected_date` | — |

### 4.4 `list_pending` 实现策略
复用 `implt.rs` 现有 `cnt` 容错模式（各域 `service.list(Filter{status}, page)`），**放大 `page_size`** 取真实 items（而非仅 total），映射成 `PendingTask`：

- 各域 list 返回实体类型不同（`ArrivalNotice` / `PickList` / `ShippingRequest` …）→ 在 WorkCenterService 内各自 `to_pending_task` 适配成统一结构
- **紧急度排序**：MVP 在 WorkCenterService 内存按 `urgency` → `expected_at` 排序，**避免改 6 个域 service 的排序参数**；top N 截断
- 查询失败容错沿用 `cnt` 模式（`tracing::warn`，该环节记空队列，不连累整页）

## 5. HTMX 契约（对齐 shipping-hub §5）

### 5.1 disclosure 懒加载
```
GET /admin/wms/work-center/fragments/{domain}
  domain ∈ {arrival, inspection, pick, outbound, requisition, transfer, cycle-count}
  → 返回该环节 top N 待办队列 HTML 片段（Maud 渲染）
```
disclosure header `hx-get` + `hx-target="next .di-body"`，首展拉取一次。

### 5.2 就地操作 + 多区联动
drawer 提交后 `HX-Trigger: "taskDone"`：
```
→ 当前 disclosure：hx-trigger="taskDone from:body" 重拉队列（单据出列、计数下降）
→ 摘要带：         hx-trigger="taskDone from:body" 重拉 summary / urgent_summary
```

### 5.3 与约束的关系
fragment 端点是 WorkCenter 页的**子资源**（按环节），不违反「列表页单端点」约束（同 shipping-hub §5.4）。

## 6. 约束兼容（同 shipping-hub §7）
事务包裹 / TypedPath / `hx-target="closest"` / hyperscript `_=` / UnoCSS 原子类 / fragment 子资源。drawer 直接复用 Doc Hub 组件（`hx-target="this"` 自包含）。

## 7. 与 #93 followup 关系
- **P1 item 4（紧急/临期真实数据）**：★ **消化**。`urgent_summary` + 各 disclosure 红点 + 队列紧急度排序，替掉原假数据墙。
- **P1 item 5（拣货录入页）**：与 Doc Hub 共享 drawer，一并消化。
- P1 item 6（ship 校验 Picked）：Doc Hub 侧。

## 8. 实施阶段
- **Phase A（后端）**：`WorkCenterService` 加 `list_pending` / `urgent_summary` + model；紧急度计算 + 单测。✅ 已上线
- **Phase B（前端骨架）**：`wms_work_center.rs` 重写为 Hub（摘要带 + 7 disclosure + drawer 骨架），**删卡片网格**。✅ 已上线
- **Phase C（就地操作）**：拣货 / 收货 / 发货 / 领料 / 调拨 drawer 接各域 service + 全域深链 + 质检/盘点跳转。✅ 已落地（PR1-5，见 §3.1）
- **Phase D（配置化）**：临期 N 天 / 拣货超时阈值进 `wms/settings`。⏳ 待办

每阶段独立 PR，远程 `weichen`，feature 分支（`feat/wms-wc-*`）。

## 9. 风险与决议点
1. **紧急度排序性能**：`list_pending` 拉各域 top N 后内存排序；某环节待办量大（>100）时需各域 service 支持按 `expected_date` 排序查询。**建议**：MVP 内存排序 + top N 截断（如 20）。
2. **跨域映射成本**：7 域实体各异，每域一个 `to_pending_task` 适配。可接受。
3. **就地操作 vs 跳 Doc Hub 边界**：见 §3，快操作就地、复杂跳转。
4. **默认展开策略**：有逾期/紧急的环节默认展开（异常驱动），其余折叠。**建议采纳**。
5. **阈值配置**：临期 N 天 / 拣货超时阈值，硬编码 vs `wms/settings`。**建议** settings。
