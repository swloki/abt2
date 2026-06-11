# 权限系统自动化测试实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 通过 agent-browser 自动化测试 ABT 系统的 RBAC 权限控制，覆盖 7 个测试用户、121 条测试用例

**Architecture:** 先通过 SQL 种子脚本插入测试数据（部门、角色、权限、用户），再用 agent-browser 逐用户登录，按 [测试用例文档](../specs/2026-06-10-permission-test-cases.md) 执行验证。测试结果实时记录到 Markdown 表格，最终汇总为测试报告。

**Tech Stack:** agent-browser (Playwright)、PostgreSQL (psql)、argon2 (密码哈希)

**关联文档：**
- [测试计划](../specs/2026-06-10-permission-test-plan.md)
- [测试用例](../specs/2026-06-10-permission-test-cases.md)

---

## 文件结构

| 文件 | 职责 |
|------|------|
| `tests/permission/seed.sql` | 测试数据种子脚本（部门、角色、权限、用户） |
| `tests/permission/cleanup.sql` | 测试数据清理脚本 |
| `tests/permission/results/` | 测试结果目录（每用户一份） |
| `docs/superpowers/reports/2026-06-10-permission-test-report.md` | 最终测试报告 |

---

## Task 1: 生成测试密码哈希

**Files:**
- Create: `tests/permission/gen_hash.rs`（临时工具，用完可删）

因为用户密码使用 argon2id 哈希，不能明文插入数据库。先用 Rust 计算测试密码的哈希值。

- [ ] **Step 1: 创建临时哈希工具**

```rust
// tests/permission/gen_hash.rs
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHasher};

fn main() {
    let password = "test1234";
    let salt = SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .expect("hash failed")
        .to_string();
    println!("{hash}");
}
```

- [ ] **Step 2: 运行工具获取哈希值**

```bash
cd E:/work/abt && cargo run --example gen_hash
```

Expected: 输出类似 `$argon2id$v=19$m=19456,t=2,p=1$xxxx...` 的哈希字符串

- [ ] **Step 3: 记录哈希值**（将输出复制到下一步的 SQL 中）

---

## Task 2: 创建测试数据种子脚本

**Files:**
- Create: `tests/permission/seed.sql`
- Create: `tests/permission/cleanup.sql`

- [ ] **Step 1: 创建 cleanup.sql**（先写清理脚本，方便重置）

```sql
-- tests/permission/cleanup.sql
-- 清理权限测试数据（逆序删除，避免外键约束）

-- 1. 删除测试用户
DELETE FROM users WHERE username IN (
    'test_sales', 'test_warehouse', 'test_production',
    'test_guest', 'test_empty', 'test_multi', 'test_inherit'
);

-- 2. 删除测试角色的权限
DELETE FROM role_permissions WHERE role_id IN (
    SELECT role_id FROM roles WHERE role_code IN (
        'sales_manager', 'warehouse_keeper', 'production_supervisor',
        'readonly_guest', 'empty_role', 'base_role', 'derived_role'
    )
);

-- 3. 删除 viewer 角色的权限（测试时添加的）
DELETE FROM role_permissions WHERE role_id IN (
    SELECT role_id FROM roles WHERE role_code = 'viewer'
);

-- 4. 删除用户-角色关联
DELETE FROM user_roles WHERE user_id IN (
    SELECT user_id FROM users WHERE username IN (
        'test_sales', 'test_warehouse', 'test_production',
        'test_guest', 'test_empty', 'test_multi', 'test_inherit'
    )
);

-- 5. 删除测试角色
DELETE FROM roles WHERE role_code IN (
    'sales_manager', 'warehouse_keeper', 'production_supervisor',
    'readonly_guest', 'empty_role', 'base_role', 'derived_role'
);

-- 6. 删除测试部门
DELETE FROM departments WHERE department_code IN (
    'SALES', 'WAREHOUSE_DEPT', 'PRODUCTION', 'MANAGEMENT'
);
```

- [ ] **Step 2: 创建 seed.sql**

