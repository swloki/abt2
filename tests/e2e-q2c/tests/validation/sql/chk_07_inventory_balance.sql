-- CHK-07: 库存余额正确
-- 验证: stock_ledger 中无负库存
SELECT sl.product_id, p.product_code, sl.warehouse_id, w.code AS wh_code,
       sl.quantity AS current_qty
FROM stock_ledger sl
JOIN products p ON sl.product_id = p.product_id
JOIN warehouses w ON sl.warehouse_id = w.id
WHERE sl.deleted_at IS NULL
  AND sl.quantity < 0;
-- 预期: 0 行返回（无负库存）
