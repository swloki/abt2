# Workflow Trigger System Design

## Problem

业务代码直接调用 `WorkflowEngine::start_instance()` 启动工作流，存在两个问题：

1. **前后端耦合**：前端无法感知系统有哪些可用的触发事件，无法在前端配置"哪个事件触发哪个工作流"
2. **业务代码绑定**：库存服务直接依赖 `start_instance`，换工作流需要改业务代码

## Solution

在 `workflow_templates` 表上加 `trigger_event` 字段，实现"触发器 → 模板"的一对一绑定。业务代码只调 `trigger(event, entity_id, initiator_id)`，具体启动哪个工作流由数据库配置决定。

## Design Decisions

| 决策 | 选择 | 理由 |
|------|------|------|
| 绑定方式 | 模板自带 trigger_event 字段 | 一个触发器只绑定一个活跃模板，满足当前需求 |
| 事件定义来源 | 后端代码静态定义 | 触发器必须有对应的 `trigger()` 调用才有效，前端自定义无意义 |
| trigger 签名 | `(event, entity_id, initiator_id)` | 工作流内部通过 entity_id 查业务数据，不需要额外 context |
| 无绑定行为 | 静默返回 None | 触发器没绑定模板时不报错，避免业务代码加防御逻辑 |

## Data Model

### workflow_templates 变更

```sql
ALTER TABLE workflow_templates ADD COLUMN trigger_event VARCHAR(100);

CREATE INDEX idx_workflow_templates_trigger
    ON workflow_templates(trigger_event)
    WHERE status = 'active' AND deleted_at IS NULL;
```

`trigger_event` 可为空——不绑定触发器的模板仍可手动启动实例。

### TriggerEventDef（代码结构体）

```rust
pub struct TriggerEventDef {
    pub name: &'static str,        // "inventory_updated"
    pub label: &'static str,       // "库存变更"
    pub description: &'static str, // "当库存数量发生增减时触发"
}
```

定义在 `abt/src/models/workflow.rs` 中，通过 `all_trigger_events()` 返回所有可用事件。新增触发器 = 在此数组加一条 + 在业务代码中调用 `trigger()`。

## API

### 新增 RPC：ListTriggerEvents

```protobuf
rpc ListTriggerEvents(ListTriggerEventsRequest) returns (TriggerEventListResponse);

message ListTriggerEventsRequest {}

message TriggerEventDef {
  string name = 1;
  string label = 2;
  string description = 3;
  int64 bound_template_id = 4;
  string bound_template_name = 5;
}

message TriggerEventListResponse {
  repeated TriggerEventDef items = 1;
}
```

返回后端定义的所有触发事件，以及每个事件已绑定的活跃模板信息（id、name）。前端可以展示触发器列表及其绑定状态。

### 现有 RPC 变更

`CreateWorkflowTemplateRequest` 和 `UpdateWorkflowTemplateRequest` 加 `trigger_event` 字段，创建/编辑模板时可指定触发器。

## Core Logic

### trigger() 方法

```rust
impl WorkflowEngine {
    pub async fn trigger(
        &self,
        event: &str,
        entity_id: i64,
        initiator_id: i64,
    ) -> Result<Option<i64>> {
        let template = WorkflowTemplateRepo::find_active_by_trigger(&self.pool, event).await?;
        match template {
            Some(t) => {
                let id = self.start_instance_internal(...).await?;
                Ok(Some(id))
            }
            None => Ok(None),
        }
    }
}
```

### Repository 新增

```rust
WorkflowTemplateRepo::find_active_by_trigger(pool, trigger_event) -> Result<Option<WorkflowTemplate>>
```

SQL: `WHERE trigger_event = $1 AND status = 'active' AND deleted_at IS NULL LIMIT 1`

## Business Integration Points

| 触发事件 | 调用位置 | 时机 |
|---------|---------|------|
| `inventory_updated` | InventoryServiceImpl | 库存增减操作后 |
| 后续按需添加 | 对应 Service 层 | 业务操作后 |

## Files Changed

| 文件 | 变更 |
|------|------|
| `abt/migrations/044_add_trigger_event.sql` | 新增列和索引 |
| `abt/src/models/workflow.rs` | 新增 `TriggerEventDef`、`all_trigger_events()` |
| `abt/src/repositories/workflow_template_repo.rs` | 新增 `find_active_by_trigger` |
| `abt/src/service/workflow_service.rs` | trait 新增 `trigger`、`list_trigger_events` |
| `abt/src/implt/workflow_engine.rs` | 实现 `trigger()`、`list_trigger_events()` |
| `abt-grpc/src/handlers/workflow.rs` | handler 新增 `list_trigger_events`、创建/更新模板支持 trigger_event |
| `proto/abt/v1/workflow.proto` | 新增 `ListTriggerEvents` RPC 和消息 |

## Out of Scope

- 一个触发器绑定多个模板（当前不需要）
- 触发器条件过滤（如"只在库存低于阈值时触发"——在 Condition 层处理）
- 触发器启用/禁用开关（通过 archive 模板实现）
- 前端 UI 设计
