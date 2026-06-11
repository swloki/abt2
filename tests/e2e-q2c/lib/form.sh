#!/usr/bin/env bash
# ============================================================================
# Q2C E2E 测试 — 表单操作工具库
# 封装表单填写、下拉选择、日期选择、按钮点击等操作
# 使用 CSS 选择器 + JavaScript eval，避免依赖动态 @e 引用
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

# --- 填写文本字段 ---
# 用法: abt_fill <session> <css_selector> <value>
# 先 clear 再 fill
abt_fill() {
    local session="$1"
    local selector="$2"
    local value="$3"

    _ab "$session" fill "$selector" "$value" > /dev/null 2>&1
}

# --- 键盘输入（模拟真实打字） ---
# 用法: abt_type <session> <css_selector> <text>
abt_type() {
    local session="$1"
    local selector="$2"
    local text="$3"

    _ab "$session" click "$selector" > /dev/null 2>&1
    _ab "$session" keyboard type "$text" > /dev/null 2>&1
}

# --- 选择下拉选项 ---
# 用法: abt_select <session> <css_selector> <value>
# 通过 agent-browser select 命令
abt_select() {
    local session="$1"
    local selector="$2"
    local value="$3"

    _ab "$session" select "$selector" "$value" > /dev/null 2>&1
}

# --- 通过文本选择下拉选项（用于 HTMX 动态下拉） ---
# 用法: abt_select_by_text <session> <css_selector> <text>
# 通过 JavaScript 查找 option 文本匹配后设置
abt_select_by_text() {
    local session="$1"
    local selector="$2"
    local text="$3"

    abt_eval "$session" "
        const sel = document.querySelector('$selector');
        if (sel) {
            const opts = sel.options;
            for (let i = 0; i < opts.length; i++) {
                if (opts[i].text.includes('$text')) {
                    sel.selectedIndex = i;
                    sel.dispatchEvent(new Event('change', {bubbles: true}));
                    break;
                }
            }
        }
    " > /dev/null 2>&1
}

# --- 检查元素是否存在 ---
# 用法: abt_has_element <session> <css_selector>
# 返回 "yes" 或 "no"（去除 abt_eval 返回的 JSON 引号）
abt_has_element() {
    local session="$1"
    local selector="$2"
    local result
    result=$(abt_eval "$session" "document.querySelector('$selector') ? 'yes' : 'no'" 2>/dev/null || echo "no")
    # 去除 agent-browser 返回的 JSON 字符串引号
    echo "${result//\"/}"
}

# --- 点击按钮/元素 ---
# 用法: abt_click <session> <css_selector>
abt_click() {
    local session="$1"
    local selector="$2"

    _ab "$session" click "$selector" > /dev/null 2>&1
}

# --- 按文字内容查找并点击 ---
# 用法: abt_click_by_text <session> <button_text>
# 查找包含指定文本的 button 或 a 元素
abt_click_by_text() {
    local session="$1"
    local text="$2"

    # 先尝试 find role button
    _ab "$session" find role button click --name "$text" > /dev/null 2>&1 && return 0

    # 备选：通过 JavaScript 查找
    abt_eval "$session" "
        const btn = Array.from(document.querySelectorAll('button, a, [role=\"button\"]'))
            .find(el => el.textContent.trim().includes('$text'));
        if (btn) { btn.click(); 'clicked'; } else { 'not_found'; }
    " > /dev/null 2>&1
}

# --- 勾选复选框 ---
# 用法: abt_check <session> <css_selector>
abt_check() {
    local session="$1"
    local selector="$2"

    _ab "$session" check "$selector" > /dev/null 2>&1
}

# --- 取消勾选复选框 ---
# 用法: abt_uncheck <session> <css_selector>
abt_uncheck() {
    local session="$1"
    local selector="$2"

    _ab "$session" uncheck "$selector" > /dev/null 2>&1
}

# --- 提交表单 ---
# 用法: abt_submit <session> [form_selector]
# 默认提交页面中第一个表单
abt_submit() {
    local session="$1"
    local form_selector="${2:-form}"

    abt_click "$session" "${form_selector} button[type='submit']" 2>/dev/null || \
    _ab "$session" press Enter > /dev/null 2>&1
}

