-- 097: DROP routing_steps.product_id / unit_price 列（clean break 收尾）
-- 设计文档: docs/uml-design/routing-decouple.md
-- 前置：096 已建 bom_routing_outputs 覆盖层并回填产出/计件价；所有代码路径（load_routings_from_template、
--   try_build_labor_from_routing、前端 routing_create）已改读覆盖层，不再引用这两列。
-- 这两列是 045/063 迁移误焊进工艺模板的（产出品/计件价应属 per-BOM 覆盖层），解耦后唯一源在 bom_routing_outputs。

ALTER TABLE routing_steps DROP COLUMN IF EXISTS product_id;
ALTER TABLE routing_steps DROP COLUMN IF EXISTS unit_price;
