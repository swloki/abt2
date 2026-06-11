-- 030: 发货单草稿支持 — order_id 改为可空，items 外键加 CASCADE
-- Issue #8: 保存草稿功能允许部分填写，order_id 可以后续补充

-- 草稿阶段可能尚未选择订单，order_id 允许 NULL
ALTER TABLE shipping_requests ALTER COLUMN order_id DROP NOT NULL;

-- items 外键加 ON DELETE CASCADE，简化"替换明细行"逻辑（删旧插新）
ALTER TABLE shipping_request_items
  DROP CONSTRAINT shipping_request_items_shipping_request_id_fkey,
  ADD CONSTRAINT shipping_request_items_shipping_request_id_fkey
    FOREIGN KEY (shipping_request_id) REFERENCES shipping_requests(id) ON DELETE CASCADE;
