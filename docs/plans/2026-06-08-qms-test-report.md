# 质量管理 (QMS) 模块测试报告

**测试日期**: 2026-06-08
**测试范围**: QMS 质量管理模块（13 个页面）
**测试数据**: `scripts/qms-insert-test-data.js` + `scripts/qms-test-data.sql`（8 specs, 10 results, 5 MRBs, 4 RMAs）

## 测试总览

| 页面 | 路径 | 状态 | 备注 |
|------|------|------|------|
| 质量管理总览 | /admin/qms | ✅ | 4 个快捷入口卡片正常 |
| 检验规格列表 | /admin/qms/specs | ✅ | Status tabs + filter + 数据 + 分页 |
| 新建检验规格 | /admin/qms/specs/create | ✅ | 表单完整，动态检验项目表 |
| 检验规格详情 | /admin/qms/specs/{id} | ✅ | 基本信息 + 抽样方案 + 检验项目表格 |
| 检验结果列表 | /admin/qms/results | ✅ | Status tabs + 筛选 + 数据 + 分页 |
| 记录检验结果 | /admin/qms/results/create | ✅ | 表单完整 |
| 检验结果详情 | /admin/qms/results/{id} | ✅ | 基本信息 + 抽样结果 + 检验项目结果 |
| MRB不良评审列表 | /admin/qms/mrb | ✅ | Status tabs + 筛选 + 数据 + 分页 |
| 新建MRB评审 | /admin/qms/mrb/create | ✅ | 表单完整 |
| MRB评审详情 | /admin/qms/mrb/{id} | ✅ | 基本信息 + 缺陷描述 + 备注 |
| RMA客诉追溯列表 | /admin/qms/rma | ✅ | Status tabs + 筛选 + 数据 + 分页 |
| 新建RMA | /admin/qms/rma/create | ✅ | 表单完整 |
| RMA详情 | /admin/qms/rma/{id} | ✅ | 基本信息 + 缺陷描述 + 根因分析 |

## 缺陷记录

### P2 一般

| # | 问题 | 修复 | 文件 |
|---|------|------|------|
| 1 | `CheckItem` 反序列化失败 — `tolerance`/`method` 字段非 Option，缺失时 JSON 解析报错导致 `unwrap_or_default()` 返回空 Vec | 已修复 — 给 `CheckItem` 4 个字段添加 `#[serde(default)]` | `abt-core/src/qms/inspection_specification/model.rs` |
| 2 | QMS sidebar 模块图标缺失 — `render_module_icon` 缺少 `"quality"` case | 已修复 — 添加 `"quality" => icon::check_circle_icon("")` | `abt-web/src/layout/sidebar.rs` |
| 3 | Create handler 缺少 Service trait import — 3 个 create 页面的 `svc.create()` 无法在 opaque type 上调用 | 已修复 — 添加 `use ...InspectionResultService;` 等 trait import | `qms_result_create.rs`, `qms_mrb_create.rs`, `qms_rma_create.rs` |

### P3 轻微

| # | 问题 | 状态 |
|---|------|------|
| 1 | Dashboard 快捷卡片使用内联 style | 已知 — stat-card CSS 不够用，可后续优化 |
| 2 | Spec create / Result create 的 select 选项（产品、规格）为空列表 | ✅ 已修复 — get_create handler 已正确加载产品和规格列表 |
## 数据验证结果

### 检验规格列表
- ✅ 8 条记录全部显示
- ✅ 产品名正确解析（非 ID）
- ✅ 检验类型标签：IQC（蓝）、IPQC（绿）、FQC（紫）、OQC（橙）
- ✅ 检验项目数正确：5项、3项、4项、5项、3项、1项、1项、3项
- ✅ 抽样方案格式：Level X, AQL Y
- ✅ 状态标签：生效（绿）、草稿（灰）、停用（红）
- ✅ Status tabs：全部(8)、草稿、生效、停用

### 检验结果列表
- ✅ 10 条记录显示
- ✅ 来源类型正确：来料通知、工单工序、发货单
- ✅ 抽样/合格/不合格格式正确
- ✅ 结果标签：合格（绿）、不合格（红）、让步接收（蓝）
- ✅ 状态标签：待检验、已完成、已处置

### MRB列表
- ✅ 5 条记录
- ✅ 关联检验单号正确显示
- ✅ 处置方式标签：返工（蓝）、退货（橙）、降级（紫）
- ✅ 责任方标签：内部（绿）、供应商（蓝）
- ✅ 状态标签：草稿、审批中、已批准、已完成

### RMA列表
- ✅ 4 条记录
- ✅ 客户名正确解析
- ✅ 严重程度标签：Minor（绿）、Major（橙）、Critical（红）
- ✅ 状态标签：已报告、调查中、已采取措施、已关闭

