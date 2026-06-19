-- 053: 安全库存主动预警表
BEGIN;

CREATE TABLE IF NOT EXISTS wms_low_stock_alerts (
    id            BIGSERIAL       PRIMARY KEY,
    product_id    BIGINT          NOT NULL,
    warehouse_id  BIGINT          NOT NULL,
    current_qty   NUMERIC(20,6)   NOT NULL,
    safety_stock  NUMERIC(20,6)   NOT NULL,
    status        SMALLINT        NOT NULL DEFAULT 1,  -- 1=Active, 2=Acknowledged
    operator_id   BIGINT,
    created_at    TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    acked_at      TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_wms_lsa_product_wh ON wms_low_stock_alerts (product_id, warehouse_id);
CREATE INDEX IF NOT EXISTS idx_wms_lsa_status ON wms_low_stock_alerts (status);

COMMIT;
