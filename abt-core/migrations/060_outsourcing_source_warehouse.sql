-- 委外单增加"发料源仓库"字段
-- 之前 send 创建 WMS 调拨单时 from_warehouse_id 硬编码 0（无效），导致调拨创建失败、
-- send 下游(调拨/sent_qty/追踪节点)全部失败。现由用户在创建委外单时指定发料源仓库。
ALTER TABLE outsourcing_orders ADD COLUMN IF NOT EXISTS source_warehouse_id BIGINT;