## 基础设施验证

| 项目 | 状态 |
|------|------|
| Sidebar 导航（质量模块 + 5 个子菜单） | ✅ |
| 模块图标 | ✅ (check_circle_icon) |
| State.rs 工厂方法（4个QMS service） | ✅ |
| 路由注册（13 页面 + 4 table 局部刷新） | ✅ |
| 编译（cargo check 0 errors） | ✅ |


## 表单提交测试

**测试日期**: 2026-06-08
**测试方法**: agent-browser + `eval` 触发 HTMX 提交

### 检验规格创建 `/admin/qms/specs/create`
- ✅ 产品下拉有 200+ 产品选项
- ✅ 检验类型 radio：IQC(默认) / IPQC / FQC / OQC
- ✅ 检验项目动态表格（3 行默认，可添加/删除行）
- ✅ 抽样方案：Level / AQL / 模式 三个下拉
- ✅ 保存草稿按钮（type=button，不触发表单提交）
- ✅ 提交审核按钮 → hx-post → 成功插入数据库 → HX-Redirect 跳转列表页
- ✅ JS `htmx:beforeRequest` 事件正确收集 check_items 并写入隐藏字段
- 验证：新记录 id=11 插入成功，product_id、inspection_type、check_items、sample_plan 数据正确

### 检验结果创建 `/admin/qms/results/create`
- ✅ 检验规格下拉：6 个 Active 状态的规格
- ✅ 来源类型下拉：来料通知、工单工序、发货单、委外单
- ✅ 来源单号、批次号、抽样数量必填输入框
- ✅ 检验结论下拉：合格、不合格、让步接收
- ✅ 合格/不合格数量输入框
- ⚠️ **检验员下拉为空** — 只有"请选择检验员"占位项（P3）
- ✅ 日期选择器（type=date）
- ✅ 5 行检验项目表格（每行：项目、标准、实测值、合格/不合格、备注）
- ✅ 提交 → 成功插入数据库 → 跳转列表页
- 验证：新记录 id=29 插入成功（source_type=1, source_id=999, batch_no=TEST-FORM-SUBMIT）
- 注意：日期为空时 `record_result` 不被调用，只创建 Pending 状态记录

### MRB评审创建 `/admin/qms/mrb/create`
- ✅ 检验结果下拉：3 条不合格+已完成的检验结果
- ✅ 产品下拉有完整产品列表
- ✅ 缺陷描述必填文本框
- ✅ 处置方式下拉：报废、退货、降级、返工
- ✅ 责任方下拉：内部、供应商、客户
- ✅ 费用影响数值输入框
- ✅ 备注文本框
- ✅ 提交审批 → 成功插入数据库 → 跳转列表页
- 验证：新记录 id=7 插入成功（inspection_result_id=5, disposition=4=返工, responsible_party=1=内部）

### RMA客诉创建 `/admin/qms/rma/create`
- ✅ 客户下拉有 4 个客户
- ✅ 销售订单下拉（可选，当前为空列表）
- ✅ 发货单下拉（可选，当前为空列表）
- ✅ 产品下拉有完整产品列表
- ✅ 检验结果下拉（可选，当前为空列表）
- ✅ 缺陷描述必填文本框
- ✅ 严重程度下拉：轻微 Minor、一般 Major、严重 Critical
- ✅ 备注文本框
- ✅ 提交 → 成功插入数据库 → 跳转列表页
- 验证：新记录 id=6 插入成功（customer_id=7, product_id=13200, severity=2=Major）

### 表单提交测试缺陷记录

#### P2 一般

| # | 问题 | 修复 | 文件 |
|---|------|------|------|
| 4 | Result create POST 返回 500 — 幂等唯一索引 `idx_inspection_results_idempotent (source_type, source_id, inspection_type)` 与测试数据冲突 | 非代码 bug — 测试数据已存在相同组合，改用 source_id=999 成功提交 | — |

#### P3 轻微

| # | 问题 | 状态 |
|---|------|------|
| 3 | 检验员下拉为空 — `inspector_id` select 无选项 | ✅ 已修复 — `qms_result_create.rs` 加载 UserService.list_users 填充下拉（Admin/Zhang San/Li Si/Wang Wu） |
| 4 | 销售订单/发货单下拉为空（RMA create） | ✅ 已修复 — `qms_rma_create.rs` 加载 SalesOrderService + ShippingRequestService 填充下拉 |
| 5 | `agent-browser click` 对 HTMX hx-post 表单提交不生效 | 测试工具限制 — 需使用 `eval` 调用 `.click()` 才能触发 HTMX 提交 |