```sql
-- tests/permission/seed.sql
-- 权限测试种子数据
-- 前置：admin 用户和 super_admin/admin/viewer 系统角色已存在

-- ⚠️ 将下方 <HASH> 替换为 Task 1 生成的 argon2 哈希值

BEGIN;

-- ============================================================
-- 1. 部门
-- ============================================================
INSERT INTO departments (department_name, department_code, description, is_active, is_default)
VALUES
    ('销售部', 'SALES', '销售团队', true, false),
    ('仓储部', 'WAREHOUSE_DEPT', '仓库管理', true, false),
    ('生产部', 'PRODUCTION', '生产制造', true, false),
    ('管理层', 'MANAGEMENT', '高层管理', true, false)
ON CONFLICT (department_code) DO NOTHING;

-- ============================================================
-- 2. 业务角色
-- ============================================================

-- 销售经理
INSERT INTO roles (role_name, role_code, is_system_role, description)
VALUES ('销售经理', 'sales_manager', false, '销售管理全流程')
ON CONFLICT (role_code) DO NOTHING;

-- 仓管员
INSERT INTO roles (role_name, role_code, is_system_role, description)
VALUES ('仓管员', 'warehouse_keeper', false, '库存仓储全流程')
ON CONFLICT (role_code) DO NOTHING;

-- 生产主管
INSERT INTO roles (role_name, role_code, is_system_role, description)
VALUES ('生产主管', 'production_supervisor', false, '生产管理（无删除权限）')
ON CONFLICT (role_code) DO NOTHING;

-- 只读访客（继承 viewer）
INSERT INTO roles (role_name, role_code, is_system_role, parent_role_id, description)
VALUES ('只读访客', 'readonly_guest', false,
    (SELECT role_id FROM roles WHERE role_code = 'viewer'),
    '继承 viewer 的只读权限')
ON CONFLICT (role_code) DO NOTHING;

-- ============================================================
-- 3. 边界测试角色
-- ============================================================

-- 空权限角色
INSERT INTO roles (role_name, role_code, is_system_role, description)
VALUES ('空权限角色', 'empty_role', false, '边界测试：零权限')
ON CONFLICT (role_code) DO NOTHING;

-- 基础角色（继承链中间层）
INSERT INTO roles (role_name, role_code, is_system_role, description)
VALUES ('基础角色', 'base_role', false, '继承链测试：基础层')
ON CONFLICT (role_code) DO NOTHING;

-- 派生角色（继承 base_role）
INSERT INTO roles (role_name, role_code, is_system_role, parent_role_id, description)
VALUES ('派生角色', 'derived_role', false,
    (SELECT role_id FROM roles WHERE role_code = 'base_role'),
    '继承链测试：派生层')
ON CONFLICT (role_code) DO NOTHING;

-- ============================================================
-- 4. 角色-权限分配
-- ============================================================

-- 4.1 viewer 角色权限补充（所有 read）
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
WHERE r.role_code = 'viewer'
ON CONFLICT (role_id, resource_code, action) DO NOTHING;

-- 4.2 销售经理权限
INSERT INTO role_permissions (role_id, resource_code, action) VALUES
    -- CUSTOMER: CRUD
    ((SELECT role_id FROM roles WHERE role_code = 'sales_manager'), 'CUSTOMER', 'create'),
    ((SELECT role_id FROM roles WHERE role_code = 'sales_manager'), 'CUSTOMER', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'sales_manager'), 'CUSTOMER', 'update'),
    ((SELECT role_id FROM roles WHERE role_code = 'sales_manager'), 'CUSTOMER', 'delete'),
    -- SALES_ORDER: CRUD
    ((SELECT role_id FROM roles WHERE role_code = 'sales_manager'), 'SALES_ORDER', 'create'),
    ((SELECT role_id FROM roles WHERE role_code = 'sales_manager'), 'SALES_ORDER', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'sales_manager'), 'SALES_ORDER', 'update'),
    ((SELECT role_id FROM roles WHERE role_code = 'sales_manager'), 'SALES_ORDER', 'delete'),
    -- SHIPPING: CRUD
    ((SELECT role_id FROM roles WHERE role_code = 'sales_manager'), 'SHIPPING', 'create'),
    ((SELECT role_id FROM roles WHERE role_code = 'sales_manager'), 'SHIPPING', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'sales_manager'), 'SHIPPING', 'update'),
    ((SELECT role_id FROM roles WHERE role_code = 'sales_manager'), 'SHIPPING', 'delete'),
    -- PRODUCT: read
    ((SELECT role_id FROM roles WHERE role_code = 'sales_manager'), 'PRODUCT', 'read'),
    -- CATEGORY: read
    ((SELECT role_id FROM roles WHERE role_code = 'sales_manager'), 'CATEGORY', 'read'),
    -- PRICE: read
    ((SELECT role_id FROM roles WHERE role_code = 'sales_manager'), 'PRICE', 'read')
ON CONFLICT (role_id, resource_code, action) DO NOTHING;

-- 4.3 仓管员权限
INSERT INTO role_permissions (role_id, resource_code, action) VALUES
    -- WAREHOUSE: CRUD
    ((SELECT role_id FROM roles WHERE role_code = 'warehouse_keeper'), 'WAREHOUSE', 'create'),
    ((SELECT role_id FROM roles WHERE role_code = 'warehouse_keeper'), 'WAREHOUSE', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'warehouse_keeper'), 'WAREHOUSE', 'update'),
    ((SELECT role_id FROM roles WHERE role_code = 'warehouse_keeper'), 'WAREHOUSE', 'delete'),
    -- LOCATION: CRUD
    ((SELECT role_id FROM roles WHERE role_code = 'warehouse_keeper'), 'LOCATION', 'create'),
    ((SELECT role_id FROM roles WHERE role_code = 'warehouse_keeper'), 'LOCATION', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'warehouse_keeper'), 'LOCATION', 'update'),
    ((SELECT role_id FROM roles WHERE role_code = 'warehouse_keeper'), 'LOCATION', 'delete'),
    -- INVENTORY: CRUD
    ((SELECT role_id FROM roles WHERE role_code = 'warehouse_keeper'), 'INVENTORY', 'create'),
    ((SELECT role_id FROM roles WHERE role_code = 'warehouse_keeper'), 'INVENTORY', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'warehouse_keeper'), 'INVENTORY', 'update'),
    ((SELECT role_id FROM roles WHERE role_code = 'warehouse_keeper'), 'INVENTORY', 'delete'),
    -- PRODUCT: read
    ((SELECT role_id FROM roles WHERE role_code = 'warehouse_keeper'), 'PRODUCT', 'read'),
    -- CATEGORY: read
    ((SELECT role_id FROM roles WHERE role_code = 'warehouse_keeper'), 'CATEGORY', 'read')
ON CONFLICT (role_id, resource_code, action) DO NOTHING;

-- 4.4 生产主管权限
INSERT INTO role_permissions (role_id, resource_code, action) VALUES
    -- WORK_ORDER: CRU（无 delete）
    ((SELECT role_id FROM roles WHERE role_code = 'production_supervisor'), 'WORK_ORDER', 'create'),
    ((SELECT role_id FROM roles WHERE role_code = 'production_supervisor'), 'WORK_ORDER', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'production_supervisor'), 'WORK_ORDER', 'update'),
    -- INSPECTION: CRU（无 delete）
    ((SELECT role_id FROM roles WHERE role_code = 'production_supervisor'), 'INSPECTION', 'create'),
    ((SELECT role_id FROM roles WHERE role_code = 'production_supervisor'), 'INSPECTION', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'production_supervisor'), 'INSPECTION', 'update'),
    -- LABOR_COST: RU（无 create/delete）
    ((SELECT role_id FROM roles WHERE role_code = 'production_supervisor'), 'LABOR_COST', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'production_supervisor'), 'LABOR_COST', 'update'),
    -- COST: read
    ((SELECT role_id FROM roles WHERE role_code = 'production_supervisor'), 'COST', 'read'),
    -- PRODUCT: read
    ((SELECT role_id FROM roles WHERE role_code = 'production_supervisor'), 'PRODUCT', 'read'),
    -- BOM: read
    ((SELECT role_id FROM roles WHERE role_code = 'production_supervisor'), 'BOM', 'read')
ON CONFLICT (role_id, resource_code, action) DO NOTHING;

-- 4.5 基础角色权限（继承链中间层）
INSERT INTO role_permissions (role_id, resource_code, action) VALUES
    ((SELECT role_id FROM roles WHERE role_code = 'base_role'), 'PRODUCT', 'read'),
    ((SELECT role_id FROM roles WHERE role_code = 'base_role'), 'CATEGORY', 'read')
ON CONFLICT (role_id, resource_code, action) DO NOTHING;

-- 4.6 派生角色自身权限（继承 base_role 的 read，自身增加 create）
INSERT INTO role_permissions (role_id, resource_code, action) VALUES
    ((SELECT role_id FROM roles WHERE role_code = 'derived_role'), 'PRODUCT', 'create')
ON CONFLICT (role_id, resource_code, action) DO NOTHING;

-- ⚠️ 注意：empty_role 不分配任何权限
-- ⚠️ 注意：readonly_guest 不额外分配权限，依赖继承 viewer

-- ============================================================
-- 5. 测试用户
-- ============================================================
-- ⚠️ 将 <HASH> 替换为 Task 1 生成的 'test1234' 的 argon2 哈希

INSERT INTO users (username, password_hash, display_name, is_super_admin, is_active) VALUES
    ('test_sales',       '<HASH>', '测试-销售经理', false, true),
    ('test_warehouse',   '<HASH>', '测试-仓管员',   false, true),
    ('test_production',  '<HASH>', '测试-生产主管', false, true),
    ('test_guest',       '<HASH>', '测试-只读访客', false, true),
    ('test_empty',       '<HASH>', '测试-空权限',   false, true),
    ('test_multi',       '<HASH>', '测试-多角色',   false, true),
    ('test_inherit',     '<HASH>', '测试-继承链',   false, true)
ON CONFLICT (username) DO NOTHING;

-- ============================================================
-- 6. 用户-角色关联
-- ============================================================

-- test_sales → sales_manager
INSERT INTO user_roles (user_id, role_id)
SELECT u.user_id, r.role_id
FROM users u, roles r
WHERE u.username = 'test_sales' AND r.role_code = 'sales_manager'
ON CONFLICT DO NOTHING;

-- test_warehouse → warehouse_keeper
INSERT INTO user_roles (user_id, role_id)
SELECT u.user_id, r.role_id
FROM users u, roles r
WHERE u.username = 'test_warehouse' AND r.role_code = 'warehouse_keeper'
ON CONFLICT DO NOTHING;

-- test_production → production_supervisor
INSERT INTO user_roles (user_id, role_id)
SELECT u.user_id, r.role_id
FROM users u, roles r
WHERE u.username = 'test_production' AND r.role_code = 'production_supervisor'
ON CONFLICT DO NOTHING;

-- test_guest → readonly_guest
INSERT INTO user_roles (user_id, role_id)
SELECT u.user_id, r.role_id
FROM users u, roles r
WHERE u.username = 'test_guest' AND r.role_code = 'readonly_guest'
ON CONFLICT DO NOTHING;

-- test_empty → empty_role
INSERT INTO user_roles (user_id, role_id)
SELECT u.user_id, r.role_id
FROM users u, roles r
WHERE u.username = 'test_empty' AND r.role_code = 'empty_role'
ON CONFLICT DO NOTHING;

-- test_multi → sales_manager + warehouse_keeper（多角色）
INSERT INTO user_roles (user_id, role_id)
SELECT u.user_id, r.role_id
FROM users u, roles r
WHERE (u.username = 'test_multi' AND r.role_code = 'sales_manager')
   OR (u.username = 'test_multi' AND r.role_code = 'warehouse_keeper')
ON CONFLICT DO NOTHING;

-- test_inherit → derived_role
INSERT INTO user_roles (user_id, role_id)
SELECT u.user_id, r.role_id
FROM users u, roles r
WHERE u.username = 'test_inherit' AND r.role_code = 'derived_role'
ON CONFLICT DO NOTHING;

COMMIT;
```

