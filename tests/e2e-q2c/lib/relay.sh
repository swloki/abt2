#!/usr/bin/env bash
# ============================================================================
# Q2C E2E 测试 — 接力状态管理工具库
# 通过 JSON 文件实现 Agent 间数据传递
# 依赖: jq
# ============================================================================

# 确保依赖已加载
if [[ -z "${RELAY_DIR:-}" ]]; then
    source "$(cd "$(dirname "${BASH_SOURCE[0]}")/../config" && pwd)/env.sh"
fi

# --- 接力文件路径 ---
RELAY_FILE="${RELAY_DIR}/relay-state.json"
RELAY_PURCHASE="${RELAY_DIR}/relay-purchase.json"
RELAY_PRODUCTION="${RELAY_DIR}/relay-production.json"

# --- 检查 jq 是否可用 ---
if ! command -v jq &>/dev/null; then
    echo "ERROR: jq is required for relay state management. Install with: choco install jq" >&2
    exit 1
fi

# --- 初始化接力文件 ---
# 用法: relay_init <run_id>
relay_init() {
    local run_id="${1:-run-$(date +%Y%m%d%H%M%S)}"
    local now
    now=$(date -u +%Y-%m-%dT%H:%M:%SZ)

    cat > "$RELAY_FILE" <<EOF
{
  "run_id": "$run_id",
  "phase": "init",
  "status": "pending",
  "started_at": "$now",
  "updated_at": "$now",
  "artifacts": {},
  "snapshots": {}
}
EOF
    log_info "Relay initialized: run_id=$run_id → $RELAY_FILE"
}

# --- 写入接力数据 ---
# 用法: relay_write <key> <value>
# 注意：value 会被视为字符串；如需写入数字/对象，请使用 relay_write_json
relay_write() {
    local key="$1"
    local value="$2"

    local tmp="${RELAY_FILE}.tmp"
    jq --arg k "$key" --arg v "$value" \
        '.artifacts[$k] = $v | .updated_at = (now | todate)' \
        "$RELAY_FILE" > "$tmp" && mv "$tmp" "$RELAY_FILE"
}

# --- 写入接力数据（JSON 值） ---
# 用法: relay_write_json <key> <json_value>
# 例: relay_write_json purchase_ids '["id1","id2"]'
relay_write_json() {
    local key="$1"
    local json_value="$2"

    local tmp="${RELAY_FILE}.tmp"
    jq --arg k "$key" --argjson v "$json_value" \
        '.artifacts[$k] = $v | .updated_at = (now | todate)' \
        "$RELAY_FILE" > "$tmp" && mv "$tmp" "$RELAY_FILE"
}

# --- 读取接力数据 ---
# 用法: relay_read <key>
# 输出值到 stdout，key 不存在时输出空字符串
relay_read() {
    local key="$1"
    local value
    value=$(jq -r --arg k "$key" '.artifacts[$k] // ""' "$RELAY_FILE" 2>/dev/null || echo "")
    echo "$value"
}

# --- 读取接力数据（JSON 格式） ---
# 用法: relay_read_json <key>
# 输出原始 JSON 值
relay_read_json() {
    local key="$1"
    jq -r --arg k "$key" '.artifacts[$k]' "$RELAY_FILE" 2>/dev/null || echo "null"
}

# --- 检查接力 key 是否存在 ---
# 用法: relay_has <key>
# 存在返回 0，不存在返回 1
relay_has() {
    local key="$1"
    local value
    value=$(jq -r --arg k "$key" '.artifacts[$k] // ""' "$RELAY_FILE" 2>/dev/null)
    [[ -n "$value" && "$value" != "null" ]]
}

# --- 更新当前阶段 ---
# 用法: relay_set_phase <phase>
relay_set_phase() {
    local phase="$1"
    local tmp="${RELAY_FILE}.tmp"
    jq --arg p "$phase" '.phase = $p | .updated_at = (now | todate)' \
        "$RELAY_FILE" > "$tmp" && mv "$tmp" "$RELAY_FILE"
    log_info "Relay phase → $phase"
}

