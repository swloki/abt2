---
title: "feat: 完成采购模块所有前端页面功能"
type: feat
status: draft
date: 2026-06-04
origin: 原型设计 02-*.html（采购模块）
---

# feat: 完成采购模块所有前端页面功能

## Summary

对照原型设计文件（`02-*.html`），逐页审查已实现的采购前端页面，补齐所有缺失功能，确保与原型设计完全对齐。

**采购模块前端页面清单（原型设计 19 个页面）：**

| 页面 | 原型文件 | 已有代码文件 | 状态 |
|------|----------|-------------|------|
| 采购总览 | `02-index.html` | `purchase_dashboard.rs` | ✅ 已实现 |
| 采购报价列表 | `02-quotation-list.html` | `purchase_quotation_list.rs` | ✅ 已实现 |
| 新建采购报价 | `02-quotation-create.html` | `purchase_quotation_create.rs` | ✅ 已实现 |
| 报价详情 | `02-quotation-detail.html` | `purchase_quotation_detail.rs` | ✅ 已实现 |
| 采购订单列表 | `02-order-list.html` | `purchase_order_list.rs` | ✅ 已实现 |
| 新建采购订单 | `02-order-create.html` | `purchase_order_create.rs` | ✅ 已实现 |
| 订单详情 | `02-order-detail.html` | `purchase_order_detail.rs` | ✅ 已实现 |
| 采购退货列表 | `02-return-list.html` | `purchase_return_list.rs` | ✅ 已实现 |
| 新建采购退货 | `02-return-create.html` | `purchase_return_create.rs` | ✅ 已实现 |
| 退货详情 | `02-return-detail.html` | `purchase_return_detail.rs` | ✅ 已实现 |
| 对账单列表 | `02-reconciliation-list.html` | `purchase_recon_list.rs` | ✅ 已实现 |
| 新建对账单 | `02-reconciliation-create.html` | `purchase_recon_create.rs` | ✅ 已实现 |
| 对账单详情 | `02-reconciliation-detail.html` | `purchase_recon_detail.rs` | ✅ 已实现 |
| 付款申请列表 | `02-payment-list.html` | `payment_request_list.rs` | ✅ 已实现 |
| 新建付款申请 | `02-payment-create.html` | `payment_request_create.rs` | ✅ 已实现 |
| 付款详情 | `02-payment-detail.html` | `payment_request_detail.rs` | ✅ 已实现 |
| 零星请购列表 | `02-misc-list.html` | `misc_request_list.rs` | ✅ 已实现 |
| 新建零星请购 | `02-misc-create.html` | `misc_request_create.rs` | ✅ 已实现 |
| 零星请购详情 | `02-misc-detail.html` | `misc_request_detail.rs` | ✅ 已实现 |

**后端服务层（abt-core）：** 全部 6 个 Service 已实现（quotation/order/return/reconciliation/payment/misc_request），migration、enums、models、repos 均已完成。

**前后端对接（abt-web）：** `state.rs` 已注册全部 7 个采购 service getter，`routes/mod.rs` 已注册全部采购路由。

---

## Problem Frame

所有 19 个采购页面的基础框架已存在，但需要逐一对比原型设计，确认每个页面的交互功能是否完整。重点检查：

1. **列表页**：状态 Tab 筛选、搜索框、下拉筛选器、分页是否齐全
2. **创建页**：表单字段、供应商联动填充、行项动态增删、金额自动计算、草稿/提交按钮
3. **详情页**：工作流步骤条、状态操作按钮（确认/取消/审批）、关联单据链接、行项明细展示

---

## Scope Boundaries

- **范围内**：原型设计 `02-*.html` 中所有采购页面的前端功能完善
- **范围外**：后端 Service 层修改（已全部实现）、新建页面文件（已全部存在）、非采购模块页面

---

## Implementation Units

### U1. 审查采购报价（Quotation）三页面

**Goal:** 对比 `02-quotation-list.html`、`02-quotation-create.html`、`02-quotation-detail.html` 原型，检查 `purchase_quotation_list.rs`、`purchase_quotation_create.rs`、`purchase_quotation_detail.rs` 功能完整性

**检查要点：**

**列表页** (`purchase_quotation_list.rs`)：
- [x] 状态 Tab：全部 / 草稿 / 生效中 / 已过期 / 已取消
- [x] 搜索框：搜索报价单号、供应商名称
- [x] 供应商下拉筛选
- [x] 报价日期范围筛选
- [x] 表格列：报价单号、供应商名称、联系人、状态、报价日期、有效期至、币种、操作
- [x] 操作列：编辑、删除按钮
- [x] 分页

