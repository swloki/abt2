# 新建表单深度测试

**每个新建表单都必须执行完整测试，不能只测第一个就跳过。**

## 完整测试步骤

```
### Step 1: 页面结构验证
agent-browser open <create_url>
agent-browser snapshot -i
# 验证：分区标题、表单字段、必填标记

### Step 2: 选择客户
agent-browser select @e<customer_select> "某客户"
agent-browser eval "onCustomerChange()"  # 如需手动触发
agent-browser snapshot -i
# 验证：联系人/电话是否自动填充

### Step 3: 打开产品/订单选择
agent-browser eval "document.querySelector('#product-modal').classList.add('is-open')"
agent-browser snapshot -i
# 验证：Modal 已打开，数据已加载

### Step 4: 选择产品/订单
agent-browser click @e<选择按钮>
agent-browser snapshot -i
# 验证：行已添加到表格

### Step 5: 填写数据
agent-browser fill @e<quantity_input> "10"
agent-browser fill @e<unit_price_input> "25"
agent-browser snapshot -i
# 验证：金额自动计算正确

### Step 6: 提交表单
agent-browser eval "<submitFunction>(); htmx.trigger(document.getElementById('<form-id>'), 'submit')"
sleep 3
agent-browser eval "document.URL"
# 验证：跳转到详情页

### Step 7: 验证创建结果
agent-browser snapshot -i
# 验证：详情页数据正确
```

### Step 6 备选：提交表单的多种方式

`htmx.trigger` 有时会被表单校验拦截，依次尝试：

```bash
# 方式 1：标准方式（推荐）
agent-browser eval "<submitFunction>(); htmx.trigger(document.getElementById('<form-id>'), 'submit')"

# 方式 2：如果方式 1 无效，用原生表单提交
agent-browser eval "document.getElementById('<form-id>').requestSubmit()"

# 方式 3：如果方式 2 也无效，用 fetch 直接提交（见 commands.md）
```

### 网络响应验证

提交后用 fetch 确认后端确实拦截了非法请求（防止前端假校验）：

```bash
# 注入 HTMX 错误监听器
agent-browser eval "
  document.body.addEventListener('htmx:responseError', function(e) {
    window._htmxError = {status: e.detail.xhr.status, body: e.detail.xhr.responseText.substring(0, 300)};
  });
"

# 提交非法数据后检查
agent-browser eval "window._htmxError || 'no error captured'"
# 预期：{status: 400, body: "错误消息"}，而非 no error（说明后端没拦截）
```

---

## JS 函数名探测

不同表单的 JS 架构不同，测试前先探测：

```bash
# 探测 form id
agent-browser eval "document.querySelector('form[id]')?.id"

# 探测 submit 函数
agent-browser eval "JSON.stringify({
  quotationSubmit: typeof window.quotationSubmit,
  salesOrderSubmit: typeof window.salesOrderSubmit,
  handleSubmit: typeof window.handleSubmit,
  collectItems: typeof window.collectItems
})"
```

| 页面 | 常见 form id | 常见提交方式 |
|------|-------------|------------|
| 报价单 | quotation-form | `quotationSubmit()` + `htmx.trigger` |
| 销售订单 | order-form | `salesOrderSubmit()`（surreal.js 内联触发） |
| 发货 | shipping-form | `handleSubmit()` → `collectItems()` + `htmx.trigger`（外部 JS） |
| 退货 | return-form | `handleSubmit()` 返回 boolean，surreal.js 内联判断 |
| 对账单 | rec-create-form | 无 submit 函数，直接 `htmx.trigger(form, 'submit')` |

---

## 业务逻辑与异常输入测试

完成正常流程后，必须追加以下测试：

### 1. 必填字段缺失

逐个留空必填字段，提交表单，验证服务端返回正确错误（而非 500）：

```bash
agent-browser eval "htmx.trigger(document.getElementById('<form-id>'), 'submit')"
agent-browser snapshot -i
# 预期：错误提示（toast/内联消息），不是白屏或 500
```

### 2. 业务规则违反

| 业务场景 | 错误输入 | 预期结果 |
|---------|---------|---------|
| 退货单 | 选择未发货的订单 | 400，提示"发货单必须为已发货状态" |
| 发货申请 | 发货数量超过订单数量 | 前端/后端拒绝 |
| 退货单 | 退货数量超过已发货数量 | 前端标红或后端拒绝 |
| 对账单 | 重复提交同一客户同一期间 | 400，提示"已存在" |
| 销售订单 | 产品单价为 0 | 400，提示"总额不能为零" |
| 发货申请 | 未选择订单直接提交 | 400，提示"请选择来源订单" |

### 3. 多组数据测试

```
测试组 1：最小数据（1 个产品，最小数量）
测试组 2：多个产品（3+ 个产品行）
测试组 3：带折扣的场景
测试组 4：边界值（数量=1，单价=0.01）
```

### 4. 提交无反馈时用 fetch 检查

```bash
agent-browser eval "
  (function(){
    var form = document.getElementById('<form-id>');
    var fd = new FormData(form);
    var params = new URLSearchParams();
    fd.forEach(function(v,k){ params.append(k,v); });
    fetch('<action_url>', {
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
# 预期：400 + 清晰错误消息，而非 500 或空响应
```

---

### 常见业务规则速查

| 模块 | 业务规则 | 验证方式 |
|------|---------|---------|
| 报价单 | 至少一个产品、单价不能为零 | 缺产品提交 / 单价填 0 |
| 销售订单 | 必须选客户和联系人、总额不能为零 | 不选客户 / 单价为 0 |
| 发货申请 | 必须选客户和来源订单 | 不选订单提交 |
| 退货单 | 关联发货单必须已发货 | 选未发货订单 |
| 对账单 | 同客户同期间不能重复创建 | 连续提交两次相同数据 |
