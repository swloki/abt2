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

# --- 事件断言 ---
# 用法: abt_assert_event <aggregate_type> <aggregate_id> <description>
# 验证 domain_events 表中存在指定聚合的事件
abt_assert_event() {
    local aggregate_type="$1"
    local aggregate_id="$2"
    local description="${3:-事件 $aggregate_type#$aggregate_id}"

    if [[ -z "$DB_URL" ]]; then
        assert_skip "事件断言跳过（无 DB_URL）: $description"
        return 0
    fi

    local count
    count=$(psql "$DB_URL" -t -A -c \
        "SELECT COUNT(*) FROM domain_events WHERE aggregate_type = '$aggregate_type' AND aggregate_id = $aggregate_id AND status IN (1,2,3)" \
        2>/dev/null || echo "0")

    if [[ "$count" -gt 0 ]]; then
        assert_pass "$description → $count events"
        return 0
    else
        assert_fail "$description → 无事件记录"
        return 1
    fi
}

# --- 通知断言 ---
# 用法: abt_assert_notification <operator_id> <description>
# 验证通知表中有给指定用户的未读/已读通知
# 注: 通知表名取决于系统实现，常见为 notifications 或 user_notifications
abt_assert_notification() {
    local operator_id="$1"
    local description="${2:-通知 user#$operator_id}"

    if [[ -z "$DB_URL" ]]; then
        assert_skip "通知断言跳过（无 DB_URL）: $description"
        return 0
    fi

    # 检查通知表是否存在
    local notify_table
    notify_table=$(psql "$DB_URL" -t -A -c \
        "SELECT table_name FROM information_schema.tables WHERE table_name IN ('notifications','user_notifications','scheduled_tasks') LIMIT 1" \
        2>/dev/null || echo "")

    if [[ -z "$notify_table" ]]; then
        assert_skip "通知断言跳过（通知表不存在）: $description"
        return 0
    fi

    local count
    count=$(psql "$DB_URL" -t -A -c \
        "SELECT COUNT(*) FROM $notify_table WHERE operator_id = $operator_id OR recipient_id = $operator_id" \
        2>/dev/null || echo "0")

    if [[ "$count" -gt 0 ]]; then
        assert_pass "$description → $count notifications"
        return 0
    else
        assert_fail "$description → 无通知记录"
        return 1
    fi
}

# --- 财务断言（借贷平衡） ---
# 用法: abt_assert_accounting_balance <journal_id> <description>
# 验证 cash_journal_lines 中指定 journal_id 的借方合计 = 贷方合计
abt_assert_accounting_balance() {
    local journal_id="$1"
    local description="${2:-借贷平衡 journal#$journal_id}"

    if [[ -z "$DB_URL" ]]; then
        assert_skip "财务断言跳过（无 DB_URL）: $description"
        return 0
    fi

    local result
    result=$(psql "$DB_URL" -t -A -c \
        "SELECT ABS(SUM(debit_amount) - SUM(credit_amount)) FROM cash_journal_lines WHERE journal_id = $journal_id" \
        2>/dev/null || echo "NULL")

    if [[ "$result" == "NULL" || -z "$result" ]]; then
        assert_skip "财务断言跳过（无 journal_lines 数据）: $description"
        return 0
    fi

    # 允许 0.01 精度误差
    if [[ "$(echo "$result < 0.01" | bc -l 2>/dev/null || echo 0)" -eq 1 ]]; then
        assert_pass "$description → balance diff=$result"
        return 0
    else
        assert_fail "$description → 借贷不平衡 diff=$result"
        return 1
    fi
}

# --- 审计断言 ---
# 用法: abt_assert_audit <entity_type> <entity_id> <description>
# 验证 audit_logs 表中存在指定实体的审计记录
abt_assert_audit() {
    local entity_type="$1"
    local entity_id="$2"
    local description="${3:-审计日志 $entity_type#$entity_id}"

    if [[ -z "$DB_URL" ]]; then
        assert_skip "审计断言跳过（无 DB_URL）: $description"
        return 0
    fi

    local count
    count=$(psql "$DB_URL" -t -A -c \
        "SELECT COUNT(*) FROM audit_logs WHERE entity_type = '$entity_type' AND entity_id = $entity_id" \
        2>/dev/null || echo "0")

    if [[ "$count" -gt 0 ]]; then
        assert_pass "$description → $count audit entries"
        return 0
    else
        assert_fail "$description → 无审计记录"
        return 1
    fi
}
