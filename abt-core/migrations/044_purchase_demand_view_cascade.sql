DROP VIEW IF EXISTS v_purchase_demands;

-- 044: 更新采购需求池视图，包含 BOM 级联来源信息
-- 参考 Odoo purchase_order_line.origin 字段追溯来源

CREATE OR REPLACE VIEW v_purchase_demands AS
SELECT
    d.id,
    d.demand_type,
    d.source_type,
    d.source_id AS order_id,
    d.source_line_id,
    d.product_id,
    d.acquire_channel,
    d.required_qty AS quantity,
    d.required_date,
    d.status AS demand_status,
    d.priority,
    d.target_doc_type,
    d.target_doc_id,
    d.cascade_from_product_id,
    d.remark,
    d.operator_id,
    d.created_at,
    so.doc_number AS order_no,
    p.pdt_name AS product_name,
    p.product_code,
    fp.pdt_name AS cascade_from_product_name,
    c.customer_name
FROM demands d
LEFT JOIN sales_orders so ON so.id = d.source_id AND d.source_type = 1
LEFT JOIN products p ON p.product_id = d.product_id
LEFT JOIN products fp ON fp.product_id = d.cascade_from_product_id
LEFT JOIN customers c ON c.customer_id = so.customer_id
WHERE d.acquire_channel = 2
  AND d.deleted_at IS NULL;
