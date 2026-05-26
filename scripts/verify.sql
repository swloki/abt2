-- 1. Users 数据一致性
SELECT '1. users' AS check_name,
  (SELECT COUNT(*) FROM abt2.public.users) AS abt2_count,
  (SELECT COUNT(*) FROM abt.public.users) AS abt_count,
  (SELECT COUNT(*) FROM abt2.public.users a2
   JOIN abt.public.users a ON a2.user_id = a.user_id
     AND a2.username = a.username
     AND a2.password_hash = a.password_hash) AS matched_rows;

-- 2. Products 数量 + 名称匹配
SELECT '2. products' AS check_name,
  (SELECT COUNT(*) FROM abt2.public.products) AS abt2_count,
  (SELECT COUNT(*) FROM abt.public.products) AS abt_count,
  (SELECT COUNT(*) FROM abt2.public.products a2
   JOIN abt.public.products a ON a2.product_id = a.product_id
     AND a2.pdt_name = a.pdt_name) AS name_matched;

-- 3. Products product_code/unit 提取正确性
SELECT '3. product_code_mismatch' AS check_name, COUNT(*) AS mismatch_count
FROM abt.public.products p
JOIN abt2.public.products p2 ON p.product_id = p2.product_id
WHERE p.product_code IS DISTINCT FROM (p2.meta->>'product_code')
   OR p.unit IS DISTINCT FROM COALESCE(p2.meta->>'unit', 'pcs');

-- 4. BOM 基本数据
SELECT '4. bom' AS check_name,
  (SELECT COUNT(*) FROM abt2.public.bom) AS abt2_count,
  (SELECT COUNT(*) FROM abt.public.bom) AS abt_count,
  (SELECT COUNT(*) FROM abt2.public.bom a2
   JOIN abt.public.bom a ON a2.bom_id = a.bom_id
     AND a2.bom_name = a.bom_name) AS name_matched;

-- 5. bom_nodes 总数 vs bom_detail nodes 数
SELECT '5. bom_nodes_vs_json' AS check_name,
  (SELECT SUM(jsonb_array_length(COALESCE(bom_detail->'nodes', '[]'::jsonb)))
   FROM abt2.public.bom WHERE bom_detail IS NOT NULL) AS nodes_in_json,
  (SELECT COUNT(*) FROM abt.public.bom_nodes) AS nodes_in_table;

-- 6. bom_nodes 每个BOM的节点数对比
SELECT '6. bom_node_count_mismatch' AS check_name, COUNT(*) AS mismatch_count
FROM (
  SELECT bom_id, jsonb_array_length(COALESCE(bom_detail->'nodes', '[]'::jsonb)) AS cnt
  FROM abt2.public.bom WHERE bom_detail IS NOT NULL
) a2
JOIN (
  SELECT bom_id, COUNT(*) AS cnt FROM abt.public.bom_nodes GROUP BY bom_id
) a ON a2.bom_id = a.bom_id
WHERE a2.cnt != a.cnt;

-- 7. bom_nodes parent_id 孤儿检查（parent_id 引用不存在的节点）
SELECT '7. orphan_parent_nodes' AS check_name, COUNT(*) AS orphan_count
FROM abt.public.bom_nodes n
WHERE n.parent_id IS NOT NULL
  AND NOT EXISTS (SELECT 1 FROM abt.public.bom_nodes p WHERE p.id = n.parent_id);

-- 8. inventory 数据
SELECT '8. inventory' AS check_name,
  (SELECT COUNT(*) FROM abt2.public.inventory) AS abt2_count,
  (SELECT COUNT(*) FROM abt.public.inventory) AS abt_count,
  (SELECT COUNT(*) FROM abt2.public.inventory a2
   JOIN abt.public.inventory a ON a2.inventory_id = a.inventory_id
     AND a2.product_id = a.product_id
     AND a2.quantity = a.quantity) AS matched;

-- 9. term_relation
SELECT '9. term_relation' AS check_name,
  (SELECT COUNT(*) FROM abt2.public.term_relation) AS abt2_count,
  (SELECT COUNT(*) FROM abt.public.term_relation) AS abt_count;

-- 10. role_permissions
SELECT '10. role_permissions' AS check_name,
  (SELECT COUNT(*) FROM abt2.public.role_permissions) AS abt2_count,
  (SELECT COUNT(*) FROM abt.public.role_permissions) AS abt_count;

-- 11. product_price_log_archived vs product_price_log
SELECT '11. price_log_archived' AS check_name,
  (SELECT COUNT(*) FROM abt2.public.product_price_log) AS abt2_count,
  (SELECT COUNT(*) FROM abt.public.product_price_log_archived) AS abt_count;

-- 12. product_price (should = number of distinct product_id in price_log)
SELECT '12. product_price' AS check_name,
  (SELECT COUNT(DISTINCT product_id) FROM abt2.public.product_price_log) AS distinct_products,
  (SELECT COUNT(*) FROM abt.public.product_price) AS price_rows;

-- 13. 抽样验证一个 BOM 的 bom_nodes 树结构是否与原始 JSON 一致
SELECT '13. sample_bom_1000157' AS check_name,
  (SELECT jsonb_array_length(bom_detail->'nodes') FROM abt2.public.bom WHERE bom_id = 1000157) AS json_nodes,
  (SELECT COUNT(*) FROM abt.public.bom_nodes WHERE bom_id = 1000157) AS table_nodes;

-- 14. 抽样验证 bom_nodes 的 quantity 和 product_id 匹配
SELECT '14. bom_node_data_mismatch' AS check_name, COUNT(*) AS mismatch
FROM abt.public.bom_nodes bn
JOIN abt2.public.bom b ON bn.bom_id = b.bom_id AND b.bom_detail IS NOT NULL
CROSS JOIN LATERAL jsonb_array_elements(b.bom_detail->'nodes') AS j(node)
WHERE (j.node->>'product_id')::bigint = bn.product_id
  AND (j.node->>'quantity')::numeric IS DISTINCT FROM bn.quantity;

-- 15. location 数据
SELECT '15. location' AS check_name,
  (SELECT COUNT(*) FROM abt2.public.location) AS abt2_count,
  (SELECT COUNT(*) FROM abt.public.location) AS abt_count;
