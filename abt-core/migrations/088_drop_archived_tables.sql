-- 088: DROP 归档表（stock_picking 统一收尾，#146 阶段 6b）
-- 所有数据已迁入 stock_pickings：
--   082 material_requisitions → picking(InternalIssue)
--   083 inventory_transfers   → picking(InternalTransfer)
--   084 shipping_requests     → picking(OutgoingSales)
--   086 production_receipts   → picking(IncomingWorkOrder)
-- 代码零 SQL 引用（仅注释 + DocumentType 枚举值保留——历史 source_type 解码需要）
-- 顺序：先 items 子表（外键→头），后头表；CASCADE 兜底未知外键
-- pick_lists（4a 拣货退场，表归档）、stock_ins/outbound（5a/4b 已移除，IF EXISTS 兜底）

-- 子表先（外键引用头表）
DROP TABLE IF EXISTS material_requisition_items CASCADE;
DROP TABLE IF EXISTS shipping_request_items CASCADE;
DROP TABLE IF EXISTS pick_list_items CASCADE;
DROP TABLE IF EXISTS transfer_items CASCADE;

-- 头表
DROP TABLE IF EXISTS material_requisitions CASCADE;
DROP TABLE IF EXISTS pick_lists CASCADE;
DROP TABLE IF EXISTS shipping_requests CASCADE;
DROP TABLE IF EXISTS production_receipts CASCADE;
DROP TABLE IF EXISTS inventory_transfers CASCADE;

-- 早期已移除表的兜底（085 stock_in / 4b outbound，若残留）
DROP TABLE IF EXISTS stock_ins CASCADE;
DROP TABLE IF EXISTS outbound CASCADE;

-- 校验：归档表应全部不存在
-- SELECT relname FROM pg_stat_user_tables WHERE relname IN
--   ('shipping_requests','shipping_request_items','pick_lists','pick_list_items',
--    'production_receipts','material_requisitions','material_requisition_items','inventory_transfers');
