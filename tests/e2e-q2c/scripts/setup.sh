#!/usr/bin/env bash
# ============================================================================
# Q2C E2E 测试 — 一键环境初始化
# 清理 → 建用户 → 建主数据 → 建库存 → 验证
# ============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
FIXTURE_DIR="$SCRIPT_DIR/../fixtures"

# 加载环境配置
source "$SCRIPT_DIR/../config/env.sh"

# --- 检查 DATABASE_URL ---
if [[ -z "$DB_URL" ]]; then
    # 尝试从 .env 读取
    if [[ -f "$PROJECT_DIR/.env" ]]; then
        DB_URL="$(grep '^DATABASE_URL=' "$PROJECT_DIR/.env" | cut -d= -f2- | tr -d '"' | tr -d "'")"
    fi
fi

if [[ -z "$DB_URL" ]]; then
    echo "ERROR: DATABASE_URL not found. Set it in environment or .env file."
    exit 1
fi

echo "============================================"
echo "  Q2C E2E Test Environment Setup"
echo "============================================"
echo "Database: $(echo "$DB_URL" | sed 's/:.*@/:***@/')"
echo ""

# --- Step 1: 清理旧数据 ---
echo "[1/5] Cleaning old test data..."
psql "$DB_URL" -f "$FIXTURE_DIR/99_cleanup.sql" 2>&1 | tail -1
echo "  → Cleaned"

# --- Step 2: 创建用户和角色 ---
echo "[2/5] Creating users and roles..."
psql "$DB_URL" -f "$FIXTURE_DIR/01_users_and_roles.sql" 2>&1 | tail -1

# 验证用户数
user_count=$(psql "$DB_URL" -t -A -c "SELECT COUNT(*) FROM users WHERE username LIKE 'q2c_%'" 2>/dev/null)
echo "  → Created $user_count test users"

# --- Step 3: 创建主数据 ---
echo "[3/5] Creating master data..."
psql "$DB_URL" -f "$FIXTURE_DIR/02_master_data.sql" 2>&1 | tail -1

# 验证物料
product_count=$(psql "$DB_URL" -t -A -c "SELECT COUNT(*) FROM products WHERE product_code LIKE 'PRD-%' AND deleted_at IS NULL" 2>/dev/null)
echo "  → Created $product_count products"

# --- Step 4: 创建初始库存 ---
echo "[4/5] Creating initial inventory..."
psql "$DB_URL" -f "$FIXTURE_DIR/03_initial_inventory.sql" 2>&1 | tail -1

# 验证库存
inv_count=$(psql "$DB_URL" -t -A -c "SELECT COUNT(*) FROM stock_ledger WHERE warehouse_id IN (SELECT id FROM warehouses WHERE code LIKE 'WH-%')" 2>/dev/null)
echo "  → Created $inv_count inventory records"

# --- Step 5: 验证完整性 ---
echo "[5/5] Verifying..."

errors=0

# 验证 15 个用户
if [[ "$user_count" != "15" ]]; then
    echo "  WARN: Expected 15 users, got $user_count"
    ((errors++))
fi

# 验证 5 个物料
if [[ "$product_count" != "5" ]]; then
    echo "  WARN: Expected 5 products, got $product_count"
    ((errors++))
fi

# 验证 BOM (用 ID 避免 bash→psql 中文编码问题)
bom_count=$(psql "$DB_URL" -t -A -c "SELECT COUNT(*) FROM boms WHERE bom_name LIKE '%-BOM' AND deleted_at IS NULL" 2>/dev/null || echo 0)
if [[ "$bom_count" -lt 2 ]]; then
    echo "  WARN: Expected at least 2 BOMs, got $bom_count"
    ((errors++))
fi

# 验证客户
cus_count=$(psql "$DB_URL" -t -A -c "SELECT COUNT(*) FROM customers WHERE customer_code IN ('CUS-001','CUS-002') AND deleted_at IS NULL" 2>/dev/null)
if [[ "$cus_count" != "2" ]]; then
    echo "  WARN: Expected 2 customers, got $cus_count"
    ((errors++))
fi

# 验证仓库
wh_count=$(psql "$DB_URL" -t -A -c "SELECT COUNT(*) FROM warehouses WHERE code IN ('WH-RAW','WH-WIP','WH-FG','WH-QC','WH-REJ','WH-SCRAP') AND deleted_at IS NULL" 2>/dev/null)
if [[ "$wh_count" != "6" ]]; then
    echo "  WARN: Expected 6 warehouses, got $wh_count"
    ((errors++))
fi

# 验证库存：原材料仓有库存，成品仓无库存
raw_qty=$(psql "$DB_URL" -t -A -c "
    SELECT COALESCE(SUM(quantity),0) FROM stock_ledger sl
    JOIN warehouses w ON sl.warehouse_id = w.id
    WHERE w.code = 'WH-RAW'" 2>/dev/null)
fg_qty=$(psql "$DB_URL" -t -A -c "
    SELECT COALESCE(SUM(quantity),0) FROM stock_ledger sl
    JOIN warehouses w ON sl.warehouse_id = w.id
    WHERE w.code = 'WH-FG'" 2>/dev/null)

echo "  → WH-RAW inventory: $raw_qty"
echo "  → WH-FG inventory: $fg_qty"

if [[ "$fg_qty" != "0" && "$fg_qty" != "0.000000" ]]; then
    echo "  WARN: WH-FG should be empty, got $fg_qty"
    ((errors++))
fi

# --- Step 6: 清理浏览器 session（用户重建后 cookie 失效） ---
echo "[6/6] Closing browser sessions (user credentials changed)..."
source "$SCRIPT_DIR/../config/agents.sh" 2>/dev/null || true
for entry in "${ALL_AGENTS[@]}"; do
    local_session="${entry#*:}"
    $AB_CMD $AB_SESSION_FLAG "$local_session" close > /dev/null 2>&1 || true
done
echo "  → All sessions closed (will re-login on next test)"

echo ""
echo "============================================"
if [[ $errors -eq 0 ]]; then
    echo "  ✅ Environment ready for Q2C testing"
else
    echo "  ⚠️  Environment setup completed with $errors warnings"
fi
echo "============================================"
