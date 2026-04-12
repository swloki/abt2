---
title: "feat: Define permission enums in proto for type-safe cross-platform access"
type: feat
status: active
date: 2026-04-12
origin: docs/brainstorms/2026-04-12-permission-proto-enums-requirements.md
---

# feat: Proto 定义权限枚举

## Overview

将权限词表（Resource 和 Action）从 Rust 硬编码字符串迁移到 proto 枚举定义，使前后端通过代码生成获得类型安全的权限常量。macro 从 `#[require_permission("warehouse", "read")]` 改为 `#[require_permission(Resource::Warehouse, Action::Read)]`，获得编译时校验。

## Problem Frame

当前三个独立的权限定义源（`resources.rs` 静态数组、101处 handler 字符串字面量、proto bare string 字段）无法保证同步。拼写错误通过编译但在运行时表现为权限拒绝。前端无编译时权限常量。(see origin: docs/brainstorms/2026-04-12-permission-proto-enums-requirements.md)

## Requirements Trace

- R1. 在 `permission.proto` 中定义 `Resource` 枚举（13个资源）
- R2. 在 `permission.proto` 中定义 `Action` 枚举（read, write, delete）
- R3. 现有 proto string 字段保持不变
- R4. macro 接受枚举路径，编译器校验枚举值存在
- R5. 所有101处 handler 调用点迁移
- R6. macro 生成代码仍与 JWT 字符串格式兼容
- R7. `resources.rs` 保留为 Rust 侧显示名映射
- R8. `collect_all_resources()` 保持功能不变
- R9. BUSINESS/SYSTEM 资源分类保持当前机制
- R10. `is_business_resource()` / `is_system_resource()` 签名不变
- R11. 前端通过 proto 编译获得 TS 枚举
- R12. 前端可 import 枚举用于权限判断

## Scope Boundaries

- 不改变 JWT 权限存储格式（仍为 `"resource:action"` 字符串数组）
- 不改变 `check_permission` 运行时逻辑
- 不改变 `role_permissions` 表 schema
- 不引入 proto method-level 自定义 option
- 不尝试编译时校验 (Resource, Action) 组合合法性——枚举定义 action 超集，各 handler 自行选择使用的 action
- 前端具体使用方式不在本次范围

## Context & Research

### Relevant Code and Patterns

- `proto/abt/v1/permission.proto` — 现有 `ResourceInfo`/`PermissionInfo` 消息
- `abt-macros/src/lib.rs` — 当前 `require_permission` macro 实现，仅校验参数数量
- `abt/src/models/resources.rs` — 77 行手写静态数组，13资源 × 1-3 actions = 35 条
- `abt/src/models/auth.rs` — `AuthContext::check_permission`、`Claims`、`ResourceActionDef`
- `abt-grpc/build.rs` — `tonic_prost_build::configure()` 编译 proto
- `abt-grpc/src/generated/abt.v1.rs` — 生成的 Rust 代码
- prost 0.14 生成的枚举带有 `as_str_name()` 方法，返回 proto 定义的枚举名（SCREAMING_SNAKE_CASE）

### Crate 依赖关系

```
common ← abt ← abt-grpc (包含生成的 proto 代码)
                ↑
            abt-macros (被 abt-grpc 的 handler 文件使用)
```

`abt` crate 无法引用 `abt-grpc` 中的类型。因此 `PermissionCode` trait 和实现必须放在 `abt-grpc` 中。

### Institutional Learnings

- `require_permission` macro 需要穿透 `#[tonic::async_trait]` 的 `Box::pin` 变换 (docs/solutions/developer-experience/require-permission-macro-async-trait-2026-04-05.md)
- Macro 生成的代码使用 call-site 解析的短名称（`extract_auth`, `error::forbidden`），不需要完全限定路径

## Key Technical Decisions

- **枚举值命名用标准 SCREAMING_SNAKE_CASE**：proto 规范要求。通过 `PermissionCode` trait 的 `.code()` 方法转换为 lowercase 运行时字符串
- **PermissionCode trait 放在 abt-grpc**：因为 `abt` 不能依赖 `abt-grpc`。trait 在 `abt-grpc` 定义，handler 文件 import 使用
- **.code() 用 match 返回 &str**：比 `as_str_name().to_lowercase()` 更安全——避免运行时字符串操作，编译器检查 match 完备性
- **resources.rs 保持现状**：显示名映射和 BUSINESS/SYSTEM 分类保留在 `abt` crate。新增一致性测试确保与 proto 枚举同步
- **不约束 (Resource, Action) 组合**：Action 枚举是超集（read/write/delete）。inventory/price/excel/permission 没有 delete，但编译时不强制——这是 code review 职责，不是类型系统职责
- **一次性迁移101处调用点**：不支持新旧语法共存。macro 改接口 + 所有 handler 更新在同一个 commit

