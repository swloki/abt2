-- H3Yun 同步映射表
CREATE TABLE IF NOT EXISTS h3yun_sync_state (
    id                SERIAL PRIMARY KEY,
    entity_type       VARCHAR(20) NOT NULL,  -- 'product' | 'inventory'
    entity_id         BIGINT NOT NULL,
    h3yun_object_id   VARCHAR(100),
    last_synced_at    TIMESTAMPTZ,
    content_hash      VARCHAR(64),
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (entity_type, entity_id)
);

CREATE INDEX IF NOT EXISTS idx_h3yun_sync_entity ON h3yun_sync_state (entity_type, entity_id);
CREATE INDEX IF NOT EXISTS idx_h3yun_sync_unsynced ON h3yun_sync_state (entity_type) WHERE last_synced_at IS NULL;
