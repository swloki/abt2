#!/bin/bash
# 从 abt2 导入缺失的 products 和 bom 数据到 abt
# 可重复执行（幂等），已存在的数据会被跳过
#
# 使用方式: bash scripts/import_from_abt2.sh

set -e

DB_HOST="127.0.0.1"
DB_USER="postgres"
DB_PASS="123456"
ABT_DB="abt"
ABT2_DB="abt2"
DATA_DIR="$(cd "$(dirname "$0")" && pwd)/data"

export PGPASSWORD="$DB_PASS"

mkdir -p "$DATA_DIR"

echo "========================================="
echo " 从 abt2 导入数据到 abt"
echo "========================================="

# Step 0: 如果数据文件不存在，从 abt2 导出
if [ ! -f "$DATA_DIR/abt2_products.csv" ]; then
    echo "--- 导出 abt2 产品数据 ---"
    psql -h "$DB_HOST" -U "$DB_USER" -d "$ABT2_DB" -c "\copy (SELECT product_id, pdt_name, meta->>'product_code' AS product_code, COALESCE(meta->>'unit', 'pcs') AS unit, COALESCE(meta->>'specification', '') AS specification, COALESCE(meta->>'acquire_channel', '') AS acquire_channel, meta->>'old_code' AS old_code FROM products WHERE meta->>'product_code' IS NOT NULL AND meta->>'product_code' != '') TO '$DATA_DIR/abt2_products.csv' WITH CSV HEADER"
    echo "导出了 $(wc -l < "$DATA_DIR/abt2_products.csv") 行"
fi

if [ ! -f "$DATA_DIR/abt2_boms.csv" ]; then
    echo "--- 导出 abt2 BOM 数据 ---"
    psql -h "$DB_HOST" -U "$DB_USER" -d "$ABT2_DB" -c "\copy (SELECT bom_id, bom_name, create_at, bom_detail::text FROM bom) TO '$DATA_DIR/abt2_boms.csv' WITH CSV HEADER"
    echo "导出了 $(wc -l < "$DATA_DIR/abt2_boms.csv") 行"
fi

echo ""
echo "========================================="
echo " Step 1: 导入产品"
echo "========================================="

psql -h "$DB_HOST" -U "$DB_USER" -d "$ABT_DB" <<'EOSQL'
BEGIN;

CREATE TEMP TABLE tmp_abt2_products (
    product_id BIGINT PRIMARY KEY,
    pdt_name TEXT,
    product_code TEXT,
    unit TEXT,
    specification TEXT,
    acquire_channel TEXT,
    old_code TEXT,
    name_conflict BOOLEAN DEFAULT FALSE
) ON COMMIT PRESERVE ROWS;

\copy tmp_abt2_products(product_id, pdt_name, product_code, unit, specification, acquire_channel, old_code) FROM 'E:/work/abt/scripts/data/abt2_products.csv' WITH CSV HEADER

-- 标记名称冲突（pdt_name 唯一约束）
UPDATE tmp_abt2_products SET name_conflict = TRUE
FROM products p WHERE p.pdt_name = tmp_abt2_products.pdt_name;

SELECT count(*) AS total FROM tmp_abt2_products;

-- 导入缺失产品（名称冲突的追加 product_code 后缀）
INSERT INTO products (pdt_name, product_code, unit, meta)
SELECT
    CASE WHEN t.name_conflict THEN t.pdt_name || ' (' || t.product_code || ')' ELSE t.pdt_name END,
    t.product_code,
    t.unit,
    jsonb_build_object('specification', t.specification, 'acquire_channel', t.acquire_channel, 'old_code', NULLIF(t.old_code, ''))
FROM tmp_abt2_products t
WHERE NOT EXISTS (SELECT 1 FROM products p WHERE p.product_code = t.product_code);

COMMIT;

SELECT count(*) AS products_total FROM products;
EOSQL

echo ""
echo "========================================="
echo " Step 2: 导入 BOM"
echo "========================================="

psql -h "$DB_HOST" -U "$DB_USER" -d "$ABT_DB" <<'EOSQL'
BEGIN;

-- 加载产品数据（用于 product_id 映射）
CREATE TEMP TABLE tmp_abt2_products (
    product_id BIGINT PRIMARY KEY,
    pdt_name TEXT,
    product_code TEXT,
    unit TEXT,
    specification TEXT,
    acquire_channel TEXT,
    old_code TEXT
) ON COMMIT PRESERVE ROWS;

\copy tmp_abt2_products (product_id, pdt_name, product_code, unit, specification, acquire_channel, old_code) FROM 'E:/work/abt/scripts/data/abt2_products.csv' WITH CSV HEADER

