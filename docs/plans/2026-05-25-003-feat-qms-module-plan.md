---
title: "feat: QMS 质量管理模块实现"
type: feat
status: active
date: 2026-05-25
---

# feat: QMS 质量管理模块实现

## Summary

在 `abt-core` 中实现完整的 QMS 质量管理模块，包含检验规格（InspectionSpecification）、检验结果（InspectionResult）、MRB 不良评审、RMA 客诉追溯四个子模块，以及独立的 QualityGateService 质量关卡接口。遵循项目已有的分层模式（Model → Repo → Service Trait → Impl → gRPC Handler），集成共享基础设施（状态机、审计日志、事件总线、幂等服务、文档编号），并通过 gRPC 对外暴露完整 API。实现严格遵循 `docs/uml-design/06-qms.html` v2.3 设计文档。

---

## Problem Frame

ABT 系统当前缺少质量管理能力。WMS 来料入库、MES 工序流转、Sales 出货发货等环节没有统一的质量关卡，无法在数据层面阻断不合格品的流转。MRB 不良评审依赖线下流程，RMA 客诉追溯缺乏系统化记录。需要实现设计文档定义的 QMS 模块，建立三道质量关口（IQC/IPQC/FQC-OQC），通过硬门机制（QualityGateService.check_gate）确保不合格品无法流转到下游。

---

## Requirements

- R1. InspectionSpecification CRUD — 创建/查询/更新检验规格，支持按产品+检验类型查找，乐观锁防覆盖
- R2. InspectionResult 创建与结果录入 — 创建检验记录（绑定来源单据），录入检验结果并返回 QualityGateStatus
- R3. QualityGateService 质量关卡 — 独立 trait，三值判定（Passed/Failed/NotRequired），InCallerTx 保证硬门一致性
- R4. MRB 不良评审 — 创建/提交评审/执行处置，submit_for_review 集成 WorkflowEngine，execute_disposition 仅通过 WorkflowHook 回调触发
- R5. RMA 客诉追溯 — 创建/录入根因/关闭，record_root_cause 自动触发状态转换，支持 linked_inspection_result_id 正逆向追溯
- R6. 共享基础设施集成 — 所有写操作集成 DocumentSequence、StateMachineService、AuditLogService、DomainEventBus；record_result 集成 IdempotencyService；MRB/RMA 集成 DocumentLink
- R7. gRPC API — 所有 Service 操作通过 Proto 定义暴露，handler 层完成 Proto↔Model 类型映射和 DomainError→tonic::Status 转换
- R8. JSONB 强类型 — CheckItem/SamplePlan/CheckResult 使用强类型结构体，禁止 Service 签名中透传 serde_json::Value

---

## Scope Boundaries

- 不修改前端代码（CLAUDE.md 约束）
- 不修改 WMS/MES/Sales 模块的源代码 — QMS 仅定义 QualityGateService trait 供未来集成，不实际修改这些模块的调用点
- 不实现 WorkflowEngine 本身 — QMS 仅通过 submit_for_review 触发工作流实例创建
- 不实现 CostEntry 的消费端 — QMS 通过 DomainEventBus 发布 MRBDispositioned 事件，成本记录由独立消费者处理
- 不替换 MES 中的 QmsInspectionStub — 此工作在 QMS 模块完成后作为单独集成任务进行
- 不实现 gRPC 层（Proto 定义、Handler、server.rs 注册）— 本 PR 仅完成 abt-core 内的业务逻辑，gRPC 接线后续单独处理
- 不修改已有 migration 文件或共享层枚举（DocumentType/DomainEventType 已预注册）
- InspectionSourceType::WorkOrderRouting 映射到已有 DocumentType::WorkOrder（设计文档映射需微调，不改共享枚举）
- RMA close 不发布 RMAClosed 事件（DomainEventType 未注册此变体），仅触发状态转换+审计日志
- QMS 使用 DomainError（非设计文档提到的 QmsError），遵循 abt-core 统一错误模式

