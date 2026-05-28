# CLAUDE.md

## Constraints（必须遵守）

- **使用中文沟通**
- **不要用 `cargo run` 启动服务**，服务已在运行中。验证代码正确性主要用 `cargo clippy`
- **编写 `abt-web/` 组件前，必须先读 `abt-web/CLAUDE.md`**（组件化三原则、抗碎片化实践等约束）

## Project Overview

ABT 是 BOM（物料清单）和库存管理系统，基于 Rust 构建。底层数据库为 PostgreSQL。

**环境变量**（`.env` 文件）：
- `DATABASE_URL`（必须，PostgreSQL 连接串，指向 `abt_v2` 数据库）
- `JWT_SECRET`（必须）
- `WEB_PORT`（默认 `3000`）、`WEB_HOST`（默认 `0.0.0.0`）、`MAX_CONNECTION`（默认 `20`）

## Design Principle: Interface & Model First（接口与模型先行）

新功能开发强制执行以下顺序，不可跳步：

**接口 + 模型设计 → 评审确认 → 交互设计 → 实现**

1. **接口先行** — 先定义清晰稳定的 Service trait，确认后不可随意变更
2. **模型先行** — 同步设计领域模型（请求/响应结构、实体、值对象），语义清晰、边界明确、职责单一
3. **基于接口设计交互** — 禁止在接口未定义时设计前端交互或 UI
4. **文档化** — 接口和模型以 `docs/uml-design/` 设计文档为骨架和共享语言

## Design Authority（设计文档权威性）

`docs/uml-design/` 是系统的唯一设计文档，代码与设计文档必须**双向同步**，不允许脱节：

- **所有实现必须严格遵守 `docs/uml-design/` 中的设计文档**（接口签名、数据模型、组件关系）
- **改代码 → 必须同步更新设计文档** — 任何代码变更（接口签名、数据模型、组件关系、新增/删除方法等），必须同时更新对应的设计文档，保持一致
- **改设计文档 → 必须同步修改代码** — 任何设计文档的变更，必须同时更新代码实现，保持一致
- **如果实现过程中发现设计与现实不符**，必须先更新设计文档（经用户确认），再修改代码
- **禁止在未更新设计文档的情况下偏离设计** — 包括但不限于：修改接口签名、增删方法、改变数据模型、调整组件关系
- **设计文档的修改必须经过用户确认** — 不能擅自更改设计文档内容
- **每次提交代码变更时，自检**：设计文档是否已同步更新？如果未更新，先更新文档再提交

## Build & Verification

```bash
cargo build          # 构建所有 crate
cargo clippy         # 主要验证手段
cargo test           # 运行所有测试
cargo test -p abt-core          # 运行 abt-core 测试
cargo test -p abt-core -- test_name  # 运行单个测试
```

## Architecture

### Workspace Structure

```
abt-core/     — 核心业务库（lib），按业务域组织模块，暴露 Service trait
abt-web/      — Web 前端（Axum + Maud + HTMX），直接调用 abt-core Service trait
abt-macros/   — 过程宏
```

### Module Structure（abt-core）

每个业务模块采用**高内聚的文件组织**，所有层共处同一目录：

```
abt-core/src/sales/order/
├── mod.rs       # 导出 + 工厂函数（new_order_service）
├── service.rs   # Service trait 定义
├── implt.rs     # Service trait 实现
├── model.rs     # 数据模型（请求/响应/实体）
└── repo.rs      # 数据库访问（sqlx 原始 SQL）
```

**业务域模块**：`sales`、`master_data`、`purchase`、`wms`、`mes`、`qms`、`om`（委外）、`fms`（财务）、`workflow`

### Shared Infrastructure（共享基础设施层）

`abt-core/src/shared/` 提供横切关注点的共享服务。每个共享模块暴露工厂函数，业务模块按需调用。

**实现共享基础设施前，必须先读 `docs/uml-design/README.md`**（接口签名、类型定义、集成规则）。

### Database

PostgreSQL + sqlx（通过 `sqlx::query!` 宏实现编译期检查）。Migration 在 `abt-core/migrations/` 中，按序编号的纯 SQL 文件。

- JSONB 列用于灵活元数据（如 `products.meta`、`boms.bom_detail`）
- 通过 `deleted_at` 时间戳实现软删除
- `Decimal(10,6)` 用于财务/数量精度
- 通过 `operator_id` 追踪操作审计