**创建页** (`purchase_quotation_create.rs`)：
- [x] 供应商信息区：供应商选择 → 联系人/电话/地址自动填充
- [x] 报价信息区：报价日期、有效期开始/结束、币种、采购员、备注
- [x] 报价产品明细表：行号、物料编码、物料名称、规格描述、单位、单价、最小起订量、交货天数、是否首选
- [x] 添加行 / 删除行
- [x] 底部汇总栏：报价项目数、首选供应商数
- [x] 保存草稿 / 提交报价 按钮

**详情页** (`purchase_quotation_detail.rs`)：
- [x] 工作流步骤条
- [x] 报价基础信息展示
- [x] 报价行项明细
- [x] 状态操作：生效（activate）、取消（cancel）

**可能的缺失功能（需确认）：**
- 联系人列在列表中展示 → 需从 Supplier 数据中获取联系人信息
- 编辑按钮跳转到编辑页 → 需确认是否有 quotation_edit 页面

---

### U2. 审查采购订单（Order）三页面

**Goal:** 对比 `02-order-list.html`、`02-order-create.html`、`02-order-detail.html` 原型

**检查要点：**

**列表页** (`purchase_order_list.rs`)：
- [x] 状态 Tab：全部 / 草稿 / 已确认 / 部分收货 / 已收货 / 已关闭 / 已取消
- [x] 搜索框
- [x] 供应商筛选
- [x] 订单日期筛选
- [x] 表格列：订单编号、供应商名称、订单日期、预计到货、状态、总金额、业务员、操作
- [x] 操作列：编辑、删除
- [x] 分页
- [x] 行可点击跳转详情

**创建页** (`purchase_order_create.rs`)：
- [x] 供应商信息区：选择 → 联系人/电话/地址自动填充 + 供应商信息栏
- [x] 订单信息区：订单日期、预计到货日期、付款条款、币种、交货地址、关联报价、采购员、备注
- [x] 采购产品明细表：行号、物料编码、物料名称、规格、单位、数量、单价、金额、预计到货
- [x] 金额自动计算（数量×单价）
- [x] 添加行 / 删除行
- [x] 底部汇总：订单项目数、订单总额
- [x] 保存草稿 / 提交订单

**详情页** (`purchase_order_detail.rs`)：
- [x] 工作流步骤条（Draft→Confirmed→PartiallyReceived→Received→Closed）
- [x] 订单基础信息展示
- [x] 订单行项明细（含收货量/检验量/退货量）
- [x] 状态操作：确认（confirm）、取消（cancel）

---

### U3. 审查采购退货（Return）三页面

**Goal:** 对比 `02-return-list.html`、`02-return-create.html`、`02-return-detail.html` 原型

**检查要点：**

**列表页** (`purchase_return_list.rs`)：
- [x] 状态 Tab：全部 / 草稿 / 已确认 / 已发货 / 已结算 / 已取消
- [x] 搜索框
- [x] 供应商筛选
- [x] 退货日期筛选
- [x] 表格列：退货单号、关联订单、供应商名称、退货日期、退货原因、状态、总金额、操作
- [x] 分页

**创建页** (`purchase_return_create.rs`)：
- [x] 选择关联采购订单
- [x] 供应商信息自动填充
- [x] 退货信息区：退货日期、退货原因、备注
- [x] 退货行项：从订单行项中选择，填写退货数量
- [x] 金额自动计算
- [x] 保存草稿 / 提交

**详情页** (`purchase_return_detail.rs`)：
- [x] 工作流步骤条（Draft→Confirmed→Shipped→Settled）
- [x] 退货基础信息 + 关联订单链接
- [x] 退货行项明细
- [x] 状态操作：确认（confirm）、取消（cancel）

---

### U4. 审查采购对账（Reconciliation）三页面

**Goal:** 对比 `02-reconciliation-list.html`、`02-reconciliation-create.html`、`02-reconciliation-detail.html` 原型

**检查要点：**

**列表页** (`purchase_recon_list.rs`)：
- [x] 状态 Tab：全部 / 草稿 / 已确认 / 已结算
- [x] 搜索框
- [x] 供应商筛选
- [x] 期间筛选
- [x] 表格列：对账单号、供应商名称、对账期间、状态、应付金额、确认金额、差异、操作
- [x] 分页

**创建页** (`purchase_recon_create.rs`)：
- [x] 选择供应商
- [x] 选择对账期间
- [x] 自动汇总该供应商+期间的入库明细
- [x] 订单选择弹窗（手动添加订单）
- [x] 行项：收货量、退货量、退货金额、单价、金额、是否确认
- [x] 底部汇总
- [x] 保存草稿 / 提交

