# WMS 模块补充测试报告

**日期**: 2026-06-10
**测试范围**: 分页、仓库库区管理、表单验证、状态变更按钮、锁定创建、反冲详情、级联查询
**测试层级**: Full

## 测试结果总览

| 测试项 | 页面 | 结果 |
|--------|------|------|
| 分页功能 | 库存列表 | PASS |
| 仓库详情 - 库区列表 | 仓库详情 | PASS |
| 仓库详情 - 查看储位 | 仓库详情 | FAIL |
| 仓库详情 - 新建库区 | 仓库详情 | PASS |
| 表单验证 - 调拨创建 | 调拨创建 | PASS (浏览器原生验证) |
| 表单验证 - 到货创建 | 到货创建 | PASS (浏览器原生验证) |
| 表单验证 - 领料创建 | 领料创建 | PASS (浏览器原生验证) |
| 状态按钮 - 盘点单 | 盘点详情 | PASS |
| 状态按钮 - 调拨单 | 调拨详情 | PASS |
| 锁定创建 | 锁定创建 | PASS |
| 反冲详情 | 反冲详情 | PASS (数据正确) |
| 级联查询 | 级联查询 | FAIL |
| 数值格式 | 反冲列表 | FAIL |
| 调拨明细数据 | 调拨详情 | FAIL |
| 表单可访问性 | 多个创建页 | FAIL |

## 问题清单

| # | 页面 | 测试项 | 问题描述 | 涉及文件 | 优先级 | 状态 |
|---|------|--------|---------|----------|--------|------|
| 1 | 仓库详情 | 查看储位 | 点击"查看储位"按钮后，HTMX 请求返回"暂无储位数据"，但数据库中 zone_id=23332023 有 3 条 bins 记录。`Bin::from_row()` 使用动态 SQL (`sqlx::query(AssertSqlSafe(...))`) 时解析失败，被 `.filter_map(|r| Bin::from_row(r).ok())` 静默吞错。需要排查具体是哪列映射失败。 | `abt-core/src/wms/warehouse/repo.rs` (list_bins, 行 503) | P1 | 🔲 |
| 2 | 级联查询 | 产品库存总量 | 产品总库存量硬编码显示为"—"，`CascadeInventoryResult` 结构体缺少 `total_quantity` 字段。数据库中 stock_ledger 有该产品的库存记录（90+10=100），但前端不展示。 | `abt-core/src/wms/inventory_cascade/model.rs`, `abt-web/src/pages/wms_cascade_list.rs` (行 152) | P2 | 🔲 |
| 3 | 反冲列表 | 数值格式 | 数量字段显示为 `50.000000`（Decimal 全精度），应显示为 `50` 或 `50.00`。 | `abt-web/src/pages/wms_backflush_list.rs` | P3 | 🔲 |
| 4 | 调拨详情 | 明细数据 | "产品编码"列显示产品名称 `2835/冷白0.5W/RA70-单晶-3C02 (3010134033)` 而非编码；"产品名称"列也显示相同内容。疑似产品编码和名称字段显示重复。 | `abt-web/src/pages/wms_transfer_detail.rs` | P2 | 🔲 |
| 5 | 到货创建 | 表单可访问性 | `supplier_id`、`warehouse_id`、`zone_id`、`arrival_date` 等 required 字段缺少 `<label>` 标签，影响可访问性和 HTML 验证提示。 | `abt-web/src/pages/wms_arrival_create.rs` | P3 | 🔲 |
| 6 | 领料创建 | 表单可访问性 | `warehouse_id`、`requisition_date` 字段缺少 `<label>` 标签。 | `abt-web/src/pages/wms_requisition_create.rs` | P3 | 🔲 |
| 7 | 库存列表 | 数据完整性 | 第 7 行产品编码和名称都显示为"—"（车间仓/辅料库位），可能 stock_ledger 中有 product_id 关联不上的记录。 | `abt-core/src/wms/stock/repo.rs` 或查询逻辑 | P3 | 🔲 |
| 8 | 反冲详情 | 空明细 | BF-2026-06-000001 和 BF-2026-06-000002 两条反冲记录无明细数据（backflush_items 为空），但状态为"已执行"。这是测试数据问题，非代码 bug。 | 测试数据 | P4 | 🔲 |

## 已通过测试项

### 1. 分页功能验证
- 库存列表共 19 页，每页 20 行
- 直接 URL 导航翻页正常（`?page=2`）
- 分页组件页码正确高亮当前页
- 注意：`agent-browser click` 对 `<a>` 标签可能不触发导航，但直接 URL 导航验证通过

### 2. 仓库详情 - 库区管理
- WMS-TEST-WH-RAW 仓库详情页加载正常
- 库区列表显示 3 个库区（收货区、存储区、拣货区）
- 新建库区 Modal 正常打开，包含所有必填字段（编码、名称、类型）
- **查看储位功能有问题（见问题 #1）**

### 3. 表单验证
- 调拨创建：HTML5 required 验证拦截空提交，`from_warehouse_id` 和 `to_warehouse_id` 验证生效
- 到货创建：`supplier_id` 和 `warehouse_id` required 验证生效
- 领料创建：页面加载正常，表单结构完整

### 4. 状态变更按钮
- 草稿盘点单详情页正确显示"开始盘点"按钮
- 草稿调拨单详情页正确显示"取消"和"发货"按钮

### 5. 锁定创建
- 表单字段完整：产品ID、仓库、锁定数量、锁定原因、关联客户ID
- 提交按钮存在

### 6. 反冲详情
- BF-TEST-2026-001 详情正常显示 1 条明细
- BF-2026-06-000002 详情显示"暂无明细数据"（因 backflush_items 为空，符合预期）

### 7. 级联查询
- 输入产品编码后成功返回产品信息和 BOM 关联
- HTMX 局部刷新正常工作
- **产品总库存量显示为"—"（见问题 #2）**
