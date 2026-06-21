-- Issue#67：工序产出品 product_id + 物料最小包装量 + 委外单冗余工序名
ALTER TABLE routing_steps       ADD COLUMN IF NOT EXISTS product_id   BIGINT REFERENCES products(product_id);
ALTER TABLE work_order_routings ADD COLUMN IF NOT EXISTS product_id   BIGINT REFERENCES products(product_id);
ALTER TABLE products            ADD COLUMN IF NOT EXISTS min_pack_qty DECIMAL(18,6);
ALTER TABLE outsourcing_orders  ADD COLUMN IF NOT EXISTS process_name VARCHAR(200);
