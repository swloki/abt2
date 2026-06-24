-- 070: ar_ap_ledger 幂等根治——partial unique index（Issue #89）
--
-- 业财一体下，业务单据直接立 ar_ap 台账（不经发票）。原各 handler 用「SELECT 防重 + INSERT」
-- 非原子，并发/重放可能重复立账（fms-ar-ap.md 留口③）。本 migration 加 partial unique index，
-- 配合 ArApLedgerRepo::insert 的 ON CONFLICT DO NOTHING 实现原子幂等。
--
-- 排除 OutsourcingOrder(11)：委外允许分次收货，同一委外单按 transaction_date 多行立账
-- （见 outsourcing_order/implt.rs 的 SELECT ... AND transaction_date 防重）。其余 source
-- （采购入库/采购退货/销售退货/销售发货/收付款/调整）一单一行。

-- 1. 清理历史竞态残留（同 source_type+source_id 保留最小 id；委外分次多行不计）
DELETE FROM ar_ap_ledger a
USING ar_ap_ledger b
WHERE a.source_type = b.source_type
  AND a.source_id = b.source_id
  AND a.id > b.id
  AND a.source_type <> 11;

-- 2. partial unique index（排除委外 OutsourcingOrder=11）
CREATE UNIQUE INDEX IF NOT EXISTS ar_ap_ledger_source_uniq
    ON ar_ap_ledger (source_type, source_id)
    WHERE source_type <> 11;