## Open Questions

### Resolved During Planning

- **大小写转换策略**：用 `PermissionCode` trait 的 `match` 返回硬编码 lowercase 字符串，避免 `to_lowercase()` 的运行时开销和风险
- **business/system 分类**：保留 Rust 侧硬编码列表（`resources.rs` 中的 `BUSINESS_RESOURCE_CODES` / `SYSTEM_RESOURCE_CODES`），通过一致性测试保证与 proto 枚举同步
- **ResourceActionDef 与 &str**：保持 `&'static str` 字段不变，`resources.rs` 继续用手写字符串字面量（已经都是 `&'static str`）

### Deferred to Implementation

- 前端 TS 枚举的具体生成工具和配置（取决于前端项目的 proto 编译工具链）
- 生成的 `abt.v1.rs` 是否应该从 git 中移除（影响 CI 和协作流程）

## High-Level Technical Design

> *This illustrates the intended approach and is directional guidance for review, not implementation specification. The implementing agent should treat it as context, not code to reproduce.*

**Proto 枚举定义：**
```protobuf
enum Resource {
  PRODUCT = 0;
  TERM = 1;
  BOM = 2;
  WAREHOUSE = 3;
  LOCATION = 4;
  INVENTORY = 5;
  PRICE = 6;
  LABOR_PROCESS = 7;
  USER = 8;
  ROLE = 9;
  PERMISSION = 10;
  DEPARTMENT = 11;
  EXCEL = 12;
}

enum Action {
  READ = 0;
  WRITE = 1;
  DELETE = 2;
}
```

**PermissionCode trait 和实现（在 abt-grpc 中）：**
```rust
// abt-grpc/src/permissions/mod.rs
pub trait PermissionCode {
    fn code(&self) -> &'static str;
}

impl PermissionCode for crate::generated::abt::v1::Resource {
    fn code(&self) -> &'static str {
        match self {
            Self::Product => "product",
            Self::Term => "term",
            // ... 每个 variant 硬编码 lowercase
        }
    }
}
// 类似实现 Action
```

**Macro 生成的代码（在 handler 文件中）：**
```rust
// 之前: #[require_permission("warehouse", "read")]
// 之后: #[require_permission(Resource::Warehouse, Action::Read)]
//
// Macro 展开为:
let auth = extract_auth(&request)?;
auth.check_permission(Resource::Warehouse.code(), Action::Read.code())
    .map_err(|_e| error::forbidden(Resource::Warehouse.code(), Action::Read.code()))?;
```

**数据流：**
```
Proto enums → prost 生成 Rust 类型 → PermissionCode trait → .code() 返回 lowercase 字符串
    ↓                                           ↓
前端 TS 枚举                          macro 展开中使用 → check_permission(&str, &str)
```

## Implementation Units

- [ ] **Unit 1: 添加 Resource 和 Action 枚举到 proto**

**Goal:** 在 `permission.proto` 中定义枚举，确认 Rust 代码生成正确

**Requirements:** R1, R2

**Dependencies:** None

**Files:**
- Modify: `proto/abt/v1/permission.proto`
- Verify: `abt-grpc/src/generated/abt.v1.rs` (重新生成后)

**Approach:**
- 在 `permission.proto` 文件顶部（service 定义之前）添加 `Resource` 和 `Action` 枚举
- 枚举值用标准 SCREAMING_SNAKE_CASE，与 proto 规范一致
- 运行 `cargo build` 重新生成 Rust 代码
- 确认生成的代码包含 `Resource` 和 `Action` 枚举（注意：本设计不依赖 `as_str_name()`，而是通过 `PermissionCode` trait 的手动 match 映射）

**Patterns to follow:**
- Proto 枚举从 0 开始编号（proto3 默认第一个值是默认值）

**Test scenarios:**
- Test expectation: none — proto 编译成功即为验证

**Verification:**
- `cargo build` 成功
- 生成的 `abt.v1.rs` 中包含 `pub enum Resource` 和 `pub enum Action`
- `Resource` 有 13 个变体，`Action` 有 3 个变体

---

- [ ] **Unit 2: 添加 PermissionCode trait 和实现**

