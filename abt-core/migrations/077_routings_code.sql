-- routings 加业务编码 code（全局编码规则 DocumentSequence::Routing，RT 前缀）
ALTER TABLE routings ADD COLUMN IF NOT EXISTS code VARCHAR;

-- 回填现有 routing：RT + id 补零 6 位（如 RT000001）；新建的走 next_number（RT-YYYY-MM-NNNNNN）
UPDATE routings SET code = 'RT' || lpad(id::text, 6, '0') WHERE code IS NULL;

-- 部分唯一索引（排除软删除行，允许软删除的 code 被复用）
CREATE UNIQUE INDEX IF NOT EXISTS uk_routings_code ON routings (code) WHERE deleted_at IS NULL;