### Deferred to Follow-Up Work

- Proto 定义 + gRPC Handler + server.rs 注册（abt-grpc 接线，需先解决 abt-core↔abt-grpc 依赖问题）
- Permission Resource 枚举添加 QMS 条目（随 gRPC handler 一起）
- MES QmsInspectionStub 替换为实际 QualityGateService 引用（QMS 完成后单独 PR）
- WMS.confirm() / MES 工序报工 调用 check_gate() 的集成（需修改 WMS/MES 源码，单独 PR）
- CostEntry 异步消费 MRBDispositioned 事件的 handler 实现
- WMS 库存调整的异步 handler（MRB 报废/返工触发库存变动）
- abt-core 服务初始化模式设计（AppContext / PgPool / 共享服务工厂函数）

---

## Context & Research

### Relevant Code and Patterns

- **模块结构模式** — `abt-core/src/sales/quotation/` 提供完整的 model/repo/service/implt 分层范例
- **枚举宏模式** — `abt-core/src/mes/enums.rs` 中的 `define_mes_enum!` 宏生成 i16 枚举样板代码
- **乐观锁模式** — `abt-core/src/purchase/order/implt/mod.rs` 中 PurchaseOrderServiceImpl::confirm 的 expected_version 实现
- **ServiceContext 模式** — `abt-core/src/shared/types/context.rs` 中的 ServiceContext + reborrow 用法
- **DomainError 模式** — `abt-core/src/shared/types/error.rs` 统一错误模型
- **共享基础设施集成序列** — `abt-core/src/purchase/order/implt/mod.rs` 展示了完整的 Idempotency → DocSeq → StateMachine → Audit → EventBus 调用链
- **Repo 模式** — `abt-core/src/sales/quotation/repo.rs` 零大小结构体 + 原始 SQL + 动态查询构建
- **gRPC Handler 模式** — `abt-grpc/src/handlers/quotation.rs` 展示 Proto↔Model 映射和 require_permission 宏用法
- **已有预注册** — `abt-core/src/shared/enums/document_type.rs` 已含 DocumentType 26-29（QMS 实体）；`abt-core/src/shared/enums/event.rs` 已含 DomainEventType 14-17（QMS 事件）

### Institutional Learnings

- **并发安全** — "先读后写"场景必须在事务内使用 SELECT FOR UPDATE；execute_disposition 涉及状态读取+更新，应套用此模式
- **sqlx QueryBuilder 限制** — push_values 闭包内只有 push_bind，无 push_raw；批量插入需子查询时改用 query_as 手动构建
- **应用层引用检查** — 删除前用 SELECT EXISTS 检查引用关系，替代外键约束，返回友好错误消息
- **迁移安全** — 不用 TRUNCATE，用 INSERT ON CONFLICT DO NOTHING 保持幂等；旧表归档而非 DROP
- **错误分层** — handler 中 business_error() 用于业务规则、validation() 用于输入格式、err_to_status() 仅用于基础设施错误
- **权限集成** — 在 permission.proto 的 Resource enum 中添加 QMS 变体，实现 PermissionCode trait，使用 require_permission 宏

---

## Key Technical Decisions