**Goal:** 定义 `PermissionCode` trait，为生成的枚举实现 `.code()` 方法

**Requirements:** R4, R6

**Dependencies:** Unit 1

**Files:**
- Create: `abt-grpc/src/permissions/mod.rs`
- Modify: `abt-grpc/src/lib.rs`（添加 `mod permissions`）

**Approach:**
- 定义 `PermissionCode` trait（`fn code(&self) -> &'static str`）
- 为 `Resource` 枚举实现——用 `match` 将每个变体映射到 lowercase 字符串
- 为 `Action` 枚举实现——同上
- `match` 必须覆盖所有变体，编译器保证完备性
- 当新增枚举变体时，`match` 不完备会导致编译错误——这是期望的安全行为

**Patterns to follow:**
- `abt-grpc/src/generated/abt.v1.rs` 中生成类型的导入路径：`crate::generated::abt::v1::*`

**Test scenarios:**
- Happy path: `Resource::Warehouse.code()` 返回 `"warehouse"`
- Happy path: `Action::Read.code()` 返回 `"read"`
- Edge case: `LABOR_PROCESS` 映射到 `"labor_process"`
- Completeness: 当 proto 新增枚举变体但 `PermissionCode` impl 的 `match` 未更新时，编译失败

**Verification:**
- `cargo build` 成功
- `Resource::Product.code()` == `"product"`
- `Action::Delete.code()` == `"delete"`

---

- [ ] **Unit 3: 更新 require_permission macro 和迁移所有 handler**

**Goal:** Macro 接受枚举路径，所有101处调用点迁移完成

**Requirements:** R4, R5, R6

**Dependencies:** Unit 2

**Files:**
- Modify: `abt-macros/src/lib.rs`
- Modify: `abt-grpc/src/handlers/*.rs`（12 个文件，101 处调用点）
  - `bom.rs` (20), `inventory.rs` (14), `department.rs` (10), `location.rs` (9), `user.rs` (8), `role.rs` (7), `product.rs` (7), `permission.rs` (6), `term.rs` (6), `warehouse.rs` (6), `excel.rs` (5), `price.rs` (3)
  - Note: `labor_process.rs` 不在迁移范围内——它的函数没有 `#[require_permission]` 注解（权限由 `bom.rs` 调用时控制）

**Approach:**
- **Macro 修改：**
  - 解析两个路径 token（而非字符串字面量）
  - 生成代码调用 `.code()` 方法：`auth.check_permission(#resource.code(), #action.code()).map_err(|_e| error::forbidden(#resource.code(), #action.code()))?;`
  - `Box::pin` 穿透逻辑保持不变
  - `extract_request_ident` 逻辑保持不变
  - 错误信息更新为提示期望枚举路径格式

- **Handler 迁移：**
  - 每个 handler 文件添加 `use crate::permissions::PermissionCode;`
  - 每个 handler 文件添加 `Resource` 和 `Action` 到现有 `use crate::generated::abt::v1::*` 导入（proto 生成后已在通配符导入范围内）
  - 将 `#[require_permission("warehouse", "read")]` 改为 `#[require_permission(Resource::Warehouse, Action::Read)]`
  - 映射规则：`"product"` → `Resource::Product`，`"labor_process"` → `Resource::LaborProcess`，等等

**Technical design:**
```
旧语法: #[require_permission("warehouse", "read")]
新语法: #[require_permission(Resource::Warehouse, Action::Read)]

Macro 解析: 两个路径表达式 (Expr::Path)
Macro 生成:
  let auth = extract_auth(&#request_ident)?;
  auth.check_permission(#resource.code(), #action.code())
      .map_err(|_e| error::forbidden(#resource.code(), #action.code()))?;
```

**Execution note:** 一次性修改 macro 和所有 handler 文件。Macro 和 handler 必须同步更新——不支持新旧语法共存。

**Patterns to follow:**
- 现有 macro 的 `Box::pin` 穿透逻辑保持不变
- Handler 文件的 `use` 导入模式保持一致

**Test scenarios:**
- Happy path: `#[require_permission(Resource::Warehouse, Action::Read)]` 编译通过，运行时行为与字符串版本一致
- Error path: `#[require_permission(Resource::Invalid, Action::Read)]` 编译失败
- Error path: `#[require_permission(Resource::Warehouse, Action::Invalid)]` 编译失败
- Integration: 每个 handler 的权限检查在 migration 后返回与之前相同的结果

