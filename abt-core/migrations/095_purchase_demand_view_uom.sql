-- 095: 采购需求池视图增加物料单位（uom），供采购作业中心明细展示数量单位
-- Issue #210：采购明细「数量」列误用货币格式化，应显示物料单位（个/米/KG 等）
--
-- CREATE OR REPLACE VIEW 要求新视图前 N 列与旧视图（044）完全一致，只能在末尾追加列，
-- 故 uom 追加在 customer_name 之后（products.unit NOT NULL，LEFT JOIN 下若物料缺失则 NULL）。
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
    c.customer_name,
    p.unit AS uom
FROM demands d
LEFT JOIN sales_orders so ON so.id = d.source_id AND d.source_type IN (1, 2)
LEFT JOIN products p ON p.product_id = d.product_id
LEFT JOIN products fp ON fp.product_id = d.cascade_from_product_id
LEFT JOIN customers c ON c.customer_id = so.customer_id
WHERE d.acquire_channel = 2
  AND d.deleted_at IS NULL;
