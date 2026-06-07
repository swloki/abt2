# MES-07 完工入库

> 路由前缀: `/admin/mes/receipts`
> 代码文件: `mes_receipt_list.rs`, `mes_receipt_create.rs`, `mes_receipt_detail.rs`
> 原型文件: `04-receipt-list.html`, `04-receipt-create.html`, `04-receipt-detail.html`

## 1. 入库列表页

### 1.1 页面访问

| ID | 测试项 | 操作 | 预期结果 |
|----|--------|------|---------|
| RC-01 | 直接访问 | GET /admin/mes/receipts | 200，标题 "完工入库" |
| RC-02 | 侧栏导航 | 点击"完工入库" | 跳转 /admin/mes/receipts |
| RC-03 | Dashboard 入口 | 点击"完工入库"快捷卡片 | 跳转 /admin/mes/receipts |

### 1.2 页面头部

| ID | 测试项 | 预期结果 |
|----|--------|---------|
| RC-10 | 页面标题 | "完工入库" |
| RC-11 | 新建按钮 | 右侧 "新建入库" 按钮，链接 /admin/mes/receipts/create |

### 1.3 数据表格（当前为 Stub）

> **注意**: 列表页是 stub，始终显示"暂无入库记录"。

表头：

| 列 | 对齐 |
|----|------|
| 单号 | 左 |
| 工单 | 左 |
| 批次 | 左 |
| 产品ID | 左 |
| 入库数量 | 右 |
| 仓库 | 左 |
| 状态 | 左 |
| 操作 | 左 |

### 1.4 入库状态标签

| 值 | 中文 | 背景色 | 文字色 |
|----|------|--------|--------|
| Draft | 草稿 | rgba(0,0,0,0.04) | var(--muted) |
| Confirmed | 已确认 | rgba(82,196,26,0.08) | var(--success) |
| Cancelled | 已取消 | rgba(245,63,63,0.06) | #f53f3f |

### 1.5 Stub 状态测试

| ID | 测试项 | 预期结果 |
|----|--------|---------|
| RC-20 | 空数据提示 | "暂无入库记录" 居中 |
| RC-21 | 表格结构 | 8 列 thead |

---

## 2. 新建入库页

### 2.1 页面访问

| ID | 测试项 | 操作 | 预期结果 |
|----|--------|------|---------|
| RC-30 | 从列表进入 | 点击"新建入库" | 跳转 /admin/mes/receipts/create |
| RC-31 | 页面标题 | — | "新建入库" |
| RC-32 | 返回链接 | "← 返回列表" | 跳回 /admin/mes/receipts |

### 2.2 表单字段

"入库信息" form-section + form-grid：

| 字段 | 控件 | name | 必填 | 说明 |
|------|------|------|------|------|
| 工单ID | number | work_order_id | ✅ | — |
| 批次ID | number | batch_id | — | 可选 |
| 产品ID | number | product_id | ✅ | — |
| 入库数量 | number | received_qty | ✅ | step=0.01 |
| 仓库ID | number | warehouse_id | ✅ | — |
| 库区ID | number | zone_id | — | 可选 |
| 储位ID | number | bin_id | — | 可选 |
| 入库日期 | date | receipt_date | ✅ | — |

### 2.3 提交测试

| ID | 测试项 | 操作 | 预期结果 |
|----|--------|------|---------|
| RC-40 | 空提交 | 不填直接提交 | HTML5 校验阻止 |
| RC-41 | 有效提交 | 填写所有必填项 | hx-post → HX-Redirect 到列表 |
| RC-42 | 不带批次ID | 只填必填项 | 提交成功（batch_id 可选） |
| RC-43 | 不带库区/储位 | 只填仓库 | 提交成功（zone_id/bin_id 可选） |
| RC-44 | 无效产品ID | 非数字 | 服务端返回错误 |
| RC-45 | 取消 | 点击"取消" | 跳回列表 |

### 2.4 ReceiptCreateForm 结构

