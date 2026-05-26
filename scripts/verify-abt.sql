-- abt verification queries
SELECT 'users' AS tbl, COUNT(*) FROM users;
SELECT 'products' AS tbl, COUNT(*) FROM products;
SELECT 'bom' AS tbl, COUNT(*) FROM bom;
SELECT 'bom_nodes' AS tbl, COUNT(*) FROM bom_nodes;
SELECT 'bom_sample_1000157_nodes' AS tbl, COUNT(*) FROM bom_nodes WHERE bom_id = 1000157;
SELECT 'inventory' AS tbl, COUNT(*) FROM inventory;
SELECT 'inventory_log' AS tbl, COUNT(*) FROM inventory_log;
SELECT 'term_relation' AS tbl, COUNT(*) FROM term_relation;
SELECT 'role_permissions' AS tbl, COUNT(*) FROM role_permissions;
SELECT 'product_price_log_archived' AS tbl, COUNT(*) FROM product_price_log_archived;
SELECT 'product_price' AS tbl, COUNT(*) FROM product_price;
SELECT 'location' AS tbl, COUNT(*) FROM location;
SELECT 'permission_audit_logs' AS tbl, COUNT(*) FROM permission_audit_logs;
SELECT 'bom_labor_process' AS tbl, COUNT(*) FROM bom_labor_process;
SELECT 'bom_routing' AS tbl, COUNT(*) FROM bom_routing;
SELECT 'roles' AS tbl, COUNT(*) FROM roles;
SELECT 'departments' AS tbl, COUNT(*) FROM departments;
SELECT 'warehouse' AS tbl, COUNT(*) FROM warehouse;
SELECT 'terms' AS tbl, COUNT(*) FROM terms;
SELECT 'user_roles' AS tbl, COUNT(*) FROM user_roles;
SELECT 'user_departments' AS tbl, COUNT(*) FROM user_departments;
SELECT 'bom_category' AS tbl, COUNT(*) FROM bom_category;
SELECT 'labor_process_dict' AS tbl, COUNT(*) FROM labor_process_dict;
SELECT 'routing' AS tbl, COUNT(*) FROM routing;
SELECT 'routing_step' AS tbl, COUNT(*) FROM routing_step;
SELECT 'product_watchers' AS tbl, COUNT(*) FROM product_watchers;
SELECT 'notifications' AS tbl, COUNT(*) FROM notifications;
-- bom_nodes parent_id integrity
SELECT 'orphan_parent_nodes' AS tbl, COUNT(*) AS count FROM bom_nodes n WHERE n.parent_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM bom_nodes p WHERE p.id = n.parent_id);
-- bom_nodes per-bom count vs source json
SELECT 'bom_node_count_mismatch' AS tbl, COUNT(*) AS count FROM (
  SELECT b.bom_id, jsonb_array_length(COALESCE(b.bom_detail->'nodes', '[]'::jsonb)) AS json_cnt
  FROM bom WHERE bom_detail IS NOT NULL
) j
JOIN (SELECT bom_id, COUNT(*) AS tbl_cnt FROM bom_nodes GROUP BY bom_id) t ON j.bom_id = t.bom_id
WHERE j.json_cnt != t.tbl_cnt;
-- products product_code/unit vs source meta
SELECT 'product_code_mismatch' AS tbl, COUNT(*) AS count FROM products WHERE product_code = '' OR product_code IS NULL;
-- product_price: verify each row has a valid product_id
SELECT 'orphan_product_price' AS tbl, COUNT(*) AS count FROM product_price pp WHERE NOT EXISTS (SELECT 1 FROM products p WHERE p.product_id = pp.product_id);
