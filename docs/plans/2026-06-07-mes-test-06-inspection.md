# MES-06 生产报检

> 路由前缀: `/admin/mes/inspections`
> 代码文件: `mes_inspection_list.rs`, `mes_inspection_create.rs`, `mes_inspection_detail.rs`
> 原型文件: `04-inspection-list.html`, `04-inspection-create.html`, `04-inspection-detail.html`

## 1. 检验列表页

### 1.1 页面访问

| ID | 测试项 | 操作 | 预期结果 |
|----|--------|------|---------|
| INSP-01 | 直接访问 | GET /admin/mes/inspections | 200，标题 "生产报检" |
| INSP-02 | 侧栏导航 | 点击"生产报检" | 跳转 /admin/mes/inspections |
| INSP-03 | Dashboard 入口 | 点击"生产报检"快捷卡片 | 跳转 /admin/mes/inspections |

### 1.2 页面头部

| ID | 测试项 | 预期结果 |
|----|--------|---------|
| INSP-10 | 页面标题 | "生产报检" |
| INSP-11 | 新建按钮 | 右侧 "新建检验" 按钮，链接 /admin/mes/inspections/create |

### 1.3 检验类型 Tab 栏

`status_tabs_with_param` 组件：

| Tab | 值 | 标签 | count |
|-----|----|------|-------|
| 全部 | (空) | 全部 | 0 |
| FirstArticle | FirstArticle | 首检 | None |
| InProcess | InProcess | 巡检 | None |
| Final | Final | 完工检 | None |

### 1.4 Tab 交互测试

| ID | 测试项 | 操作 | 预期结果 |
|----|--------|------|---------|
| INSP-20 | 点击"首检" | 点击 Tab | HTMX 请求 InspectionTablePath，参数 inspection_type=FirstArticle |
| INSP-21 | 点击"巡检" | 点击 Tab | 参数 inspection_type=InProcess |
| INSP-22 | 点击"完工检" | 点击 Tab | 参数 inspection_type=Final |
| INSP-23 | 全部 Tab 高亮 | 默认状态 | "全部" Tab 有 active 样式 |

### 1.5 数据表格（当前为 Stub）

> **注意**: 列表和 table endpoint 都是 stub。

表头：

| 列 | 对齐 |
|----|------|
| 单号 | 左 |
| 工单 | 左 |
| 类型 | 左 |
| 产品ID | 左 |
| 样本 | 右 |
| 合格 | 右 |
| 结果 | 左 |
| 操作 | 左 |

### 1.6 Stub 状态测试

| ID | 测试项 | 预期结果 |
|----|--------|---------|
| INSP-30 | 空数据提示 | "暂无检验记录" |
| INSP-31 | Table endpoint | GET /admin/mes/inspections/table | 返回 "暂无数据" |

---

## 2. 新建检验页

### 2.1 页面访问

| ID | 测试项 | 操作 | 预期结果 |
|----|--------|------|---------|
| INSP-40 | 从列表进入 | 点击"新建检验" | 跳转 /admin/mes/inspections/create |
| INSP-41 | 页面标题 | — | "新建检验" |
| INSP-42 | 返回链接 | "← 返回列表" | 跳回 /admin/mes/inspections |

### 2.2 表单字段

"检验信息" form-section + form-grid：

| 字段 | 控件 | name | 必填 | 选项/说明 |
|------|------|------|------|----------|
| 工单ID | number | work_order_id | ✅ | — |
| 产品ID | number | product_id | ✅ | — |
| 工序ID | number | routing_id | — | 可选 |
| 检验类型 | select | inspection_type | — | 首检(1) / 巡检(2) / 完工检(3) |
| 样本数量 | number | sample_qty | ✅ | step=0.01 |
| 检验日期 | date | inspection_date | ✅ | — |
| 处置意见 | text | disposition | — | span-2 |

> **注意**: 表单没有 remark 字段，但 `CreateInspectionReq` 包含 remark。UI 缺少备注输入框。

### 2.3 提交测试

