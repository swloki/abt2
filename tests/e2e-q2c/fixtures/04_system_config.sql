-- ============================================================================
-- Q2C E2E 测试 — 系统配置参数
-- 预置审批阈值、安全库存、税率等系统参数
-- 脚本幂等，可重复执行（ON CONFLICT DO NOTHING / UPDATE）
-- ============================================================================

BEGIN;

-- ============================================================
-- 1. 审批阈值配置
-- 通过 UPDATE 确保参数值正确（支持重复执行）
-- ============================================================

-- 1.1 报价审批: 折扣率 > 15% 或 金额 > 500,000 触发审批
-- 存入 products meta 或系统参数表（视实现方式）
-- 如果系统使用 products.meta 存储，在 master data 中已设置
-- 此处提供 document_sequences 起始值确保编号可控

-- 确保 document_sequences 表存在
DO $$
BEGIN
    -- 报价编号序列
    IF NOT EXISTS (SELECT 1 FROM document_sequences WHERE prefix = 'QT' AND seq_date = CURRENT_DATE) THEN
        INSERT INTO document_sequences (prefix, current_value, seq_date, padding_len, strategy)
        VALUES ('QT', 0, CURRENT_DATE, 4, 1)
        ON CONFLICT (prefix, seq_date) DO NOTHING;
    END IF;

    -- 销售订单编号序列
    IF NOT EXISTS (SELECT 1 FROM document_sequences WHERE prefix = 'SO' AND seq_date = CURRENT_DATE) THEN
        INSERT INTO document_sequences (prefix, current_value, seq_date, padding_len, strategy)
        VALUES ('SO', 0, CURRENT_DATE, 4, 1)
        ON CONFLICT (prefix, seq_date) DO NOTHING;
    END IF;

    -- 采购订单编号序列
    IF NOT EXISTS (SELECT 1 FROM document_sequences WHERE prefix = 'PO' AND seq_date = CURRENT_DATE) THEN
        INSERT INTO document_sequences (prefix, current_value, seq_date, padding_len, strategy)
        VALUES ('PO', 0, CURRENT_DATE, 4, 1)
        ON CONFLICT (prefix, seq_date) DO NOTHING;
    END IF;

    -- 工单编号序列
    IF NOT EXISTS (SELECT 1 FROM document_sequences WHERE prefix = 'WO' AND seq_date = CURRENT_DATE) THEN
        INSERT INTO document_sequences (prefix, current_value, seq_date, padding_len, strategy)
        VALUES ('WO', 0, CURRENT_DATE, 4, 1)
        ON CONFLICT (prefix, seq_date) DO NOTHING;
    END IF;

    -- 发货申请编号序列
    IF NOT EXISTS (SELECT 1 FROM document_sequences WHERE prefix = 'SR' AND seq_date = CURRENT_DATE) THEN
        INSERT INTO document_sequences (prefix, current_value, seq_date, padding_len, strategy)
        VALUES ('SR', 0, CURRENT_DATE, 4, 1)
        ON CONFLICT (prefix, seq_date) DO NOTHING;
    END IF;
END $$;

-- ============================================================
-- 2. 税率配置
-- ============================================================

-- CUS-001 和 CUS-002 税率 13% 在 customers 表中设置
-- SUP-001 和 SUP-002 税率 13% 在 suppliers 表中设置
-- 确认 products 价格表中的税率为 13%
-- （如果价格存储在单独表中，此处更新；如在 products.meta 中则由 master data 处理）

-- ============================================================
-- 3. 安全库存参数
-- ============================================================

-- 如果使用 015_add_safety_stock.sql 创建的 safety_stock 字段
-- 为测试物料设置安全库存 = 0（确保不触发额外的补货建议干扰测试）
UPDATE products SET meta = COALESCE(meta, '{}') || jsonb_build_object('safety_stock', 0)
WHERE product_code IN ('PRD-FG-001', 'PRD-SFG-001', 'PRD-RM-001', 'PRD-RM-002', 'PRD-RM-003')
  AND deleted_at IS NULL;

-- ============================================================
-- 4. 客户信用额度确认
-- ============================================================

-- CUS-001: credit_limit = 500,000（正常客户）
UPDATE customers SET credit_limit = 500000
WHERE customer_code = 'CUS-001' AND deleted_at IS NULL;

-- CUS-002: credit_limit = 0（信用冻结客户）
UPDATE customers SET credit_limit = 0
WHERE customer_code = 'CUS-002' AND deleted_at IS NULL;

COMMIT;
