-- 通用附件表（owner_type + owner_id 多态）。
-- 支持报价单(quotation)、销售订单(sales_order)等多单据的图片附件复用同一套。
-- 字节存文件系统 static/uploads/{owner_type}/{uuid}.{ext}，本表存元信息。
CREATE TABLE attachments (
    id            BIGSERIAL PRIMARY KEY,
    owner_type    VARCHAR(50) NOT NULL,       -- 'quotation' / 'sales_order' / ...
    owner_id      BIGINT NOT NULL,
    file_name     VARCHAR(255) NOT NULL,      -- 原始文件名（展示用）
    stored_path   VARCHAR(500) NOT NULL,      -- 相对 static/uploads/：{owner_type}/{uuid}.{ext}
    content_type  VARCHAR(100) NOT NULL,      -- image/png 等
    file_size     BIGINT NOT NULL,            -- bytes
    operator_id   BIGINT NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_attachments_owner ON attachments(owner_type, owner_id);