- [ ] **Step 3: 用 Task 1 的哈希值替换 seed.sql 中的 `<HASH>` 占位符**

- [ ] **Step 4: Commit 种子脚本**

```bash
git add tests/permission/seed.sql tests/permission/cleanup.sql
git commit -m "test: 权限测试种子数据和清理脚本"
```

---

## Task 3: 执行种子脚本并验证

- [ ] **Step 1: 清理旧数据（如果有）**

```bash
psql "$DATABASE_URL" -f tests/permission/cleanup.sql
```

Expected: `DELETE N`（N 为删除的行数）

- [ ] **Step 2: 执行种子脚本**

```bash
psql "$DATABASE_URL" -f tests/permission/seed.sql
```

Expected: `INSERT 0 N`（无错误，事务提交成功）

- [ ] **Step 3: 验证数据正确性**

```bash
psql "$DATABASE_URL" -c "
SELECT 'departments' as t, count(*) FROM departments WHERE department_code IN ('SALES','WAREHOUSE_DEPT','PRODUCTION','MANAGEMENT')
UNION ALL
SELECT 'roles', count(*) FROM roles WHERE role_code IN ('sales_manager','warehouse_keeper','production_supervisor','readonly_guest','empty_role','base_role','derived_role')
UNION ALL
SELECT 'users', count(*) FROM users WHERE username LIKE 'test_%'
UNION ALL
SELECT 'user_roles', count(*) FROM user_roles WHERE user_id IN (SELECT user_id FROM users WHERE username LIKE 'test_%')
UNION ALL
SELECT 'role_perms', count(*) FROM role_permissions WHERE role_id IN (SELECT role_id FROM roles WHERE role_code IN ('sales_manager','warehouse_keeper','production_supervisor','base_role','derived_role'));
"
```

