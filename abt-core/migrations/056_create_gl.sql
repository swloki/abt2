-- GL 总账内核：科目表 + 凭证 + 期间 + 映射
-- account_type: 1资产/2负债/3权益/4收入/5成本/6费用
-- balance_direction: 1借/2贷
-- entry status: 1draft/2posted/3cancelled
-- period status: 1open/2closed

CREATE TABLE gl_accounts (
    id                BIGSERIAL    PRIMARY KEY,
    code              VARCHAR(40)  NOT NULL UNIQUE,
    name              VARCHAR(100) NOT NULL,
    account_type      SMALLINT     NOT NULL,        -- 1..6
    parent_id         BIGINT       REFERENCES gl_accounts(id),
    is_detail         BOOLEAN      NOT NULL DEFAULT TRUE,
    balance_direction SMALLINT     NOT NULL,        -- 1借/2贷
    company_id        BIGINT       NOT NULL DEFAULT 1,
    reconcile         BOOLEAN      NOT NULL DEFAULT FALSE,
    disabled          BOOLEAN      NOT NULL DEFAULT FALSE,
    opening_balance   DECIMAL(18,6) NOT NULL DEFAULT 0,
    currency          VARCHAR(10)  NOT NULL DEFAULT 'CNY',
    version           INTEGER      NOT NULL DEFAULT 1,
    created_at        TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    deleted_at        TIMESTAMPTZ
);
CREATE INDEX idx_gl_accounts_parent ON gl_accounts(parent_id);
COMMENT ON TABLE gl_accounts IS '科目表 — 只有 is_detail=TRUE 的末级科目可被凭证引用';

