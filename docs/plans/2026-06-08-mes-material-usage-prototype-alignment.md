# 物料消耗追踪页面原型对齐测试报告

**测试日期**: 2026-06-08
**测试范围**: MES 物料消耗追踪页面 (`/admin/mes/material-usage`) 与 Open Design 原型 (`04-material-usage.html`) 的对齐
**测试工具**: agent-browser snapshot + cargo clippy

## 修改文件

| 文件 | 变更 |
|------|------|
| `abt-core/src/mes/dashboard/model.rs` | `WoBasicInfo` 新增 `bom_version: Option<String>`；`BomCompareItem` 新增 `picked_qty: Decimal` |
| `abt-core/src/mes/dashboard/repo.rs` | `get_wo_basic_info` SQL 增加 `LEFT JOIN boms` 取 `bom_name`；`get_bom_comparison` SQL 增加领料量子查询 |
| `abt-web/src/pages/mes_material_usage.rs` | 完整重写页面模板 |

## 测试总览

| 测试项 | 状态 | 说明 |
|--------|------|------|
| 页面加载 | ✅ | HTTP 200，无 JS 错误 |
| 页面标题 | ✅ | "物料消耗追踪" |
| 导出按钮 | ✅ | 已添加，使用 btn-default + download_icon |
| 工单下拉格式 | ✅ | `WO号 · 产品名 (数量)` 格式 |
| 工单下拉 class | ✅ | `filter-select` 替代 `form-select` |
| 批次下拉 | ✅ | 已添加（disabled，"全部批次"） |
| 工单信息头 | ✅ | 水平布局：工单号+产品名+状态+计划/完成/BOM |
| BOM标准用量卡片 | ✅ | 蓝色图标 + 副标题"按完成X件计算" |
| 实际消耗卡片 | ✅ | 绿色图标 + 副标题"含损耗余量" |
| 倒冲消耗卡片 | ✅ | 橙色图标 |
| 用量差异卡片 | ✅ | 红色图标 + 百分比副标题 |
| BOM对比表9列 | ✅ | 物料编码/名称/单位/单件用量/标准总量/领料数量/倒冲消耗/损耗率/差异 |
| 领料数量列 | ✅ | 从 material_requisition_items 聚合 issued_qty |
| 损耗率列 | ✅ | (领料-标准)/标准*100% |
| 差异指示器 | ✅ | diff-positive/diff-negative/diff-zero 三色标签 |
| 倒冲明细记录表 | ✅ | 倒冲单号/完成数量/倒冲日期/状态 |
| 领料记录表 | ✅ | 领料单号/领料日期/状态（新增） |
| 编译检查 | ✅ | cargo clippy 通过，无新增警告 |

## 与原型差异（已知限制）

| 差异 | 原因 | 优先级 |
|------|------|--------|
| 工单信息头缺少 BOM 版本号 | 数据库中 bom_snapshot_id 关联的 bom_name 可能为空 | P2 |
| 倒冲明细表列数少于原型（原型7列 vs 实现4列） | 原型的"触发单据"、"批次"等列需要关联查询，BackflushRecord 不存储 batch_id | P2 |
| 领料记录表列数少于原型（原型7列 vs 实现3列） | 缺少"仓库"、"物料数"、"领料人"等关联数据 | P2 |
| 批次下拉不实现筛选功能 | 需要额外的批次查询和联动 HTMX | P3 |
| 导出按钮无实际功能 | 需要实现 CSV/Excel 导出逻辑 | P3 |

## 修复后 SQL 错误修复记录

1. **表名错误**: `material_req_items` → `material_requisition_items`（正确的数据库表名）
2. **GROUP BY 缺失**: 子查询引用 `wo.id` 需加入 GROUP BY 子句
