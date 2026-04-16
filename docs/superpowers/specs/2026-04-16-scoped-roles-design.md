---
title: Scoped Roles 设计 — 部门级角色权限
date: 2026-04-16
status: draft
---

# Scoped Roles 设计

## 背景与问题

当前系统的权限模型：

- **角色是全局的**：用户被分配角色后，权限在所有部门中相同
- **部门控制资源可见性**：通过 `department_resource_access` 配置部门能看到哪些资源类型
- **两层叠加**：登录时取角色权限，再按部门资源可见性过滤，生成扁平权限列表存入 JWT

问题：无法实现"用户在 A 部门是经理（可读写），在 B 部门是职员（只读）"的需求。如果为每个部门创建专属角色（如"A 部门经理"），会导致角色爆炸（N 个部门 x M 种职位 = N x M 个角色）。

## 设计目标

引入 **Scoped Roles（作用域角色）**：角色只定义一次（如"经理"、"职员"），在分配给用户时绑定部门作用域。同一个人在不同部门可以拥有不同角色，从而拥有不同操作权限。

## 设计决策记录

| # | 问题 | 决定 | 原因 |
|---|---|---|---|
| 1 | 角色与部门的关系 | Scoped Roles：角色全局定义，分配时绑定部门 | 避免角色爆炸，业界主流做法 |
| 2 | department_resource_access 是否保留 | 保留 | 部门资源可见性（能否看到某类资源）和角色权限（能做什么操作）是两个维度 |
| 3 | JWT 存储策略 | JWT 存部门-角色映射 + 运行时缓存查角色权限 | JWT 保持精小，角色权限变更无需所有人重新登录 |
| 4 | 系统角色 | 精简为 super_admin + user | admin 职责拆分给部门经理角色 |
| 5 | 预置业务角色 | manager（读写删）+ staff（只读） | 开箱即用，管理员可调整权限或新建角色 |
| 6 | 业务数据是否加 department_id | 不加 | 数据全部可见，部门只控制资源类型可见性和操作权限 |
| 7 | 权限检查的部门上下文 | 前端请求中传入 department_id | 宏从请求中提取 department_id，确定权限检查的作用域 |
| 8 | require_permission 宏写法 | 不变 | 内部实现改为两步检查（部门可见性 + 角色权限），对外透明 |

## 权限检查流程

```
前端发起业务请求（携带 department_id）
         |
         v
  require_permission(Resource::Product, Action::Write)
         |
         v
  [第一步] 部门资源可见性检查
         |
         |-- 从请求获取 department_id
         |-- 从 JWT 获取用户的 dept_roles
         |-- 该部门是否关联了 Product 资源？（查 department_resource_access）
         |
         |-- 否 --> 拒绝（该部门不可见此资源）
         |-- 是 --> 继续
         |
         v
  [第二步] 角色操作权限检查
         |
         |-- 用户在该部门的 role_id 是什么？（从 JWT dept_roles 查找）
         |-- 该 role_id 有没有 product:write 权限？（从内存缓存查找）
         |
         |-- 否 --> 拒绝
         |-- 是 --> 允许
         |
         v
      执行业务逻辑
```

### 系统资源（不走部门检查）

系统资源（user、role、permission、department、excel）的管理权限由全局系统角色控制，不涉及部门作用域：

```
require_permission(Resource::User, Action::Read)
         |
         v
  直接查 JWT 中的 system_role
         |
         |-- super_admin --> 允许（绕过所有检查）
         |-- user + user:read 权限 --> 允许
         |-- 其他 --> 拒绝
```

## 数据模型变更

### 新增

**user_department_roles 表**（替代 user_roles）

| 字段 | 类型 | 说明 |
|---|---|---|
| user_id | BIGINT | 用户 ID |
| department_id | BIGINT | 部门 ID |
| role_id | BIGINT | 角色ID |

- 联合主键：(user_id, department_id)，每个用户在每个部门只能有一个角色
- 用户可以属于多个部门，每个部门分配不同角色

### 废弃

- **user_roles 表** — 被 user_department_roles 替代

### 保留不变

| 表/结构 | 说明 |
|---|---|
| roles | 角色定义（名称、编码、是否系统角色等） |
| role_permissions | 角色-权限关联（role_id + resource_code + action_code） |
| department_resource_access | 部门-资源可见性关联（department_id + resource） |
| user_departments | 用户-部门关联（保留，用于部门归属） |
| 所有业务数据表 | 不加 department_id，数据全部可见 |

