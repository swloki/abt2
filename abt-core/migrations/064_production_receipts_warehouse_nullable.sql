-- 完工入库改两步流程：生产申请（无仓库）+ 仓库确认入库（指定仓库）（#133）
-- production_receipts.warehouse_id 改 nullable，允许生产提交申请时不指定仓库，
-- 仓库确认（confirm）时再指定。warehouse_id 无外键约束（003_create_mes.sql L211），
-- 改 nullable 无外键影响。
ALTER TABLE production_receipts ALTER COLUMN warehouse_id DROP NOT NULL;
