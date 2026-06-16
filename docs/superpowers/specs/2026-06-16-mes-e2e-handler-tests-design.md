# MES 生产模块 E2E Handler 集成测试设计

> 日期: 2026-06-16
> 状态: 已确认
> 参考: 采购模块测试风格 (`abt-web/tests/purchase_flow_e2e.rs`)

## 一、测试目标

对 MES（生产管理）模块进行全方位 Handler 级集成测试，覆盖从**需求池 → 生产计划 → 工单下达 → 批次创建 → 工序报工 → 质检 → 完工入库**的完整生产流程。

每条测试验证：
1. HTTP 状态码正确性（200/400/404）
2. 数据库字段级正确性（通过 Service trait 查询验证 status / qty / 关联字段）
3. 异常边界处理（非法输入、重复操作、状态冲突）

## 二、测试文件结构

```
abt-web/tests/
├── mes_flow_e2e.rs       — 全流程端到端（主文件，串联所有阶段）
├── mes_plan.rs           — 生产计划深度测试
├── mes_order.rs          — 工单生命周期深度测试
├── mes_batch.rs          — 批次 + 工序报工深度测试
├── mes_receipt.rs        — 完工入库 + FQC 门控测试
├── mes_inspection.rs     — 质检生命周期测试
├── mes_demand_pool.rs    — 需求池查询/转计划测试
└── mes_pages.rs          — 页面可达性批量测试
```

## 三、核心链路测试设计（mes_flow_e2e.rs）

### 全流程 happy path

```
创建生产计划(MTS) → 添加计划项 → 确认计划
  → 直接创建工单(Draft)
  → 下达工单(Released) — 验证工序自动创建
  → 拆批创建批次(Pending)
  → 报工 step=1 (InProgress) — 验证 completed_qty 累加
  → 推进入库 (PendingReceipt)
  → 创建入库单(Draft)
  → 确认入库(Confirmed)
  → 关闭工单(Closed)
```

每步通过 Service trait 读取数据库验证状态机正确转换。

### 关键测试常量

```rust
const PRODUCT_ID: i64 = 565;       // 2835/冷白0.5W (有库存的成品)
const WAREHOUSE_ID: i64 = 23320;   // 备料周转仓
const WORK_CENTER_ID: i64 = 1;     // 总装线A
const ROUTING_ID: i64 = 1;         // 模组工艺
const OPERATOR_ID: i64 = 1;        // admin
```

## 四、各模块测试要点

### A. 生产计划 (mes_plan.rs)
- 创建 Draft 计划 + 验证 items
- 确认计划: Draft → Confirmed
- 重复确认返回错误
- 排程 schedule_v1
- 不存在的计划: 404
- 无效日期: 400

### B. 工单 (mes_order.rs)
- 直接创建 Draft 工单（不经计划）
- 下达: Draft → Released，验证工序 (work_order_routings) 自动创建
- 反下达: Released → Draft（幂等性）
- 取消: 任意状态 → Cancelled
- 关闭: Released → Closed（需无活跃批次）
- 拆批: 创建生产批次
- 不存在工单: 404
- 乐观锁冲突

### C. 批次 + 工序 (mes_batch.rs)
- 拆批创建批次，验证 batch_no / card_sn 生成
- 报工 confirm_routing_step: Pending → InProgress
  - 验证 completed_qty 累加
  - 验证 work_report 自动创建
  - 验证 batch_routing_progress 更新
- 防跳序: 前道工序未完成时报工下一道 → 错误
- 幂等报工: 相同参数重复报工 → 返回已有结果
- 暂停/恢复: InProgress → Suspended → InProgress
- 推进入库: InProgress → PendingReceipt
- 报废: 任意 → Cancelled
- 批次详情页渲染

### D. 完工入库 (mes_receipt.rs)
- 创建入库单(Draft)
- 确认入库: Draft → Confirmed
- FQC 门控查询
- 不存在入库单: 404
- 无效数量: 400

### E. 质检 (mes_inspection.rs)
- 创建检验单
- 记录结果: Pass / Fail / Conditional
- 首检/巡检/完工检
- 不存在检验单: 404

### F. 需求池 (mes_demand_pool.rs)
- 列表页渲染
- 物料维度聚合查询
- 按产品筛选

### G. 页面可达性 (mes_pages.rs)
- 所有 MES 列表页 GET 返回 200
- 所有 MES 列表页 HTMX 返回 fragment
- 详情页 404 for 不存在 ID

## 五、Helper 函数设计

```rust
// URL 编码
fn urlenc(s: &str) -> String

// items_json 构造
fn plan_items_json(items: &[(&str, &str, &str)]) -> String  // (product_id, qty, date)

// 创建生产计划并返回 plan_id
async fn create_plan(app: &TestApp, plan_type: &str, items: ...) -> i64

// 创建工单并返回 (wo_id, version)
async fn create_work_order(app: &TestApp, product_id: i64, qty: &str) -> (i64, i32)

// 下达工单
async fn release_work_order(app: &TestApp, wo_id: i64)

// 拆批创建批次并返回 batch_id
async fn create_batch(app: &TestApp, wo_id: i64, qty: &str) -> i64

// Service 层验证
async fn get_plan(app: &TestApp, id: i64) -> ProductionPlan
async fn get_work_order(app: &TestApp, id: i64) -> WorkOrder
async fn get_batch(app: &TestApp, id: i64) -> ProductionBatch
```

## 六、验收标准

1. `cargo clippy` 无新增警告
2. 所有测试 `#[tokio::test]` 可独立运行（无测试间状态依赖）
3. 全流程测试可串联跑通完整链路
4. 覆盖 MES 核心状态机的每个转换路径