Expected: departments=4, roles=7, users=7, user_roles=8 (test_multi 有 2 个), role_perms 合计约 50 条

- [ ] **Step 4: 快速验证 test_sales 能登录**

```bash
agent-browser open https://localhost:8000/login
agent-browser snapshot -i
# 填写用户名
agent-browser fill @e<username_ref> "test_sales"
# 填写密码
agent-browser fill @e<password_ref> "test1234"
# 点击登录
agent-browser click @e<login_button_ref>
agent-browser wait --load networkidle
agent-browser snapshot -i
```

Expected: 登录成功，页面跳转到 `/admin`，侧边栏可见

---

## Task 4: 创建测试结果记录模板

**Files:**
- Create: `tests/permission/results/_template.md`

- [ ] **Step 1: 创建每用户结果记录模板**

```markdown
# 测试结果：{用户名}（{角色名}）

> 测试时间：YYYY-MM-DD HH:MM
> 测试用例文档：[链接](../../docs/superpowers/specs/2026-06-10-permission-test-cases.md)

## 1. 菜单可见性

| 用例 ID | 预期 | 实际 | 结果 |
|---------|------|------|------|
| TP-XXX-MENU-01 | ... | ... | ✅/❌ |

## 2. 页面与按钮权限

| 用例 ID | 预期 | 实际 | 结果 |
|---------|------|------|------|
| TP-XXX-YYY-01 | ... | ... | ✅/❌ |

## 3. 越权访问测试

| 用例 ID | 预期 | 实际 | 结果 |
|---------|------|------|------|
| TP-XXX-SEC-01 | ... | ... | ✅/❌ |

## 缺陷清单

| # | 严重程度 | 用例 ID | 描述 | 截图 |
|---|----------|---------|------|------|
| 1 | Critical/Major/Minor | ... | ... | ... |
```

- [ ] **Step 2: 创建 results 目录和所有用户的结果文件**

```
tests/permission/results/
├── 01_test_sales.md
├── 02_test_warehouse.md
├── 03_test_production.md
├── 04_test_guest.md
├── 05_test_empty.md
├── 06_test_multi.md
├── 07_test_inherit.md
└── 08_general.md
```

- [ ] **Step 3: Commit**

```bash
git add tests/permission/results/
git commit -m "test: 权限测试结果记录模板"
```

---

## Task 5: 测试销售经理（test_sales）

**测试用例参考：** TP-SAL-MENU-01 ~ TP-SAL-SEC-05（共 26 条）
**结果记录：** `tests/permission/results/01_test_sales.md`

### 5.1 登录

- [ ] **Step 1: 打开登录页并登录**

```bash
agent-browser open https://localhost:8000/login
agent-browser snapshot -i
# 填写用户名
agent-browser fill @e<username> "test_sales"
# 填写密码
agent-browser fill @e<password> "test1234"
# 点击登录
agent-browser click @e<login_btn>
agent-browser wait --load networkidle
```

