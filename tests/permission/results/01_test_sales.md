# 测试结果：test_sales（销售经理）

> 测试时间：2026-06-10
> 权限：CUSTOMER/SALES_ORDER/SHIPPING CRUD + PRODUCT/CATEGORY/PRICE read

## 1. 菜单可见性

| 用例 ID | 预期 | 实际 | 结果 |
|---------|------|------|------|
| TP-SAL-MENU-01 | 仅显示「销售管理」和「主数据」 | | |
| TP-SAL-MENU-02 | 销售管理子菜单 7 项 | | |
| TP-SAL-MENU-03 | 主数据显示产品管理、产品分类 | | |
| TP-SAL-MENU-04 | 不显示「采购管理」 | | |
| TP-SAL-MENU-05 | 不显示「库存管理」 | | |
| TP-SAL-MENU-06 | 不显示「生产管理」 | | |
| TP-SAL-MENU-07 | 不显示「委外管理」 | | |
| TP-SAL-MENU-08 | 不显示「质量管理」 | | |
| TP-SAL-MENU-09 | 不显示「财务管理」 | | |
| TP-SAL-MENU-10 | 不显示「系统管理」 | | |

## 2. 销售管理页面

| 用例 ID | 预期 | 实际 | 结果 |
|---------|------|------|------|
| TP-SAL-SALES-01 | 销售总览加载正常 | | |
| TP-SAL-SALES-02 | 客户列表加载正常 | | |
| TP-SAL-SALES-03 | 显示新增客户按钮 | | |
| TP-SAL-SALES-04 | 可创建客户 | | |
| TP-SAL-SALES-05 | 可编辑客户 | | |
| TP-SAL-SALES-06 | 可删除客户 | | |
| TP-SAL-SALES-07 | 报价单列表加载正常 | | |
| TP-SAL-SALES-08 | 显示新增报价单按钮 | | |
| TP-SAL-SALES-09 | 销售订单列表加载正常 | | |
| TP-SAL-SALES-10 | 显示新增订单按钮 | | |
| TP-SAL-SALES-11 | 发货申请列表加载正常 | | |
| TP-SAL-SALES-12 | 显示新增发货申请按钮 | | |
| TP-SAL-SALES-13 | 销售退货列表加载正常 | | |
| TP-SAL-SALES-14 | 月对账单列表加载正常 | | |

## 3. 主数据页面（部分权限）

| 用例 ID | 预期 | 实际 | 结果 |
|---------|------|------|------|
| TP-SAL-MD-01 | 产品列表加载正常 | | |
| TP-SAL-MD-02 | 不显示创建按钮 | | |
| TP-SAL-MD-03 | 不显示编辑按钮 | | |
| TP-SAL-MD-04 | 不显示删除按钮 | | |
| TP-SAL-MD-05 | 产品分类加载正常 | | |
| TP-SAL-MD-06 | BOM管理 403 | | |
| TP-SAL-MD-07 | 供应商 403 | | |

## 4. 越权访问测试

| 用例 ID | 预期 | 实际 | 结果 |
|---------|------|------|------|
| TP-SAL-SEC-01 | /admin/system/users → 403 | | |
| TP-SAL-SEC-02 | /admin/wms/warehouses → 403 | | |
| TP-SAL-SEC-03 | /admin/purchase/orders → 403 | | |
| TP-SAL-SEC-04 | /admin/mes/orders → 403 | | |
| TP-SAL-SEC-05 | /admin/system/permissions → 403 | | |

## 缺陷清单

| # | 严重程度 | 用例 ID | 描述 | 截图 |
|---|----------|---------|------|------|
