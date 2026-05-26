---
title: "feat: JWT 内嵌已解析权限 — 前端权限地图"
type: feat
status: active
date: 2026-04-17
origin: docs/ideation/2026-04-17-permission-design-quality-ideation.md (Idea #4)
---

# feat: JWT 内嵌已解析权限 — 前端权限地图

## Overview

在 JWT Claims 中添加 `permissions: Vec<String>` 字段，登录时从 `RolePermissionCache` 解析用户角色的完整权限列表并写入 JWT。前端已有的 `PermissionGuard` / `hasPermission` 基础设施无需修改即可消费权限数据，实现按钮/菜单的预隐藏。

同时清理前端 `SessionData` 中旧系统遗留的 `currentDepartmentId`、`deptRoles` 和 `switchDepartment` action。

## Problem Frame

当前非 super_admin 用户的前端权限完全失效：
- 后端 JWT 只含 `role_ids: Vec<i64>`（角色 ID），不含已解析权限
- 前端 `SessionData.permissions` 从 `payload.permissions` 提取，但 JWT 无此字段 → 始终为 `[]`
- `PermissionGuard` 检查 `permissions.includes(code)` 永远返回 false
- 结果：只有 super_admin（`isSuperAdmin` 绕过）能正常看到操作按钮

## Requirements Trace

- R1. JWT Claims 添加 `permissions: Vec<String>` 字段
- R2. 登录时从 `RolePermissionCache::get_merged_permissions(role_ids)` 解析权限，转换为大写格式（如 `"PRODUCT:WRITE"`）写入 JWT
- R3. `refresh_token` 同样重新解析权限，角色变更后刷新 token 即可生效
- R4. `get_user_claims` 同样包含 permissions
- R5. 前端 `SessionData` 移除 `currentDepartmentId`、`deptRoles` 字段
- R6. 前端移除 `switchDepartment` action
- R7. 前端中间件移除 `context.locals.currentDepartmentId`、`context.locals.deptRoles`
- R8. 验证非 super_admin 用户的按钮/菜单正确显示

## Scope Boundaries

- 不修改 proto 定义（permissions 在 JWT payload 中，不在 proto message 中）
- 不修改 `PermissionGuard` 组件、`hasPermission` 函数、`toPermissionCode` 函数
- 不修改 `RolePermissionCache` 的内部存储格式
- 不实现缓存自动失效（Idea #3，单独计划）
- 不实现独立权限查询端点（登录 + refresh 已满足当前需求）

## Context & Research

### Relevant Code and Patterns

**后端：**
- `abt/src/models/auth.rs` — `Claims` 结构体，需添加 `permissions` 字段
- `abt/src/implt/auth_service_impl.rs` — `build_claims()` 签发 JWT，需解析权限；`login()`/`refresh_token()`/`get_user_claims()` 调用 `build_claims`
- `abt/src/permission_cache.rs` — `RolePermissionCache::get_merged_permissions(&role_ids)` 返回 `HashSet<String>`（小写格式 `"product:write"`）
- `abt/src/lib.rs:73-78` — `get_permission_cache()` 全局单例访问器
- `abt/src/lib.rs:187-199` — `get_auth_service()` 工厂函数

**前端（`E:\work\front\abt_front`）：**
- `src/lib/session.ts` — `createSession()` 已从 `payload.permissions` 提取权限，无需修改
- `src/lib/permissions.ts` — `hasPermission()` 已支持 super_admin 绕过 + 精确匹配 + 通配符
- `src/lib/permission-codes.ts` — `toPermissionCode()` 生成大写格式 `"PRODUCT:WRITE"`
- `src/lib/usePermission.svelte.ts` — Svelte context 注入 `hasPermission` 函数
- `src/components/ui/PermissionGuard.svelte` — 使用 `hasPermission(toPermissionCode(resource, action))` 条件渲染
- `src/middleware/index.ts` — 从 session 注入 `context.locals.permissions`
- `src/components/admin/AdminProviders.svelte` — 创建 permission context
- `src/actions/auth.ts` — 登录/登出/switchDepartment actions

### Institutional Learnings

- RolePermissionCache 存储格式为小写 `"product:write"`，前端 `toPermissionCode` 生成大写 `"PRODUCT:WRITE"` — 需在写入 JWT 时转换大小写
- `get_permission_cache()` 是全局 OnceLock 单例，可在 auth service 中直接调用

### 数据流

```
登录请求 → AuthServiceImpl::login()
  → AuthRepo::get_user_role_ids() → role_ids
  → get_permission_cache().get_merged_permissions(&role_ids) → HashSet<String> (小写)
  → .to_uppercase() 转换 → Vec<String>
  → build_claims(... permissions) → Claims { permissions: ["PRODUCT:WRITE", ...] }
  → sign_jwt(claims) → JWT token

前端:
  JWT payload.permissions → SessionData.permissions (已有提取逻辑 ✅)
  → middleware → context.locals.permissions → AdminProviders → Svelte context
  → PermissionGuard → hasPermission("PRODUCT:WRITE") → 显示/隐藏按钮
```

## Key Technical Decisions

- **权限放 JWT 而非独立端点**: 前端 `createSession()` 已从 JWT payload 提取 `permissions`，无需前端登录流程改动。角色变更后刷新 token 即可生效。
- **大写格式 `"PRODUCT:WRITE"`**: 前端 `toPermissionCode()` 生成大写，`hasPermission()` 做精确匹配。后端写入 JWT 时统一 `.to_uppercase()` 转换。
- **不修改 RolePermissionCache 内部格式**: 缓存内部保持小写，仅在 JWT 输出时转换。避免影响现有权限检查路径。

## Open Questions

### Deferred to Implementation

- `AuthServiceImpl` 当前不持有 `RolePermissionCache` 引用。实现时决定是注入还是通过全局单例 `get_permission_cache()` 访问（推荐后者，因为 cache 是 OnceLock 单例）。

## Implementation Units

### 后端（Backend）

- [ ] **Unit 1: Claims 添加 permissions 字段**

**Goal:** 在 Claims 结构中添加 `permissions: Vec<String>` 字段，更新 `build_claims` 方法签名。

**Requirements:** R1, R2

**Dependencies:** None

**Files:**
- Modify: `abt/src/models/auth.rs`
- Modify: `abt/src/implt/auth_service_impl.rs`
- Test: `abt/src/tests/auth_tests.rs`

**Approach:**
- `Claims` 添加 `pub permissions: Vec<String>` 字段
- `build_claims` 签名新增 `permissions: Vec<String>` 参数，直接赋值给 Claims
- `build_claims` 的调用方（login、refresh_token、get_user_claims）负责解析 permissions 并传入

**Patterns to follow:**
- 现有 `build_claims` 参数传递模式

**Test scenarios:**
- Happy path: build_claims 接收 permissions 并正确写入 Claims
- Edge case: 空 role_ids 产生空 permissions
- Serialization: Claims 序列化为 JSON 后 permissions 字段正确

**Verification:**
- `cargo test -p abt -- auth_tests` 通过
- `cargo build -p abt` 成功

---

- [ ] **Unit 2: 登录/刷新时解析权限**

**Goal:** login、refresh_token、get_user_claims 三个方法中通过 RolePermissionCache 解析 role_ids → permission strings，传入 build_claims。

**Requirements:** R2, R3, R4

**Dependencies:** Unit 1

**Files:**
- Modify: `abt/src/implt/auth_service_impl.rs`

**Approach:**
- 在 `login`、`refresh_token`、`get_user_claims` 中，获取 `role_ids` 后调用 `get_permission_cache().get_merged_permissions(&role_ids)` 获取 `HashSet<String>`
- 将小写权限字符串转为大写：`.iter().map(|p| p.to_uppercase()).collect::<Vec<String>>()`
- 传入 `build_claims` 的新参数
- 使用 `crate::get_permission_cache()` 全局单例访问缓存（无需注入）

**Patterns to follow:**
- 现有 `AuthRepo::get_user_role_ids()` 调用模式
- 全局单例 `get_permission_cache()` 访问模式（参考 `abt/src/lib.rs:76`）

**Test scenarios:**
- Happy path: 用户有 manager 角色 → JWT 包含 `["PRODUCT:READ", "PRODUCT:WRITE", ...]`
- Happy path: super_admin 用户 → JWT 包含所有权限（因为角色关联了所有权限）
- Edge case: 用户无角色 → `permissions: []`
- Integration: refresh_token 返回新 JWT 包含最新权限
- Integration: get_user_claims 返回的 Claims 包含 permissions

**Verification:**
- `cargo test -p abt` 通过
- `cargo test -p abt-grpc` 通过
- 手动验证：登录后 JWT payload 中包含 permissions 字段

### 前端（Frontend）

- [ ] **Unit 3: 清理 SessionData 旧字段**

**Goal:** 从 `SessionData` 和相关代码中移除 scoped roles 遗留字段。

**Requirements:** R5, R7

**Dependencies:** Unit 2 (后端 JWT 不再包含旧字段)

**Files:**
- Modify: `src/lib/session.ts`
- Modify: `src/middleware/index.ts`

**Approach:**
- `SessionData` 接口移除 `currentDepartmentId`、`deptRoles` 字段
- `createSession()` 移除 `rawDeptRoles` 解析逻辑和 `currentDepartmentId` 赋值
- `middleware/index.ts` 移除 `context.locals.currentDepartmentId`、`context.locals.deptRoles` 注入
- 保留 `permissions: string[]` — 这是核心字段
- 保留 `systemRole: string` — 用于判断 super_admin

**Patterns to follow:**
- 现有 TypeScript 接口清理模式

**Test scenarios:**
- Happy path: 登录后 session 包含 `permissions`、`systemRole`，不含 `currentDepartmentId`/`deptRoles`
- Integration: middleware 正确注入 `context.locals.permissions`

**Verification:**
- `npm run build` 成功（TypeScript 编译通过）
- 无 TypeScript 类型错误

---

- [ ] **Unit 4: 移除 switchDepartment action**

**Goal:** 删除 `switchDepartment` action 及相关代码。

**Requirements:** R6

**Dependencies:** Unit 3

**Files:**
- Modify: `src/actions/auth.ts`

**Approach:**
- 删除 `switchDepartment` action 定义（约第 77-110 行）
- 搜索代码库中是否有其他引用 `switchDepartment` 的地方（组件、页面等），一并清理

**Patterns to follow:**
- 现有 action 定义结构

**Test scenarios:**
- Test expectation: none — 纯删除操作，TypeScript 编译通过即可验证

**Verification:**
- `npm run build` 成功
- 搜索确认无 `switchDepartment` 引用残留

---

- [ ] **Unit 5: 端到端验证**

**Goal:** 验证非 super_admin 用户的完整权限链路。

**Requirements:** R8

**Dependencies:** Unit 2 (后端), Unit 3, Unit 4 (前端)

**Files:**
- None (测试验证)

**Approach:**
- 使用测试账号（非 super_admin）登录
- 检查浏览器 DevTools → Application → Cookie → session_id
- 解码 JWT，确认 `permissions` 数组包含预期的大写权限码（如 `["WAREHOUSE:READ", "PRODUCT:READ", ...]`）
- 验证前端页面：该用户角色应有权限的按钮显示，无权限的按钮隐藏
- 验证侧边栏菜单根据 READ 权限正确过滤

**Test scenarios:**
- Integration: staff 角色用户登录 → 只看到 READ 权限对应的按钮和菜单
- Integration: admin 角色用户登录 → 看到 READ + WRITE 权限对应的按钮
- Integration: super_admin 登录 → 所有按钮和菜单可见（isSuperAdmin 绕过）
- Integration: refresh_token 后权限保持一致

**Verification:**
- 三种角色用户分别验证 UI 表现符合预期
- JWT payload 中 permissions 格式为大写 `"RESOURCE:ACTION"`

## System-Wide Impact

- **JWT 格式变更:** 添加 `permissions` 字段，旧 JWT 无此字段。旧 JWT 的 `permissions` 在前端回退为 `[]` — 不影响 super_admin（绕过检查），但普通用户需重新登录
- **JWT 体积:** 13 资源 × 3 操作 = 最大 39 个权限字符串，每个约 15 字节 → 约 600 字节增量，在 JWT 可接受范围内
- **refresh_token 语义增强:** 刷新 token 时重新解析权限，角色变更无需等到 JWT 过期
- **前端无破坏性变更:** `hasPermission()`、`PermissionGuard`、`toPermissionCode()` 完全不改

## Risks & Dependencies

| Risk | Mitigation |
|------|------------|
| 旧 JWT 无 permissions 字段，普通用户需重新登录 | 部署时通知用户刷新页面（重新登录）；与当前简化改造的强制重登录策略一致 |
| RolePermissionCache 启动时未加载完成 | 现有 cache 使用 `.expect()` 阻止启动；登录请求在服务就绪后才能到达 |
| 权限字符串大小写不匹配 | 后端统一 `.to_uppercase()` 输出；前端 `toPermissionCode()` 生成大写；格式一致 |
| 前端 `context.locals` 类型定义需同步更新 | 中间件修改时同步更新 `src/env.d.ts` 或 `src/types/` 中的类型定义 |

## 前端实施说明

此计划的前端部分（Unit 3-5）可直接交给前端团队实施。关键信息：

**前端不需要改的部分：**
- `src/lib/permissions.ts` — `hasPermission()` 函数不变
- `src/lib/permission-codes.ts` — `toPermissionCode()` 不变
- `src/components/ui/PermissionGuard.svelte` — 组件逻辑不变
- `src/lib/usePermission.svelte.ts` — Svelte context 不变
- `src/components/admin/AdminProviders.svelte` — 不变

**前端需要改的部分：**
- `src/lib/session.ts` — `SessionData` 接口清理，`createSession()` 简化
- `src/middleware/index.ts` — 移除旧字段注入
- `src/actions/auth.ts` — 删除 `switchDepartment` action
- `src/env.d.ts` — 更新 `Astro.GlobalLocals` 类型（如果有类型定义）

**权限数据来源：**
- 后端 JWT payload 新增 `permissions: string[]` 字段（大写格式如 `"PRODUCT:WRITE"`）
- 前端 `createSession()` 已有 `permissions: (payload.permissions as string[]) || []` 提取逻辑 — **无需修改**

**验证方法：**
- 登录后在浏览器控制台执行 `atob(document.cookie...)` 解码 JWT 查看 permissions 字段
- 或在 DevTools Network 面板查看登录响应中的 token

## Sources & References

- **Origin document:** [docs/ideation/2026-04-17-permission-design-quality-ideation.md](docs/ideation/2026-04-17-permission-design-quality-ideation.md) (Idea #4)
- **Related simplification plan:** [docs/plans/2026-04-17-001-refactor-simplify-permission-plan.md](docs/plans/2026-04-17-001-refactor-simplify-permission-plan.md)
- **Related solution:** [docs/solutions/security-issues/permission-cache-fail-open-and-migration-data-loss-2026-04-17.md](docs/solutions/security-issues/permission-cache-fail-open-and-migration-data-loss-2026-04-17.md)
