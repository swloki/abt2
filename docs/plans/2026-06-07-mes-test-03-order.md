# MES-03 工单管理

> 路由前缀: `/admin/mes/orders`
> 代码文件: `mes_order_list.rs`, `mes_order_create.rs`, `mes_order_detail.rs`
> 原型文件: `04-order-list.html`, `04-order-create.html`, `04-order-detail.html`

## 1. 工单列表页

### 1.1 页面访问

| ID | 测试项 | 操作 | 预期结果 |
|----|--------|------|---------|
| WO-01 | 直接访问 | GET /admin/mes/orders | 200，标题 "工单管理" |
| WO-02 | 侧栏导航 | 点击"工单管理" | 跳转 /admin/mes/orders |
| WO-03 | Dashboard 入口 | 点击"工单管理"快捷卡片 | 跳转 /admin/mes/orders |

### 1.2 页面头部

| ID | 测试项 | 预期结果 |
|----|--------|---------|
| WO-10 | 页面标题 | "工单管理" |
| WO-11 | 新建按钮 | 右侧 "新建工单" 按钮 (带 plus icon)，链接 /admin/mes/orders/create |

### 1.3 状态 Tab 栏

`status_tabs_with_param` 组件，点击通过 HTMX 刷新 `#order-data-card`。

| Tab | 值 | 标签 | count |
|-----|----|------|-------|
| 全部 | (空) | 全部 | 总数 |
| Draft | Draft | 待计划 | None |
| Planned | Planned | 已计划 | None |
| Released | Released | 已下达 | None |
| Closed | Closed | 已关闭 | None |

### 1.4 搜索框

| ID | 测试项 | 预期结果 |
|----|--------|---------|
| WO-20 | 搜索框 | search-wrap 样式，带放大镜图标 |
| WO-21 | placeholder | "搜索工单编号…" |
| WO-22 | HTMX 触发 | keyup changed delay:300ms，hx-get=OrderTablePath，hx-target="#order-data-card" |
| WO-23 | 输入关键词 | 输入后 300ms 触发搜索 |

### 1.5 数据表格

表头：

| 列 | 对齐 | 说明 |
|----|------|------|
| 工单编号 | 左/mono/蓝色 | 点击跳转详情 |
| 产品ID | 左 | — |
| 计划数量 | 右/mono | — |
| 开始日期 | 左 | — |
| 结束日期 | 左 | — |
| 状态 | 左 | 颜色 pill |
| 创建人 | 左 | 解析为用户名 |
| 创建时间 | 左/12px/灰色 | YYYY-MM-DD HH:mm |
| 操作 | 左 | "查看"链接 |

### 1.6 列表交互测试

| ID | 测试项 | 操作 | 预期结果 |
|----|--------|------|---------|
| WO-30 | 行点击 | 点击表格行 | 跳转 /admin/mes/orders/{id} (cursor:pointer + onclick) |
| WO-31 | 查看链接 | 点击"查看" | 跳转详情 |
| WO-32 | 空数据 | 无工单 | 显示"暂无工单" |
| WO-33 | Tab 切换 | 点击"已下达" | 表格只显示 Released 状态 |
| WO-34 | 搜索+Tab | 先选 Tab 再搜索 | 两个条件 AND 生效 |
| WO-35 | 分页 | 数据 > 20 条 | 显示 pagination 组件 |
| WO-36 | 工单编号样式 | — | mono 字体，蓝色 (var(--accent)) |
| WO-37 | 创建人显示 | — | 显示 display_name，非 ID |

---

## 2. 新建工单页

### 2.1 页面访问

| ID | 测试项 | 操作 | 预期结果 |
|----|--------|------|---------|
| WO-40 | 从列表进入 | 点击"新建工单" | 跳转 /admin/mes/orders/create |
| WO-41 | 页面标题 | — | "新建工单" |
| WO-42 | 返回链接 | "← 返回列表" | 跳回 /admin/mes/orders |

### 2.2 表单字段

"基本信息" form-section + form-grid 布局：

| 字段 | 控件 | name | 必填 | 说明 |
|------|------|------|------|------|
| 产品ID | number input | product_id | ✅ | — |
| 计划数量 | number input | planned_qty | ✅ | step=0.01 |
| 开始日期 | date input | scheduled_start | ✅ | — |
| 结束日期 | date input | scheduled_end | ✅ | — |
| 工作中心ID | number input | work_center_id | — | 可选 |
| 备注 | textarea | remark | — | rows=2, span-2 |

### 2.3 提交测试

