#!/usr/bin/env bash
# ============================================================================
# Q2C E2E 测试 — 登录/登出/导航 工具库
# 封装 agent-browser 的登录、会话管理、页面导航为可复用函数
# ============================================================================

# 确保基础环境已加载
if [[ -z "${ABT_URL:-}" ]]; then
    source "$(cd "$(dirname "${BASH_SOURCE[0]}")/../config" && pwd)/env.sh"
fi
if [[ -z "${Q2C_PASSWORD:-}" ]]; then
    source "$(cd "$(dirname "${BASH_SOURCE[0]}")/../config" && pwd)/agents.sh"
fi

# --- 内部辅助 ---
# 调用 agent-browser（带 session）
_ab() {
    local session="$1"; shift
    $AB_CMD $AB_SESSION_FLAG "$session" "$@"
}

# --- 登录 ---
# 用法: abt_login <session> <username> <password>
# 成功返回 0，失败返回 1
abt_login() {
    local session="$1"
    local username="$2"
    local password="$3"

    log_step "Login session=$session user=$username"

    # 1. 打开登录页
    _ab "$session" open "${ABT_URL}/login" > /dev/null 2>&1
    sleep 0.5

    # 2. 填写用户名和密码（使用 CSS 选择器）
    _ab "$session" fill "input[name='username']" "$username" > /dev/null 2>&1
    _ab "$session" fill "input[name='password']" "$password" > /dev/null 2>&1

    # 3. 点击登录按钮
    _ab "$session" find role button click --name "登录" > /dev/null 2>&1 || \
    _ab "$session" click "button[type='submit']" > /dev/null 2>&1

    # 4. 等待页面跳转
    sleep "$((LOGIN_WAIT / 1000))"

    # 5. 验证登录成功 — 检查 URL 不再包含 /login
    local current_url
    current_url=$(_ab "$session" get url 2>/dev/null || echo "")

    if [[ "$current_url" != *"/login"* ]]; then
        log_pass "Login OK: $username (session=$session)"
        return 0
    else
        log_fail "Login FAILED: $username (still on login page)"
        return 1
    fi
}

# --- 登出 ---
# 用法: abt_logout <session>
abt_logout() {
    local session="$1"
    log_step "Logout session=$session"

    # 导航到首页，点击登出（如有登出按钮）
    _ab "$session" eval "document.querySelector('a[href*=\"logout\"], button[data-action=\"logout\"]')?.click()" > /dev/null 2>&1 || true

    # 清除 cookies 作为兜底
    _ab "$session" cookies clear > /dev/null 2>&1 || true

    log_info "Session $session logged out"
}

# --- 页面导航 ---
# 用法: abt_navigate <session> <path>
# 例: abt_navigate q2c_sales /admin/quotations
abt_navigate() {
    local session="$1"
    local path="$2"
    local url="${ABT_URL}${path}"

    log_step "Navigate session=$session → $path"
    _ab "$session" open "$url" > /dev/null 2>&1
    sleep "$((PAGE_LOAD_WAIT / 1000))"

    # 验证 URL 包含目标路径
    local current_url
    current_url=$(_ab "$session" get url 2>/dev/null || echo "")

    if [[ "$current_url" == *"$path"* ]]; then
        return 0
    else
        log_warn "URL mismatch: expected *$path*, got $current_url"
        return 0  # 不阻断，某些 SPA 可能有 URL 差异
    fi
}

# --- 获取当前页面 URL ---
# 用法: abt_get_url <session>
abt_get_url() {
    local session="$1"
    _ab "$session" get url 2>/dev/null
}

# --- 获取页面文本 ---
# 用法: abt_get_text <session> [selector]
abt_get_text() {
    local session="$1"
    local selector="${2:-}"
    if [[ -n "$selector" ]]; then
        _ab "$session" get text "$selector" 2>/dev/null
    else
        _ab "$session" get text 2>/dev/null
    fi
}

# --- 获取页面快照（用于调试） ---
# 用法: abt_snapshot <session>
abt_snapshot() {
    local session="$1"
    _ab "$session" snapshot -i 2>/dev/null
}

# --- 截图 ---
# 用法: abt_screenshot <session> [path]
abt_screenshot() {
    local session="$1"
    local path="${2:-}"
    if [[ -n "$path" ]]; then
        _ab "$session" screenshot "$path" > /dev/null 2>&1
    else
        _ab "$session" screenshot > /dev/null 2>&1
    fi
}

# --- 等待元素出现 ---
# 用法: abt_wait_for <session> <selector> [timeout_ms]
abt_wait_for() {
    local session="$1"
    local selector="$2"
    local timeout="${3:-$TEST_TIMEOUT}"

    _ab "$session" wait "$selector" > /dev/null 2>&1 || \
    _ab "$session" wait "$((timeout / 1000))" > /dev/null 2>&1
}

# --- 执行 JavaScript ---
# 用法: abt_eval <session> <js_code>
abt_eval() {
    local session="$1"
    local js="$2"
    _ab "$session" eval "$js" 2>/dev/null
}

# --- 关闭会话 ---
# 用法: abt_close <session>
abt_close() {
    local session="$1"
    _ab "$session" close > /dev/null 2>&1 || true
}
