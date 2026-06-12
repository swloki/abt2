-- 销售订单履行功能数据库迁移
-- 033_sales_order_fulfillment.sql

BEGIN;

-- 1. sales_order_items 表新增列
-- 添加取消数量、行状态和版本号字段
ALTER TABLE sales_order_items
  ADD COLUMN IF NOT EXISTS cancelled_qty DECIMAL(18,6) NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS line_status   SMALLINT     NOT NULL DEFAULT 1,
  ADD COLUMN IF NOT EXISTS version       INT          NOT NULL DEFAULT 1;

-- line_status 枚举值说明:
-- 1 = Pending (待处理)
-- 2 = Allocated (已分配)
-- 3 = Producing (生产中)
-- 4 = Purchasing (采购中)
-- 5 = Shipped (已发货)
-- 6 = Cancelled (已取消)

-- 添加剩余数量约束：剩余数量 = 总数量 - 已发货数量 - 取消数量
ALTER TABLE sales_order_items
  ADD CONSTRAINT chk_soi_open_qty_nonneg
  CHECK (quantity - shipped_qty - cancelled_qty >= 0);

-- 2. 重建状态机转换矩阵
-- 删除现有的销售订单状态转换定义
DELETE FROM state_transition_defs WHERE entity_type = 'SalesOrderStatus';

-- 重新创建状态转换定义（移除 InProduction 状态）
INSERT INTO state_transition_defs (entity_type, from_state, to_state, trigger_event, sort_order) VALUES
    ('SalesOrderStatus', '',          'Draft',            NULL, 1),
    ('SalesOrderStatus', 'Draft',     'Confirmed',        NULL, 2),
    ('SalesOrderStatus', 'Confirmed', 'PartiallyShipped', NULL, 3),
    ('SalesOrderStatus', 'Confirmed', 'Shipped',          NULL, 4),
    ('SalesOrderStatus', 'PartiallyShipped', 'Shipped',   NULL, 5),
    ('SalesOrderStatus', 'Shipped',   'Completed',        NULL, 6),
    ('SalesOrderStatus', 'Draft',     'Cancelled',        NULL, 7),
    ('SalesOrderStatus', 'Confirmed', 'Cancelled',        NULL, 8),
    ('SalesOrderStatus', 'PartiallyShipped', 'Cancelled', NULL, 9)
ON CONFLICT DO NOTHING;

-- 安全性检查：将现有的 InProduction(3) 状态更新为 Confirmed(2)
UPDATE sales_orders SET status = 2 WHERE status = 3 AND deleted_at IS NULL;

-- 3. 创建履行计划明细表 fulfillment_plan_lines
CREATE TABLE fulfillment_plan_lines (
    id                  BIGSERIAL   PRIMARY KEY,
    order_id            BIGINT      NOT NULL REFERENCES sales_orders(id),
    order_line_id       BIGINT      NOT NULL REFERENCES sales_order_items(id),
    product_id          BIGINT      NOT NULL,
    acquire_channel     SMALLINT    NOT NULL,
    required_qty        DECIMAL(18,6) NOT NULL,
    reserved_qty        DECIMAL(18,6) NOT NULL DEFAULT 0,
    shortage_qty        DECIMAL(18,6) NOT NULL DEFAULT 0,
    status              SMALLINT    NOT NULL DEFAULT 1,
    source_doc_type     SMALLINT,
    source_doc_id       BIGINT,
    reservation_details JSONB,
    required_date       DATE,
    version             INT         NOT NULL DEFAULT 1,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 添加约束条件
-- 状态约束：1=待处理, 2=已预留, 3=采购中, 4=生产中, 5=已完成
ALTER TABLE fulfillment_plan_lines
  ADD CONSTRAINT chk_fpl_status CHECK (status IN (1, 2, 3, 4, 5));

-- 获取渠道约束：1=库存, 2=采购, 3=生产, 4=委外, 9=其他
ALTER TABLE fulfillment_plan_lines
  ADD CONSTRAINT chk_fpl_acquire_channel CHECK (acquire_channel IN (1, 2, 3, 4, 9));

-- 创建索引
-- 唯一索引：确保每个订单行在履行计划中唯一
CREATE UNIQUE INDEX idx_fpl_order_line_unique ON fulfillment_plan_lines (order_line_id);

-- 普通索引：按订单ID查询
CREATE INDEX idx_fpl_order_id ON fulfillment_plan_lines (order_id);

-- 复合索引：按产品ID和状态查询，仅包含活跃状态
CREATE INDEX idx_fpl_product_status ON fulfillment_plan_lines (product_id, status) WHERE status IN (1, 2, 3, 4);

COMMIT;