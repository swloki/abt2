---
date: 2026-04-12
topic: permission-proto-enums
---

# Proto 定义权限枚举

## Problem Frame

当前权限系统存在三个独立的定义源：`resources.rs` 的静态数组（13个资源、3个操作）、101处 handler 中 `#[require_permission("warehouse", "read")]` 的字符串字面量、以及 proto 中 `ResourceInfo`/`PermissionInfo` 的 bare string 字段。三处不同步时，拼写错误可以通过编译但在运行时静默失败。前端没有编译时权限常量，必须通过 `ListPermissions` RPC 在运行时获取权限词表。

目标：将权限词表（资源和操作）的唯一真相源移至 proto，使前后端都通过代码生成获得类型安全的权限常量。

## Requirements

**Proto 枚举定义**

- R1. 在 `permission.proto` 中定义 `Resource` 枚举，包含当前所有13个资源：product, term, bom, warehouse, location, inventory, price, labor_process, user, role, permission, department, excel
- R2. 在 `permission.proto` 中定义 `Action` 枚举，包含 read, write, delete
- R3. 现有的 `ResourceInfo.resource_code` 和 `PermissionInfo.action_code` 等 string 字段保持不变，用于传输层；新增的枚举用于编译时校验和代码生成

**Macro 迁移**

- R4. `#[require_permission]` macro 改为接受枚举路径：`#[require_permission(Resource::Warehouse, Action::Read)]`，编译器自动校验枚举值是否存在
- R5. 所有101处 handler 调用点迁移到新的枚举路径语法
- R6. macro 生成的代码调用 `check_permission` 时仍传入字符串形式，确保与现有 JWT 权限格式 (`"warehouse:read"`) 兼容

**Rust 侧资源注册**

- R7. `resources.rs` 简化为显示名映射表：从 proto 生成的枚举 variant 映射到中文显示名（resource_name, action_name, description）
- R8. `collect_all_resources()` 从枚举 variant + 显示名映射构建 `Vec<ResourceActionDef>`，而非手写静态数组
- R9. `BUSINESS_RESOURCE_CODES` 和 `SYSTEM_RESOURCE_CODES` 从枚举元数据（如 proto enum value option 或 Rust 侧标记）自动派生
- R10. `is_business_resource()` / `is_system_resource()` 函数签名和语义不变，仅数据源改为从枚举派生

**前端代码生成**

- R11. 前端通过现有 proto 编译工具链获得 `Resource` 和 `Action` 的 TypeScript 枚举定义
- R12. 前端可直接 import 这些枚举用于权限判断，无需运行时调用 `ListPermissions` 来获取权限词表

## Success Criteria

- 新增/删除一个资源需修改 proto 文件和 Rust 侧显示名映射表，`cargo build` 自动校验枚举值同步
- 任何 `#[require_permission]` 中使用了不存在的资源或操作，编译直接报错
- 前端 TS 代码可 import `Resource` 和 `Action` 枚举，获得 IDE 自动补全
- `ListPermissions` 和 `ListResources` RPC 行为不变

## Scope Boundaries

- 不改变 JWT 中权限的存储格式（仍为 `"resource:action"` 字符串数组）
- 不改变 `check_permission` 的运行时匹配逻辑（仍为字符串比较）
- 不改变数据库中 `role_permissions` 表的 schema
- 不引入 proto method-level 自定义 option 或 interceptor 级别的权限执行
- 前端的具体使用方式（如何在组件中使用枚举）不在本次范围内

## Key Decisions

- **Macro 接受枚举路径而非字符串**：牺牲101处调用点的迁移工作量，换取编译时校验和 IDE 支持
- **中文显示名保留在 Rust 侧映射**：proto 只负责定义枚举值（英文代码），中文显示名通过 Rust 侧的映射表关联，避免 proto 承载 i18n 职责
- **前端用现有 proto 工具链**：前端项目已有 proto 编译流程，无需额外引入新工具
- **运行时仍用字符串匹配**：JWT 和 `check_permission` 不改，降低迁移风险

## Dependencies / Assumptions

- 前端项目已有 proto 到 TypeScript 的编译工具链
- `tonic_prost_build` 能正确生成 proto enum 的 Rust 表示和 `as_str_name()` 方法
- 所有 handler 中 `require_permission` 的字符串值当前与 `resources.rs` 中定义一致

## Outstanding Questions

### Deferred to Planning

- [Affects R4, R6][Technical] proto enum 的 Rust 生成代码中 `as_str_name()` 返回的是 SCREAMING_SNAKE_CASE（如 `"WAREHOUSE"`），而 `check_permission` 期望 lowercase（如 `"warehouse"`）。具体转换策略在 planning 中确定
- [Affects R7, R8][Technical] `resources.rs` 简化的具体形式（build.rs 生成 vs 运行时从枚举构建）在 planning 中确定
- [Affects R9][Technical] business/system 资源分类的标记方式（proto custom option vs Rust 侧硬编码列表 vs 枚举 variant 属性）在 planning 中确定

## Next Steps

→ `/ce:plan` 进行结构化实现规划
