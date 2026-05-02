BEGIN;

CREATE TABLE IF NOT EXISTS bom_nodes (
    id          BIGSERIAL PRIMARY KEY,
    bom_id      BIGINT NOT NULL,
    product_id  BIGINT NOT NULL,
    product_code VARCHAR(255),
    quantity    DECIMAL(10,6) NOT NULL,
    parent_id   BIGINT,
    loss_rate   DECIMAL(10,6) NOT NULL DEFAULT 0,
    "order"     INT NOT NULL DEFAULT 0,
    unit        VARCHAR(50),
    remark      TEXT,
    position    VARCHAR(255),
    work_center VARCHAR(255),
    properties  TEXT
);

CREATE INDEX IF NOT EXISTS idx_bom_nodes_bom_id ON bom_nodes(bom_id);
CREATE INDEX IF NOT EXISTS idx_bom_nodes_parent_id ON bom_nodes(parent_id);
CREATE INDEX IF NOT EXISTS idx_bom_nodes_product_id ON bom_nodes(product_id);

COMMIT;