# --- HTMX 触发（通过 htmx.ajax 或 hx-post） ---
# 用法: abt_htmx_trigger <session> <element_selector> <event>
# 例: abt_htmx_trigger q2c_sales "select[name='customer_id']" "change"
abt_htmx_trigger() {
    local session="$1"
    local selector="$2"
    local event="${3:-change}"

    abt_eval "$session" "
        const el = document.querySelector('$selector');
        if (el) { el.dispatchEvent(new Event('$event', {bubbles: true})); 'triggered'; }
        else { 'not_found'; }
    " > /dev/null 2>&1
}

# --- 等待 HTMX 请求完成 ---
# 用法: abt_wait_htmx <session> [timeout_ms]
abt_wait_htmx() {
    local session="$1"
    local timeout="${2:-5000}"
    local interval=200
    local elapsed=0

    while [[ $elapsed -lt $timeout ]]; do
        local pending
        pending=$(abt_eval "$session" "
            (typeof htmx !== 'undefined' && htmx.activeRequests && htmx.activeRequests > 0)
                ? 'pending'
                : 'idle';
        " 2>/dev/null || echo "idle")

        if [[ "$pending" == "idle" ]]; then
            return 0
        fi

        sleep "$((interval / 1000))"
        elapsed=$((elapsed + interval))
    done

    log_warn "HTMX wait timed out after ${timeout}ms"
    return 1
}

# --- 设置 hidden input 值（用于 Alpine.js 表单桥接） ---
# 用法: abt_set_hidden <session> <name> <value>
abt_set_hidden() {
    local session="$1"
    local name="$2"
    local value="$3"

    abt_eval "$session" "
        const inp = document.querySelector('input[name=\"$name\"]');
        if (inp) {
            inp.value = '$value';
            inp.dispatchEvent(new Event('input', {bubbles: true}));
            inp.dispatchEvent(new Event('change', {bubbles: true}));
            'set';
        } else { 'not_found'; }
    " > /dev/null 2>&1
}

# --- 获取 input/select 值 ---
# 用法: abt_get_value <session> <css_selector>
abt_get_value() {
    local session="$1"
    local selector="$2"

    abt_eval "$session" "document.querySelector('$selector')?.value || ''" 2>/dev/null
}

# --- 按下键盘按键 ---
# 用法: abt_press <session> <key>
# 例: abt_press q2c_sales Enter
abt_press() {
    local session="$1"
    local key="$2"

    _ab "$session" press "$key" > /dev/null 2>&1
}

# --- 滚动到元素可见 ---
# 用法: abt_scroll_to <session> <css_selector>
abt_scroll_to() {
    local session="$1"
    local selector="$2"

    _ab "$session" scrollintoview "$selector" > /dev/null 2>&1
}

# --- HTMX 按钮/操作触发 ---
# 用法: abt_htmx_post <session> <post_path>
# 直接调用 htmx.ajax POST，绕过 JS click() 不触发 HTMX 的问题
# 例: abt_htmx_post q2c_sales "/admin/quotations/37/submit"
abt_htmx_post() {
    local session="$1"
    local path="$2"

    abt_eval "$session" "
        var xhr = new XMLHttpRequest();
        xhr.open('POST', '$path', false);
        xhr.setRequestHeader('HX-Request', 'true');
        xhr.setRequestHeader('HX-Target', 'body');
        xhr.send();
        xhr.status + ' ' + xhr.responseText.substring(0, 200);
    " 2>/dev/null
}

# --- HTMX 表单提交 ---
# 用法: abt_htmx_submit_form <session> <form_selector> <submit_fn_name>
# 先调用 submit 函数收集 items_json，再 htmx.trigger 提交表单
# 例: abt_htmx_submit_form q2c_sales "#quotation-form" "quotationSubmit"
abt_htmx_submit_form() {
    local session="$1"
    local form_selector="$2"
    local submit_fn="$3"

    abt_eval "$session" "
        if (typeof $submit_fn === 'function') { $submit_fn(); }
        htmx.trigger(document.querySelector('$form_selector'), 'submit');
        'form_submitted';
    " > /dev/null 2>&1 || true
}
