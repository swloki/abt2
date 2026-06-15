-- 043: 为 demands 表添加 BOM 级联来源字段
-- 参考 Odoo Procurement.values['origin'] 的来源追踪机制
ALTER TABLE demands
    ADD COLUMN IF NOT EXISTS cascade_from_product_id BIGINT REFERENCES products(product_id);

-- 级联需求去重查询索引（Odoo _make_mo_get_domain 等价）
CREATE INDEX IF NOT EXISTS idx_demands_cascade
    ON demands (source_id, source_line_id, cascade_from_product_id, product_id)
    WHERE deleted_at IS NULL AND demand_type = 2;

COMMENT ON COLUMN demands.cascade_from_product_id IS
    'BOM展开来源产品ID。demand_type=2时记录此原材料属于哪个成品的BOM。NULL表示直接需求(demand_type=1)';