- **QMS 模块独立于 abt crate** — 所有新代码在 abt-core/src/qms/ 中实现，遵循 CLAUDE.md "新功能一律在 abt-core 中开发" 的约束
- **QualityGateService 作为独立 trait** — 接口隔离，WMS/MES 只需依赖这一个轻量 trait，不需要了解 QMS 内部结构。InCallerTx 语义确保调用方事务内的硬门一致性
- **QMS 定义自己的枚举宏** — MES 的 `define_mes_enum!` 无 `#[macro_export]`，仅模块内可见。QMS 在 `qms/enums.rs` 中定义 `define_qms_enum!` 宏（参照 MES 的模式），避免 10 个枚举的手动样板代码。未来可提取到 shared/ 统一
- **check_gate 签名包含 inspection_type** — 一个来源可能有多种检验（IQC+OQC），调用方通过 inspection_type 参数指定检查哪个关卡（IQC for WMS 来料, IPQC for MES 工序, OQC for Sales 出货）
- **InspectionSourceType 与 DocumentType 分离** — QMS 内部使用 InspectionSourceType 保持语义清晰，与共享层交互时通过 from_document_type() 映射转换。WorkOrderRouting 映射到已有的 DocumentType::WorkOrder
- **JSONB 列在 Repo 层序列化** — CheckItem/SamplePlan/CheckResult 在 Model 中是强类型 Rust 结构体，Repo 层通过 serde_json::to_value/from_value 处理 JSONB 列的存取
- **MRB execute_disposition 禁止前端直调** — 仅通过 WorkflowHook.on_approved 回调触发，API 层面不做暴露或做权限硬限制
- **RMA record_root_cause 自动触发状态转换** — Investigating → ActionTaken 由 Service 层内部处理，调用方不需要手动管理状态

---

## Open Questions

### Resolved During Planning

- **迁移编号** — 使用 007，接续现有 006_create_reconciliation.sql
- **DocumentType/DomainEventType** — 已预注册，无需修改共享层枚举
- **QualityGateService 事务模式** — 设计文档明确为 InCallerTx

### Deferred to Implementation

- MRB submit_for_review 与 WorkflowEngine 的具体 API 对接 — WorkflowEngine 模块尚未实现，QMS 先定义接口调用签名，实际对接时可能需要适配
- permission.proto 中 QMS Resource 枚举值的具体命名 — 需要与前端协调
- RMA 追溯链中 DocumentLink 的具体关联范围（RMA → WorkOrder → Routing → ArrivalNotice 的完整链路构建时机）

---

## Output Structure

```
abt-core/src/qms/
  mod.rs                                    -- 模块声明 + pub use
  enums.rs                                  -- 10 个 QMS 枚举定义
  inspection_specification/
    mod.rs                                  -- pub mod implt, model, repo, service
    model.rs                                -- InspectionSpecification + CheckItem + SamplePlan + Req/Filter
    repo.rs                                 -- InspectionSpecificationRepo
    service.rs                              -- InspectionSpecificationService trait
    implt/mod.rs                            -- InspectionSpecificationServiceImpl
  inspection_result/
    mod.rs
    model.rs                                -- InspectionResult + CheckResult + Req/Filter
    repo.rs                                 -- InspectionResultRepo
    service.rs                              -- InspectionResultService trait
    implt/mod.rs                            -- InspectionResultServiceImpl
  quality_gate/
    mod.rs                                  -- QualityGateService trait
    implt/mod.rs                            -- QualityGateServiceImpl（读取 InspectionResult 判定状态）
  mrb/
    mod.rs
    model.rs                                -- MRB + Req/Filter
    repo.rs                                 -- MRBRepo
    service.rs                              -- MRBService trait
    implt/mod.rs                            -- MRBServiceImpl
  rma/
    mod.rs
    model.rs                                -- RMA + Req/Filter
    repo.rs                                 -- RMARepo
    service.rs                              -- RMAService trait
    implt/mod.rs                            -- RMAServiceImpl

abt-core/migrations/
  007_create_qms.sql                        -- 4 张主表 + 索引 + 唯一约束

proto/abt/v1/
  qms.proto                                 -- gRPC 消息和服务定义

abt-grpc/src/handlers/
  qms.rs                                    -- QMS gRPC handler（新建）
```

---

## Implementation Units

### U1. Foundation — Migration, Enums, JSONB Types

**Goal:** 建立 QMS 模块的数据库 schema、枚举定义和 JSONB 值类型基础

**Requirements:** R8

**Dependencies:** None

**Files:**
- Create: `abt-core/migrations/007_create_qms.sql`
- Create: `abt-core/src/qms/mod.rs`
- Create: `abt-core/src/qms/enums.rs`

