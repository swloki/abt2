# MES-04 生产批次 + 流转卡查询

> 批次路由前缀: `/admin/mes/batches`
> 流转卡路由: `/admin/mes/cards`
> 代码文件: `mes_batch_list.rs`, `mes_batch_detail.rs`, `mes_card_query.rs`
> 原型文件: `04-batch-list.html`, `04-batch-detail.html`, `04-card-query.html`

## 1. 批次列表页

### 1.1 页面访问

| ID | 测试项 | 操作 | 预期结果 |
|----|--------|------|---------|
| BT-01 | 直接访问 | GET /admin/mes/batches | 200，标题 "生产批次" |
| BT-02 | 侧栏导航 | 点击"生产批次" | 跳转 /admin/mes/batches |
| BT-03 | Dashboard 入口 | 点击"生产批次"快捷卡片 | 跳转 /admin/mes/batches |

### 1.2 状态 Tab 栏

| Tab | 值 | 标签 | count |
|-----|----|------|-------|
| 全部 | (空) | 全部 | 0 |
| Pending | Pending | 待生产 | None |
| InProgress | InProgress | 进行中 | None |
| PendingReceipt | PendingReceipt | 待入库 | None |
| Completed | Completed | 已完成 | None |

### 1.3 数据表格（当前为 Stub）

> **注意**: 当前列表页是 stub 实现，不从数据库查询，始终显示"暂无批次数据"。

表头结构已定义：

| 列 | 对齐 |
|----|------|
| 批次号 | 左 |
| 工单 | 左 |
| 产品ID | 左 |
| 数量 | 右 |
| 完成 | 右 |
| 当前工序 | 左 |
| 状态 | 左 |
| 操作 | 左 |

### 1.4 Stub 状态测试

| ID | 测试项 | 预期结果 |
|----|--------|---------|
| BT-10 | 空数据提示 | 显示"暂无批次数据"居中 |
| BT-11 | Tab 切换 | 点击不同 Tab，HTMX 请求但返回 stub |
| BT-12 | 表格结构 | thead 有 8 列 |

---

## 2. 批次详情页

### 2.1 页面访问

| ID | 测试项 | 操作 | 预期结果 |
|----|--------|------|---------|
| BT-20 | 直接访问 | GET /admin/mes/batches/{id} | 200，标题 "批次 {batch_no}" |
| BT-21 | 返回链接 | "← 返回列表" | 跳回 /admin/mes/batches |
| BT-22 | 无效 ID | /admin/mes/batches/999999 | 错误页 |

### 2.2 批次信息卡片

info-grid 布局：

| 字段 | 样式 | 说明 |
|------|------|------|
| 批次号 | mono | batch_no |
| 流转卡号 | mono | card_sn |
| 产品ID | — | — |
| 数量 | mono | batch_qty |
| 已完成 | mono | completed_qty |
| 报废 | mono | scrap_qty |
| 当前工序 | — | current_step (数字) |
| 状态 | 颜色 pill | 内联样式 |

### 2.3 条件显示的操作按钮

根据批次状态，header 区域显示不同操作：

#### InProgress 状态

| ID | 测试项 | 预期结果 |
|----|--------|---------|
| BT-30 | 暂停按钮 | 显示 "暂停" 按钮 (default) |
| BT-31 | 暂停操作 | hx-post /admin/mes/batches/{id}/suspend，隐藏字段 reason="手动暂停" |
| BT-32 | 暂停成功 | HX-Redirect 回详情，状态变 Suspended |

#### Suspended 状态

| ID | 测试项 | 预期结果 |
|----|--------|---------|
| BT-40 | 恢复按钮 | 显示 "恢复" 按钮 (primary) |
| BT-41 | 恢复操作 | hx-post /admin/mes/batches/{id}/resume |
| BT-42 | 恢复成功 | 状态变回 InProgress |

#### PendingReceipt 状态

| ID | 测试项 | 预期结果 |
|----|--------|---------|
| BT-50 | 推进入库按钮 | 显示 "推进入库" 按钮 (primary) |
| BT-51 | 推进操作 | hx-post /admin/mes/batches/{id}/advance |
| BT-52 | 推进成功 | 创建入库单，跳转 |

#### 其他状态

| ID | 测试项 | 预期结果 |
|----|--------|---------|
| BT-60 | Pending/Completed/Cancelled | 不显示任何操作按钮 |

