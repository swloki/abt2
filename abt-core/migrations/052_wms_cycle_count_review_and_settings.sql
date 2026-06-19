-- 052: 盘点差异调账 + 金额阈值审批；WMS 全局设置（单行）
BEGIN;

-- ============================================================================
-- 1. cycle_counts 增加差异金额与审批审计字段
-- ============================================================================
ALTER TABLE cycle_counts
    ADD COLUMN IF NOT EXISTS variance_amount NUMERIC(20,6) NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS reviewer_id    BIGINT,
    ADD COLUMN IF NOT EXISTS reviewed_at    TIMESTAMPTZ;

COMMENT ON COLUMN cycle_counts.variance_amount IS '盘点差异金额 = Σ |variance_qty| × unit_cost，complete 时计算';
COMMENT ON COLUMN cycle_counts.reviewer_id IS '审批人（差异超阈值时）';
COMMENT ON COLUMN cycle_counts.reviewed_at IS '审批时间';

-- ============================================================================
-- 2. WMS 全局设置（单行配置，参考 purchase_settings）
--    cycle_count_variance_threshold: 盘点差异金额阈值，超过则进入 PendingReview 审批
-- ============================================================================
CREATE TABLE IF NOT EXISTS wms_settings (
    id                              BIGSERIAL    PRIMARY KEY,
    cycle_count_variance_threshold  NUMERIC(20,6) NOT NULL DEFAULT 0,
    created_at                      TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_at                      TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);

-- 单行约束：只允许一条设置
INSERT INTO wms_settings (id) VALUES (1)
ON CONFLICT (id) DO NOTHING;

-- 不允许新增第二条
CREATE OR REPLACE FUNCTION wms_settings_single_row() RETURNS TRIGGER AS $$
BEGIN
    IF (SELECT COUNT(*) FROM wms_settings) >= 1 AND NEW.id <> 1 THEN
        RAISE EXCEPTION 'wms_settings 只允许单行（id=1）';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_wms_settings_single_row ON wms_settings;
CREATE TRIGGER trg_wms_settings_single_row BEFORE INSERT OR UPDATE ON wms_settings
    FOR EACH ROW EXECUTE FUNCTION wms_settings_single_row();

COMMIT;