**Approach:**
- 迁移文件创建 4 张主表（inspection_specifications, inspection_results, mrbs, rmas），包含所有设计文档定义的字段、JSONB 列、唯一约束（inspection_results 的 UNIQUE(source_type, source_id, inspection_type) WHERE deleted_at IS NULL）、索引
- 枚举定义复用 MES 的宏模式，定义 10 个 QMS 枚举（InspectionType, InspectionSourceType, InspectionResultType, InspectionStatus, QualityGateStatus, SpecStatus, MRBDisposition, ResponsibleParty, MRBStatus, Severity, RMAStatus）
- qms/mod.rs 作为模块根文件，声明所有子模块

**Patterns to follow:**
- `abt-core/migrations/003_create_mes.sql` — 迁移 SQL 格式
- `abt-core/src/mes/enums.rs` — 枚举宏模式

**Test scenarios:**
- Happy path: migration up/down 可重复执行，表和索引创建正确
- Edge case: 唯一约束在并发插入同一 (source_type, source_id, inspection_type) 时正确拒绝
- Happy path: 每个枚举的 from_i16/as_i16 双向转换正确

**Verification:**
- `cargo clippy -p abt-core` 通过
- 所有枚举可被 abt-core 内的其他模块引用

---

### U2. InspectionSpecification Sub-module

**Goal:** 实现检验规格的完整 CRUD，含状态机驱动（Draft→Active→Inactive）、乐观锁、文档编号生成和审计日志

**Requirements:** R1, R6, R8

**Dependencies:** U1

**Files:**
- Create: `abt-core/src/qms/inspection_specification/mod.rs`
- Create: `abt-core/src/qms/inspection_specification/model.rs`
- Create: `abt-core/src/qms/inspection_specification/repo.rs`
- Create: `abt-core/src/qms/inspection_specification/service.rs`
- Create: `abt-core/src/qms/inspection_specification/implt/mod.rs`
- Modify: `abt-core/src/qms/mod.rs` — 添加子模块声明和 pub use

**Approach:**
- Model 定义 InspectionSpecification 实体（sqlx::FromRow）、CheckItem/SamplePlan JSONB 强类型、CreateInspectionSpecificationReq/UpdateInspectionSpecificationReq、InspectionSpecFilter
- Repo 实现基础 CRUD + find_by_product_and_type + 动态条件列表查询 + 乐观锁 update（WHERE id = ? AND version = expected_version）
- Service trait 定义 5 个方法：create, get, find_by_product_and_type, update, list
- Impl 集成 DocumentSequenceService（生成 QS-YYYY-MM-xxxxx 编号）、StateMachineService（状态转换）、AuditLogService（写操作审计）
- update 方法通过 expected_version 实现乐观锁，version 不匹配时返回 DomainError::ConcurrentConflict

**Patterns to follow:**
- `abt-core/src/sales/quotation/` — 完整的 model/repo/service/implt 分层
- `abt-core/src/purchase/order/implt/mod.rs` — 乐观锁和共享服务集成序列

- **MRB WorkflowEngine 集成策略** — 定义最小 WorkflowService trait（create_instance），提供 stub 实现。submit_for_review 完成状态转换并调用 stub，未来替换为真实 WorkflowEngine
- **MRB 临时审批路径** — 在 WorkflowEngine 就绪前，提供 approve 方法（权限门控）供管理员直接审批，WorkflowEngine 就绪后切换为回调触发

**Test scenarios:**
- Happy path: create 生成正确编号，初始状态为 Draft，check_items/sample_plan 作为 JSONB 正确存取
- Happy path: update with correct expected_version 成功，version 自增
- Edge case: update with wrong expected_version 返回 ConcurrentConflict
- Happy path: StateMachineService.transition Draft→Active→Inactive 各阶段正确
- Happy path: list with InspectionSpecFilter 各字段组合筛选正确
- Integration: create 同事务内调用 AuditLogService.record
- Happy path: find_by_product_and_type 返回正确结果（含 None case）

