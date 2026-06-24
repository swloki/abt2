-- 072: 委外台账纳入 DB 幂等（修正 070 排除 OutsourcingOrder=11 的设计）
--
-- 070 故意排除委外(11)以支持「分次收货，按 transaction_date 多行立账」。但实际委外均为一次性收货，
-- 且 receive 内 dup_ledger(transaction_date=today) 防重存在「同日多次部分收货少立」缺陷，
-- 历史已出现 OO-10/OO-14 收货入库但漏立应付台账的断链。
-- 改为「一单一账」（unique 全覆盖），receive 改用 ArApService::post_entry（ON CONFLICT DO NOTHING 幂等）。
-- 前置条件：已确认 ar_ap_ledger 无 source_type=11 重复行（OO-9/11 各一条，source_id 不同）。

DROP INDEX IF EXISTS ar_ap_ledger_source_uniq;
CREATE UNIQUE INDEX ar_ap_ledger_source_uniq ON ar_ap_ledger (source_type, source_id);
