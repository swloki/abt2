-- 报价单图片附件
-- 存储元信息；图片字节存文件系统 uploads/{stored_path}。
CREATE TABLE quotation_attachments (
    id            BIGSERIAL PRIMARY KEY,
    quotation_id  BIGINT NOT NULL REFERENCES quotations(id) ON DELETE CASCADE,
    file_name     VARCHAR(255) NOT NULL,       -- 原始文件名（展示用）
    stored_path   VARCHAR(500) NOT NULL,       -- 相对 uploads/：quotation/{quo_id}/{uuid}.{ext}
    content_type  VARCHAR(100) NOT NULL,       -- image/png 等
    file_size     BIGINT NOT NULL,             -- bytes
    operator_id   BIGINT NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_quotation_attachments_quotation ON quotation_attachments(quotation_id);
