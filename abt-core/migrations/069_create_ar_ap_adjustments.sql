-- 应收应付调整单 — 手工调整 AR/AP 余额（坏账/折扣/抹零/错误更正/汇兑差等）
-- 创建即过账：插入本表 + 同事务写 ar_ap_ledger（ledger_id 回填）
-- direction: 1=Increase(增加) 2=Decrease(减少) —— 业务方向
-- 过账时按 party_type 映射到 ar_ap_ledger.direction (Debit/Credit)：
--   Customer + Increase → Debit(应收增)   Customer + Decrease → Credit(应收减)
--   Supplier + Increase → Credit(应付增)  Supplier + Decrease → Debit(应付减)
-- 项目约定：无 FK 约束，应用层强制

CREATE TABLE ar_ap_adjustments (
    id              BIGSERIAL      PRIMARY KEY,
    doc_number      VARCHAR(40)    NOT NULL,
    -- 往来方
    party_type      SMALLINT       NOT NULL,       -- 1=Customer(应收) 2=Supplier(应付)
    party_id        BIGINT         NOT NULL,
    -- 调整方向与金额
    direction       SMALLINT       NOT NULL,       -- 1=Increase 2=Decrease
    amount          DECIMAL(18,6)  NOT NULL CHECK (amount > 0),
    currency        VARCHAR(10)    NOT NULL DEFAULT 'CNY',
    exchange_rate   DECIMAL(18,6)  NOT NULL DEFAULT 1,
    -- 日期与期间
    adjustment_date DATE           NOT NULL,
    period          VARCHAR(20)    NOT NULL,
    -- 可选订单号（参考记录，不强关联订单实体）
    int_order_no    VARCHAR(40),                    -- 内部订单号
    ext_order_no    VARCHAR(60),                    -- 客户/供应商订单号
    -- 说明与回账
    description     VARCHAR(300)   NOT NULL DEFAULT '',
    ledger_id       BIGINT,                         -- 过账生成的 ar_ap_ledger.id（回填）
    -- 审计
    operator_id     BIGINT         NOT NULL,
    created_at      TIMESTAMPTZ    NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX idx_aaadj_doc_number ON ar_ap_adjustments (doc_number);
CREATE INDEX idx_aaadj_party ON ar_ap_adjustments (party_type, party_id);
CREATE INDEX idx_aaadj_period ON ar_ap_adjustments (period, adjustment_date);
CREATE INDEX idx_aaadj_int_order ON ar_ap_adjustments (int_order_no) WHERE int_order_no IS NOT NULL;
CREATE INDEX idx_aaadj_ledger ON ar_ap_adjustments (ledger_id) WHERE ledger_id IS NOT NULL;

COMMENT ON TABLE ar_ap_adjustments IS '应收应付调整单 — 手工调整 AR/AP 余额，创建即过账写 ar_ap_ledger';