### Key Conventions

- **模块边界**：跨模块调用只允许通过 Service trait 和 Model。禁止跨模块直接访问 Repository 或 Service impl（`implt`）。同模块内部可自由调用自身 Repository
- **错误处理**：使用 `thiserror` 定义的 `DomainError` 枚举，返回 `Result<T, DomainError>`。Web handler 层将 `DomainError` 映射为 HTTP 响应
- **禁止静默丢弃错误**：严禁使用 `let _ = expr.await;` 或 `let _ = result;` 来忽略错误。所有错误必须通过 `?` 传播、`map_err` 转换、或 `if let Err(e) { ... }` 显式处理（至少记录日志）
- **共享基础设施**：集成共享服务前必须读 `docs/uml-design/README.md`（接口签名、AuditAction / SideEffect / EventPublishRequest 等类型定义、集成规则）
- 所有 service trait 使用 `async-trait` crate 的 `#[async_trait]`
- 所有 crate edition 统一为 2024
- `abt-core/src/shared/types/` 提供 `PgExecutor` 类型别名（`&mut PgConnection`）、`ServiceContext`、`DomainError`、`PageParams` 等共享类型

### 共享服务调用：按需工厂模式（On-Demand Factory）

业务 Service impl 使用共享服务时，遵循以下规则：

**1. 每个共享模块的 `mod.rs` 暴露工厂函数**，返回 `impl Trait`，内部自行封装子依赖：

```rust
// shared/audit_log/mod.rs
pub fn new_audit_log_service(pool: PgPool) -> impl AuditLogService {
    implt::AuditLogServiceImpl::new(pool)
}

// shared/state_machine/mod.rs （内部处理自己的依赖）
pub fn new_state_machine_service(pool: PgPool) -> impl StateMachineService {
    let event_bus = Arc::new(DomainEventBusImpl::new(pool.clone()));
    implt::StateMachineServiceImpl::new(pool, event_bus)
}
```

**2. 消费方 struct 只持 `PgPool`**，不持有任何 `Arc<dyn Trait>` 字段：

```rust
pub struct XxxServiceImpl {
    repo: XxxRepo,
    pool: PgPool,
}
```

**3. 方法体只依赖接口（trait）**，通过工厂函数获取接口实例，禁止直接依赖实现类型：

```rust
// ✓ 正确：use 引入 trait + 工厂函数，方法体只面向接口编程
use crate::shared::audit_log::{new_audit_log_service, service::AuditLogService};
use crate::shared::state_machine::{new_state_machine_service, service::StateMachineService};

impl XxxService for XxxServiceImpl {
    async fn some_method(&self, ...) -> Result<()> {
        new_audit_log_service(self.pool.clone())
            .record(ctx, db, ...).await?;

        new_state_machine_service(self.pool.clone())
            .transition(ctx, db, ...).await?;
    }
}

// ✗ 禁止：使用全限定路径
crate::shared::audit_log::new_audit_log_service(self.pool.clone())

// ✗ 禁止：直接依赖实现类型
use crate::shared::audit_log::implt::AuditLogServiceImpl;
AuditLogServiceImpl::new(self.pool.clone())
```

**核心原则：**
- struct 不持有 `Arc<dyn Trait>`，只持 `PgPool`
- 方法体只面向 trait 接口编程，不依赖实现（implt）
- 工厂函数在 `use` 中引入，代码中使用短名称
- 共享服务按需创建，用完即弃，不做 upfront 注入

### Documented Solutions

`docs/solutions/` — 记录历史问题的解决方案（bug、最佳实践、工作流模式），按类别组织，使用 YAML frontmatter（`module`、`tags`、`problem_type`）。在已记录的领域实现或调试时参考。

### Adding a New Feature

1. 在 `abt-core/src/<domain>/<module>/` 下创建模块文件：
   - `model.rs` — 数据模型
   - `repo.rs` — 数据库访问
   - `service.rs` — Service trait 定义
   - `implt.rs` — Service trait 实现（struct 只持 `PgPool`，共享服务通过工厂函数按需获取）
   - `mod.rs` — 导出 + 工厂函数
3. 在 `abt-core/migrations/` 添加数据库迁移
4. 在 `abt-web/src/pages/` 创建页面（如需 UI）
5. 同步更新 `docs/uml-design/` 设计文档
