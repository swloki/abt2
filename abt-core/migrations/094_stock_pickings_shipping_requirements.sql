-- Issue #218：订单申请发货页增加「发货要求」字段
-- stock_pickings 新增 shipping_requirements 列，存储销售在申请发货时填写的发货要求
-- （如指定快递、防震包装等），供仓库/物流发货时参考。
-- 注：stock_pickings.remark 已被收货地址占用，发货要求用独立列。
ALTER TABLE stock_pickings ADD COLUMN shipping_requirements TEXT NOT NULL DEFAULT '';

COMMENT ON COLUMN stock_pickings.shipping_requirements IS '发货要求（销售在申请发货时填写，供仓库/物流参考）';
