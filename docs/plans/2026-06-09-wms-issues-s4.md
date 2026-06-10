# WMS 测试问题清单 — Session 4（调拨/到货/领料）

测试时间：2026-06-09
测试层级：Full
测试工具：agent-browser --session s4

## 测试页面

| 页面 | URL | 状态 |
|------|-----|------|
| 调拨列表 | /admin/wms/transfers | 通过（有显示问题） |
| 调拨创建 | /admin/wms/transfers/create | 通过（有显示问题） |
| 调拨详情 | /admin/wms/transfers/{id} | 通过（有显示问题） |
| 到货列表 | /admin/wms/arrivals | 通过（搜索不工作） |
| 到货创建 | /admin/wms/arrivals/create | 通过（日期无默认值） |
| 到货详情 | /admin/wms/arrivals/{id} | 通过 |
| 领料列表 | /admin/wms/requisitions | 通过（搜索不工作） |
| 领料创建 | /admin/wms/requisitions/create | 通过（日期无默认值） |
| 领料详情 | /admin/wms/requisitions/{id} | 通过 |

## 问题清单

| # | 页面 | 测试项 | 问题描述 | 涉及文件 | 优先级 | 状态 |
|---|------|--------|---------|----------|--------|------|
| 1 | 调拨列表 | 仓库显示 | 调出仓库、调入仓库列全部硬编码为"—"，未查询实际仓库名称 | wms_transfer_list.rs:264-266 | P2 | 🔲 |
| 2 | 调拨列表 | 物料项数 | 物料项数列硬编码为"—"，未统计实际行项目数量 | wms_transfer_list.rs:271 | P2 | 🔲 |
| 3 | 调拨列表 | 操作员 | 操作员列硬编码为"—"，未查询实际操作员名称 | wms_transfer_list.rs:272 | P2 | 🔲 |
| 4 | 调拨列表 | 搜索功能 | 搜索框存在但搜索不起作用：`TransferFilter` 缺少 `doc_number` 字段，前端传了参数但后端未使用 | wms_transfer_list.rs:150-156, abt-core transfer/model.rs:56-60 | P1 | 🔲 |
| 5 | 调拨详情 | 规格列 | 行项目规格列显示"—"而非实际规格值 | wms_transfer_detail.rs | P2 | 🔲 |
| 6 | 到货列表 | 搜索功能 | 单据编号搜索框存在但搜索不起作用：`ArrivalNoticeFilter` 缺少 `doc_number` 字段 | wms_arrival_list.rs:43-49 | P1 | 🔲 |
| 7 | 到货创建 | 日期默认值 | 到货日期输入框默认值为空（年:0 月:0 日:0），应默认当天日期 | wms_arrival_create.rs:264 | P2 | 🔲 |
| 8 | 到货详情 | 来源采购单 | 来源采购单字段显示 ID 数字而非采购单号（如"—"或数字 ID） | wms_arrival_detail.rs:265-266 | P2 | 🔲 |
| 9 | 到货详情 | 库区显示 | 到货库区字段显示 zone_id 数字（如 23332022）而非库区名称 | wms_arrival_detail.rs:279-281 | P2 | 🔲 |
| 10 | 领料列表 | 搜索功能 | 单据编号搜索框存在但搜索不起作用：`RequisitionFilter` 缺少 `doc_number` 字段 | wms_requisition_list.rs:41-46 | P1 | 🔲 |
| 11 | 领料创建 | 日期默认值 | 领料日期输入框默认值为空（年:0 月:0 日:0），应默认当天日期 | wms_requisition_create.rs:213 | P2 | 🔲 |
| 12 | 领料列表 | 操作员 | WMS-TEST-MR-* 单据的操作员显示"—"，MR-2026-06-* 显示"Admin"，测试数据的 operator_id 可能不同 | wms_requisition_list.rs | P2 | 🔲 |

## 按优先级汇总

### P1（核心功能不可用）— 3 项
- #4 调拨列表搜索不工作
- #6 到货列表搜索不工作
- #10 领料列表搜索不工作

**根因**：三个列表的 Filter struct 均缺少 `doc_number` 字段，需要同时修改 abt-core 的 Filter model、repo SQL、以及 abt-web 的 build_filter 函数。

### P2（显示问题）— 9 项
- #1-3 调拨列表硬编码"—"（仓库/物料项数/操作员）
- #5 调拨详情规格列空
- #7 到货创建日期无默认值
- #8 到货详情来源采购单显示 ID
- #9 到货详情库区显示 ID
- #11 领料创建日期无默认值
- #12 领料列表部分操作员为"—"

### 通过的测试项
- 所有 9 个页面加载无 500 错误
- 调拨列表表头正确、数据行完整、状态标签正确
- 调拨创建表单字段完整、提交按钮存在
- 调拨详情基本信息正确显示（仓库/日期/状态）、行项目数据正确
- 到货列表表头正确、数据行完整、状态标签正确、筛选标签完整
- 到货创建表单字段完整、提交按钮存在
- 到货详情基本信息正确、行项目数据正确、状态标签正确、工作流步骤条正确
- 领料列表表头正确、数据行完整、状态标签正确
- 领料创建表单字段完整、提交按钮存在
- 领料详情基本信息正确、行项目数据正确、操作按钮（取消/确认发料）正确
- 三个列表的状态 Tab 筛选均正常工作
- 所有页面无 JS 错误
