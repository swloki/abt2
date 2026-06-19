-- 054: 回填 bom_nodes.product_code（根因治理）
-- 遗留 BOM 节点的 product_code 为空（早于该字段或经旧导入路径创建），
-- 导致 find_published_by_product_code 等"按 product_code 链接产品"的查询失效
-- → WO release 取不到 bom_snapshot_id → 自制 Picking/倒冲消耗路径断裂。
-- bom_nodes.product_id 始终非空且可靠，据此从 products 回填 product_code。
BEGIN;

UPDATE bom_nodes bn
SET product_code = p.product_code
FROM products p
WHERE bn.product_id = p.product_id
  AND (bn.product_code IS NULL OR bn.product_code = '');

COMMIT;
