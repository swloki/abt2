CREATE TABLE h3yun_sync_state (
    id              SERIAL PRIMARY KEY,
    entity_type     VARCHAR(32) NOT NULL,
    entity_id       BIGINT NOT NULL,
    h3yun_object_id VARCHAR(64),
    last_synced_at  TIMESTAMPTZ,
    content_hash    VARCHAR(64),
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(entity_type, entity_id)
);

COMMENT ON TABLE h3yun_sync_state IS 'H3Yun 同步映射表：ABT 实体与 H3Yun ObjectId 的映射关系';
COMMENT ON COLUMN h3yun_sync_state.entity_type IS '实体类型：product | inventory';
COMMENT ON COLUMN h3yun_sync_state.entity_id IS 'ABT 中的 product_id / inventory_id';
COMMENT ON COLUMN h3yun_sync_state.h3yun_object_id IS 'H3Yun 返回的 ObjectId（首次同步后填充）';
COMMENT ON COLUMN h3yun_sync_state.last_synced_at IS '上次成功同步时间';
COMMENT ON COLUMN h3yun_sync_state.content_hash IS '上次同步的内容哈希（用于去重）';
