#!/usr/bin/env bash
# ============================================================================
# Q2C E2E 测试 — 页面断言工具库
# 封装页面元素断言、文本验证、URL 检查、Toast 提示检测
# ============================================================================

# 确保依赖已加载
if [[ -z "${ABT_URL:-}" ]]; then
    source "$(cd "$(dirname "${BASH_SOURCE[0]}")/../config" && pwd)/env.sh"
fi

# --- 内部辅助 ---
_ab() {
    local session="$1"; shift
    $AB_CMD $AB_SESSION_FLAG "$session" "$@"
}

# --- 断言元素可见 ---
# 用法: abt_assert_visible <session> <css_selector> [error_msg]
abt_assert_visible() {
    local session="$1"
    local selector="$2"
    local msg="${3:-Element should be visible: $selector}"

    local result
    result=$(_ab "$session" is visible "$selector" 2>/dev/null || echo "false")

    if [[ "$result" == "true" ]]; then
        assert_pass "$msg"
        return 0
    else
        assert_fail "$msg (not visible)"
        return 1
    fi
}

# --- 断言元素包含文本 ---
# 用法: abt_assert_text <session> <css_selector> <expected> [error_msg]
abt_assert_text() {
    local session="$1"
    local selector="$2"
    local expected="$3"
    local msg="${4:-Text check: $selector}"

    local actual
    actual=$(_ab "$session" get text "$selector" 2>/dev/null || echo "")

    if [[ "$actual" == *"$expected"* ]]; then
        assert_pass "$msg → contains '$expected'"
        return 0
    else
        assert_fail "$msg → expected '$expected', got '$actual'"
        return 1
    fi
}

# --- 断言页面包含文本 ---
# 用法: abt_assert_page_contains <session> <expected_text> [error_msg]
abt_assert_page_contains() {
    local session="$1"
    local expected="$2"
    local msg="${3:-Page should contain: $expected}"

    local page_text
    page_text=$(_ab "$session" get text 2>/dev/null || echo "")

    if [[ "$page_text" == *"$expected"* ]]; then
        assert_pass "$msg"
        return 0
    else
        assert_fail "$msg (text not found on page)"
        return 1
    fi
}

# --- 断言当前 URL 包含路径 ---
# 用法: abt_assert_url_contains <session> <path> [error_msg]
abt_assert_url_contains() {
    local session="$1"
    local path="$2"
    local msg="${3:-URL should contain: $path}"

    local url
    url=$(_ab "$session" get url 2>/dev/null || echo "")

    if [[ "$url" == *"$path"* ]]; then
        assert_pass "$msg → $url"
        return 0
    else
        assert_fail "$msg → actual: $url"
        return 1
    fi
}

# --- 断言 Toast 提示 ---
# 用法: abt_assert_toast <session> <expected_text> [error_msg]
# 检查页面中的成功/错误提示（常见于 HTMX 响应的 toast 区域）
abt_assert_toast() {
    local session="$1"
    local expected="$2"
    local msg="${3:-Toast should contain: $expected}"

    # 尝试多种常见的 toast/notification 选择器
    local toast_text
    toast_text=$(abt_eval "$session" "
        const toast = document.querySelector('.toast, .notification, [role=\"alert\"], .success-msg, .error-msg, #toast-container, .htmx-indicator + .message');
        toast ? toast.textContent.trim() : '';
    " 2>/dev/null || echo "")

    if [[ -n "$toast_text" && "$toast_text" == *"$expected"* ]]; then
        assert_pass "$msg → toast: '$toast_text'"
        return 0
    fi

    # 备选：在页面全文中查找
    local page_text
    page_text=$(_ab "$session" get text 2>/dev/null || echo "")

    if [[ "$page_text" == *"$expected"* ]]; then
        assert_pass "$msg (found in page)"
        return 0
    else
        assert_fail "$msg → toast not found, page text doesn't contain '$expected'"
        return 1
    fi
}

# --- 断言元素不存在 ---
# 用法: abt_assert_not_visible <session> <css_selector> [error_msg]
abt_assert_not_visible() {
    local session="$1"
    local selector="$2"
    local msg="${3:-Element should NOT be visible: $selector}"

    local result
    result=$(_ab "$session" is visible "$selector" 2>/dev/null || echo "false")

    if [[ "$result" != "true" ]]; then
        assert_pass "$msg"
        return 0
    else
        assert_fail "$msg (element IS visible)"
        return 1
    fi
}

# --- 断言 input 值 ---
# 用法: abt_assert_value <session> <css_selector> <expected> [error_msg]
abt_assert_value() {
    local session="$1"
    local selector="$2"
    local expected="$3"
    local msg="${4:-Value check: $selector}"

    local actual
    actual=$(abt_eval "$session" "document.querySelector('$selector')?.value || ''" 2>/dev/null || echo "")

    if [[ "$actual" == "$expected" ]]; then
        assert_pass "$msg → value='$actual'"
        return 0
    else
        assert_fail "$msg → expected '$expected', got '$actual'"
        return 1
    fi
}

# --- 断言元素数量 ---
# 用法: abt_assert_count <session> <css_selector> <expected_count> [error_msg]
abt_assert_count() {
    local session="$1"
    local selector="$2"
    local expected="$3"
    local msg="${4:-Element count: $selector}"

    local count
    count=$(_ab "$session" get count "$selector" 2>/dev/null || echo "0")

    if [[ "$count" == "$expected" ]]; then
        assert_pass "$msg → count=$count"
        return 0
    else
        assert_fail "$msg → expected $expected, got $count"
        return 1
    fi
}

# --- 数据库断言 ---
# 用法: abt_assert_db <sql> <description>
# 执行 SQL 并断言返回行数 > 0
abt_assert_db() {
    local sql="$1"
    local description="$2"

    if [[ -z "$DB_URL" ]]; then
        assert_skip "DB assertion skipped (no DATABASE_URL): $description"
        return 0
    fi

    local count
    count=$(psql "$DB_URL" -t -A -c "SELECT COUNT(*) FROM ($sql) sub" 2>/dev/null || echo "0")

    if [[ "$count" -gt 0 ]]; then
        assert_pass "$description → $count rows"
        return 0
    else
        assert_fail "$description → 0 rows (expected > 0)"
        return 1
    fi
}

# --- 数据库断言（无结果） ---
# 用法: abt_assert_db_empty <sql> <description>
abt_assert_db_empty() {
    local sql="$1"
    local description="$2"

    if [[ -z "$DB_URL" ]]; then
        assert_skip "DB assertion skipped (no DATABASE_URL): $description"
        return 0
    fi

    local count
    count=$(psql "$DB_URL" -t -A -c "SELECT COUNT(*) FROM ($sql) sub" 2>/dev/null || echo "0")

    if [[ "$count" -eq 0 ]]; then
        assert_pass "$description → 0 rows (expected empty)"
        return 0
    else
        assert_fail "$description → $count rows (expected 0)"
        return 1
    fi
}