# --- 更新状态 ---
# 用法: relay_set_status <status>
# status: pending | running | completed | failed | blocked
relay_set_status() {
    local status="$1"
    local tmp="${RELAY_FILE}.tmp"
    jq --arg s "$status" '.status = $s | .updated_at = (now | todate)' \
        "$RELAY_FILE" > "$tmp" && mv "$tmp" "$RELAY_FILE"
    log_info "Relay status → $status"
}

# --- 创建数据快照标记 ---
# 用法: relay_snapshot <snapshot_point>
# 例: relay_snapshot SNAP-1
relay_snapshot() {
    local point="$1"
    local now
    now=$(date -u +%Y-%m-%dT%H:%M:%SZ)
    local tmp="${RELAY_FILE}.tmp"
    jq --arg p "$point" --arg t "$now" \
        '.snapshots[$p] = $t | .updated_at = (now | todate)' \
        "$RELAY_FILE" > "$tmp" && mv "$tmp" "$RELAY_FILE"
    log_info "Relay snapshot → $point at $now"
}

# --- 打印接力文件内容 ---
# 用法: relay_dump
relay_dump() {
    echo "=== Relay State ==="
    jq '.' "$RELAY_FILE" 2>/dev/null || echo "(empty or invalid)"
    echo "==================="
}

# --- 初始化并行分支接力文件 ---
# 用法: relay_init_branch <branch>
# branch: purchase | production
relay_init_branch() {
    local branch="$1"
    local file="${RELAY_DIR}/relay-${branch}.json"
    local run_id
    run_id=$(jq -r '.run_id' "$RELAY_FILE" 2>/dev/null || echo "unknown")
    local now
    now=$(date -u +%Y-%m-%dT%H:%M:%SZ)

    cat > "$file" <<EOF
{
  "run_id": "$run_id",
  "branch": "$branch",
  "status": "pending",
  "started_at": "$now",
  "updated_at": "$now",
  "artifacts": {}
}
EOF
    log_info "Relay branch initialized: $branch → $file"
}

# --- 并行分支写入 ---
# 用法: relay_branch_write <branch> <key> <value>
relay_branch_write() {
    local branch="$1"
    local key="$2"
    local value="$3"
    local file="${RELAY_DIR}/relay-${branch}.json"
    local tmp="${file}.tmp"

    jq --arg k "$key" --arg v "$value" \
        '.artifacts[$k] = $v | .updated_at = (now | todate)' \
        "$file" > "$tmp" && mv "$tmp" "$file"
}

# --- 并行分支读取 ---
# 用法: relay_branch_read <branch> <key>
relay_branch_read() {
    local branch="$1"
    local key="$2"
    local file="${RELAY_DIR}/relay-${branch}.json"

    jq -r --arg k "$key" '.artifacts[$k] // ""' "$file" 2>/dev/null || echo ""
}

# --- 合并并行分支到主接力文件 ---
# 用法: relay_merge_branch <branch>
relay_merge_branch() {
    local branch="$1"
    local file="${RELAY_DIR}/relay-${branch}.json"
    local tmp="${RELAY_FILE}.tmp"

    # 将 branch 的 artifacts 合并到主文件的 artifacts 中
    jq --slurpfile branch "$file" \
        '.artifacts += ($branch[0].artifacts) | .updated_at = (now | todate)' \
        "$RELAY_FILE" > "$tmp" && mv "$tmp" "$RELAY_FILE"

    log_info "Relay merged branch: $branch"
}

# --- 清理接力文件 ---
# 用法: relay_clean
relay_clean() {
    echo '{}' > "$RELAY_FILE"
    rm -f "${RELAY_DIR}"/relay-purchase.json "${RELAY_DIR}"/relay-production.json
    relay_init "clean-$(date +%Y%m%d%H%M%S)"
    log_info "Relay files cleaned"
}
