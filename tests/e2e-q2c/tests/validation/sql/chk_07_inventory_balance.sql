-- CHK-07: 库存余额正确（测试仓库范围）
-- 验证: stock_ledger.quantity = SUM(inventory_transactions.quantity) 按 product_id, warehouse_id
-- 限定: 只检查测试仓库 (WH-RAW, WH-FG)，排除历史导入数据干扰
-- 返回 0 行 = PASS
SELECT sl.product_id, sl.warehouse_id,
       sl.quantity AS ledger_qty,
       txn.txn_sum,
       sl.quantity - txn.txn_sum AS diff
FROM stock_ledger sl
INNER JOIN (
    SELECT product_id, warehouse_id, SUM(quantity) AS txn_sum
    FROM inventory_transactions
    WHERE transaction_type IN (1, 2, 3, 4, 5, 6, 12)
    GROUP BY product_id, warehouse_id
) txn ON txn.product_id = sl.product_id AND txn.warehouse_id = sl.warehouse_id
WHERE sl.warehouse_id IN (
    SELECT id FROM warehouses WHERE code IN ('WH-RAW', 'WH-FG') AND deleted_at IS NULL
)
AND ABS(sl.quantity - txn.txn_sum) > 0.001;
-- 预期: 0 行返回（测试仓库库存台账与事务汇总一致）