## JWT Claims 结构变更

```
当前:
  {
    sub: user_id,
    username: "...",
    display_name: "...",
    is_super_admin: false,
    permissions: ["product:read", "product:write", "bom:read", ...],  // 扁平列表
    exp: ...,
    iat: ...
  }

之后:
  {
    sub: user_id,
    username: "...",
    display_name: "...",
    system_role: "user",                          // 系统角色: "super_admin" | "user"
    dept_roles: [                                  // 部门-角色映射
      { department_id: 1, role_id: 2 },           // 部门1 -> 经理角色
      { department_id: 2, role_id: 3 }            // 部门2 -> 职员角色
    ],
    exp: ...,
    iat: ...
  }
```

关键变化：

- 移除 `permissions` 扁平列表和 `is_super_admin` 标志
- 新增 `system_role`：标识系统角色
- 新增 `dept_roles`：部门-角色映射列表
- 角色的具体权限不在 JWT 中，由运行时内存缓存提供

## 角色体系

### 系统角色（is_system_role = true）

| 角色 | 说明 | 权限 |
|---|---|---|
| super_admin | 超级管理员 | 绕过所有权限检查 |
| user | 普通用户 | user:read, department:read, permission:read |

系统角色全局生效，不绑定部门，管理系统资源（用户、角色、权限、部门、Excel）。

### 业务角色（预置，可修改，可新建）

| 角色 | 说明 | 权限 |
|---|---|---|
| manager | 经理 | 所有业务资源的 read + write + delete |
| staff | 职员 | 所有业务资源的 read |

业务角色在分配给用户时绑定部门作用域，控制业务资源（product、bom、warehouse 等）的操作权限。

管理员可以：
- 修改预置角色的权限（如给 staff 增加 product:write）
- 创建自定义角色并配置权限

## 权限检查的两层模型

### 第一层：部门资源可见性

由 `department_resource_access` 控制。配置某个部门能看到哪些资源类型。

- 部门 A 可见：product、bom、warehouse
- 部门 B 可见：product、bom

如果部门没有关联某个资源，该部门的所有用户（包括经理）都无法操作该资源。

### 第二层：角色操作权限

由 `role_permissions` 控制。定义角色对资源能执行什么操作。

- manager 角色：product:read, product:write, product:delete, bom:read, bom:write, ...
- staff 角色：product:read, bom:read, ...

两层叠加示例：

| 用户 | 部门 | 角色 | 部门可见资源 | 最终权限 |
|---|---|---|---|---|
| 张三 | A 部门 | manager | product, bom, warehouse | product:RWD, bom:RWD, warehouse:RWD |
| 张三 | B 部门 | staff | product, bom | product:R, bom:R |
| 李四 | A 部门 | staff | product, bom, warehouse | product:R, bom:R, warehouse:R |

## 运行时权限缓存

角色权限变更不频繁，启动时加载到内存缓存：

```
启动时: 从 role_permissions 表加载所有角色的权限到 HashMap<role_id, Vec<String>>
运行时: 权限检查从缓存查找，不查数据库
更新时: 角色/权限变更时刷新缓存（可接受短暂延迟）
```

好处：
- JWT 保持精小（只存角色映射，不存权限详情）
- 修改角色权限只需刷新缓存，不需要所有用户重新登录
- 新建部门、分配角色后用户重新登录即可

## gRPC API 影响

### 新增/修改

- 分配用户部门角色：传入 user_id + [(department_id, role_id)] 列表
- 查询用户部门角色：返回 user_id 对应的 [(department, role)] 列表
- 前端请求业务资源接口时需传入 department_id 参数

### 不变

- 角色管理接口（创建/编辑/删除角色、分配权限）
- 部门管理接口（创建/编辑/删除部门、配置资源可见性）
- 所有业务资源的 CRUD 接口（handler 层的 require_permission 写法不变）

## 待后续讨论的问题

- 用户登录时只有一个部门，是否自动选择该部门作为默认上下文？
- 用户被移除某部门后，该部门下的未完成操作如何处理？
- 是否需要一个"获取我所有部门及角色"的接口供前端初始化部门选择器？