**Verification:**
- `cargo build` 成功
- `cargo test -p abt-grpc` 通过（现有权限测试不受影响）
- 101 处 `#[require_permission]` 均使用枚举路径语法
- 不存在使用字符串字面量的 `#[require_permission]` 调用

---

- [ ] **Unit 4: 添加一致性测试**

**Goal:** 确保 `resources.rs` 中的资源代码和 `PermissionCode` impl 保持同步

**Requirements:** R7, R9

**Dependencies:** Unit 2, Unit 3

**Files:**
- Create: `abt-grpc/src/permissions/tests.rs`（单元测试模块，可访问 crate 内部类型和 `abt` crate）

**Approach:**
- 测试1：`Resource` 枚举的所有变体在 `PermissionCode::code()` 返回值都能在 `resources.rs` 的 `RESOURCES` 数组中找到对应 `resource_code`
- 测试2：`Action` 枚举的所有变体都能在 `RESOURCES` 数组中找到对应 `action`
- 测试3：`BUSINESS_RESOURCE_CODES` 和 `SYSTEM_RESOURCE_CODES` 的并集覆盖所有 `Resource` 变体，且交集为空
- 当 proto 新增资源但忘记更新 `resources.rs` 或分类列表时，测试失败

**Note:** 由于 `abt` crate 无法引用 `abt-grpc` 中的枚举，测试需要放在 `abt-grpc` 中。测试通过 `PermissionCode::code()` 获取字符串，再与 `abt::models::resources::collect_all_resources()` 的结果对比。

**Test scenarios:**
- Integration: `Resource::Product.code()` 存在于 `collect_all_resources()` 返回的 `resource_code` 列表中
- Integration: `BUSINESS_RESOURCE_CODES` ∪ `SYSTEM_RESOURCE_CODES` 包含所有 `Resource` 变体的 `.code()` 值
- Edge case: `Resource::LaborProcess.code()` == `"labor_process"` 在 `RESOURCES` 中可找到
- Error detection: 如果 `resources.rs` 缺少某个资源的显示名，测试报错

**Verification:**
- `cargo test` 中新测试通过

---

- [ ] **Unit 5: 验证前端 TypeScript 枚举生成**

**Goal:** 确认前端 proto 编译工具链能生成可用的 TS 枚举

**Requirements:** R11, R12

**Dependencies:** Unit 1

**Files:**
- Verify: 前端项目中 proto 编译生成的 TS 文件

**Approach:**
- 在 `permission.proto` 添加枚举后，运行前端 proto 编译
- 确认生成的 TS 代码包含 `Resource` 和 `Action` 枚举（或等效的类型定义）
- 确认枚举值包含所有 13 个资源和 3 个操作
- 如果前端工具链生成的是数字常量而非字符串枚举，可能需要配置调整——记录此为已知问题

**Test scenarios:**
- Test expectation: none — 验证性检查，确认前端工具链行为

**Verification:**
- 前端项目编译成功
- 生成的 TS 代码可被 import 使用

## System-Wide Impact

- **Interaction graph:** Macro 展开代码改变，但运行时 `check_permission` 调用签名不变（仍为 `&str`）。JWT 格式不变。RPC 接口不变。
- **Error propagation:** 权限拒绝的错误信息保持不变（`error::forbidden` 接收的字符串值不变）
- **Unchanged invariants:** `ListPermissions`、`ListResources`、`CheckPermission` RPC 的行为和响应格式不变。`resources.rs` 的 `collect_all_resources()` 返回值不变。

## Risks & Dependencies

| Risk | Mitigation |
|------|------------|
| 101处 handler 迁移可能有遗漏 | Grep 确认无字符串字面量残留 |
| Proto 新增资源但 PermissionCode impl 未更新 | `match` 完备性检查导致编译失败 |
| Proto 新增资源但 resources.rs 显示名未更新 | 一致性测试失败 |
| `PermissionCode` trait 不在 scope 导致 handler 编译失败 | 每个文件显式 `use crate::permissions::PermissionCode` |
| 前端 proto 工具链不生成可用的 TS 枚举 | 记录为已知问题，需要时切换到 JSON manifest 方案 |

## Sources & References

- **Origin document:** docs/brainstorms/2026-04-12-permission-proto-enums-requirements.md
- **Ideation document:** docs/ideation/2026-04-12-permission-proto-enums-ideation.md
- **Solution doc:** docs/solutions/developer-experience/require-permission-macro-async-trait-2026-04-05.md
- Related code: abt-macros/src/lib.rs, abt/src/models/resources.rs, proto/abt/v1/permission.proto
