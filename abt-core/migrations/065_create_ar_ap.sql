-- AR/AP 应收应付台账 + 核销明细
-- direction: 1=Debit(AR增加/AP减少), 2=Credit(AR减少/AP增加)

CREATE TABLE ar_ap_ledger (
    id               BIGSERIAL      PRIMARY KEY,
    -- 往来方
    party_type       SMALLINT       NOT NULL,       -- 1=Customer, 2=Supplier
    party_id         BIGINT         NOT NULL,
    -- 科目（应收账款/应付账款）
    account_id       BIGINT         NOT NULL REFERENCES gl_accounts(id),
    -- 来源单据
    source_type      SMALLINT       NOT NULL,       -- DocumentType
    source_id        BIGINT         NOT NULL,
    source_doc_no    VARCHAR(40)    NOT NULL DEFAULT '',
    -- 核销对方单据（发票被付款核销时指向付款，付款核销发票时指向发票）
    against_type     SMALLINT,
    against_id       BIGINT,
    -- 金额与方向
    direction        SMALLINT       NOT NULL,       -- 1=Debit, 2=Credit
    amount           DECIMAL(18,6)  NOT NULL,
    amount_applied   DECIMAL(18,6)  NOT NULL DEFAULT 0,  -- 已核销金额（settle 时更新）
    -- 币种
    currency         VARCHAR(10)    NOT NULL DEFAULT 'CNY',
    exchange_rate    DECIMAL(18,6)  NOT NULL DEFAULT 1,
    -- 日期
    transaction_date DATE           NOT NULL,
    due_date         DATE,                           -- 到期日（账龄分析基准）
    period           VARCHAR(20)    NOT NULL,
    -- 关联 GL
    gl_entry_id      BIGINT         REFERENCES gl_entries(id),
    -- 元数据
    description      VARCHAR(300)   NOT NULL DEFAULT '',
    operator_id      BIGINT         NOT NULL,
    created_at       TIMESTAMPTZ    NOT NULL DEFAULT NOW()
);

-- 按往来方高效查询未清项
CREATE INDEX idx_aal_party       ON ar_ap_ledger(party_type, party_id);
CREATE INDEX idx_aal_party_due   ON ar_ap_ledger(party_type, party_id, due_date);
CREATE INDEX idx_aal_source      ON ar_ap_ledger(source_type, source_id);
CREATE INDEX idx_aal_against     ON ar_ap_ledger(against_type, against_id);
CREATE INDEX idx_aal_gl          ON ar_ap_ledger(gl_entry_id);
CREATE INDEX idx_aal_period_date ON ar_ap_ledger(period, transaction_date);

COMMENT ON TABLE ar_ap_ledger IS '应收应付台账 — 记录每一笔影响 AR/AP 余额的交易，支持按往来方维度的快速查询和账龄分析';

-- ---------------------------------------------------------------------------
-- 核销明细表（多对多：一笔付款可核销多张发票，一张发票可被多次付款核销）
-- ---------------------------------------------------------------------------

CREATE TABLE ar_ap_settlements (
    id                  BIGSERIAL      PRIMARY KEY,
    -- 付款侧
    payment_source_type SMALLINT       NOT NULL,       -- DocumentType (CashJournal)
    payment_source_id   BIGINT         NOT NULL,
    -- 发票侧
    invoice_source_type SMALLINT       NOT NULL,       -- DocumentType (SalesInvoice / PurchaseInvoice)
    invoice_source_id   BIGINT         NOT NULL,
    -- 核销金额
    amount              DECIMAL(18,6)  NOT NULL,
    -- 关联台账记录
    payment_ledger_id   BIGINT         REFERENCES ar_ap_ledger(id),
    invoice_ledger_id   BIGINT         REFERENCES ar_ap_ledger(id),
    -- 汇兑损益
    exchange_gain_loss  DECIMAL(18,6)  NOT NULL DEFAULT 0,
    -- 日期 & 操作人
    settlement_date     DATE           NOT NULL,
    operator_id         BIGINT         NOT NULL,
    created_at          TIMESTAMPTZ    NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_aas_payment ON ar_ap_settlements(payment_source_type, payment_source_id);
CREATE INDEX idx_aas_invoice ON ar_ap_settlements(invoice_source_type, invoice_source_id);
CREATE UNIQUE INDEX uk_aas ON ar_ap_settlements(payment_source_type, payment_source_id, invoice_source_type, invoice_source_id);

COMMENT ON TABLE ar_ap_settlements IS '核销明细 — 付款与发票的多对多匹配关系';

-- ---------------------------------------------------------------------------
-- 增强发票表：增加到期日、未清金额、已付金额字段
-- ---------------------------------------------------------------------------

ALTER TABLE sales_invoices ADD COLUMN IF NOT EXISTS due_date            DATE;
ALTER TABLE sales_invoices ADD COLUMN IF NOT EXISTS outstanding_amount  DECIMAL(18,6) NOT NULL DEFAULT 0;
ALTER TABLE sales_invoices ADD COLUMN IF NOT EXISTS paid_amount         DECIMAL(18,6) NOT NULL DEFAULT 0;

ALTER TABLE purchase_invoices ADD COLUMN IF NOT EXISTS due_date          DATE;
ALTER TABLE purchase_invoices ADD COLUMN IF NOT EXISTS outstanding_amount DECIMAL(18,6) NOT NULL DEFAULT 0;
ALTER TABLE purchase_invoices ADD COLUMN IF NOT EXISTS paid_amount        DECIMAL(18,6) NOT NULL DEFAULT 0;