### 2.4 报工表单

仅在 `Pending` 或 `InProgress` 状态下显示。

#### 表单字段

| 字段 | 控件 | name | 必填 | 默认值 | 说明 |
|------|------|------|------|--------|------|
| 工序号 | number | step_no | — | current_step + 1 | width=80px |
| 工人ID | number | worker_id | ✅ | — | — |
| 班次 | select | shift | — | 白班 | 白班(value=1) / 夜班(value=2) |
| 完成数量 | number | completed_qty | ✅ | — | step=0.01 |
| 不良数量 | number | defect_qty | — | 0 | step=0.01 |
| 工时 | number | work_hours | ✅ | — | step=0.01 |
| 报工日期 | date | report_date | ✅ | — | — |

#### 报工交互测试

| ID | 测试项 | 操作 | 预期结果 |
|----|--------|------|---------|
| BT-70 | 报工表单可见性 | InProgress 批次 | 显示报工表单 |
| BT-71 | 报工表单隐藏 | Completed 批次 | 不显示报工表单 |
| BT-72 | 工序号默认值 | — | current_step + 1 |
| BT-73 | 提交报工 | 填写完整信息 | hx-post /admin/mes/batches/{id}/confirm-step |
| BT-74 | 提交成功 | — | HX-Redirect 回详情页 |
| BT-75 | 空提交 | 不填工人ID | HTML5 校验阻止 |

### 2.5 ConfirmStepForm 结构

```rust
pub struct ConfirmStepForm {
    pub step_no: i32,
    pub worker_id: i64,
    pub shift: ShiftType,
    pub completed_qty: Decimal,
    pub defect_qty: Decimal,
    pub defect_reason: Option<DefectReason>,
    pub work_hours: Decimal,
    pub report_date: NaiveDate,
    pub remark: Option<String>,
}
```

> **注意**: 报工表单中 **没有** defect_reason 和 remark 的 UI 输入控件，但 ConfirmStepForm 包含这些字段。需确认是否遗漏 UI 或服务端会正确处理缺失字段。

### 2.6 报废操作

| ID | 测试项 | 预期结果 |
|----|--------|---------|
| BT-80 | 报废路由 | POST /admin/mes/batches/{id}/scrap |
| BT-81 | 报废表单 | SuspendForm { reason: String } |
| BT-82 | 报废成功 | 状态变更 |

---

## 3. 流转卡查询页

### 3.1 页面访问

| ID | 测试项 | 操作 | 预期结果 |
|----|--------|------|---------|
| CQ-01 | 直接访问 | GET /admin/mes/cards | 200，标题 "流转卡查询" |
| CQ-02 | 侧栏导航 | 点击"流转卡查询" | 跳转 /admin/mes/cards |

### 3.3 页面内容（当前为静态页面）

| 元素 | 说明 |
|------|------|
| 页面标题 | "流转卡查询" |
| 提示信息 | "请输入流转卡序列号进行查询" (居中，灰色) |
| 输入框 | "流转卡序列号"，placeholder="扫描或输入卡号…" |
| 表单布局 | max-width=400px 居中 |

### 3.4 测试要点

| ID | 测试项 | 预期结果 |
|----|--------|---------|
| CQ-10 | 输入框显示 | 显示流转卡序列号输入框 |
| CQ-11 | placeholder | "扫描或输入卡号…" |
| CQ-12 | 查询功能 | **未实现** — 输入后无查询逻辑 |
| CQ-13 | 布局 | 居中，最大宽度 400px |

## 4. 与原型设计对比

| 对比项 | 原型 | 实现 | 差异 |
|--------|------|------|------|
| 批次列表 | 有数据查询+分页+搜索 | ⚠️ stub 空数据 | 未实现列表查询 |
| 批次详情-工序进度 | 原型有工序进度条 | ❌ 未实现 | 只显示 current_step 数字 |
| 批次详情-报工记录 | 原型有报工历史列表 | ❌ 未实现 | 只显示内联报工表单 |
| 流转卡查询 | 原型有完整查询结果展示 | ⚠️ 静态输入框 | 无查询逻辑 |
| 报工表单 defect_reason | 确认表单有该字段 | ⚠️ UI 缺失 | 表单无不良原因选择 |
| 报工表单 remark | 确认表单有该字段 | ⚠️ UI 缺失 | 表单无备注输入 |