Expected: 页面跳转到 `/admin`，显示仪表板页面

- [ ] **Step 2: 截图记录登录后状态**

```bash
agent-browser screenshot
```

### 5.2 菜单可见性测试（TP-SAL-MENU-01 ~ 10）

- [ ] **Step 3: 获取侧边栏快照，记录所有可见菜单模块和子菜单项**

```bash
agent-browser snapshot -i
```

验证清单（逐一对照）：
- ✅/❌ TP-SAL-MENU-01: 仅显示「销售管理」和「主数据」模块
- ✅/❌ TP-SAL-MENU-02: 销售管理子菜单有 7 项（销售总览、客户管理、报价单、销售订单、发货申请、销售退货、月对账单）
- ✅/❌ TP-SAL-MENU-03: 主数据子菜单仅显示产品管理、产品分类
- ✅/❌ TP-SAL-MENU-04~10: 不显示采购、库存、生产、委外、质量、财务、系统管理

- [ ] **Step 4: 将结果记录到 `01_test_sales.md`**

### 5.3 销售管理页面测试（TP-SAL-SALES-01 ~ 14）

- [ ] **Step 5: 导航到销售总览，验证页面加载（TP-SAL-SALES-01）**

```bash
agent-browser click @e<sales_overview_menu>
agent-browser wait --load networkidle
agent-browser snapshot -i
agent-browser screenshot
```

- [ ] **Step 6: 导航到客户列表，验证页面和按钮（TP-SAL-SALES-02 ~ 06）**

```bash
agent-browser click @e<customer_menu>
agent-browser wait --load networkidle
agent-browser snapshot -i
```

验证：
- ✅/❌ TP-SAL-SALES-02: 页面正常加载
- ✅/❌ TP-SAL-SALES-03: 显示「新增客户」按钮

- [ ] **Step 7: 依次测试报价单、销售订单、发货申请、销售退货、月对账单（TP-SAL-SALES-07 ~ 14）**

对每个页面：
```bash
agent-browser click @e<对应菜单项>
agent-browser wait --load networkidle
agent-browser snapshot -i
agent-browser screenshot
```

验证页面是否正常加载、创建按钮是否显示。

### 5.4 主数据页面测试（TP-SAL-MD-01 ~ 07）

- [ ] **Step 8: 导航到产品管理，验证只读（TP-SAL-MD-01 ~ 04）**

```bash
agent-browser click @e<product_menu>
agent-browser wait --load networkidle
agent-browser snapshot -i
agent-browser screenshot
```

验证：
- ✅/❌ TP-SAL-MD-01: 页面正常加载
- ✅/❌ TP-SAL-MD-02: 不显示创建按钮
- ✅/❌ TP-SAL-MD-03: 不显示编辑按钮
- ✅/❌ TP-SAL-MD-04: 不显示删除按钮

- [ ] **Step 9: 导航到产品分类，验证只读（TP-SAL-MD-05）**

- [ ] **Step 10: 直接访问 BOM 管理和供应商页面（TP-SAL-MD-06 ~ 07）**

```bash
agent-browser open https://localhost:8000/admin/md/boms
agent-browser wait --load networkidle
agent-browser snapshot -i
agent-browser screenshot
```

Expected: 403 或重定向到登录页

同理测试 `/admin/md/suppliers`

### 5.5 越权访问测试（TP-SAL-SEC-01 ~ 05）

- [ ] **Step 11: 直接访问无权限页面**

依次访问以下 URL，每个记录是否被拒绝：
- `/admin/system/users` → ✅/❌ TP-SAL-SEC-01
- `/admin/wms/warehouses` → ✅/❌ TP-SAL-SEC-02
- `/admin/purchase/orders` → ✅/❌ TP-SAL-SEC-03
- `/admin/mes/orders` → ✅/❌ TP-SAL-SEC-04
- `/admin/system/permissions` → ✅/❌ TP-SAL-SEC-05

```bash
agent-browser open https://localhost:8000/admin/system/users
agent-browser wait --load networkidle
agent-browser snapshot -i
agent-browser screenshot
```

Expected: 403 Forbidden 页面或重定向

- [ ] **Step 12: 将所有结果写入 `01_test_sales.md`**

- [ ] **Step 13: Commit 结果**

```bash
git add tests/permission/results/01_test_sales.md
git commit -m "test: 销售经理权限测试结果"
```

---

## Task 6: 测试仓管员（test_warehouse）

**测试用例参考：** TP-WH-MENU-01 ~ TP-WH-SEC-04（共 25 条）
**结果记录：** `tests/permission/results/02_test_warehouse.md`

### 6.1 登录

- [ ] **Step 1: 登出当前用户（如果已登录），登录 test_warehouse**

```bash
# 先登出
agent-browser open https://localhost:8000/logout
agent-browser wait --load networkidle
# 再登录
agent-browser open https://localhost:8000/login
agent-browser snapshot -i
agent-browser fill @e<username> "test_warehouse"
agent-browser fill @e<password> "test1234"
agent-browser click @e<login_btn>
agent-browser wait --load networkidle
agent-browser screenshot
```

### 6.2 菜单可见性测试（TP-WH-MENU-01 ~ 04）

