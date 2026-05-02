-- 新增 created_by 列（从 JSONB bom_detail 中提取到顶层，便于可见性过滤）
ALTER TABLE bom ADD COLUMN created_by BIGINT;

-- 回填 created_by（从 bom_detail JSONB 中提取）
UPDATE bom SET created_by = (bom_detail->>'created_by')::bigint;

-- 新增 status 列，默认 'published' 向后兼容
ALTER TABLE bom ADD COLUMN status VARCHAR(20) NOT NULL DEFAULT 'published';

-- CHECK 约束防止脏数据
ALTER TABLE bom ADD CONSTRAINT bom_status_check CHECK (status IN ('draft', 'published'));

-- 审计列：记录发布时间和发布人
ALTER TABLE bom ADD COLUMN published_at TIMESTAMPTZ;
ALTER TABLE bom ADD COLUMN published_by BIGINT;

-- 已有数据：published_at 回填为 create_at，published_by 回填为 created_by
UPDATE bom SET published_at = create_at, published_by = created_by;

-- 索引：支持按状态过滤和可见性查询
CREATE INDEX idx_bom_status ON bom(status);
CREATE INDEX idx_bom_created_by ON bom(created_by);
