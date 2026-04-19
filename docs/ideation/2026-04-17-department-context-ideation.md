---
date: 2026-04-17
topic: department-context-elimination
focus: 消除"每次操作需先确定部门"的摩擦，探索更好的权限模型
---

# Ideation: 部门上下文消除方案

## Codebase Context

**项目**: ABT — Rust gRPC BOM/库存管理系统，PostgreSQL 后端。

**当前权限模型 (Scoped Roles)**:
- JWT 存储 `current_department_id`（登录时自动选定，单部门用户自动选，多部门用户需选择）
- JWT 存储 `dept_roles: HashMap<部门ID, 角色ID列表>`
- `switch_department` 端点切换部门（刷新 token 中的 current_department_id）
- `require_permission` 宏从 auth context 提取 department_id 做权限检查
- 权限检查三步：部门归属校验 → 部门资源可见性 → 角色操作权限

**关键发现**:
- `department_resource_access` 的 seed 给所有部门分配了全部资源，部门资源隔离在默认配置下从未生效
- `check_business_permission` 中的部门资源可见性检查在默认配置下永远通过
- `PermissionRepo::check_permission` 查询旧 `user_roles` 表，与新的 `check_business_permission` 存在两条平行检查路径
- `RolePermissionCache.has_permission` 已支持多角色权限合并
- `DeptResourceAccessRepo.get_departments_accessible_resources` 已有跨部门 UNION 查询
- 业务数据表没有 `department_id` 字段，部门只控制资源类型可见性和操作权限

## Ranked Ideas

### 1. 读写分离 + 请求级部门上下文 (Selected for Brainstorm)
**Description:** JWT 移除 `current_department_id`。读取操作自动遍历用户所有部门取权限并集，写入操作通过 gRPC metadata `x-department-id` header 传入部门上下文（无需刷新 token）。创建操作也通过 header 指定。配合权限地图推送到前端，前端知道在每个部门能做什么。
**Rationale:** 最务实的"最佳平衡点"——既保留了"不同部门不同权限"的语义，又消除了"先选部门"的交互摩擦。`switch_department` API 废弃或降级，JWT 不再因部门切换而重新签发。前端写入请求通过 header 指定部门，登录响应推送完整权限地图控制 UI 可见性。
**Downsides:** 前端写入请求需要附加 header；需要制定"创建操作默认哪个部门"的策略；多部门有同一资源同一权限时需要优先级策略。
**Confidence:** 85%
**Complexity:** Medium
**Status:** Selected for brainstorm

### 2. 读写分离策略 (Read-Union / Write-Single)
**Description:** 读取操作自动合并用户所有部门的可见资源和权限，写入操作才要求明确的部门上下文。80% 的操作是查看/搜索，天然跨部门。
**Rationale:** `check_business_permission` 对三种操作用完全相同的检查逻辑是不必要的约束。`RolePermissionCache.has_permission` 已支持多角色合并，`DeptResourceAccessRepo` 已有跨部门 UNION 查询。
**Downsides:** 写入操作仍需部门上下文，前端需要区分读写请求。
**Confidence:** 90%
**Complexity:** Low
**Status:** Unexplored

### 3. 多部门权限并集 (彻底消除部门选择)
**Description:** 废弃 `current_department_id`，合并所有部门角色权限取并集，任何请求都使用合并后的权限集。`switch_department` API 直接废弃。
**Rationale:** `dept_roles` HashMap 已包含全量信息，`current_department_id` 是人为窄化。基础设施 90% 就位。
**Downsides:** 可能过度授权——合并所有部门的最大权限，对"同人在不同部门权限应不同"的需求不满足。
**Confidence:** 85%
**Complexity:** Low
**Status:** Unexplored

### 4. 隐式部门路由 (资源为第一公民)
**Description:** 颠覆"先选部门再操作资源"为"直接操作资源，系统自动推断部门"。权限检查时从用户所有部门中找到第一个同时满足"有该资源权限"+"有对应操作角色"的部门。
**Rationale:** 用户心智模型是"编辑这个 BOM"而非"我现在处于哪个部门"。`DeptResourceAccessCache` 的倒排索引可在缓存加载时构建。
**Downsides:** 多部门有同一资源同一权限时需优先级策略；创建操作仍需部门上下文。
**Confidence:** 80%
**Complexity:** Medium
**Status:** Unexplored