**Verification:**
- `cargo clippy -p abt-core` 通过
- InspectionSpecificationServiceImpl 可被实例化（依赖注入通过构造函数）

---

### U3. InspectionResult + QualityGateService

**Goal:** 实现检验结果管理和独立的质量关卡服务。record_result 录入结果并返回 QualityGateStatus，check_gate 提供三值判定（Passed/Failed/NotRequired）

**Requirements:** R2, R3, R6, R8

**Dependencies:** U1, U2（QualityGateServiceImpl 需要读取 InspectionSpecification 判断是否存在活跃规格）

**Files:**
- Create: `abt-core/src/qms/inspection_result/mod.rs`
- Create: `abt-core/src/qms/inspection_result/model.rs`
- Create: `abt-core/src/qms/inspection_result/repo.rs`
- Create: `abt-core/src/qms/inspection_result/service.rs`
- Create: `abt-core/src/qms/inspection_result/implt/mod.rs`
- Create: `abt-core/src/qms/quality_gate/mod.rs`
- Create: `abt-core/src/qms/quality_gate/implt/mod.rs`
- Modify: `abt-core/src/qms/mod.rs` — 添加子模块声明和 pub use（含 pub use quality_gate::QualityGateService）

**Approach:**
- InspectionResult Model 定义实体（含 deleted_at）、CheckResult JSONB 强类型、CreateInspectionResultReq/RecordInspectionResultReq、InspectionResultFilter
- InspectionResultRepo 实现基础 CRUD + 按 source_type/source_id 查询 + 幂等唯一约束检查
- InspectionResultService trait：create, get, record_result（返回 QualityGateStatus）, list_by_source
- InspectionResultServiceImpl 集成：DocumentSequence（QR-YYYY-MM-xxxxx）、IdempotencyService（幂等键 qms:record:{source_type}:{source_id}:{inspection_type}）、StateMachineService（Pending→Completed）、AuditLogService、DomainEventBus（InspectionPassed/InspectionFailed 事件）
- record_result 的 Guard 条件：qualified_qty + unqualified_qty == sample_qty
- QualityGateService 独立 trait：check_gate(ctx, source_type, source_id, inspection_type) → QualityGateStatus。调用方通过 inspection_type 指定关卡类型
- QualityGateServiceImpl 读取对应 source+inspection_type 的最新 InspectionResult，结合 InspectionSpecification 的 status 判定

**Patterns to follow:**
- `abt-core/src/purchase/order/implt/mod.rs` — IdempotencyService 集成
- `abt-core/src/shared/types/context.rs` — ServiceContext reborrow 模式

**Test scenarios:**
- Happy path: create 创建 Pending 状态的检验结果，UNIQUE 约束防重复
- Happy path: record_result 录入 Pass 结果，状态变为 Completed，返回 QualityGateStatus::Passed
- Happy path: record_result 录入 Fail 结果，发布 InspectionFailed 事件，返回 QualityGateStatus::Failed
- Edge case: record_result 数量不守恒（qualified + unqualified ≠ sample）返回 BusinessRule 错误
- Edge case: 重复调用 record_result（幂等键冲突）返回 Idempotency 错误
- Happy path: check_gate 有活跃规格 + Pass 结果 → Passed
- Happy path: check_gate 有活跃规格 + Pending 结果 → Failed
- Happy path: check_gate 无活跃规格 → NotRequired
- Happy path: check_gate Conditional 结果映射为 Passed
- Integration: record_result 同事务内调用 StateMachine + AuditLog + EventBus

**Verification:**
- `cargo clippy -p abt-core` 通过
- QualityGateService trait 可被其他模块（如 MES）引用而无需依赖整个 QMS 模块

---

### U4. MRB Sub-module

**Goal:** 实现 MRB 不良评审的完整生命周期，包括 submit_for_review 集成 WorkflowEngine 和 execute_disposition 限制为仅 WorkflowHook 回调触发

