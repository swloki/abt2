-- ============================================================================
-- Product Watchers — 用户关注产品
-- Database: abt_v2
-- ============================================================================

BEGIN;

CREATE TABLE IF NOT EXISTS product_watchers (
    user_id              BIGINT        NOT NULL,
    product_id           BIGINT        NOT NULL,
    safety_stock_override NUMERIC(18,6) NULL,
    alert_active         BOOLEAN       NOT NULL DEFAULT false,
    last_notified_at     TIMESTAMPTZ,
    created_at           TIMESTAMPTZ   NOT NULL DEFAULT now(),
    updated_at           TIMESTAMPTZ   NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, product_id)
);

CREATE INDEX IF NOT EXISTS idx_product_watchers_product ON product_watchers (product_id);

COMMENT ON TABLE product_watchers IS '用户关注的产品列表';
COMMENT ON COLUMN product_watchers.safety_stock_override IS '用户自定义告警阈值，NULL 则使用 stock_ledger.safety_stock';
COMMENT ON COLUMN product_watchers.alert_active IS '当前是否处于活跃告警状态（库存低于阈值且已发送通知）';
COMMENT ON COLUMN product_watchers.last_notified_at IS '上次发送库存告警通知的时间';

COMMIT;
