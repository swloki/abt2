-- ============================================================================
-- Customers — 新增行业、等级、区域、币种、来源字段
-- ============================================================================

BEGIN;

ALTER TABLE customers
    ADD COLUMN IF NOT EXISTS industry       VARCHAR(100),
    ADD COLUMN IF NOT EXISTS customer_level SMALLINT     DEFAULT 1,  -- 1=普通客户, 2=关键客户, 3=潜在客户
    ADD COLUMN IF NOT EXISTS region         VARCHAR(100),
    ADD COLUMN IF NOT EXISTS currency       VARCHAR(10)  DEFAULT 'CNY',
    ADD COLUMN IF NOT EXISTS source         VARCHAR(100);

-- 联系人表新增传真和固定电话字段
ALTER TABLE customer_contacts
    ADD COLUMN IF NOT EXISTS fax            VARCHAR(50),
    ADD COLUMN IF NOT EXISTS fixed_phone    VARCHAR(50);

COMMIT;
