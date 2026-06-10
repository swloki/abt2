# WMS 测试问题清单 — Session 5（转换/盘点/锁定/反冲/事务/策略/级联）

测试时间：2026-06-09
测试层级：Full

## 测试页面汇总

| 页面 | URL | 状态 |
|------|-----|------|
| 形态转换列表 | /admin/wms/conversions | 加载正常，数据正常，筛选正常 |
| 形态转换创建 | /admin/wms/conversions/create | 加载正常，表单完整 |
| 形态转换详情 | /admin/wms/conversions/{id} | 加载正常，数据完整 |
| 循环盘点列表 | /admin/wms/cycle-counts | 加载正常，数据正常 |
| 循环盘点创建 | /admin/wms/cycle-counts/create | 加载正常，表单完整 |
| 循环盘点详情 | /admin/wms/cycle-counts/{id} | 加载正常，但明细为占位文本 |
| 库存锁定列表 | /admin/wms/locks | 加载正常，数据正常 |
| 库存锁定创建 | /admin/wms/locks/create | 加载正常，表单完整 |
| 库存锁定详情 | /admin/wms/locks/{id} | 加载正常，信息完整 |
| 反冲列表 | /admin/wms/backflushes | 加载正常，数据正常 |
| 反冲详情 | /admin/wms/backflushes/{id} | 加载正常，数据完整 |
| 事务日志 | /admin/wms/transactions | 加载正常，数据正常，筛选正常 |
| 策略管理 | /admin/wms/strategies | 加载正常，暂无数据 |
| 级联查询 | /admin/wms/cascade | 加载正常，查询功能正常 |

## 问题清单

| # | 页面 | 测试项 | 问题描述 | 涉及文件 | 优先级 | 状态 |
|---|------|--------|---------|----------|--------|------|
| 1 | 形态转换列表 | 消耗项/产出项列 | 消耗项和产出项列硬编码显示 "—"，但详情页有真实数据（产品编码、名称、数量） | `abt-web/src/pages/wms_conversion_list.rs` L214-215 | P1 | 待修复 |
| 2 | 形态转换列表 | 操作员列 | 操作员列硬编码显示 "—"，未关联实际操作员信息 | `abt-web/src/pages/wms_conversion_list.rs` L216 | P2 | 待修复 |
| 3 | 形态转换列表 | 转换仓库列 | 转换仓库列硬编码显示 "—"，未关联实际仓库信息 | `abt-web/src/pages/wms_conversion_list.rs` L209 | P2 | 待修复 |
| 4 | 循环盘点列表 | 物料项数列 | "物料项数"列全部显示 "—"，未聚合显示实际盘点物料数量 | `abt-web/src/pages/wms_cycle_count_list.rs` | P2 | 待修复 |
| 5 | 循环盘点列表 | 操作员列 | 操作员列全部显示 "—"，未关联实际操作员信息 | `abt-web/src/pages/wms_cycle_count_list.rs` | P2 | 待修复 |
| 6 | 循环盘点详情 | 盘点明细 | 盘点明细表格硬编码占位文本"盘点明细将通过后续版本加载"，未加载真实明细数据 | `abt-web/src/pages/wms_cycle_count_detail.rs` L216-222 | P1 | 待修复 |
| 7 | 库存锁定列表 | 锁定原因 | LCK-2026-06-000002 的锁定原因显示乱码 `客户订单预留`（数据库编码问题） | 数据库数据问题 | P2 | 待确认 |
| 8 | 反冲列表 | 关联工单/完工产品 | "关联工单"和"完工产品"列全部显示 "—"，未关联实际工单和产品信息 | `abt-web/src/pages/wms_backflush_list.rs` | P2 | 待修复 |
| 9 | 反冲列表 | 操作员列 | 操作员列全部显示 "—"，未关联实际操作员信息 | `abt-web/src/pages/wms_backflush_list.rs` | P2 | 待修复 |
| 10 | 事务日志 | 来源类型翻译 | 来源类型列直接显示英文值（`lock`、`manual`、`transfer`、`inventory_transfer`），未翻译为中文 | `abt-web/src/pages/wms_transaction_log_list.rs` L283 | P2 | 待修复 |
| 11 | 事务日志 | 操作员显示 | 操作员显示为"操作员#1"、"操作员#6"，仅显示 ID 而非用户名 | `abt-web/src/pages/wms_transaction_log_list.rs` L291 | P2 | 待修复 |
| 12 | 事务日志 | 数量格式 | 数量显示 6 位小数（如 `+10.000000`），过长，应格式化为 2 位 | `abt-web/src/pages/wms_transaction_log_list.rs` L275-279 | P2 | 待修复 |
| 13 | 事务日志 | 销售出库数量符号 | "销售出库"事务数量显示为正数（绿色 `+5.000000`），出库数量应为负数（红色） | 数据层面：事务记录的 quantity 应为负数 | P2 | 待确认 |
| 14 | 级联查询 | 库存总量 | 查询产品 3010134033 后"当前库存总量"显示 "—"，未聚合实际库存数据 | `abt-web/src/pages/wms_cascade_list.rs` 或 `abt-core` 服务层 | P2 | 待修复 |

## 无问题页面

- 形态转换创建：表单字段完整（仓库选择、日期、消耗物料表、产出物料表、提交按钮）
- 循环盘点创建：表单字段完整（仓库、库区、日期、盲盘选项、物料表、备注）
- 库存锁定创建：表单字段完整（产品ID、仓库、数量、原因、备注）
- 库存锁定详情：信息展示完整（单号、产品、仓库、数量、原因、客户、操作员、时间）
- 反冲详情：数据完整（子件信息、BOM理论用量、实际倒冲量、差异量/率）
- 策略管理：页面正常，上架策略和拣货策略两个表格，暂无数据
- 筛选功能：形态转换、循环盘点、事务日志的状态/类型筛选均正常工作
- 无 JS 错误