```rust
pub struct ReceiptCreateForm {
    pub work_order_id: i64,
    pub batch_id: Option<i64>,
    pub product_id: i64,
    pub received_qty: Decimal,
    pub warehouse_id: i64,
    pub zone_id: Option<i64>,
    pub bin_id: Option<i64>,
    pub receipt_date: NaiveDate,
    pub remark: Option<String>,
}
```

> **注意**: 表单没有 remark 字段，但 `ReceiptCreateForm` 和 `CreateReceiptReq` 包含 remark。UI 缺少备注输入框。

---

## 3. 入库详情页

### 3.1 页面访问

| ID | 测试项 | 操作 | 预期结果 |
|----|--------|------|---------|
| RC-50 | 直接访问 | GET /admin/mes/receipts/{id} | 200，标题 "入库单 {doc_number}" |
| RC-51 | 返回链接 | "← 返回列表" | 跳回 /admin/mes/receipts |
| RC-52 | 无效 ID | /admin/mes/receipts/999999 | 错误页 |

### 3.2 详情头部

| ID | 元素 | 说明 |
|----|------|------|
| RC-53 | 入库单号 | detail-no class, mono 大号 |
| RC-54 | 操作按钮区 | 仅 Draft 状态显示"确认入库"按钮 |

### 3.3 详情信息卡片

info-grid 布局：

| 字段 | 样式 | 说明 |
|------|------|------|
| 单号 | mono | doc_number |
| 工单ID | — | — |
| 批次ID | — | 有值显示数字，无值显示"—" |
| 产品ID | — | — |
| 入库数量 | mono | received_qty |
| 仓库ID | — | — |
| 入库日期 | — | — |
| 状态 | 颜色 pill | 草稿/已确认/已取消 |
| 倒冲触发 | — | "是" 或 "否" |
| 创建时间 | — | YYYY-MM-DD HH:mm |
| 备注 (有值时) | span-2 | — |

### 3.4 批次ID显示测试

| ID | 测试项 | 预期结果 |
|----|--------|---------|
| RC-60 | 有批次ID | 显示批次 ID 数字 |
| RC-61 | 无批次ID (None) | 显示 "—" |

### 3.5 倒冲触发显示测试

| ID | 测试项 | 预期结果 |
|----|--------|---------|
| RC-70 | backflush_triggered=true | 显示 "是" |
| RC-71 | backflush_triggered=false | 显示 "否" |

### 3.6 确认入库（Draft → Confirmed）

| ID | 测试项 | 预期结果 |
|----|--------|---------|
| RC-80 | 按钮可见性 | 状态=Draft 时显示"确认入库" (primary) |
| RC-81 | 确认操作 | hx-post /admin/mes/receipts/{id}/confirm |
| RC-82 | 确认成功 | HX-Redirect 回详情，状态变"已确认" |
| RC-83 | 确认后按钮 | 状态=Confirmed | "确认入库"按钮消失 |
| RC-84 | 非 Draft 状态 | — | 不显示"确认入库"按钮 |
| RC-85 | 倒冲触发 | 确认入库后 | 检查 backflush_triggered 是否变为 true |

### 3.7 完整状态流转

```
Draft → Confirmed
  ↘
   Cancelled (如果服务端支持)
```

## 4. 与原型设计对比

| 对比项 | 原型 | 实现 | 差异 |
|--------|------|------|------|
| 列表查询+分页 | 有 | ⚠️ stub | 未实现数据查询 |
| 创建表单 | 有工单搜索+仓库选择下拉 | ⚠️ 手动输入 ID | 无搜索/选择器 |
| 创建表单 remark | 有备注输入 | ⚠️ UI 缺失 | form 结构有 remark 但无输入框 |
| 详情页倒冲信息 | 有 | ✅ 显示 backflush_triggered | — |
| 确认入库触发倒冲 | 原型设计有 | ⚠️ 待验证 | 需确认 confirm 是否触发 WMS 倒冲 |