**详情页** (`purchase_recon_detail.rs`)：
- [x] 对账单基础信息
- [x] 对账行项明细（含退货冲减）
- [x] 状态操作：确认（confirm）

**可能的缺失功能：**
- 对账创建页原型中有「订单选择弹窗」(order-picker) → 需确认实现中是否有

---

### U5. 审查付款申请（Payment）三页面

**Goal:** 对比 `02-payment-list.html`、`02-payment-create.html`、`02-payment-detail.html` 原型

**检查要点：**

**列表页** (`payment_request_list.rs`)：
- [x] 状态 Tab：全部 / 草稿 / 已审批 / 已付款 / 已取消
- [x] 搜索框
- [x] 供应商筛选
- [x] 付款日期筛选
- [x] 付款方式筛选
- [x] 表格列：付款单号、供应商名称、关联对账、付款日期、金额、付款方式、发票号、状态、操作
- [x] 分页

**创建页** (`payment_request_create.rs`)：
- [x] 选择供应商
- [x] 选择关联对账单
- [x] 付款信息：付款日期、金额、付款方式、银行账户
- [x] 发票信息：发票号、发票金额
- [x] 备注
- [x] 保存草稿 / 提交

**详情页** (`payment_request_detail.rs`)：
- [x] 付款申请基础信息
- [x] 关联对账单链接
- [x] 发票信息
- [x] 状态操作：审批（approve）、取消（cancel）

---

### U6. 审查零星请购（MiscRequest）三页面

**Goal:** 对比 `02-misc-list.html`、`02-misc-create.html`、`02-misc-detail.html` 原型

**检查要点：**

**列表页** (`misc_request_list.rs`)：
- [x] 状态 Tab：全部 / 草稿 / 已审批 / 采购中 / 已收货 / 已关闭 / 已取消
- [x] 搜索框
- [x] 部门筛选
- [x] 请购日期筛选
- [x] 表格列：请购单号、请购部门、请购日期、用途、总金额、状态、操作
- [x] 分页

**创建页** (`misc_request_create.rs`)：
- [x] 基本信息：请购部门、请购日期、用途、备注
- [x] 请购明细表：行号、品名、规格、数量、单位、预估单价、备注
- [x] 添加行 / 删除行
- [x] 金额自动计算
- [x] 底部汇总：项目数、总金额
- [x] 保存草稿 / 提交

**详情页** (`misc_request_detail.rs`)：
- [x] 工作流步骤条（Draft→Approved→Purchasing→Received→Closed）
- [x] 请购基础信息
- [x] 请购行项明细
- [x] 状态操作：审批（approve）、取消（cancel）

---

### U7. 审查采购总览（Dashboard）

**Goal:** 对比 `02-index.html` 原型

**检查要点：**
- [x] 统计卡片：活跃供应商数、待比价报价数、进行中订单数、待付款金额、退货处理中数
- [x] 采购业务流程图：供应商→采购报价→采购订单→采购对账→付款申请 + 退货/零星请购
- [x] 最近活动列表
- [x] 导出报表按钮（可占位）

---

## Execution Strategy

**审查顺序**：按业务流转顺序（总览 → 报价 → 订单 → 退货 → 对账 → 付款 → 零星请购）逐一审查每个页面代码与原型的对齐情况。

**每个页面的审查步骤**：
1. 在浏览器中打开原型 HTML 页面，确认所有交互元素
2. 对照 `abt-web/src/pages/` 中的 Rust 代码，检查每个功能点是否实现
3. 标记缺失的功能，创建修复任务
4. 逐个修复

**验证方式**：
- `cargo clippy` 编译通过
- 启动服务器后在浏览器中逐页操作验证

---

## Acceptance Criteria

1. 所有 19 个采购页面与原型设计完全对齐
2. 所有列表页的状态筛选、搜索、下拉筛选、分页功能齐全
3. 所有创建页的表单联动、行项增删、金额计算、草稿/提交流程完整
4. 所有详情页的工作流步骤条、状态操作按钮、行项明细完整
5. `cargo clippy` 无错误
6. 浏览器中每个页面可正常操作

---

## Key Technical Decisions

- **不创建新页面文件**：所有页面文件已存在，只做功能修补
- **不修改后端 Service 层**：后端已全部实现，只做前端适配
- **严格遵循组件化三原则**：`hx-target="this"` + `hx-vals` 状态随身 + `hx-indicator` 视觉闭环
- **样式通过 UnoCSS 统一管理**：不新增内联 style
