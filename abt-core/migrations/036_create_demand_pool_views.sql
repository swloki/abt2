-- 036_create_demand_pool_views.sql
-- 采购需求池视图：封装 demands + products + sales_orders 的 JOIN
CREATE OR REPLACE VIEW v_purchase_demands AS
SELECT
    d.id,
    d.source_id          AS order_id,
    d.source_line_id     AS order_line_id,
    d.product_id,
    d.required_qty       AS quantity,
    d.required_date,
    d.priority,
    d.status             AS demand_status,
    d.acquire_channel,
    d.target_doc_id,
    d.target_doc_type,
    d.created_at,
    p.pdt_name           AS product_name,
    p.product_code       AS product_code,
    so.doc_number        AS order_no
FROM demands d
JOIN products p   ON p.product_id = d.product_id
JOIN sales_orders so ON so.id = d.source_id
WHERE d.acquire_channel = 2    -- Purchased
  AND d.deleted_at IS NULL;

-- 生产需求池视图：封装 demands + products + sales_orders 的 JOIN
CREATE OR REPLACE VIEW v_production_demands AS
SELECT
    d.id,
    d.source_id          AS order_id,
    d.source_line_id     AS order_line_id,
    d.product_id,
    d.required_qty       AS quantity,
    d.required_date,
    d.priority,
    d.status             AS demand_status,
    d.acquire_channel,
    d.target_doc_id,
    d.target_doc_type,
    d.created_at,
    p.pdt_name           AS product_name,
    p.product_code       AS product_code,
    so.doc_number        AS order_no
FROM demands d
JOIN products p   ON p.product_id = d.product_id
JOIN sales_orders so ON so.id = d.source_id
WHERE d.acquire_channel = 1    -- SelfProduced
  AND d.deleted_at IS NULL;

-- 注：034_demands.sql 已创建以下索引，此处不再重复：
--   idx_demands_acquire_status ON (acquire_channel, status) WHERE deleted_at IS NULL
--   idx_demands_product_status ON (product_id, status) WHERE deleted_at IS NULL