-- routings.code 历史遗留 NULL 回填 + NOT NULL 约束
--
-- 背景：077_routings_code.sql 加 code 列时为 VARCHAR（允许 NULL），仅回填当时的 NULL 行，
-- 未加 NOT NULL 约束。Rust 模型 Routing.code: String（非 Option），若再出现 code IS NULL
-- 的行（077 未覆盖的历史行），list/find_by_id 解码即报：
--   error occurred while decoding column "code": unexpected null; try decoding as an `Option`
--
-- 修复：回填所有 NULL 行（与 077 同逻辑 RT + 6 位 id）+ 加 NOT NULL 约束，与 Rust 模型对齐。
-- create 路径已通过 DocumentSequence 填 code，NOT NULL 不会破坏后续插入。

UPDATE routings SET code = 'RT' || lpad(id::text, 6, '0') WHERE code IS NULL;

ALTER TABLE routings ALTER COLUMN code SET NOT NULL;
