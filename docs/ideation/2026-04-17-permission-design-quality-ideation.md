---
date: 2026-04-17
topic: permission-design-quality-audit
focus: 当前权限设计的质量审计——设计缺陷、简化机会、与大厂实践的差距
---

# Ideation: 权限系统设计质量审计

## Codebase Context

**项目**: ABT — Rust gRPC BOM/库存管理系统，PostgreSQL 后端。

**当前权限模型 (Global RBAC，简化后)**:
- 全局角色：`user_roles` 表 (user_id + role_id)，无部门范围
- JWT Claims：sub, username, display_name, system_role, role_ids: Vec\<i64\>, exp/iat
- RolePermissionCache：启动时全量加载，role_id → HashSet\<resource:action\>，含继承解析和循环检测
- 权限检查：super_admin 绕过 → RolePermissionCache 查询
- `require_permission` 宏：104 个调用点，支持 async_trait Box::pin 穿透
- 4 角色 (super_admin, admin, manager, staff)，13 资源，3 操作 (read/write/delete)

**与大厂对比的关键差距**:
- 无 "deny override" 能力（仅 allow-based）
- 无条件检查（时间、IP、上下文）
- 无自动缓存失效机制
- 无权限检查审计轨迹（仅变更审计）
- 两条平行权限检查路径（PermissionService 查 DB vs check_permission_for_resource 查缓存）
- 核心权限检查函数零测试覆盖
- BatchAssignRoles 存在 PostgreSQL 65535 参数溢出风险

**Industry 参照**:
- Google Zanzibar (ReBAC): 规模远超需要，13 资源不需要关系型权限
- K8s RBAC: namespace 隔离模式可参考但当前不需要
- AWS IAM: 条件策略、Deny 优先原则值得借鉴但复杂度不匹配
- OPA / Casbin: 外部策略引擎对单体 gRPC 服务过度架构
- **结论**: 当前简化 RBAC 模型适合系统规模，不需要升级到 ABAC/ReBAC/PBAC

## Ranked Ideas

### 1. 权限测试基础建设 (Permission Testing Foundation)
**Description:** 为 `check_permission_for_resource`、`check_system_permission`、`check_business_permission` 建立单元测试覆盖，可选扩展为宏自动生成测试用例（为所有 104 个 `require_permission` 调用点生成 super_admin/普通用户/空角色/系统vs业务资源 分支测试）。
**Rationale:** 零测试覆盖是当前最大的安全风险。104 个端点的访问控制依赖这 3 个函数，已经因此出过 fail-open 漏洞（`docs/solutions/security-issues/permission-cache-fail-open-and-migration-data-loss-2026-04-17.md`）。测试生成宏让每个新增端点自动获得覆盖。
**Downsides:** 纯投入型工作，不直接产生功能。宏测试生成器需约 3 天开发。
**Confidence:** 95%
**Complexity:** Low (手写测试) / Medium (宏生成)
**Status:** Unexplored

### 2. 统一双轨权限检查路径 (Unify Dual Permission Paths)
**Description:** 消除 `PermissionService::check_permission`（查 DB `user_roles` 表）和 `check_permission_for_resource`（查内存 RolePermissionCache）两条平行路径，统一为单一权威路径。
**Rationale:** 两条路径是当前最核心的设计缺陷。它们可能对相同输入返回不同结果（缓存失效期间、super_admin 判断逻辑差异），且开发者不知道该调用哪个。`PermissionRepo::check_permission` 和 `check_business_permission` 在数据源和判断逻辑上都存在分歧。
**Downsides:** 需审计所有调用点确定哪个路径被实际使用。如果 PermissionService 有外部消费者（如供前端查询权限），需保留查询能力但移除检查能力。
**Confidence:** 85%
**Complexity:** Medium
**Status:** Unexplored

### 3. 数据库通知自动刷新缓存 (Auto Cache Invalidation via DB Notifications)
**Description:** 在 `role_permissions` 和 `roles` 表上添加 PostgreSQL `LISTEN/NOTIFY` 触发器，权限变更时自动刷新 `RolePermissionCache`，消除手动 `refresh()` 调用。
**Rationale:** 手动刷新是安全风险——紧急撤销权限时如果忘记刷新，用户在 JWT 过期前（最长 1 小时）仍可使用旧权限。LISTEN/NOTIFY 是 PostgreSQL 原生能力，sqlx 已支持 `PgListener`。可扩展为 `invalidate_role(role_id)` 增量刷新。
**Downsides:** 需后台任务管理。多实例部署时每个实例需独立监听。需数据库触发器。
**Confidence:** 80%
**Complexity:** Medium
**Status:** Unexplored

### 4. 前端权限地图 (Frontend Permission Map)
**Description:** 添加 `GetPermissionMatrix` gRPC 端点，返回从 proto `Resource`/`Action` enums 派生的结构化权限矩阵，前端据此预隐藏无权限 UI 元素。
**Rationale:** 当前前端只能通过 403 响应发现权限不足，用户体验差。Proto enums 已经是权限定义的来源，只需暴露。`PermissionCode` trait 已有 `code()` 转换逻辑。预估减少 60-80% 无效 API 调用。
**Downsides:** 登录响应体积增大（~24 权限码 × 角色数，可接受）。前端需权限消费逻辑。
**Confidence:** 90%
**Complexity:** Low
**Status:** Unexplored

