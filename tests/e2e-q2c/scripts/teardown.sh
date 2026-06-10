#!/usr/bin/env bash
# ============================================================================
# Q2C E2E 测试 — 一键环境清理
# 清理所有 Q2C 测试数据
# ============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
FIXTURE_DIR="$SCRIPT_DIR/../fixtures"

source "$SCRIPT_DIR/../config/env.sh"

# --- 检查 DATABASE_URL ---
if [[ -z "$DB_URL" ]]; then
    if [[ -f "$PROJECT_DIR/.env" ]]; then
        DB_URL="$(grep '^DATABASE_URL=' "$PROJECT_DIR/.env" | cut -d= -f2- | tr -d '"' | tr -d "'")"
    fi
fi

if [[ -z "$DB_URL" ]]; then
    echo "ERROR: DATABASE_URL not found. Set it in environment or .env file."
    exit 1
fi

echo "============================================"
echo "  Q2C E2E Test Environment Teardown"
echo "============================================"

# 执行清理
echo "Cleaning all Q2C test data..."
psql "$DB_URL" -f "$FIXTURE_DIR/99_cleanup.sql" 2>&1 | tail -1

# 验证清理
user_count=$(psql "$DB_URL" -t -A -c "SELECT COUNT(*) FROM users WHERE username LIKE 'q2c_%'" 2>/dev/null || echo "?")
product_count=$(psql "$DB_URL" -t -A -c "SELECT COUNT(*) FROM products WHERE product_code LIKE 'PRD-%' AND deleted_at IS NULL" 2>/dev/null || echo "?")

echo ""
echo "Remaining Q2C users: $user_count (expected: 0)"
echo "Remaining Q2C products: $product_count (expected: 0)"

echo ""
echo "============================================"
echo "  ✅ Environment cleaned"
echo "============================================"
