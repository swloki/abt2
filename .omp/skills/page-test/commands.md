# agent-browser 命令库与问题排查

## 常用命令

```bash
# 页面导航
agent-browser open <url>                # 打开页面
agent-browser back                      # 后退
agent-browser eval "document.URL"       # 获取当前 URL

# 获取页面状态
agent-browser snapshot -i               # 无障碍树 + 交互元素引用
agent-browser errors                    # 检查 JS 错误
agent-browser errors --clear            # 清空错误记录

# 交互操作
agent-browser click @e<ref>             # 点击元素
agent-browser fill @e<ref> "value"      # 填写输入框
agent-browser select @e<ref> "option"   # 选择下拉选项

# 等待
agent-browser wait 2000                 # 等待毫秒
```

## eval 技巧

### onclick 不触发的解决方案

agent-browser 的 `click` 不触发 `onclick` 属性中的 JS。用 `eval` 替代：

```bash
# 打开 Modal
agent-browser eval "document.querySelector('#product-modal').classList.add('is-open')"

# 关闭 Modal
agent-browser eval "document.querySelector('#product-modal').classList.remove('is-open')"

# 触发 change 事件
agent-browser eval "document.getElementById('customer-select').dispatchEvent(new Event('change'))"

# 触发 input 事件（金额计算）
agent-browser eval "document.querySelector('#order-item-tbody tr').dispatchEvent(new Event('input', {bubbles: true}))"
```

如果 `eval` 操作 DOM 后仍不生效，尝试用 HTMX API 触发：

```bash
agent-browser eval "htmx.trigger(document.querySelector('#btn'), 'click')"
```

### 选择器降级策略

`@e` 序号依赖 DOM 顺序，前端改动容易导致错位。优先使用语义选择器：

```bash
# 优先：CSS 选择器（稳定，不受 DOM 顺序影响）
agent-browser click "button[type='submit']"
agent-browser fill "input[name='quantity']" "10"
agent-browser select "select[name='customer_id']" "某客户"

# 次选：@e ref（操作前先校验 ref 对应的元素文本）
# 先 snapshot 确认 @e8 确实是"提交"按钮，再 click
agent-browser snapshot -i | grep "@e8"
agent-browser click @e8
```

### 表单状态检查

```bash
# 检查隐藏字段值
agent-browser eval "document.getElementById('items-json').value"

# 检查函数是否可访问
agent-browser eval "JSON.stringify({calcRow: typeof window.calcRow, handleSubmit: typeof window.handleSubmit})"

# 枚举表单数据
agent-browser eval "
  (function(){
    var fd = new FormData(document.getElementById('<form-id>'));
    var obj = {};
    fd.forEach(function(v,k){ obj[k] = v; });
    return JSON.stringify(obj);
  })()
"

# 用 fetch 直接提交看服务端响应
agent-browser eval "
  (function(){
    var form = document.getElementById('<form-id>');
    var fd = new FormData(form);
    var params = new URLSearchParams();
    fd.forEach(function(v,k){ params.append(k,v); });
    fetch('<url>', {
      method: 'POST',
      headers: {'Content-Type': 'application/x-www-form-urlencoded', 'HX-Request': 'true'},
      body: params.toString()
    }).then(function(r) {
      return r.text().then(function(t) {
        window._debug = JSON.stringify({status: r.status, body: t.substring(0, 500)});
      });
    });
  })()
"
agent-browser eval "window._debug"
```

### HTMX 属性检查

```bash
# 检查元素的 HTMX 属性
agent-browser eval "
  var el = document.getElementById('<id>');
  JSON.stringify({
    hxGet: el?.getAttribute('hx-get'),
    hxTrigger: el?.getAttribute('hx-trigger'),
    hxTarget: el?.getAttribute('hx-target'),
    hxInclude: el?.getAttribute('hx-include')
  });
"
```

---

## 轻量级断言

先用 `eval` 做快速断言，只有 FAIL 时才 `snapshot -i` 排查，节省 Token：

```bash
# 断言页面标题包含关键词
agent-browser eval "document.title.includes('订单') ? 'PASS' : 'FAIL: title=' + document.title"

# 断言表格有数据行
agent-browser eval "document.querySelectorAll('tbody tr').length > 0 ? 'PASS: ' + document.querySelectorAll('tbody tr').length + ' rows' : 'FAIL: no rows'"

# 断言特定文本存在
agent-browser eval "document.body.innerText.includes('深圳光电') ? 'PASS' : 'FAIL'"

# 断言 URL 已跳转
agent-browser eval "document.URL.includes('/admin/orders/') ? 'PASS: ' + document.URL : 'FAIL: ' + document.URL"

# 断言无 JS 错误
agent-browser errors --clear
# ... 执行操作 ...
agent-browser errors  # 有输出就是 FAIL

# 断言表单隐藏字段已填充
agent-browser eval "JSON.parse(document.getElementById('items-json').value).length > 0 ? 'PASS' : 'FAIL: items-json empty'"
```

---

## HTMX 错误捕获

遇到 400/500 时，用 HTMX 事件机制直接获取错误响应：

```bash
# 在操作前注入 HTMX 错误监听器
agent-browser eval "
  document.body.addEventListener('htmx:responseError', function(e) {
    window._htmxError = {
      status: e.detail.xhr.status,
      body: e.detail.xhr.responseText.substring(0, 300)
    };
  });
"

# 执行 HTMX 操作后检查
agent-browser eval "window._htmxError || 'no error'"
```

---

## 故障恢复

```bash
# 页面卡死时强制停止加载
agent-browser eval "window.stop()"

# 刷新页面
agent-browser open <url>  # 重新导航

# 截图兜底（仅在 UI 交互异常时使用：元素不可见、点击无响应、白屏）
# 功能验证与数据断言禁止依赖截图
agent-browser screenshot
```

---

## 常见问题排查

| 现象 | 可能原因 | 排查方法 |
|------|----------|----------|
| 提交后页面不变 | `items-json` 仍为 `[]` | `eval "document.getElementById('items-json').value"` |
| 提交后报 422 | 表单字段重复或缺失 | `eval` 枚举 FormData 检查 name 属性 |
| 提交后报 400 | 业务规则违反 | 用 `fetch` 直接提交查看响应体 |
| 提交后报 500 | 后端代码错误 | 检查 `server_out.log` |
| Modal 打不开 | onclick 未触发 | 用 `eval` 直接操作 DOM |
| 产品行未添加 | HTMX target 不匹配 | 检查 `hx-target` 和 `hx-swap` |
| 小计不计算 | oninput 函数未暴露全局 | `eval "typeof calcRow"` 检查（IIFE 闭包问题） |
| 提交后跳转白屏 | HX-Redirect 失败 | `eval "document.URL"` |
| HTMX swap 后功能丢失 | 服务端返回 HTML 缺 HTMX 属性 | `eval` 检查 swap 后元素的 `hx-get`/`hx-trigger` |
| surreal.js `me('#id')` 失败 | `me()` 在外部 JS 或回调中不可靠 | 改用 `document.getElementById()` |
