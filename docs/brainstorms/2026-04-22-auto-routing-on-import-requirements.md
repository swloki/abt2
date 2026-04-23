---
date: 2026-04-22
topic: auto-routing-on-import
status: approved
---

# 需求：Excel 导入时自动匹配/创建工艺路线

## 背景

工艺路线设计（`docs/superpowers/specs/2026-04-22-labor-process-routing-design.md`）引入了 `routing`、`routing_step`、`bom_routing` 等表来校验产品工序完整性。但存在冷启动问题：产品首次导入时没有路线，无法校验；用户需要手动为每个产品创建路线，工作量大。

## 需求

在 Excel 导入 `bom_labor_process` 时，如果产品未绑定工艺路线，系统自动根据导入的工序数据匹配或创建路线，并绑定到产品上。

### 核心流程

1. 用户上传 Excel 导入工序数据（含 `process_code` 列）
2. 系统校验 Excel 中所有 `process_code` 是否存在于 `labor_process_dict`
   - 若存在未知编码 → 返回错误，列出所有未知工序编码
3. 检查产品是否已绑定路线（`bom_routing` 中有记录）
   - **已绑定** → 按现有逻辑校验导入完整性
   - **未绑定** → 进入自动匹配/创建流程（步骤 4-6）
4. 从 Excel 提取所有不重复的 `process_code` 集合
5. 在 `routing` + `routing_step` 中查找**完全匹配**的路线（路线的工序集合与 Excel 工序集合完全一致）
   - **找到匹配** → 复用该路线，在 `bom_routing` 中创建绑定
   - **未找到匹配** → 新建路线 + 路线工序明细，然后在 `bom_routing` 中创建绑定
6. 继续正常导入 `bom_labor_process` 记录

### 导入响应增强

在导入结果中返回路线操作信息（透明化）：

- 是否自动创建了新路线（`auto_created_routing`）
- 是否匹配到了已有路线（`matched_existing_routing`）
- 路线名称（`routing_name`）
- 路线 ID（`routing_id`）

前端可据此向用户展示"已自动创建路线 XXX"或"已匹配到已有路线 XXX"。

### 路线命名规则

自动创建的路线命名格式：`Auto-{product_code}-{YYYYMMDD}`

示例：`Auto-P001-20260422`

### 匹配逻辑

"完全匹配"定义：路线中所有 `routing_step.process_code` 的集合与 Excel 中所有不重复 `process_code` 的集合**完全相同**（不考虑顺序，不考虑 `is_required` 标志）。

## 范围

### 包含

- 导入流程中自动匹配/创建路线的逻辑
- 导入响应 proto 增加 auto-routing 信息字段
- 自动创建的路线中，所有步骤默认 `is_required = true`，`step_order` 按 Excel 中出现顺序设置

### 不包含

- 路线审核/审批流程（自动创建的路线即为正式路线）
- 手动创建路线的 UI（属于路线 CRUD 的范畴）
- 路线模板继承（独立需求）
- 路线变更迁移工具（独立需求）

## 向后兼容

- 产品已绑定路线时，行为不变（走现有校验逻辑）
- 产品未绑定路线时，自动匹配/创建路线，导入照常进行
- 导入响应中的 auto-routing 字段为可选，不影响现有客户端