- [ ] **Step 2: 截图侧边栏，记录可见菜单模块**

验证：
- ✅/❌ TP-WH-MENU-01: 仅显示「库存管理」和「主数据」
- ✅/❌ TP-WH-MENU-02: 库存管理子菜单有 16 项
- ✅/❌ TP-WH-MENU-03: 主数据仅显示产品管理、产品分类
- ✅/❌ TP-WH-MENU-04: 其他模块不可见

### 6.3 库存管理页面测试（TP-WH-WMS-01 ~ 17）

- [ ] **Step 3: 逐项测试库存管理 16 个子页面**

对每个子菜单项，点击后验证：
1. 页面是否正常加载
2. 创建按钮是否显示（对应有 create 权限的资源）

重点验证：
- TP-WH-WMS-02: 仓库管理页显示创建按钮
- TP-WH-WMS-03: 可成功创建仓库（CRUD 权限）
- TP-WH-WMS-04: 储位管理页显示创建按钮

每个页面截图记录。

### 6.4 主数据只读测试（TP-WH-MD-01 ~ 04）

- [ ] **Step 4: 验证产品管理、产品分类页面只读**

验证：
- ✅/❌ TP-WH-MD-01: 产品页面加载正常
- ✅/❌ TP-WH-MD-02: 不显示创建/编辑/删除按钮
- ✅/❌ TP-WH-MD-03: 分类页面加载正常
- ✅/❌ TP-WH-MD-04: 不显示创建/编辑/删除按钮

### 6.5 越权访问测试（TP-WH-SEC-01 ~ 04）

- [ ] **Step 5: 直接访问无权限页面**

依次访问：
- `/admin/orders` → ✅/❌ TP-WH-SEC-01
- `/admin/system/users` → ✅/❌ TP-WH-SEC-02
- `/admin/mes/orders` → ✅/❌ TP-WH-SEC-03
- `/admin/customers` → ✅/❌ TP-WH-SEC-04

- [ ] **Step 6: 记录结果并 Commit**

```bash
git add tests/permission/results/02_test_warehouse.md
git commit -m "test: 仓管员权限测试结果"
```

---

## Task 7: 测试生产主管（test_production）

**测试用例参考：** TP-PRD-MENU-01 ~ TP-PRD-SEC-04（共 34 条）
**结果记录：** `tests/permission/results/03_test_production.md`

### 7.1 登录

- [ ] **Step 1: 登出并登录 test_production**

### 7.2 菜单可见性测试（TP-PRD-MENU-01 ~ 05）

- [ ] **Step 2: 验证侧边栏**

验证：
- ✅/❌ TP-PRD-MENU-01: 显示「生产管理」「质量管理」「主数据」
- ✅/❌ TP-PRD-MENU-02: 生产管理子菜单 12 项
- ✅/❌ TP-PRD-MENU-03: 质量管理子菜单 5 项
- ✅/❌ TP-PRD-MENU-04: 主数据显示产品管理、BOM管理；不显示产品分类、供应商管理
- ✅/❌ TP-PRD-MENU-05: 不显示其他模块

### 7.3 生产管理页面测试（TP-PRD-MES-01 ~ 18）

- [ ] **Step 3: 逐项测试 12 个生产子页面**

重点验证：
- TP-PRD-MES-03: 生产计划页显示创建按钮
- TP-PRD-MES-05: 工单管理页显示创建按钮
- TP-PRD-MES-06: **工单管理页不显示删除按钮**（无 WORK_ORDER:delete）
- TP-PRD-MES-12: 计件工资页显示编辑按钮，**不显示创建按钮**（有 update 无 create）
- TP-PRD-MES-14: 报检页显示创建按钮
- TP-PRD-MES-15: **报检页不显示删除按钮**（无 INSPECTION:delete）

### 7.4 质量管理页面测试（TP-PRD-QMS-01 ~ 07）

- [ ] **Step 4: 测试质量管理 5 个子页面**

重点验证：
- TP-PRD-QMS-03: 检验规格页显示创建按钮
- TP-PRD-QMS-04: **检验规格页不显示删除按钮**

### 7.5 主数据只读测试（TP-PRD-MD-01 ~ 04）

- [ ] **Step 5: 验证产品管理、BOM管理只读**

### 7.6 越权访问测试（TP-PRD-SEC-01 ~ 04）

- [ ] **Step 6: 直接访问无权限页面**

- `/admin/wms/warehouses` → ✅/❌ TP-PRD-SEC-01
- `/admin/orders` → ✅/❌ TP-PRD-SEC-02
- `/admin/system/users` → ✅/❌ TP-PRD-SEC-03
- `/admin/purchase/orders` → ✅/❌ TP-PRD-SEC-04

- [ ] **Step 7: 记录结果并 Commit**

```bash
git add tests/permission/results/03_test_production.md
git commit -m "test: 生产主管权限测试结果"
```

---

## Task 8: 测试只读访客（test_guest）

**测试用例参考：** TP-GST-MENU-01 ~ TP-GST-SEC-03（共 8 条）
**结果记录：** `tests/permission/results/04_test_guest.md`

### 8.1 登录与菜单

- [ ] **Step 1: 登出并登录 test_guest**

