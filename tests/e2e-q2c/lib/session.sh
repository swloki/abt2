#!/usr/bin/env bash
# ============================================================================
# Q2C E2E 测试 — Agent 会话管理
# 批量初始化/清理 Agent 浏览器会话
# ============================================================================

# 确保依赖已加载
if [[ -z "${ABT_URL:-}" ]]; then
    source "$(cd "$(dirname "${BASH_SOURCE[0]}")/../config" && pwd)/env.sh"
fi
if [[ -z "${Q2C_PASSWORD:-}" ]]; then
    source "$(cd "$(dirname "${BASH_SOURCE[0]}")/../config" && pwd)/agents.sh"
fi
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/login.sh"

# --- 初始化所有 Agent 会话 ---
# 用法: init_all_sessions
# 为 15 个角色创建独立浏览器会话并完成登录
init_all_sessions() {
    log_info "Initializing all agent sessions..."

    local success=0
    local fail=0

    for agent_def in "${ALL_AGENTS[@]}"; do
        local role="${agent_def%%:*}"
        local user="${agent_def##*:}"
        local session
        session=$(get_session "$role")

        log_step "Initializing Agent-$role ($user)..."

        if abt_login "$session" "$user" "$Q2C_PASSWORD"; then
            ((success++))
        else
            log_fail "Agent-$role ($user) login failed"
            ((fail++))
        fi
    done

    log_info "Session init complete: $success OK, $fail FAILED"

    if [[ $fail -gt 0 ]]; then
        return 1
    fi
    return 0
}

# --- 初始化指定角色的会话 ---
# 用法: init_session <role_prefix>
# 例: init_session S1
init_session() {
    local role="$1"
    local user
    local session
    user=$(get_user "$role")
    session=$(get_session "$role")

    log_step "Initializing Agent-$role ($user)..."
    abt_login "$session" "$user" "$Q2C_PASSWORD"
}

# --- 清理所有 Agent 会话 ---
# 用法: cleanup_all_sessions
cleanup_all_sessions() {
    log_info "Cleaning up all agent sessions..."

    for agent_def in "${ALL_AGENTS[@]}"; do
        local role="${agent_def%%:*}"
        local session
        session=$(get_session "$role")
        abt_close "$session"
    done

    # 终极清理：关闭所有 session
    $AB_CMD close --all > /dev/null 2>&1 || true

    log_info "All sessions cleaned up"
}

# --- 清理指定角色的会话 ---
# 用法: cleanup_session <role_prefix>
cleanup_session() {
    local role="$1"
    local session
    session=$(get_session "$role")
    abt_close "$session"
}

# --- 验证所有会话已就绪 ---
# 用法: verify_sessions_ready
# 检查每个 session 是否仍处于登录状态
verify_sessions_ready() {
    local ready=0
    local not_ready=0

    for agent_def in "${ALL_AGENTS[@]}"; do
        local role="${agent_def%%:*}"
        local session
        session=$(get_session "$role")

        local url
        url=$(abt_get_url "$session" 2>/dev/null || echo "")

        if [[ "$url" == *"/login"* ]] || [[ -z "$url" ]]; then
            log_warn "Agent-$role session NOT ready (url=$url)"
            ((not_ready++))
        else
            ((ready++))
        fi
    done

    log_info "Sessions ready: $ready OK, $not_ready NOT READY"

    [[ $not_ready -eq 0 ]]
}
