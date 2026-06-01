-- 补全 bom_nodes 中 product_code 为空但 product_id 有效的记录
-- 根因：add_bom_node 接口曾经不填充 product_code，导致节点存储了 NULL

UPDATE bom_nodes n
SET product_code = p.product_code
FROM products p
WHERE n.product_id = p.product_id
  AND (n.product_code IS NULL OR n.product_code = '')
  AND p.product_code IS NOT NULL
  AND p.product_code != '';
