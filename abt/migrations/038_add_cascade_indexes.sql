-- 级联查询库存所需索引
CREATE INDEX IF NOT EXISTS idx_bom_nodes_product_bom_parent
  ON bom_nodes(product_id, bom_id, parent_id);

CREATE INDEX IF NOT EXISTS idx_bom_nodes_parent_bom_order
  ON bom_nodes(parent_id, bom_id, "order");

CREATE INDEX IF NOT EXISTS idx_inventory_product
  ON inventory(product_id);
