# 物料消耗追踪实现计划

**路径**: `/admin/mes/material-usage`
**复杂度**: ★★☆

## Context

当前 `mes_material_usage.rs` 是 stub。原型 `04-material-usage.html` 展示工单维度下的 BOM 标准用量 vs 实际消耗 vs 倒冲消耗的对比分析。

**数据来源**：
- BOM 标准用量：`work_orders.bom_snapshot_id` → `bom_nodes` 叶子节点 → 单件用量 × 完成数量
- 倒冲消耗：`WMS BackflushService.list()` 按 `work_order_id` 过滤 → `BackflushItem` 明细
- 实际领料：WMS 领料模块（如已有）或从 backflush 记录汇总

## 原型设计要点

- **工单选择器**：下拉选择工单（显示工单号+产品名）
- **工单头部信息**：工单号、产品名、状态标签、计划数量、完成数量、BOM版本
- **4 个统计卡片**：BOM标准用量、实际消耗/领料、倒冲消耗、用量差异
- **BOM 对比表**：按物料行逐行对比（物料编码/名称/单位/单件用量/标准总量/领料/倒冲/损耗率/差异）
- **倒冲明细表**：该工单的所有倒冲记录（时间/触发单据/批次/入库数量/物料数/差异/状态）

## 实现步骤

### Step 1: 后端 — 新增 Material Usage 查询

不需要新建 service 模块，复用现有服务。在 dashboard 或独立创建一个查询方法。

**方案A（推荐）**：在 `mes/dashboard/` 扩展，新增物料对比查询

**`abt-core/src/mes/dashboard/model.rs`** — 新增：

```rust
/// 工单物料消耗汇总
#[derive(Debug, Clone, FromRow)]
pub struct MaterialUsageSummary {
    pub standard_qty: Decimal,       // BOM 标准总量
    pub backflush_qty: Decimal,      // 倒冲消耗总量
    pub variance_qty: Decimal,       // 差异
    pub variance_rate: Decimal,      // 差异率
}

/// BOM 对比行
#[derive(Debug, Clone, FromRow)]
pub struct BomCompareItem {
    pub component_id: i64,
    pub component_code: Option<String>,
    pub component_name: Option<String>,
    pub unit: Option<String>,
    pub per_unit_qty: Decimal,       // 单件用量
    pub standard_total: Decimal,     // 标准总量 = per_unit × completed_qty
    pub backflush_total: Decimal,    // 倒冲消耗总量
    pub loss_rate: Decimal,          // 损耗率
    pub diff_qty: Decimal,           // 差异数量
}
```

**`abt-core/src/mes/dashboard/repo.rs`** — 新增方法：

```rust
pub async fn get_material_usage_summary(executor, work_order_id) -> Result<MaterialUsageSummary>
pub async fn get_bom_comparison(executor, work_order_id) -> Result<Vec<BomCompareItem>>
pub async fn get_wo_basic_info(executor, work_order_id) -> Result<WoBasicInfo>
```

SQL 逻辑：
- `get_bom_comparison`: JOIN `work_orders` → `bom_nodes`（叶子节点）→ 聚合 `backflush_items`
  - `SELECT bn.component_id, p.pdt_code, p.pdt_name, p.unit,
    bn.quantity AS per_unit_qty,
    bn.quantity * wo.completed_qty AS standard_total,
    COALESCE(SUM(bi.actual_qty), 0) AS backflush_total,
    ...`
- `get_material_usage_summary`: 汇总上述数据

**`abt-core/src/mes/dashboard/service.rs`** — trait 新增：
```rust
async fn get_material_usage_summary(...) -> Result<MaterialUsageSummary>;
async fn get_bom_comparison(...) -> Result<Vec<BomCompareItem>>;
async fn get_wo_basic_info(...) -> Result<WoBasicInfo>;
```

### Step 2: 后端 — 路由

**`abt-web/src/routes/mes_receipt.rs`** — `MaterialUsagePath` 已定义在 receipt routes 中。

新增 HTMX 查询路径：
```rust
#[derive(TypedPath, Deserialize)]
#[typed_path("/admin/mes/material-usage/data")]
pub struct MaterialUsageDataPath;
```
用于 HTMX 异步加载工单数据（选择工单后加载对比数据）。

### Step 3: 前端 — 重写 `mes_material_usage.rs`

1. **GET `/admin/mes/material-usage`** — 初始页面
   - 工单选择器（下拉框，`hx-get="/admin/mes/material-usage/data"` + `hx-trigger="change"` + `hx-target="#usage-content"`）
   - 加载所有非 Cancelled 的工单列表供选择
   - 结果区：空 div `#usage-content`

2. **GET `/admin/mes/material-usage/data?wo_id=xxx`** — HTMX 数据片段
   - 调用 `dashboard_svc.get_wo_basic_info()` 获取工单信息
   - 调用 `dashboard_svc.get_material_usage_summary()` 获取统计
   - 调用 `dashboard_svc.get_bom_comparison()` 获取对比表
   - 调用 `backflush_svc.list(BackflushFilter { work_order_id: Some(wo_id), status: None }, 1, 50)` 获取倒冲记录
   - 渲染：工单头部 + 4 统计卡 + BOM 对比表 + 倒冲明细表

### Step 4: CSS — 物料对比样式

**`uno.config.ts`** — 新增：
- `.usage-summary` — 4 列网格
- `.bom-compare` — 对比表特殊样式
- `.diff-indicator` / `.diff-positive` / `.diff-negative` / `.diff-zero` — 差异标签颜色

### 涉及文件

| 文件 | 改动 |
|------|------|
| `abt-core/src/mes/dashboard/model.rs` | 新增 `MaterialUsageSummary` + `BomCompareItem` + `WoBasicInfo` |
| `abt-core/src/mes/dashboard/repo.rs` | 新增 3 个查询方法 |
| `abt-core/src/mes/dashboard/service.rs` | trait 新增 3 方法 |
| `abt-core/src/mes/dashboard/implt.rs` | 实现 3 方法 |
| `abt-web/src/routes/mes_receipt.rs` | 新增 `MaterialUsageDataPath` |
| `abt-web/src/pages/mes_material_usage.rs` | 完整重写 |
| `uno.config.ts` | 新增物料对比 CSS 类 |
