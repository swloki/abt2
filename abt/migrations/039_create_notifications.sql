CREATE TABLE IF NOT EXISTS notifications (
    notification_id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL,
    type VARCHAR(32) NOT NULL DEFAULT 'system',
    title VARCHAR(256) NOT NULL,
    content TEXT,
    related_type VARCHAR(64),
    related_id BIGINT,
    is_read BOOLEAN NOT NULL DEFAULT false,
    read_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    metadata JSONB NULL
);

CREATE INDEX IF NOT EXISTS idx_notifications_user_unread
    ON notifications(user_id, is_read, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_notifications_user_created
    ON notifications(user_id, created_at DESC);

COMMENT ON TABLE notifications IS '通用通知中心';
COMMENT ON COLUMN notifications.type IS '通知类型: stock_alert / system / approval ...';
COMMENT ON COLUMN notifications.metadata IS 'JSONB 扩展数据，如 {current_quantity, safety_stock, product_name}';