### 5. 视图过滤 (角色管操作，部门管可见性)
**Description:** 用户拥有全局角色，角色→权限是全局的；部门只作为数据过滤条件（SQL WHERE），决定查询返回哪些行。
**Rationale:** `department_resource_access` 的 seed 给所有部门分配了全部资源，部门资源隔离从未被真正启用。
**Downsides:** 需要业务数据表加 `department_id` 实现行级过滤，与设计决策 #6（不加 department_id）矛盾。
**Confidence:** 75%
**Complexity:** High
**Status:** Unexplored

### 6. 去部门化纯角色 (部门退化为组织属性)
**Description:** 砍掉 `department_resource_access` 表和 JWT 中 `current_department_id`，角色分配回到全局。部门仅作为组织结构展示。
**Rationale:** 只有 2 个业务角色（manager/staff），继承机制完全未使用。两条平行检查路径存在不一致。
**Downsides:** 丧失"同人在不同部门有不同权限"的能力。
**Confidence:** 70%
**Complexity:** Medium
**Status:** Unexplored

### 7. 权限地图推送 (前端一次性获取完整权限)
**Description:** 登录时在 `LoginResponse` 中推送结构化权限地图。前端据此控制 UI 元素可见性，提前隐藏/禁用无权限按钮，提示"切换到部门 B 即可执行此操作"。
**Rationale:** `RolePermissionCache` 和 `DeptResourceAccessCache` 已在内存中维护完整数据，只差组装和暴露到 API。可与任何上述方案组合。
**Downsides:** 登录响应体积增大（约 24 权限码 × 部门数，可接受）。
**Confidence:** 95%
**Complexity:** Low
**Status:** Unexplored

## Rejection Summary

| # | Idea | Reason Rejected |
|---|------|-----------------|
| 1 | 智能登录选择（记住上次部门） | 不够根本——只是在选择体验上优化，没解决"为什么需要选" |
| 2 | 前端多标签页 | 纯前端方案，后端仍需支持切换，token 管理复杂 |
| 3 | ABAC 多属性策略引擎 | 13 资源 × 3 操作规模不需要 ABAC，过度工程 |
| 4 | 能力矩阵（砍掉角色层） | 太激进，不符合企业系统惯例 |
| 5 | 标签空间 | 没有行级隔离需求，标签系统过度设计 |
| 6 | 权限即代码 | 与可动态配置权限的需求方向相反 |
| 7 | 事件驱动外部策略服务 | 对单体 gRPC 服务过度架构 |
| 8 | 声明式 proto 元数据 | 与当前宏方案冲突太大，收益不在部门选择维度 |
| 9 | 部门标签化 | 太模糊，被去部门化方案更好覆盖 |
| 10 | 预计算权限矩阵 | 与多部门并集重叠，额外缓存层无额外收益 |
| 11 | Interceptor 预推断 | 是实现方式而非方案本身，可并入其他方案 |
| 12 | 能力令牌 (JWT 编码权限) | 被方案 7（请求级部门）更好覆盖 |
| 13 | 资源级 ACL | 过度设计，当前需求是消除部门选择而非细化到实例级 |
| 14 | GitHub 三级关系模型 | 重构太大，视图过滤方案更实用 |
| 15 | 缓存自动失效 | 正确性改进，但不解决部门选择摩擦 |
| 16 | 编译期 RPC 覆盖检查 | 安全改进，不在本次焦点上 |
| 17 | 宏增强审计日志 | 不在本次焦点上 |
| 18 | 消除三源定义漂移 | 不在本次焦点上 |

## Session Log
- 2026-04-17: Initial ideation — ~40 raw ideas generated across 4 frames (pain/friction, inversion/automation, assumption-breaking, leverage/compounding), deduped to ~25 unique candidates, 7 survived adversarial filtering. User selected idea #1 (读写分离+请求级部门上下文) for brainstorm.
