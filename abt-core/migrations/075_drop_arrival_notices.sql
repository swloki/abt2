-- 取消来料通知：DROP arrival_notices/arrival_notice_items 表（来料通知已取消，历史台账 source_type=16 无业务）
DROP INDEX IF EXISTS idx_ani_product;
DROP INDEX IF EXISTS idx_ani_order_item;
DROP INDEX IF EXISTS idx_ani_notice;
DROP INDEX IF EXISTS idx_arrival_status;
DROP INDEX IF EXISTS idx_arrival_supplier;
DROP INDEX IF EXISTS idx_arrival_warehouse;
DROP INDEX IF EXISTS idx_arrival_zone;
DROP TABLE IF EXISTS arrival_notice_items;
DROP TABLE IF EXISTS arrival_notices;
