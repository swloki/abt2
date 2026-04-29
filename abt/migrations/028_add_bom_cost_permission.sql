-- BOM 成本权限资源
INSERT INTO resources (resource_name, resource_code, group_name, sort_order) VALUES
('BOM_COST', 'BOM_COST', 'BOM管理', 15)
ON CONFLICT (resource_code) DO NOTHING;

-- BOM_COST:READ 权限
INSERT INTO permissions (permission_name, resource_id, action_code, sort_order)
SELECT 'BOM_COST-read', r.resource_id, 'READ', (r.sort_order * 10 + 1)
FROM resources r
WHERE r.resource_code = 'BOM_COST'
ON CONFLICT (resource_id, action_code) DO NOTHING;
