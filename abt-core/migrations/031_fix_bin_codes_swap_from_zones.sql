-- 修复迁移脚本错误：旧的库位编码(location_code)被错误地放到了 zones.code，
-- 而所有 bins.code 都被硬编码为 'DEFAULT'。
-- 正确的做法是：旧库位编码应该是储位(bins)的编码。
-- 此迁移将 zones.code/name 交换到 bins，然后给 zones 赋予通用库区编码。

BEGIN;

-- Step 1: 将 zones.code → bins.code, zones.name → bins.name
UPDATE bins b
SET code = z.code,
    name = z.name
FROM zones z
WHERE b.zone_id = z.id
  AND b.code = 'DEFAULT'
  AND b.deleted_at IS NULL;

-- Step 2: 给 zones 赋予通用库区编码（避免与 bins 重复）
-- 格式: ZONE-{zone_id}，保证 warehouse_id 内唯一
UPDATE zones
SET code = 'ZONE-' || id
WHERE deleted_at IS NULL
  AND code NOT LIKE 'ZONE-%';

COMMIT;
