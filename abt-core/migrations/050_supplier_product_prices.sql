BEGIN;

-- ============================================================================
-- 供应商产品价格目录
-- ============================================================================

CREATE TABLE supplier_product_prices (
    id                  BIGSERIAL      PRIMARY KEY,
    supplier_id         BIGINT         NOT NULL,
    product_id          BIGINT         NOT NULL,
    supplier_item_code  VARCHAR(64),
    supplier_item_name  VARCHAR(256),
    min_order_qty       NUMERIC(18,6)  NOT NULL DEFAULT 1,
    price               NUMERIC(18,6)  NOT NULL,
    currency_code       VARCHAR(3)     NOT NULL DEFAULT 'CNY',
    discount_pct        NUMERIC(5,2)   NOT NULL DEFAULT 0,
    lead_time_days      INTEGER        NOT NULL DEFAULT 0,
    tax_rate_id         BIGINT,
    valid_from          DATE,
    valid_until         DATE,
    sequence            INTEGER        NOT NULL DEFAULT 10,
    is_active           BOOLEAN        NOT NULL DEFAULT TRUE,
    created_at          TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    deleted_at          TIMESTAMPTZ
);

CREATE INDEX idx_spp_supplier_product ON supplier_product_prices (supplier_id, product_id)
    WHERE deleted_at IS NULL AND is_active = TRUE;
CREATE INDEX idx_spp_product ON supplier_product_prices (product_id)
    WHERE deleted_at IS NULL AND is_active = TRUE;

COMMIT;
