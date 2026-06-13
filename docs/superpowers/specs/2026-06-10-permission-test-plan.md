# 权限系统测试计划

> 日期：2026-06-10
> 状态：待审核
> 范围：RBAC 权限系统全量功能测试 + 边界条件测试

## 1. 测试目标

验证 ABT 系统的 RBAC 权限控制在以下三个层面是否正确工作：

1. **菜单级**：侧边栏只显示用户有权限访问的菜单模块和菜单项
2. **按钮级**：页面内操作按钮（新增、编辑、删除等）根据权限动态显示/隐藏
3. **接口级**：直接访问无权限的 API 端点时，服务端正确拒绝（403）

同时验证权限继承、多角色合并、零权限等边界场景。

## 2. 测试方法

- **工具**：agent-browser 自动化浏览器测试
- **地址**：`https://localhost:8000/login`
- **策略**：先完整执行所有测试用例，记录问题，最后统一修复
- **方式**：通过 SQL 插入测试角色和用户 → 逐用户登录 → 逐页面验证

## 3. 测试环境准备

### 3.1 部门

| 部门名 | department_code | 说明 |
|--------|----------------|------|
| 销售部 | `SALES` | 销售团队 |
| 仓储部 | `WAREHOUSE_DEPT` | 仓库管理 |
| 生产部 | `PRODUCTION` | 生产制造 |
| 管理层 | `MANAGEMENT` | 高层管理（多角色测试用） |

### 3.2 业务角色

#### 销售经理（`sales_manager`）

| 资源 | 权限 |
|------|------|
| CUSTOMER | create, read, update, delete |
| SALES_ORDER | create, read, update, delete |
| SHIPPING | create, read, update, delete |
| PRODUCT | read |
| CATEGORY | read |
| PRICE | read |

#### 仓管员（`warehouse_keeper`）

| 资源 | 权限 |
|------|------|
| WAREHOUSE | create, read, update, delete |
| LOCATION | create, read, update, delete |
| INVENTORY | create, read, update, delete |
| PRODUCT | read |
| CATEGORY | read |

#### 生产主管（`production_supervisor`）

| 资源 | 权限 |
|------|------|
| WORK_ORDER | create, read, update（**无 delete**） |
| INSPECTION | create, read, update（**无 delete**） |
| LABOR_COST | read, update（**无 create/delete**） |
| COST | read |
| PRODUCT | read |
| BOM | read |

#### 只读访客（`readonly_guest`）

- **父角色**：`viewer`（系统内置角色）
- **权限**：继承 viewer 的所有 read 权限
- **注意**：此角色本身不额外授权，纯粹依赖继承
- **⚠️ 前置条件**：viewer 角色当前未分配任何权限，需先为 viewer 角色分配所有资源的 read 权限（CUSTOMER:read, PRODUCT:read, CATEGORY:read, BOM:read, BOM_CATEGORY:read, WAREHOUSE:read, LOCATION:read, INVENTORY:read, PRICE:read, SALES_ORDER:read, PURCHASE_ORDER:read, WORK_ORDER:read, INSPECTION:read, COST:read, LABOR_COST:read, USER:read, ROLE:read, DEPARTMENT:read, SHIPPING:read, FMS:read）

### 3.3 边界测试角色

#### 空权限角色（`empty_role`）

- **权限**：无任何权限
- **用途**：测试零权限时系统的 UI 表现和错误处理

#### 基础角色（`base_role`）

- **权限**：PRODUCT:read, CATEGORY:read
- **用途**：作为继承链的中间层

#### 派生角色（`derived_role`）

- **父角色**：`base_role`
- **权限**：PRODUCT:create（自身授权）+ 继承 base_role 的 PRODUCT:read, CATEGORY:read
- **用途**：测试 2 层继承是否正确合并权限

### 3.4 测试用户

| 用户名 | 密码 | 角色 | 部门 | 测试目的 |
|--------|------|------|------|----------|
| `test_sales` | `test1234` | sales_manager | 销售部 | 销售全流程权限验证 |
| `test_warehouse` | `test1234` | warehouse_keeper | 仓储部 | 库存全流程权限验证 |
| `test_production` | `test1234` | production_supervisor | 生产部 | 生产流程（无删除）验证 |
| `test_guest` | `test1234` | readonly_guest | 无 | 只读继承权限验证 |
| `test_empty` | `test1234` | empty_role | 无 | 零权限边界测试 |
| `test_multi` | `test1234` | sales_manager + warehouse_keeper | 管理层 | 多角色权限合并测试 |
| `test_inherit` | `test1234` | derived_role | 无 | 继承链权限合并测试 |

## 4. 前置条件

### 4.1 viewer 角色权限补充

当前 viewer 系统角色未分配任何权限。测试前需要为 viewer 角色分配所有资源的 read 权限：

```sql
INSERT INTO role_permissions (role_id, resource_code, action)
SELECT r.role_id, v.resource_code, 'read'
FROM roles r
CROSS JOIN (VALUES
    ('CUSTOMER'), ('PRODUCT'), ('CATEGORY'), ('BOM'), ('BOM_CATEGORY'),
    ('WAREHOUSE'), ('LOCATION'), ('INVENTORY'), ('PRICE'),
    ('SALES_ORDER'), ('PURCHASE_ORDER'), ('WORK_ORDER'),
    ('INSPECTION'), ('COST'), ('LABOR_COST'),
    ('USER'), ('ROLE'), ('DEPARTMENT'), ('SHIPPING'), ('FMS')
) v(resource_code)
WHERE r.role_code = 'viewer';
```

## 5. 测试执行顺序

### 阶段 1：环境准备
1. 通过 SQL 插入部门和角色数据
2. 为角色分配权限（role_permissions 表）
3. 创建测试用户（密码使用 argon2 哈希）
4. 验证数据插入正确

### 阶段 2：业务场景测试
1. 销售经理（test_sales）— 全量遍历
2. 仓管员（test_warehouse）— 全量遍历
3. 生产主管（test_production）— 全量遍历
4. 只读访客（test_guest）— 全量遍历

### 阶段 3：边界条件测试
1. 空权限用户（test_empty）
2. 多角色用户（test_multi）
3. 继承链用户（test_inherit）

### 阶段 4：结果汇总
1. 汇总所有发现的问题
2. 按严重程度分级（Critical / Major / Minor）
3. 输出测试报告文档

## 6. 通过标准

| 级别 | 标准 |
|------|------|
| **通过** | 所有测试用例的实际结果与预期一致 |
| **条件通过** | 存在 Minor 级别问题，不影响核心权限功能 |
| **不通过** | 存在 Critical 或 Major 级别问题（如权限绕过、菜单泄露） |

### 问题严重程度定义

| 级别 | 定义 | 示例 |
|------|------|------|
| **Critical** | 安全漏洞，可绕过权限控制 | 无权限用户能通过直接 URL 访问受保护页面 |
| **Major** | 权限控制逻辑错误 | 有 read 权限但菜单不显示；按钮权限与配置不一致 |
| **Minor** | UI 显示问题，不影响安全 | 权限提示文案不友好；按钮显示/隐藏有轻微延迟 |

## 6. 关联文档

- [测试用例](./2026-06-10-permission-test-cases.md) — 详细的测试用例清单
