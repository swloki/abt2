-- BOM 人工工序表（通过产品编码关联 BOM）
CREATE TABLE bom_labor_process (
    id BIGSERIAL PRIMARY KEY,
    product_code VARCHAR(100) NOT NULL,  -- 产品编码，关联 BOM 的产品
    site_id BIGINT NOT NULL DEFAULT 1,
    language_id BIGINT NOT NULL DEFAULT 1,
    name VARCHAR(255) NOT NULL,
    unit_price DECIMAL(12,2) NOT NULL,
    quantity DECIMAL(12,2) NOT NULL DEFAULT 1,
    sort_order INT NOT NULL DEFAULT 0,
    remark TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ,
    UNIQUE(product_code, name)
);

-- 索引
CREATE INDEX idx_bom_labor_process_product_code ON bom_labor_process(product_code);
CREATE INDEX idx_bom_labor_process_site_lang ON bom_labor_process(site_id, language_id);

-- 注释
COMMENT ON TABLE bom_labor_process IS 'BOM 人工工序表';
COMMENT ON COLUMN bom_labor_process.product_code IS '产品编码，关联 BOM 的产品';
COMMENT ON COLUMN bom_labor_process.name IS '工序名称';
COMMENT ON COLUMN bom_labor_process.unit_price IS '工序单价';
COMMENT ON COLUMN bom_labor_process.quantity IS '数量';
COMMENT ON COLUMN bom_labor_process.sort_order IS '排序顺序';
