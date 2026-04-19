---
date: 2026-04-17
topic: simplify-permission-remove-dept
---

# Simplify Permission System — Remove Department from Permission Checks

## Problem Frame

当前权限系统引入了 Scoped Roles，要求"用户在不同部门可以有不同角色"，导致每次操作都需要先确定部门上下文。具体表现为：

- JWT 存储 `current_department_id`（单值），多部门用户需要手动选择或切换
- `switch_department` API 切换部门需要重新签发 JWT（完整网络往返 + token 刷新）
- `require_permission` 宏在每次权限检查时执行三步：部门归属 → 部门资源可见性 → 角色权限
- `department_resource_access` 的 seed 给所有部门分配了全部资源，部门隔离在默认配置下未生效

实际业务中，部门只用于组织结构，不需要控制权限。权限应该跟着人走——用户有什么角色就有什么权限，与部门无关。

## Requirements

**JWT 与认证上下文**

- R1. JWT Claims 移除 `current_department_id` 和 `dept_roles` 字段，新增 `role_ids: Vec<i64>` 存储用户的全局角色 ID 列表
- R2. AuthContext 结构同步简化：移除 `current_department_id` 和 `dept_roles`，新增 `role_ids: Vec<i64>`
- R3. 登录时不再需要部门选择步骤（移除 `resolve_default_department` 逻辑）

**权限检查**

- R4. 业务资源权限检查简化为两步：(1) super_admin 直接放行 (2) 查 RolePermissionCache 判断用户角色是否包含所需权限
- R5. 移除权限检查中的部门归属校验（`belongs_to_department`）和部门资源可见性检查（`DeptResourceAccessCache`）
- R6. `require_permission` 宏的对外签名不变：`#[require_permission(Resource::X, Action::Y)]`，内部移除 `department_id` 参数传递
- R7. 系统资源权限检查（`check_system_permission`）保持不变

**数据模型**

- R8. 复用现有 `user_roles` 表（migration 010 已创建，migration 017 保留）用于全局角色分配（联合主键 user_id + role_id，支持多角色）
- R9. 移除 `user_department_roles` 表
- R10. 移除 `department_resource_access` 表
- R11. 保留 `departments` 表和 `user_departments` 表（部门仅作为组织结构，用于展示和报表）

**API 变更**

- R12. 移除 `switch_department` gRPC 端点
- R13. 角色分配 API 改为全局操作：复用 UserRepo 现有的 assign_roles/remove_roles 方法（操作 user_roles 表），要求调用者为 super_admin
- R14. 部门基础管理 API 保持不变（创建/编辑/删除部门、分配用户到部门）
- R14a. 移除部门 handler 中的 scoped-role 相关端点：assign_user_department_roles、remove_user_department_roles、get_user_department_roles、set_department_resources、get_department_resources（这些端点依赖 R9/R10 中被移除的表）

**迁移**

- R15. 从 `user_department_roles` 迁移数据到 `user_roles`：先清空旧数据，再 INSERT DISTINCT user_id, role_id（多部门角色取并集，详见 Outstanding Questions）
- R16. 部署时强制所有旧 JWT 失效（用户需重新登录）。Claims 结构不兼容旧格式，不做向后兼容

**缓存**

- R17. 移除 `DeptResourceAccessCache` 及其全局单例
- R18. 保留 `RolePermissionCache`（含继承链解析和循环检测）
- R19. 审计所有直接查询 user_roles 的代码路径（PermissionRepo、UserRepo），确认 user_roles 成为唯一角色数据源，消除与 JWT 中 role_ids 的不一致风险

## Before/After 对比

| 维度 | Before (Scoped Roles) | After (Global Roles) |
|---|---|---|
| JWT 部门相关字段 | `dept_roles: HashMap` + `current_department_id` | 无 |
| JWT 角色字段 | 通过 dept_roles 间接获取 | `role_ids: Vec<i64>` |
| 权限检查步骤 | 部门归属 → 部门可见性 → 角色权限 | super_admin? → 角色权限 |
| 部门切换 | `switch_department` API + token 刷新 | 不需要 |
| 部门可见性表 | `department_resource_access` | 移除 |
| 用户角色表 | `user_department_roles(user_id, dept_id, role_id)` | 现有 `user_roles(user_id, role_id)` |
| 部门用途 | 权限边界 + 组织结构 | 仅组织结构 |
| 登录部门选择 | 多部门用户需选择 | 不需要 |

## Success Criteria

- 所有 `require_permission` 注解的 handler 方法正常工作，宏签名不变
- 权限检查不再依赖部门上下文，不需要 `current_department_id`
- 用户拥有哪些角色就拥有哪些权限，与部门无关
- 部门组织结构功能（列表、成员）不受影响
- 角色继承和权限缓存正常工作
- 编写测试覆盖权限检查路径：super_admin、普通用户单角色、普通用户多角色

## Scope Boundaries

- 不改变角色定义（roles 表结构不变）
- 不改变角色权限配置（role_permissions 表不变）
- 不改变业务数据表结构（不加 department_id）
- 不实现前端权限地图推送
- 不引入新的权限模型（ABAC、标签等）
- 部门成员管理 API 保留但与权限脱钩

## Key Decisions

- **权限跟人走，不跟部门走**: 用户的全局角色决定权限，部门只用于组织归属。Rationale: 当前 `department_resource_access` 给所有部门分配了全部资源，部门隔离未生效；实际业务不需要同人在不同部门有不同权限。
- **JWT 存 role_ids 而非 permissions**: 运行时查 RolePermissionCache。Rationale: 角色权限变更只需刷新缓存，无需所有用户重新登录；JWT 保持精小。
- **直接简化而非渐进**: 直接移除部门参与权限的代码和表结构。Rationale: scoped roles 已实现但使用时间短，前端和其他消费者可能已依赖这些 API（需确认）；如果依赖存在，需要协调部署。
- **前端不做权限预判**: 前端按 403 响应处理无权限情况，不推送权限列表。Rationale: 减少改动范围，前端可以后续迭代。

## Dependencies / Assumptions

- `user_department_roles` 表已存在并有数据（需要迁移）
- `RolePermissionCache` 已实现继承链解析和循环检测（可复用）
- `user_roles` 表已存在（migration 010），migration 017 保留了旧数据，迁移前需清空
- R15 迁移取角色并集可能导致权限扩大：跨部门拥有不同角色的用户将获得所有角色的全局权限。需在生产数据中验证影响范围

## Outstanding Questions

### Resolve Before Planning

无（所有决策已确认：JWT 强制重新登录、角色分配复用 UserRepo + super_admin 权限、移除 scoped-role 端点）

### Deferred to Planning

- [Affects R6][Technical] `require_permission` 宏如何处理 `department_id` 参数的移除？推荐：删除参数，宏直接调用 `check_permission_for_resource(&auth, resource, action)` 无 department_id
- [Affects R13][Technical] 角色分配 gRPC API 的 proto 定义如何调整？
- [Affects R15][Technical] 迁移前需清空旧 user_roles 中的过期数据（migration 017 保留了旧数据）
- [Affects R14a][Technical] 需移除的 5 个 scoped-role 端点的 proto message 和 service 定义如何清理？
- [Affects R19][Technical] 确认 PermissionRepo::check_permission 和 PermissionRepo::get_user_permission_codes 在新架构下的角色（这些方法直接查 user_roles 表）

## Next Steps

-> `/ce:plan` for structured implementation planning