CREATE TABLE accounting_periods (
    id           BIGSERIAL   PRIMARY KEY,
    name         VARCHAR(20) NOT NULL UNIQUE,       -- 2026-06
    start_date   DATE        NOT NULL,
    end_date     DATE        NOT NULL,
    status       SMALLINT    NOT NULL DEFAULT 1,    -- 1open/2closed
    fiscal_year  VARCHAR(10) NOT NULL,
    closed_at    TIMESTAMPTZ,
    closed_by    BIGINT,
    version      INTEGER     NOT NULL DEFAULT 1,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE gl_entries (
    id            BIGSERIAL   PRIMARY KEY,
    doc_number    VARCHAR(40) NOT NULL,
    period        VARCHAR(20) NOT NULL,
    entry_date    DATE        NOT NULL,
    source_type   SMALLINT    NOT NULL,             -- DocumentType
    source_id     BIGINT      NOT NULL DEFAULT 0,
    description   VARCHAR(300) NOT NULL DEFAULT '',
    voucher_type VARCHAR(20) NOT NULL DEFAULT 'Journal Entry',
    is_opening    BOOLEAN      NOT NULL DEFAULT FALSE,
    status        SMALLINT    NOT NULL DEFAULT 1,   -- 1draft/2posted/3cancelled
    total_debit   DECIMAL(18,6) NOT NULL DEFAULT 0,
    total_credit  DECIMAL(18,6) NOT NULL DEFAULT 0,
    operator_id   BIGINT      NOT NULL,
    version       INTEGER     NOT NULL DEFAULT 1,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at    TIMESTAMPTZ
);
CREATE INDEX idx_gl_entries_source ON gl_entries(source_type, source_id);
CREATE INDEX idx_gl_entries_period ON gl_entries(period);
CREATE INDEX idx_gl_entries_doc     ON gl_entries(doc_number);

CREATE TABLE gl_entry_lines (
    id              BIGSERIAL   PRIMARY KEY,
    entry_id        BIGINT      NOT NULL REFERENCES gl_entries(id),
    account_id      BIGINT      NOT NULL REFERENCES gl_accounts(id),
    debit           DECIMAL(18,6) NOT NULL DEFAULT 0,
    credit          DECIMAL(18,6) NOT NULL DEFAULT 0,
    amount_currency DECIMAL(18,6) NOT NULL DEFAULT 0,
    currency        VARCHAR(10)  NOT NULL DEFAULT 'CNY',
    exchange_rate   DECIMAL(18,6) NOT NULL DEFAULT 1,
    cost_center     BIGINT,
    profit_center   BIGINT,
    project_id      BIGINT,
    memo            VARCHAR(200) NOT NULL DEFAULT ''
);
CREATE INDEX idx_gl_entry_lines_entry   ON gl_entry_lines(entry_id);
CREATE INDEX idx_gl_entry_lines_account ON gl_entry_lines(account_id);

CREATE TABLE gl_account_mappings (
    id          BIGSERIAL   PRIMARY KEY,
    mapping_key VARCHAR(40) NOT NULL,    -- default_ar/default_ap/default_revenue/default_inventory/default_tax_output/default_tax_input/default_bank/default_expense
    account_id  BIGINT      NOT NULL REFERENCES gl_accounts(id),
    product_id  BIGINT,                  -- null=全局默认；非 null=产品级覆盖
    UNIQUE(mapping_key, product_id)
);

-- ── 默认科目 seed（中国准则常用末级科目）──
INSERT INTO gl_accounts (code, name, account_type, parent_id, is_detail, balance_direction, reconcile) VALUES
    ('1002',    '银行存款',         1, NULL, TRUE, 1, FALSE),
    ('1122',    '应收账款',         1, NULL, TRUE, 1, TRUE),
    ('2202',    '应付账款',         2, NULL, TRUE, 2, TRUE),
    ('2221.01', '应交税费-销项税',  2, NULL, TRUE, 2, TRUE),
    ('2221.02', '应交税费-进项税',  2, NULL, TRUE, 2, TRUE),
    ('4001',    '实收资本',         3, NULL, TRUE, 2, FALSE),
    ('5001',    '主营业务收入',     4, NULL, TRUE, 2, FALSE),
    ('1405',    '库存商品',         1, NULL, TRUE, 1, FALSE),
    ('6601',    '销售费用',         6, NULL, TRUE, 1, FALSE),
    ('4103',    '本年利润',         3, NULL, TRUE, 2, FALSE)
ON CONFLICT (code) DO NOTHING;

-- ── 默认科目映射（指向上面 seed 的科目，按 code 反查 id）──
INSERT INTO gl_account_mappings (mapping_key, account_id, product_id)
SELECT m.key, a.id, NULL
FROM (VALUES
    ('default_ar'), ('default_ap'), ('default_revenue'), ('default_inventory'),
    ('default_tax_output'), ('default_tax_input'), ('default_bank'), ('default_expense')
) AS m(key)
JOIN gl_accounts a ON a.code = CASE m.key
    WHEN 'default_ar'          THEN '1122'
    WHEN 'default_ap'          THEN '2202'
    WHEN 'default_revenue'     THEN '5001'
    WHEN 'default_inventory'   THEN '1405'
    WHEN 'default_tax_output'  THEN '2221.01'
    WHEN 'default_tax_input'  THEN '2221.02'
    WHEN 'default_bank'        THEN '1002'
    WHEN 'default_expense'     THEN '6601'
END
ON CONFLICT (mapping_key, product_id) DO NOTHING;

-- ── 默认期间 seed（2026 上半年）──
INSERT INTO accounting_periods (name, start_date, end_date, status, fiscal_year)
SELECT to_char(d, 'YYYY-MM'), d, (d + INTERVAL '1 month - 1 day')::date, 1, '2026'
FROM generate_series('2026-01-01'::date, '2026-06-01'::date, INTERVAL '1 month') AS d
ON CONFLICT (name) DO NOTHING;

-- ── GL 权限 seed（管理员 role_id=1）──
INSERT INTO role_permissions (role_id, resource_code, action) VALUES
    (1, 'GL', 'read'), (1, 'GL', 'create'), (1, 'GL', 'update'), (1, 'GL', 'delete')
ON CONFLICT DO NOTHING;
