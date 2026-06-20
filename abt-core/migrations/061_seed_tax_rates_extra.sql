-- 补全常用税率档位：1%（小规模减按）、5%（简易征收 / 不动产租赁等）
-- 与 046 的初始字典合并后覆盖国内采购/销售主流档位。幂等，可重复执行。
INSERT INTO tax_rates (code, name, rate, tax_type) VALUES
    ('VAT1', '增值税 1%（小规模减按）', 1.00, 3),
    ('VAT5', '增值税 5%（简易征收）',   5.00, 3)
ON CONFLICT (code) WHERE deleted_at IS NULL DO NOTHING;
