-- 生产异常主表
CREATE TABLE IF NOT EXISTS production_exceptions (
    id BIGSERIAL PRIMARY KEY,
    doc_number VARCHAR(64) NOT NULL,
    exception_type SMALLINT NOT NULL,         -- 1=批次暂停 2=批次报废 3=不良异常 4=报检不合格 5=设备故障
    status SMALLINT NOT NULL DEFAULT 1,       -- 1=待处理 2=处理中 3=已关闭 4=条件放行 5=已恢复
    severity SMALLINT NOT NULL DEFAULT 2,     -- 1=紧急 2=一般 3=低
    reason_category SMALLINT,                 -- 1=物料不良 2=设备故障 3=操作失误 4=工艺问题
    work_order_id BIGINT,
    batch_id BIGINT,
    product_id BIGINT,
    current_step INTEGER,
    impact_qty NUMERIC(10,6),
    description TEXT,
    disposition VARCHAR(255),
    found_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    finder_id BIGINT,
    owner_id BIGINT,
    operator_id BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_exceptions_type ON production_exceptions(exception_type);
CREATE INDEX IF NOT EXISTS idx_exceptions_status ON production_exceptions(status);
CREATE INDEX IF NOT EXISTS idx_exceptions_work_order ON production_exceptions(work_order_id);
CREATE INDEX IF NOT EXISTS idx_exceptions_batch ON production_exceptions(batch_id);
CREATE INDEX IF NOT EXISTS idx_exceptions_found_at ON production_exceptions(found_at);

-- 异常事件表（处理时间线）
CREATE TABLE IF NOT EXISTS production_exception_events (
    id BIGSERIAL PRIMARY KEY,
    exception_id BIGINT NOT NULL REFERENCES production_exceptions(id),
    event_type VARCHAR(64) NOT NULL,
    description TEXT,
    operator_id BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_exception_events_exception ON production_exception_events(exception_id);
