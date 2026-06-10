#!/usr/bin/env bash
# ============================================================================
# Q2C E2E 测试 — 环境配置
# 所有测试脚本必须 source 此文件
# ============================================================================

set -euo pipefail

# --- 应用配置 ---
ABT_URL="${ABT_URL:-http://localhost:8000}"
ABT_HOST="${ABT_HOST:-localhost}"
ABT_PORT="${ABT_PORT:-8000}"

# --- 数据库配置（从 .env 或环境变量读取） ---
if [[ -f "$(git rev-parse --show-toplevel 2>/dev/null || echo .)/.env" ]]; then
    # 从 .env 提取 DATABASE_URL
    DB_URL="$(grep '^DATABASE_URL=' "$(git rev-parse --show-toplevel)/.env" | cut -d= -f2- | tr -d '"' | tr -d "'")"
fi
DB_URL="${DB_URL:-${DATABASE_URL:-}}"

# --- 测试配置 ---
TEST_TIMEOUT="${TEST_TIMEOUT:-30000}"      # agent-browser 操作超时（毫秒）
PAGE_LOAD_WAIT="${PAGE_LOAD_WAIT:-2000}"   # 页面加载等待时间（毫秒）
LOGIN_WAIT="${LOGIN_WAIT:-1500}"           # 登录后等待时间
RELAY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../relay" && pwd)"
FIXTURE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../fixtures" && pwd)"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../scripts" && pwd)"
LIB_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../lib" && pwd)"
TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../tests" && pwd)"

# --- agent-browser 配置 ---
AB_CMD="agent-browser"
AB_SESSION_FLAG="--session"

# --- 颜色输出 ---
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# --- 通用函数 ---
log_info()  { echo -e "${BLUE}[INFO]${NC} $*"; }
log_pass()  { echo -e "${GREEN}[PASS]${NC} $*"; }
log_fail()  { echo -e "${RED}[FAIL]${NC} $*"; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC} $*"; }
log_step()  { echo -e "${BLUE}[STEP]${NC} $*"; }

# 断言计数
ASSERT_PASS=0
ASSERT_FAIL=0
ASSERT_SKIP=0

assert_pass() { ((ASSERT_PASS++)); log_pass "$*"; }
assert_fail() { ((ASSERT_FAIL++)); log_fail "$*"; }
assert_skip() { ((ASSERT_SKIP++)); log_warn "SKIP: $*"; }

# 汇总报告
print_summary() {
    local total=$((ASSERT_PASS + ASSERT_FAIL + ASSERT_SKIP))
    echo ""
    echo "==========================================="
    echo "  Test Summary"
    echo "==========================================="
    echo -e "  PASS:  ${GREEN}${ASSERT_PASS}${NC}"
    echo -e "  FAIL:  ${RED}${ASSERT_FAIL}${NC}"
    echo -e "  SKIP:  ${YELLOW}${ASSERT_SKIP}${NC}"
    echo "  Total: ${total}"
    echo "==========================================="

    if [[ $ASSERT_FAIL -gt 0 ]]; then
        echo -e "${RED}RESULT: FAILED${NC}"
        return 1
    else
        echo -e "${GREEN}RESULT: ALL PASSED${NC}"
        return 0
    fi
}