- [ ] **Step 2: 验证侧边栏显示所有模块（继承 viewer 的全部 read 权限）**

验证：
- ✅/❌ TP-GST-MENU-01: 显示所有有 read 权限的模块
- ✅/❌ TP-GST-MENU-02: 所有页面不显示创建/编辑/删除按钮

### 8.2 只读页面测试（TP-GST-RO-01 ~ 05）

- [ ] **Step 3: 进入客户、产品、仓库、工单列表页，验证无操作按钮**

每个页面：
```bash
agent-browser click @e<对应菜单>
agent-browser wait --load networkidle
agent-browser snapshot -i
```

验证无创建/编辑/删除按钮。

- [ ] **Step 4: 在权限配置页验证继承标识（TP-GST-RO-05）**

以 admin 登录后访问 `/admin/system/permissions`，找到 readonly_guest 角色，检查从 viewer 继承的权限是否标记为继承（灰色/只读）。

### 8.3 越权访问测试（TP-GST-SEC-01 ~ 03）

- [ ] **Step 5: 尝试直接 POST 创建/删除接口**

使用 agent-browser eval 发起 fetch 请求：
```bash
agent-browser eval "fetch('/admin/customers', {method:'POST', headers:{'Content-Type':'application/json'}, body:'{}'}).then(r => r.status)"
```

Expected: 返回 403

- [ ] **Step 6: 记录结果并 Commit**

```bash
git add tests/permission/results/04_test_guest.md
git commit -m "test: 只读访客权限测试结果"
```

---

## Task 9: 测试空权限用户（test_empty）

**测试用例参考：** TP-EMP-EMPTY-01 ~ 05（共 5 条）
**结果记录：** `tests/permission/results/05_test_empty.md`

### 9.1 登录与零权限表现

- [ ] **Step 1: 登出并登录 test_empty**

- [ ] **Step 2: 验证零权限表现**

验证：
- ✅/❌ TP-EMP-EMPTY-01: 登录成功不报错
- ✅/❌ TP-EMP-EMPTY-02: 侧边栏无菜单项或仅空框架
- ✅/❌ TP-EMP-EMPTY-03: 首页不崩溃（显示空状态或提示）
- ✅/❌ TP-EMP-EMPTY-04: 访问 `/admin/customers` → 403
- ✅/❌ TP-EMP-EMPTY-05: 访问 `/admin/system/users` → 403

重点截图记录零权限时的 UI 表现。

- [ ] **Step 3: 记录结果并 Commit**

```bash
git add tests/permission/results/05_test_empty.md
git commit -m "test: 空权限用户边界测试结果"
```

---

## Task 10: 测试多角色用户（test_multi）

**测试用例参考：** TP-MUL-MERGE-01 ~ 06（共 6 条）
**结果记录：** `tests/permission/results/06_test_multi.md`

### 10.1 权限合并验证

- [ ] **Step 1: 登出并登录 test_multi（拥有 sales_manager + warehouse_keeper）**

- [ ] **Step 2: 验证菜单合并**

验证：
- ✅/❌ TP-MUL-MERGE-01: 同时显示「销售管理」「库存管理」「主数据」
- ✅/❌ TP-MUL-MERGE-02: 客户管理页显示完整 CRUD 按钮
- ✅/❌ TP-MUL-MERGE-03: 仓库管理页显示完整 CRUD 按钮
- ✅/❌ TP-MUL-MERGE-04: 产品管理页仅显示列表，不显示创建/编辑/删除

### 10.2 越权测试

- [ ] **Step 3: 验证无交叉越权**

- `/admin/mes/orders` → ✅/❌ TP-MUL-MERGE-05
- `/admin/system/users` → ✅/❌ TP-MUL-MERGE-06

- [ ] **Step 4: 记录结果并 Commit**

```bash
git add tests/permission/results/06_test_multi.md
git commit -m "test: 多角色用户权限合并测试结果"
```

---

## Task 11: 测试继承链用户（test_inherit）

**测试用例参考：** TP-INH-CHAIN-01 ~ 09（共 9 条）
**结果记录：** `tests/permission/results/07_test_inherit.md`

### 11.1 继承验证

- [ ] **Step 1: 登出并登录 test_inherit（拥有 derived_role，继承 base_role）**

- [ ] **Step 2: 验证继承链权限**

验证：
- ✅/❌ TP-INH-CHAIN-01: 显示「主数据」模块
- ✅/❌ TP-INH-CHAIN-02: 产品管理页正常加载
- ✅/❌ TP-INH-CHAIN-03: **显示**创建按钮（derived_role 自身的 PRODUCT:create）
- ✅/❌ TP-INH-CHAIN-04: **不显示**编辑按钮（无 PRODUCT:update）
- ✅/❌ TP-INH-CHAIN-05: **不显示**删除按钮（无 PRODUCT:delete）
- ✅/❌ TP-INH-CHAIN-06: 产品分类页正常加载（继承 CATEGORY:read）
- ✅/❌ TP-INH-CHAIN-07: 分类页不显示创建/编辑/删除按钮
- ✅/❌ TP-INH-CHAIN-08: 访问销售管理 → 403
- ✅/❌ TP-INH-CHAIN-09: 访问库存管理 → 403