**Requirements:** R4, R6

**Dependencies:** U1, U3（MRB 关联 InspectionResult）

**Files:**
- Create: `abt-core/src/qms/mrb/mod.rs`
- Create: `abt-core/src/qms/mrb/model.rs`
- Create: `abt-core/src/qms/mrb/repo.rs`
- Create: `abt-core/src/qms/mrb/service.rs`
- Create: `abt-core/src/qms/mrb/implt/mod.rs`
- Modify: `abt-core/src/qms/mod.rs` — 添加子模块声明和 pub use

**Approach:**
- Model 定义 MRB 实体、CreateMRBReq/ExecuteDispositionReq、MRBFilter
- Repo 实现 CRUD + 动态条件列表查询
- Service trait：create, get, submit_for_review, approve（临时直接审批，权限门控）, execute_disposition, list
- submit_for_review 完成状态 Draft→UnderReview，调用 WorkflowService stub（当前为空操作）
- approve 提供临时审批路径（UnderReview→Approved），需审批权限，WorkflowEngine 就绪后替换为回调触发
- execute_disposition 在 Approved 状态执行，触发 Approved→Completed，发布 MRBDispositioned 事件

**Patterns to follow:**
- `abt-core/src/sales/quotation/implt/mod.rs` — 共享服务集成序列
- 设计文档 note 中 MRB 的事务模式和 WorkflowHook 回调约定

**Test scenarios:**
- Happy path: create 关联 InspectionResult，初始状态为 Draft
- Happy path: submit_for_review 触发 Draft→UnderReview，调用 WorkflowService stub
- Edge case: submit_for_review 在非 Draft 状态调用返回 BusinessRule 错误
- Happy path: approve 触发 UnderReview→Approved（临时审批路径，权限门控）
- Edge case: approve 在非 UnderReview 状态调用返回 BusinessRule 错误
- Happy path: execute_disposition 在 Approved 状态执行，触发 Approved→Completed
- Edge case: execute_disposition 在非 Approved 状态调用返回 BusinessRule 错误
- Integration: execute_disposition 发布 MRBDispositioned 事件 + AuditLog
- Happy path: list with MRBFilter 各字段组合筛选正确

**Verification:**
- `cargo clippy -p abt-core` 通过
- MRBServiceImpl 依赖 InspectionResultService（通过 Arc<dyn trait>）验证跨子模块引用正确

---

### U5. RMA Sub-module

**Goal:** 实现 RMA 客诉追溯的完整生命周期，含 record_root_cause 自动状态转换和 DocumentLink 追溯链构建

**Requirements:** R5, R6

**Dependencies:** U1, U3（RMA 可选关联 InspectionResult）

**Files:**
- Create: `abt-core/src/qms/rma/mod.rs`
- Create: `abt-core/src/qms/rma/model.rs`
- Create: `abt-core/src/qms/rma/repo.rs`
- Create: `abt-core/src/qms/rma/service.rs`
- Create: `abt-core/src/qms/rma/implt/mod.rs`
- Modify: `abt-core/src/qms/mod.rs` — 添加子模块声明和 pub use

**Approach:**
- Model 定义 RMA 实体、CreateRMAReq/RecordRootCauseReq、RMAFilter
- Repo 实现 CRUD + 动态条件列表查询
- Service trait：create, get, record_root_cause, close, list
- create 时若携带 linked_inspection_result_id，同事务构建 DocumentLink（RMA → InspectionResult）
- record_root_cause 写入 root_cause + corrective_action，同时触发 Investigating→ActionTaken 状态转换
- close 触发 ActionTaken→Closed 状态转换（不发布事件，DomainEventType 未注册 RMAClosed）

**Patterns to follow:**
- `abt-core/src/sales/quotation/implt/mod.rs` — 共享服务集成序列
- `abt-core/src/shared/document_link/service.rs` — DocumentLink 集成模式

