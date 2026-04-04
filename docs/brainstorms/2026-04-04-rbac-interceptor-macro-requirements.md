---
date: 2026-04-04
topic: rbac-interceptor-macro
---

# Requirements: Declarative RBAC via Handler Permission Macro

## Problem Frame

当前每个 gRPC handler 方法都手动重复相同的权限检查样板代码：`extract_auth(&request)?` + `auth.check_permission("resource", "action").map_err(...)`. 101 个调用点分布在 12 个 handler 文件中。新增 RPC 时容易遗漏权限检查，导致安全漏洞。

tonic 的 `with_interceptor` 无法获取 RPC 方法路径，因此无法在 interceptor 层统一做权限检查。需要一种在 handler 层声明式定义权限的方式。

## Requirements

**权限宏设计**

- R1. 创建 `#[require_permission("resource", "action")]` 属性宏（proc macro），标注在 handler 方法上
- R2. 宏自动生成：`extract_auth(&request)?` + `check_permission(resource, action).map_err(error::forbidden)?`
- R3. 宏生成的 `auth` 变量（类型 `AuthContext`）对 handler 方法体可见，无需再手动调用 `extract_auth`
- R4. 宏生成的错误使用现有的 AIP-193 rich error 格式（`error::forbidden(resource, action)`）

**兼容性**

- R5. AuthService 的 handler 方法不加宏，因为 AuthService 不注册 `auth_interceptor`，无需任何改动
- R6. `is_super_admin` 绕过逻辑已在 `AuthContext::check_permission()` 中实现，宏无需额外处理
- R7. `*:*` 通配权限的检查同样由现有 `AuthContext::check_permission()` 处理

**迁移**

- R8. 逐步迁移所有现有 handler 方法，将手动的 `extract_auth` + `check_permission` 替换为宏注解
- R9. 迁移完成后删除 `interceptors::auth::extract_auth` 的 handler 层调用（函数本身保留用于兼容）

## Success Criteria

- 所有 101 个手动权限检查调用点被替换为宏注解
- 新增 RPC 时，handler 方法上必须标注 `#[require_permission]` 才能访问 `auth` 变量（编译期保证）
- 无行为变更：权限检查结果与手动代码完全一致
- handler 代码量减少约 3 行/方法（去掉 extract_auth + check_permission + map_err）

## Scope Boundaries

- **不做**: 在 interceptor 层统一拦截（tonic 限制）
- **不做**: 审计日志（职责分离，单独解决）
- **不做**: 部门级别的数据可见性过滤（另一个需求）
- **不做**: 修改 `AuthContext::check_permission()` 的逻辑
- **不做**: 修改 `RESOURCES` 静态数组或权限模型

## Key Decisions

- **宏 vs Tower 中间件**: 选择宏方案。权限声明与 handler 同位，类似 Spring `@PreAuthorize`，比 Tower 层集成更简单直接
- **宏范围**: 只做权限检查 + AuthContext 提取，不涉及事务管理或 AppState 获取
- **排除机制**: 不需要特殊处理——AuthService 不加 `auth_interceptor`，自然绕过
- **审计日志**: 与权限宏解耦，未来用单独机制解决

## Dependencies / Assumptions

- 现有 `auth_interceptor` 保持不变（JWT 解码 → AuthContext 注入 extensions）
- 现有 `AuthContext::check_permission()` 和 `error::forbidden()` 函数保持不变
- proc macro crate 需要添加到 workspace（宏不能定义在使用它的 crate 里）

## Outstanding Questions

### Deferred to Planning

- [Affects R1][Technical] proc macro crate 放在哪里？`abt-grpc/src/macros/` 还是单独的 workspace crate（如 `abt-macros/`）？
- [Affects R3][Technical] 宏如何让 `auth` 变量对 handler 可见？生成 `let auth = ...;` 语句放在方法体开头？还是通过其他方式？
- [Affects R8][Technical] 迁移顺序：按 handler 文件逐个迁移，还是按权限分组？

## Next Steps

→ `/ce:plan` for structured implementation planning