| ID | 测试项 | 操作 | 预期结果 |
|----|--------|------|---------|
| INSP-50 | 空提交 | 不填直接提交 | HTML5 校验阻止 |
| INSP-51 | 有效提交-首检 | 选择首检+填写必填项 | hx-post → HX-Redirect 到列表 |
| INSP-52 | 有效提交-巡检 | 选择巡检 | 同上 |
| INSP-53 | 有效提交-完工检 | 选择完工检 | 同上 |
| INSP-54 | 无效检验类型 | 修改 value 为不存在的值 | 服务端返回"无效检验类型" |
| INSP-55 | 取消 | 点击"取消" | 跳回列表 |

### 2.4 InspectionType 枚举映射

| 表单 value | 枚举值 | 中文 |
|------------|--------|------|
| 1 | FirstArticle | 首检 |
| 2 | InProcess | 巡检 |
| 3 | Final | 完工检 |

---

## 3. 检验详情页

### 3.1 页面访问

| ID | 测试项 | 操作 | 预期结果 |
|----|--------|------|---------|
| INSP-60 | 直接访问 | GET /admin/mes/inspections/{id} | 200，标题 "检验 {doc_number}" |
| INSP-61 | 返回链接 | "← 返回列表" | 跳回 /admin/mes/inspections |
| INSP-62 | 无效 ID | /admin/mes/inspections/999999 | 错误页 |

### 3.3 详情信息卡片

info-grid 布局：

| 字段 | 样式 | 说明 |
|------|------|------|
| 单号 | mono | doc_number |
| 工单ID | — | — |
| 产品ID | — | — |
| 检验类型 | — | 首检/巡检/完工检 |
| 样本数量 | mono | sample_qty |
| 合格数量 | mono | qualified_qty |
| 不合格数量 | mono | unqualified_qty |
| 结果 | 颜色 pill | 合格(绿)/不合格(红)/让步接收(橙) |
| 检验员 | — | inspector_id |
| 检验日期 | — | inspection_date |

### 3.4 检验结果颜色测试

| ID | 测试项 | 预期结果 |
|----|--------|---------|
| INSP-70 | Pass (合格) | 绿色背景 rgba(82,196,26,0.08)，绿色文字 |
| INSP-71 | Fail (不合格) | 红色背景 rgba(245,63,63,0.06)，红色文字 |
| INSP-72 | Conditional (让步接收) | 橙色背景 rgba(250,140,22,0.08)，橙色文字 |

### 3.5 记录检验结果表单

在详情页底部有"记录检验结果"区域：

| 字段 | 控件 | name | 选项 |
|------|------|------|------|
| 结果 | select | result | 合格(1) / 不合格(2) / 让步接收(3) |

提交按钮："提交" (primary)，hx-post /admin/mes/inspections/{id}/record-result

### 3.6 记录结果交互测试

| ID | 测试项 | 操作 | 预期结果 |
|----|--------|------|---------|
| INSP-80 | 选择"合格"并提交 | — | hx-post → HX-Redirect 回详情，结果变"合格" |
| INSP-81 | 选择"不合格"并提交 | — | 结果变"不合格" |
| INSP-82 | 选择"让步接收"并提交 | — | 结果变"让步接收" |
| INSP-83 | 无效结果值 | 手动修改 value | 服务端返回"无效检验结果" |
| INSP-84 | select 宽度 | — | width=200px, inline-block |
| INSP-85 | 提交按钮间距 | — | margin-left=var(--space-3) |

## 4. 与原型设计对比

| 对比项 | 原型 | 实现 | 差异 |
|--------|------|------|------|
| 列表查询+分页 | 有 | ⚠️ stub | 未实现数据查询 |
| 检验类型 Tab | 首检/巡检/完工检 | ✅ 一致 | — |
| 创建表单 | 有工单搜索 | ⚠️ 手动输入 ID | 无搜索功能 |
| 详情页检验结果 | 有结果录入 | ✅ 有 select 录入 | — |
| 创建时 remark | 原型有备注 | ⚠️ UI 缺失 | CreateInspectionReq 有 remark 但表单无输入 |
| disposition 处置意见 | 原型有 | ✅ 有 | — |
| 检验员选择 | 原型有下拉 | ⚠️ 自动取当前用户 | inspector_id 由服务端设置 |
