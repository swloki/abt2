BEGIN;

-- =====================================================
-- demands 需求池表
-- =====================================================

CREATE TABLE demands (
    id              BIGSERIAL   PRIMARY KEY,
    demand_type     SMALLINT    NOT NULL DEFAULT 1,
    -- demand_type: 1=SalesOrder
    source_type     SMALLINT    NOT NULL,
    -- source_type: 2=SalesOrder (对应 DocumentType)
    source_id       BIGINT      NOT NULL,
    source_line_id  BIGINT      NOT NULL,
    product_id      BIGINT      NOT NULL,
    acquire_channel SMALLINT    NOT NULL,
    required_qty    DECIMAL(18,6) NOT NULL,
    required_date   DATE,
    status          SMALLINT    NOT NULL DEFAULT 1,
    -- status: 1=Pending, 2=Confirmed, 3=InProgress, 4=Fulfilled, 5=Rejected
    target_doc_type SMALLINT,
    target_doc_id   BIGINT,
    priority        INT         NOT NULL DEFAULT 5,
    remark          TEXT        NOT NULL DEFAULT '',
    operator_id     BIGINT      NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at      TIMESTAMPTZ
);

ALTER TABLE demands
  ADD CONSTRAINT chk_demands_status
  CHECK (status IN (1, 2, 3, 4, 5));

ALTER TABLE demands
  ADD CONSTRAINT chk_demands_acquire_channel
  CHECK (acquire_channel IN (1, 2, 3, 4, 9));

-- 索引
CREATE INDEX idx_demands_source
  ON demands (source_type, source_id);
CREATE INDEX idx_demands_product_status
  ON demands (product_id, status)
  WHERE deleted_at IS NULL;
CREATE INDEX idx_demands_acquire_status
  ON demands (acquire_channel, status)
  WHERE deleted_at IS NULL;
CREATE INDEX idx_demands_source_line
  ON demands (source_type, source_line_id)
  WHERE deleted_at IS NULL;

COMMIT;
