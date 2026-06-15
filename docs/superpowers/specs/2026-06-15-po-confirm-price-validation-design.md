# 采购订单明细价格校验修复

> Issue #38: 采购订单详情页点击"确认"按钮后无法成功确认订单

## 问题根因

采购订单的校验存在**创建/确认不对称**：

| 环节 | 数量校验 | 单价校验 |
|------|---------|---------|
| 前端输入框 | `min="0"`（允许零） | 无 `min` 属性（允许负数） |
| 后端 `create()` | 无校验 | 无校验 |
| 后端 web handler | parse 失败 → `unwrap_or(0)` 静默归零 | 同上 |
| 后端 `confirm()` | `quantity > 0` | `unit_price > 0` |

零价/负价明细在创建时通过，到确认时才被拦截，用户看到 toast 报错但不知道原因。

数据库实况：6 个 Draft 状态 PO 中有 4 个存在零价/负价明细（PO-29/35/37/38）。

## 决策

- 零价/负价是录入错误，**创建时拦截**
- 存量数据由用户通过"编辑"按钮自行修复，不做批量迁移
- `confirm()` 中现有校验保留作为安全网

## 改动清单

### 1. 后端 Service 层：`create()` 添加逐行校验

文件：`abt-core/src/purchase/order/implt.rs`

在 `create()` 方法 step 4（插入明细）之后、step 5（审计日志）之前，添加：

```rust
// 4.5 校验明细：quantity > 0 且 unit_price > 0
for (i, item) in req.items.iter().enumerate() {
    if item.quantity <= Decimal::ZERO {
        return Err(DomainError::validation(
            format!("订单明细第 {} 行数量必须大于 0", i + 1)
        ));
    }
    if item.unit_price <= Decimal::ZERO {
        return Err(DomainError::validation(
            format!("订单明细第 {} 行单价必须大于 0", i + 1)
        ));
    }
}
```

### 2. 后端 Service 层：`update()` 添加相同校验

同一文件，`update()` 方法内添加相同的逐行校验逻辑。

### 3. 后端 Web Handler：解析容错修复

文件：`abt-web/src/pages/purchase_order_create.rs`

第 272-276 行，`unwrap_or(Decimal::ZERO)` 改为显式报错：

```rust
let quantity: Decimal = item.quantity.parse()
    .map_err(|_| DomainError::validation("无效数量"))?;
let unit_price: Decimal = item.unit_price.parse()
    .map_err(|_| DomainError::validation("无效单价"))?;
```

`purchase_order_edit.rs` 中的 update handler 同样处理。

### 4. 前端：输入框属性

文件：`abt-web/src/pages/purchase_order_create.rs` 和 `purchase_order_edit.rs`

`item_row_fragment` 中的输入框：
- `quantity`: `min="0"` → `min="0.01"`
- `unit_price`: 添加 `min="0.01"`

### 5. 前端：提交前 JS 校验

文件：`abt-web/src/pages/purchase_order_create.rs`

修改现有 submit handler，在收集 items 之前添加逐行校验：

```js
var errors = [];
document.querySelectorAll('#po-item-tbody tr').forEach(function(row, i) {
    var q = parseFloat(row.querySelector('[name=quantity]').value) || 0;
    var p = parseFloat(row.querySelector('[name=unit_price]').value) || 0;
    if (q <= 0) errors.push('第' + (i+1) + '行数量必须大于0');
    if (p <= 0) errors.push('第' + (i+1) + '行单价必须大于0');
});
if (errors.length > 0) {
    alert(errors.join('\n'));
    ev.preventDefault();
    return;
}
```

`purchase_order_edit.rs` 中的 submit handler 同样处理。

## 不涉及的范围

- `confirm()` 校验保持不变（安全网，兼容历史数据）
- 存量零价 Draft PO 由用户编辑修复
- 不涉及报价单校验、供应商校验等现有逻辑
- 不涉及设计文档同步（无接口/模型变更）

## 验证

1. `cargo clippy` 通过
2. 创建采购订单时输入零价/负价 → 前端拦截 + 后端拦截
3. 编辑现有零价 Draft PO → 修改价格后可正常确认
4. 正常价格 PO 创建 → 确认 → 流程顺畅