**这是最关键的继承测试**——验证：
- 子角色能获得父角色的权限（base_role 的 PRODUCT:read + CATEGORY:read）
- 子角色自身权限与继承权限正确合并（PRODUCT:read + PRODUCT:create + CATEGORY:read）

- [ ] **Step 3: 记录结果并 Commit**

```bash
git add tests/permission/results/07_test_inherit.md
git commit -m "test: 继承链用户权限测试结果"
```

---

## Task 12: 通用测试（跨用户验证）

**测试用例参考：** TP-GEN-AUTH-01 ~ 05 + TP-GEN-NAV-01 ~ 03（共 8 条）
**结果记录：** `tests/permission/results/08_general.md`

### 12.1 登录与会话测试

- [ ] **Step 1: 测试登录失败（TP-GEN-AUTH-02）**

```bash
agent-browser open https://localhost:8000/login
agent-browser snapshot -i
agent-browser fill @e<username> "test_sales"
agent-browser fill @e<password> "wrongpassword"
agent-browser click @e<login_btn>
agent-browser wait --load networkidle
agent-browser snapshot -i
agent-browser screenshot
```

Expected: 显示错误提示，留在登录页

- [ ] **Step 2: 测试登出（TP-GEN-AUTH-03）**

用任意测试用户登录后登出：

```bash
agent-browser open https://localhost:8000/login
agent-browser snapshot -i
agent-browser fill @e<username> "test_sales"
agent-browser fill @e<password> "test1234"
agent-browser click @e<login_btn>
agent-browser wait --load networkidle
# 找到登出按钮并点击
agent-browser click @e<logout_btn>
agent-browser wait --load networkidle
agent-browser snapshot -i
agent-browser screenshot
```

Expected: 跳转回登录页

- [ ] **Step 3: 测试未登录访问（TP-GEN-AUTH-05）**

```bash
agent-browser open https://localhost:8000/admin
agent-browser wait --load networkidle
agent-browser snapshot -i
agent-browser screenshot
```

Expected: 重定向到登录页

### 12.2 菜单过滤一致性验证

- [ ] **Step 4: 对比所有用户的菜单与权限配置（TP-GEN-NAV-01 ~ 03）**

此步骤为汇总分析——对比前面 7 个用户测试中收集的菜单数据，验证：
1. 菜单项显示与 role_permissions 完全一致
2. 无权限的模块完全隐藏（无空模块组）
3. URL 直接访问与菜单过滤一致（均有 Handler 保护）

- [ ] **Step 5: 记录结果并 Commit**

```bash
git add tests/permission/results/08_general.md
git commit -m "test: 通用权限测试结果"
```

---

## Task 13: 生成测试报告

**Files:**
- Create: `docs/superpowers/reports/2026-06-10-permission-test-report.md`

- [ ] **Step 1: 汇总所有用户测试结果**

从 `tests/permission/results/` 中提取所有缺陷，按严重程度分级。

- [ ] **Step 2: 撰写测试报告**

报告结构：

```markdown
# 权限系统测试报告

> 日期：2026-06-10
> 测试范围：RBAC 权限系统全量功能测试 + 边界条件测试
> 测试方法：agent-browser 自动化浏览器测试

## 1. 测试概况

| 指标 | 数值 |
|------|------|
| 测试用户数 | 7 |
| 测试用例总数 | 121 |
| 通过数 | N |
| 失败数 | N |
| 通过率 | N% |

## 2. 按角色测试结果

| 用户/角色 | 用例数 | 通过 | 失败 | 通过率 |
|-----------|--------|------|------|--------|
| test_sales / 销售经理 | 26 | N | N | N% |
| test_warehouse / 仓管员 | 25 | N | N | N% |
| test_production / 生产主管 | 34 | N | N | N% |
| test_guest / 只读访客 | 8 | N | N | N% |
| test_empty / 空权限 | 5 | N | N | N% |
| test_multi / 多角色 | 6 | N | N | N% |
| test_inherit / 继承链 | 9 | N | N | N% |
| 通用测试 | 8 | N | N | N% |

## 3. 缺陷清单

### Critical（安全漏洞）

| # | 用例 ID | 描述 | 复现步骤 |
|---|---------|------|----------|
| ... | ... | ... | ... |

### Major（权限逻辑错误）

| # | 用例 ID | 描述 | 复现步骤 |
|---|---------|------|----------|
| ... | ... | ... | ... |

### Minor（UI 显示问题）

| # | 用例 ID | 描述 | 复现步骤 |
|---|---------|------|----------|
| ... | ... | ... | ... |

## 4. 总体评估

[通过 / 条件通过 / 不通过]

## 5. 建议

...
```

- [ ] **Step 3: Commit 测试报告**

```bash
git add docs/superpowers/reports/2026-06-10-permission-test-report.md
git commit -m "test: 权限系统测试报告（121条用例）"
```

---

## Task 14: 清理测试数据

- [ ] **Step 1: 执行清理脚本（可选，如需保留测试数据可跳过）**

```bash
psql "$DATABASE_URL" -f tests/permission/cleanup.sql
```

- [ ] **Step 2: 验证清理结果**

```bash
psql "$DATABASE_URL" -c "SELECT count(*) FROM users WHERE username LIKE 'test_%'"
```

Expected: `0`