### 5. 移除僵尸复杂度 (Remove Dead Complexity)
**Description:** 三合一清理：(a) JWT 移除 `system_role` 字段，用 `role_ids` 判断 super_admin；(b) 移除未使用的角色继承机制（`parent_role_id`、DFS 解析、循环检测）；(c) 合并 AuthContext 到 Claims 消除冗余转换层。
**Rationale:** 4 个角色的系统不需要树形继承（当前 seeded data 无任何继承关系）。`system_role` 和 `role_ids` 职责重叠。Claims → AuthContext 字段几乎相同，每次请求都做冗余转换。这些"幽灵代码"增加认知负担但无价值。
**Downsides:** 需更新 JWT 格式（强制重新登录）。移除继承后如未来需要需重新加回。AuthContext 合并涉及拦截器改造。
**Confidence:** 75%
**Complexity:** Medium
**Status:** Unexplored

### 6. 权限分配安全防护 (Permission Assignment Safety)
**Description:** 两项具体修复：(a) `BatchAssignRoles` 改用分批处理或 `unnest` 避免 PostgreSQL 65535 绑定参数溢出；(b) `role_permissions` 表添加约束，防止分配不存在的 `resource:action` 组合。
**Rationale:** 具体缺陷而非设计改进。批量导入用户时参数溢出导致静默失败和部分权限丢失。幽灵权限引用（如拼写错误 "product:delet"）永远不会匹配任何检查，造成难以调试的权限缺口。
**Downsides:** 外键约束需 `resource_actions` 参照表或 CHECK 枚举。
**Confidence:** 95%
**Complexity:** Low
**Status:** Unexplored

### 7. 权限定义单一来源 (Single Source of Truth for Permission Definitions)
**Description:** 消除 proto enums（`Resource`/`Action`）、`RESOURCES` 静态数组（`abt/src/models/resources.rs`）、`PermissionCode` trait 三处权限定义的冗余，从 proto 定义通过 build script 自动派生其他两处。
**Rationale:** 三处定义已导致过漂移（`docs/solutions/developer-experience/permission-proto-enum-migration-2026-04-12.md` 记录了手动同步的痛苦）。每次新增资源需改三个地方。统一后只需改 proto 文件。
**Downsides:** 需 build script 或 derive macro 从 proto 生成代码。改变构建流程。
**Confidence:** 80%
**Complexity:** Medium
**Status:** Unexplored

## Rejection Summary

| # | 想法 | 拒绝原因 |
|---|------|----------|
| 1 | ABAC 替代 RBAC | 13 资源 × 3 操作规模不需要 ABAC，过度工程，已在之前 ideation 中拒绝 |
| 2 | JWT 直接携带权限列表 | 角色权限变更需所有用户重新登录，与当前 role_ids + cache 方案相比降低了灵活性 |
| 3 | 移除 delete 操作合并到 write | 太主观，业务可能需要独立的删除权限控制 |
| 4 | 移除 super_admin 改为权限提升 | 过于激进，缺乏业务驱动理由，实现复杂度高 |
| 5 | 权限检查移到拦截器层 | 拦截器无法看到 gRPC 方法路径，技术上不可行（已在 ideation doc 中记录） |
| 6 | 配置文件替代数据库权限 | 与"权限可通过 gRPC API 动态管理"的需求冲突 |
| 7 | 实例级权限 | 需要业务表加 department_id，与已确定的设计决策矛盾，重大架构变更 |
| 8 | 去掉缓存直接查 DB | 对每个请求增加 DB 依赖，当前缓存方案更可靠 |
| 9 | Permission Hash 加速 | 过早优化，当前 QPS 不需要，ROI 不成立 |
| 10 | JIT 缓存加载 | 过度工程，当前启动时全量加载已够用 |
| 11 | 角色图可视化 | Nice-to-have 但不解决核心问题 |
| 12 | 权限审计日志（记录每次检查） | 高 QPS 下性能影响大，需异步写入，复杂度高 |
| 13 | 编译时权限验证 | 与 #7（单一来源）重叠，#7 方案更实际 |
| 14 | 去掉 PermissionCode trait | trait 存在是因为 crate 依赖边界（abt 不能引用 abt-grpc），移除违反架构约束 |
| 15 | ResourceActionDef 改为数据库驱动 | 13 资源不需要运行时动态扩展，编译时确定更安全 |
| 16 | 缓存反向索引 | 没有当前使用场景，YAGNI |
| 17 | 宏内联缓存查找 | 微优化，函数调用开销可忽略 |
| 18 | 条件权限引擎 | 没有业务用例驱动，纯技术前瞻 |
| 19 | Deny override 语义 | 角色继承本身都未使用，在未使用的继承上加 deny 语义没有价值 |

## Session Log
- 2026-04-17: 初始构思 — ~40 raw ideas 生成（4 frames: pain/risk, simplification, assumption-breaking, leverage），去重到 ~25 候选，7 个幸存者通过对抗性过滤。用户确认全部 7 个。