-- 加载 BOM 数据
CREATE TEMP TABLE tmp_abt2_boms (
    old_bom_id BIGINT PRIMARY KEY,
    bom_name TEXT,
    create_at TIMESTAMPTZ,
    bom_detail JSONB
) ON COMMIT PRESERVE ROWS;

\copy tmp_abt2_boms FROM 'E:/work/abt/scripts/data/abt2_boms.csv' WITH CSV HEADER

-- 删除 abt 中已存在的 BOM
DELETE FROM tmp_abt2_boms t WHERE EXISTS (SELECT 1 FROM bom b WHERE b.bom_name = t.bom_name);

SELECT count(*) AS boms_to_import FROM tmp_abt2_boms;

-- ID 映射
CREATE TEMP TABLE tmp_bom_id_map (
    old_bom_id BIGINT PRIMARY KEY,
    new_bom_id BIGINT NOT NULL
);

WITH inserted AS (
    INSERT INTO bom (bom_name, create_at, update_at, bom_category_id, status, published_at, created_by)
    SELECT bom_name, create_at, create_at, NULL, 'published', create_at, NULL
    FROM tmp_abt2_boms
    RETURNING bom_id, bom_name, create_at
)
INSERT INTO tmp_bom_id_map (old_bom_id, new_bom_id)
SELECT t.old_bom_id, i.bom_id
FROM tmp_abt2_boms t JOIN inserted i ON i.bom_name = t.bom_name AND i.create_at = t.create_at;

SELECT count(*) AS bom_id_mappings FROM tmp_bom_id_map;

-- 找出悬空的 product_id（被 BOM 引用但不存在于 abt2 products 表）
CREATE TEMP TABLE tmp_dangling_pids AS
SELECT DISTINCT (node->>'product_id')::bigint AS pid
FROM tmp_abt2_boms t
CROSS JOIN jsonb_array_elements(t.bom_detail->'nodes') AS node
WHERE NOT EXISTS (SELECT 1 FROM tmp_abt2_products tp WHERE tp.product_id = (node->>'product_id')::bigint);

SELECT count(*) AS dangling_product_ids FROM tmp_dangling_pids;

-- 为悬空 product_id 创建占位产品
INSERT INTO products (pdt_name, product_code, unit, meta)
SELECT '未知产品-' || pid::text, 'UNKNOWN-' || pid::text, 'pcs', '{"specification":"","acquire_channel":"","old_code":null}'::jsonb
FROM tmp_dangling_pids
WHERE NOT EXISTS (SELECT 1 FROM products p WHERE p.product_code = 'UNKNOWN-' || pid::text);

-- 插入 BOM 节点
INSERT INTO bom_nodes (bom_id, product_id, product_code, quantity, parent_id, loss_rate, "order", unit, remark, position, work_center, properties)
SELECT
    m.new_bom_id,
    COALESCE(p.product_id, dp.new_product_id, 0),
    COALESCE(tp.product_code, 'UNKNOWN-' || (node->>'product_id')::bigint::text),
    COALESCE((node->>'quantity')::NUMERIC, 0),
    CASE WHEN (node->>'parent_id')::BIGINT = 0 THEN NULL ELSE (node->>'parent_id')::BIGINT END,
    COALESCE((node->>'loss_rate')::NUMERIC, 0),
    COALESCE((node->>'order')::INT, 0),
    node->>'unit',
    NULLIF(node->>'remark', ''),
    NULLIF(node->>'position', ''),
    NULLIF(node->>'work_center', ''),
    node->>'properties'
FROM tmp_abt2_boms t
CROSS JOIN jsonb_array_elements(t.bom_detail->'nodes') AS node
JOIN tmp_bom_id_map m ON m.old_bom_id = t.old_bom_id
LEFT JOIN tmp_abt2_products tp ON tp.product_id = (node->>'product_id')::BIGINT
LEFT JOIN products p ON p.product_code = tp.product_code
LEFT JOIN (
    SELECT dp.pid, pr.product_id AS new_product_id
    FROM tmp_dangling_pids dp
    JOIN products pr ON pr.product_code = 'UNKNOWN-' || dp.pid::text
) dp ON dp.pid = (node->>'product_id')::BIGINT
WHERE node->>'product_id' IS NOT NULL;

COMMIT;

-- 验证
SELECT 'nodes_with_missing_product' AS check, count(*) FROM bom_nodes WHERE product_id = 0;
SELECT 'products_total' AS metric, count(*) AS count FROM products
UNION ALL SELECT 'boms_total', count(*) FROM bom
UNION ALL SELECT 'bom_nodes_total', count(*) FROM bom_nodes;
EOSQL

echo ""
echo "========================================="
echo " 导入完成"
echo "========================================="