| ID | 测试项 | 操作 | 预期结果 |
|----|--------|------|---------|
| WO-50 | 空提交 | 不填直接提交 | HTML5 校验阻止 |
| WO-51 | 有效提交 | 填写所有必填项 | hx-post → HX-Redirect 到列表 |
| WO-52 | 无效产品ID | 输入非数字 | 服务端返回"无效产品ID" |
| WO-53 | 无效数量 | 输入非数字 | 服务端返回"无效数量" |
| WO-54 | 取消 | 点击"取消" | 跳回列表 |
| WO-55 | 不带计划行 | plan_item_id=None | 工单独立创建，不关联计划 |

---

## 3. 工单详情页

### 3.1 页面访问

| ID | 测试项 | 操作 | 预期结果 |
|----|--------|------|---------|
| WO-60 | 从列表进入 | 点击工单行 | 跳转 /admin/mes/orders/{id} |
| WO-61 | 返回链接 | "返回工单列表" | 跳回 /admin/mes/orders |
| WO-62 | 无效 ID | /admin/mes/orders/999999 | 错误页 |

### 3.2 详情头部

| ID | 元素 | 说明 |
|----|------|------|
| WO-63 | 工单编号 | detail-no class, mono 大号 |
| WO-64 | 状态 pill | 与列表一致 |

### 3.3 工单信息卡片

info-card "工单信息" + info-grid：

| 字段 | 样式 |
|------|------|
| 工单编号 | mono |
| 产品ID | mono |
| 计划数量 | mono |
| 计划开始日期 | mono |
| 计划结束日期 | mono |
| 状态 | 颜色 pill |
| 版本 | mono (乐观锁版本号) |
| 创建时间 | mono, 12px |

### 3.4 备注区域

| ID | 测试项 | 预期结果 |
|----|--------|---------|
| WO-70 | 有备注 | 显示备注卡片 |
| WO-71 | 无备注 | 不显示 |

### 3.5 状态操作按钮

#### 3.5.1 下达工单（Planned → Released）

| ID | 测试项 | 预期结果 |
|----|--------|---------|
| WO-80 | 按钮可见性 | 状态=Planned 时显示"下达工单" (primary, rocket icon) |
| WO-81 | 确认弹窗 | "确认下达此工单？下达后将开始生产。" |
| WO-82 | 下达操作 | hx-post /admin/mes/orders/{id}/release |
| WO-83 | 下达成功 | HX-Redirect 回详情，状态变"已下达" |
| WO-84 | 版本校验 | 服务端使用 expected_version 乐观锁 | 

#### 3.5.2 关闭工单（Released → Closed）

| ID | 测试项 | 预期结果 |
|----|--------|---------|
| WO-90 | 按钮可见性 | 状态=Released 时显示"关闭工单" (default, check_circle icon) |
| WO-91 | 确认弹窗 | "确认关闭此工单？" |
| WO-92 | 关闭操作 | hx-post /admin/mes/orders/{id}/close |
| WO-93 | 关闭成功 | 状态变"已关闭" |
| WO-94 | 关闭后按钮 | 所有操作按钮消失 |

#### 3.5.3 取消工单（Planned/Released → Cancelled）

| ID | 测试项 | 预期结果 |
|----|--------|---------|
| WO-100 | 按钮可见性 | 状态=Planned 或 Released 时显示"取消工单" (danger, x icon) |
| WO-101 | 确认弹窗 | "确认取消此工单？取消后不可恢复。" |
| WO-102 | 取消操作 | hx-post /admin/mes/orders/{id}/cancel |
| WO-103 | 取消成功 | 状态变"已取消"，所有按钮消失 |
| WO-104 | 已关闭/已取消 | 状态=Closed 或 Cancelled | 不显示任何操作按钮 |

### 3.6 完整状态流转

```
Draft → Planned → Released → Closed
              ↘          ↘
               ↠ Cancelled ↠
```

## 4. 与原型设计对比

| 对比项 | 原型 | 实现 | 差异 |
|--------|------|------|------|
| 列表 Tab | 全部/待计划/已计划/已下达/已关闭 | ✅ 一致 | — |
| 搜索框 | 搜索工单编号 | ✅ 一致 | — |
| 创建表单 | 产品选择器 | ⚠️ 手动输入产品 ID | 原型有产品搜索下拉 |
| 详情工序表 | 原型有工序列表 | ❌ 未实现 | 详情页无工单工序表 |
| 详情批次列表 | 原型有批次列表 | ❌ 未实现 | 详情页无批次列表 |
| 下达时自动生成批次 | 原型设计有 | ⚠️ 待验证 | release 操作是否自动创建批次 |
