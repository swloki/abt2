---
date: 2026-04-25
topic: bom-product-substitution
---

# BOM 物料替换

## Problem Frame

当物料停产、缺货或需要升级时，用户需要将 BOM 中的某个物料替换为另一个物料。目前只能逐个手动编辑 BOM 节点，效率低且容易遗漏。需要一个专门的物料替换功能，支持单 BOM 和批量替换。

---

## Requirements

**替换执行**

- R1. 支持将 BOM 中指定的旧物料（product_id）替换为新物料（product_id），同时更新 product_code
- R2. 替换时可选择性覆盖节点属性：数量（quantity）、损耗率（loss_rate）、单位（unit）、备注（remark）、位置（position）、工作中心（work_center）、物料属性（properties）
- R3. 当同一物料在同一 BOM 中出现多次时，替换所有出现位置的节点

**替换范围**

- R4. 支持指定单个 BOM 进行替换
- R5. 支持不指定 BOM（即替换所有使用了该物料的 BOM）
- R6. 返回替换结果摘要：受影响的 BOM 数量、替换的节点数量

---

## Success Criteria

- 用户可以将任意物料在指定 BOM 或所有 BOM 中替换为新物料，并可调整属性
- 替换执行后返回清晰的统计信息（多少 BOM 受影响、多少节点被替换）
- 不影响 BOM 中未匹配的节点

---

## Scope Boundaries

- 不记录替换历史（无审计日志）
- 不需要预览/确认步骤
- 不涉及替代料管理（预定义可替代物料列表）
- 不涉及库存联动
- 不涉及 BOM 版本管理

---

## Key Decisions

- **替换即生效，无预览无历史**：用户直接执行替换，系统立即修改并返回结果摘要。不做 dry-run 或历史追溯。
- **属性覆盖是可选的**：不传入的属性保持原值，传入的属性覆盖原值。这允许用户只改物料不改数量，也可以同时调整。

---

## Outstanding Questions

### Deferred to Planning

- [Affects R1][Technical] `find_boms_using_product` 返回的结果结构是否满足批量替换的查询需求，还是需要新的查询方法
- [Affects R2][Technical] 属性覆盖在 proto 层面的表达方式（是每个属性一个 optional 字段，还是用一个统一的更新 mask）

---

## Next Steps

-> `/ce-plan` 进行实现规划
