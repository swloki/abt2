# abt-core

按业务域组织的 ABT 核心业务库。设计文档见 `docs/uml-design/01-architecture-layers.md`。

## 约束

- **不要用 `cargo run`**，服务由 `abt-grpc` 启动
- **不要迁移 `abt` crate 代码到此处**，除非明确指示。目前只是骨架

## 架构

```
src/
  lib.rs              # 模块声明 + 工厂函数
  shared/             # 共享基础设施（所有业务模块可依赖）
  sales/              # 销售 CRM
  purchase/           # 采购 SRM（未来）
  wms/                # 仓储 WMS（未来）
  mes/                # 生产 MES（未来）
  om/                 # 委外管理（未来）
  qms/                # 质量管理（未来）
  fms/                # 财务管理（未来）
  workflow/           # 工作流引擎（从 abt 迁移）
```

## 依赖方向

```
shared/ ← sales/    （单向，禁止反向）
shared/ ← purchase/
shared/ ← wms/
shared/ ← mes/
shared/ ← workflow/
```

业务模块之间**禁止互相依赖**（如 sales 不能依赖 wms）。跨模块交互通过 shared 层的 `event_bus` 实现。

## 模块内部结构

每个业务子模块（如 `sales/quotation/`）包含：

| 文件 | 职责 |
|------|------|
| `mod.rs` | 模块声明 + `pub use` 重新导出 |
| `model.rs` | 数据结构（数据库行映射 + Proto 中间态） |
| `repo.rs` | sqlx SQL 查询，返回 `anyhow::Result` |
| `service.rs` | `#[async_trait]` 业务接口定义 |
| `implt.rs` | 基于 repo + shared service 的具体实现 |

## 三种事务模式

| 模式 | 场景 | 失败策略 |
|------|------|---------|
| 同步强一致 | 库存预留、质量关卡 | 失败回滚主事务 |
| 独立事务 | CostEntry 成本记录 | 主事务提交后开新事务，失败不影响主业务 |
| 异步 Outbox | DocumentLink、Workflow 触发 | 写 Outbox + NOTIFY，后台消费 |

## 验证

```bash
cargo clippy -p abt-core   # 主要验证手段
cargo test -p abt-core     # 运行测试
```