**Test scenarios:**
- Happy path: create 生成 RMA-YYYY-MM-xxxxx 编号，初始状态为 Reported
- Happy path: create with linked_inspection_result_id 同事务创建 DocumentLink
- Happy path: record_root_cause 写入字段并自动触发 Investigating→ActionTaken
- Edge case: record_root_cause 在非 Investigating 状态调用返回 BusinessRule 错误
- Happy path: close 触发 ActionTaken→Closed
- Edge case: close 在非 ActionTaken 状态调用返回 BusinessRule 错误
- Integration: create + DocumentLink + AuditLog 同事务正确执行
- Happy path: list with RMAFilter 各字段组合筛选正确

**Verification:**
- `cargo clippy -p abt-core` 通过
- RMAServiceImpl 依赖 DocumentLinkService 和 InspectionResultService 验证正确

---

### U6. Proto Definition + gRPC Handlers + Wiring

*DEFERRED — 不在本 PR 范围内，gRPC 层后续单独处理。*

### U7. Integration Tests

*DEFERRED — 随 gRPC 层一起在后续 PR 中处理。*

---

## System-Wide Impact

- **Interaction graph:** QualityGateService.check_gate() 将被 WMS（来料入库）、MES（工序报工）、Sales（出货发货）在各自的事务内调用。当前这些调用点尚未实现（MES 使用 stub），但 QMS trait 定义已作为集成契约
- **Error propagation:** QMS Service 返回 DomainError，gRPC handler 映射为 tonic::Status。check_gate 的 Failed 不应被视为"错误"而是业务状态，调用方根据返回值决定是否放行
- **State lifecycle risks:** MRB execute_disposition 涉及跨模块副作用（CostEntry + WMS 库存调整），通过 DomainEventBus 异步解耦避免分布式事务。若异步消费失败，事件进入 DeadLetter 等待人工介入
- **Integration coverage:** 单元测试验证各 Service 的独立行为；集成测试验证跨 Service 交互（如 record_result → check_gate → MRB 创建的链路）
- **Unchanged invariants:** 共享层枚举（DocumentType, DomainEventType）已预注册 QMS 条目，本计划不修改这些枚举。abt crate 代码不受影响

---

## Risks & Dependencies

| Risk | Mitigation |
|------|------------|
| WorkflowEngine 尚未实现，MRB submit_for_review 无法实际触发审批流 | QMS 先定义 WorkflowEngine 调用接口，实际调用点用 TODO 标注。submit_for_review 仍完成状态转换（Draft→UnderReview），仅 WorkflowEngine 创建步骤暂为空操作或 log warning |
| QualityGateService 与 WMS/MES 的实际集成时机不确定 | 保持 QualityGateService 作为独立轻量 trait，其他模块可在各自迭代中接入。当前 MES 的 QmsInspectionStub 保持不变 |
| JSONB 强类型序列化边界情况（如 tolerance 字段的格式约定） | CheckItem/SamplePlan/CheckResult 使用简单的 String 字段（非数值类型），serde 序列化风险低。通过集成测试验证往返一致性 |
| 10 个枚举 + 4 个 Service trait 的样板代码量大 | 复用 define_mes_enum! 宏减少枚举样板；各子模块遵循相同分层模式，模式熟练后效率高 |

---

## Sources & References

- **Origin document:** [docs/uml-design/06-qms.html](../docs/uml-design/06-qms.html) — QMS v2.3 完整设计
- **Shared infrastructure spec:** [docs/uml-design/README.md](../docs/uml-design/README.md) — 共享基础设施接口规范
- **Pattern reference:** `abt-core/src/sales/quotation/` — Sales 模块分层模式
- **Pattern reference:** `abt-core/src/purchase/order/` — Purchase 模块（含幂等性和乐观锁）
- **Pattern reference:** `abt-core/src/mes/enums.rs` — 枚举宏模式
- **Pattern reference:** `abt-core/src/mes/stubs.rs` — MES stub（QmsInspectionStub）
