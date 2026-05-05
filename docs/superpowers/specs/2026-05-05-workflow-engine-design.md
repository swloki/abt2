---
name: Workflow Engine Design
date: 2026-05-05
status: approved
---

# Workflow Engine Design

## Overview

为 ABT 系统引入嵌入式工作流引擎，支持可配置的审批流程和任务分配。引擎完全集成在现有 Rust + PostgreSQL 架构中，不依赖外部服务。

## Decision: Self-built vs External Library

选择自建引擎，排除 Temporal 等外部工作流平台，原因：

- Temporal Rust SDK 仍为 prerelease，API 不稳定
- Temporal 需要独立的 Temporal Server 部署，与"嵌入式"需求冲突
- ABT 的工作流需求（审批 + 任务分配）复杂度可控，不需要分布式编排能力
- 自建方案保持所有数据在同一个 PostgreSQL 中，事务一致性更易保证

## Architecture

遵循现有分层模式：

```
proto/abt/v1/workflow.proto           ← gRPC 定义
abt/src/models/workflow.rs            ← 数据模型
abt/src/repositories/
  workflow_template_repo.rs           ← 模板 CRUD
  workflow_node_repo.rs               ← 节点 CRUD
  workflow_edge_repo.rs               ← 边 CRUD
  workflow_instance_repo.rs           ← 实例 CRUD
  workflow_task_repo.rs               ← 任务 CRUD
abt/src/service/workflow_service.rs   ← Service trait
abt/src/implt/workflow_engine.rs      ← 核心引擎
abt/src/implt/workflow_template_mgr.rs ← 模板管理
abt-grpc/src/handlers/workflow.rs     ← gRPC handler
```

## Data Model

### workflow_templates

| Column | Type | Description |
|--------|------|-------------|
| id | UUID | PK |
| name | VARCHAR | 模板名称 |
| entity_type | VARCHAR | 关联业务实体类型（product, bom, purchase_order 等） |
| version | INT | 版本号 |
| status | VARCHAR | draft / active / archived |
| created_at | TIMESTAMP | |
| updated_at | TIMESTAMP | |

### workflow_nodes

| Column | Type | Description |
|--------|------|-------------|
| id | UUID | PK |
| template_id | UUID | FK → templates |
| node_type | VARCHAR | start / end / approval / task / condition |
| name | VARCHAR | 节点名称 |
| config | JSONB | 节点配置（审批人规则、任务描述模板等） |
| position | INT | 顺序 |

### workflow_edges

| Column | Type | Description |
|--------|------|-------------|
| id | UUID | PK |
| template_id | UUID | FK → templates |
| from_node_id | UUID | FK → nodes |
| to_node_id | UUID | FK → nodes |
| condition | JSONB | 可选，流转条件 |

### workflow_instances

| Column | Type | Description |
|--------|------|-------------|
| id | UUID | PK |
| template_id | UUID | FK → templates |
| entity_type | VARCHAR | 业务实体类型 |
| entity_id | UUID | 业务实体 ID |
| status | VARCHAR | running / completed / rejected / cancelled |
| current_node_ids | UUID[] | 当前活跃节点（支持并行） |
| initiator_id | UUID | 发起人 |
| created_at | TIMESTAMP | |
| completed_at | TIMESTAMP | |

### workflow_tasks

| Column | Type | Description |
|--------|------|-------------|
| id | UUID | PK |
| instance_id | UUID | FK → instances |
| node_id | UUID | FK → nodes |
| assignee_id | UUID | 被分配人 |
| action | VARCHAR | approve / reject / complete / delegate |
| status | VARCHAR | pending / completed / rejected / delegated |
| result | JSONB | 操作结果（评论、意见等） |
| created_at | TIMESTAMP | |
| completed_at | TIMESTAMP | |

### Indexes

```sql
CREATE INDEX idx_workflow_tasks_assignee_status ON workflow_tasks(assignee_id, status);
CREATE INDEX idx_workflow_instances_entity ON workflow_instances(entity_type, entity_id);
```

## WorkflowEngine Core Logic

### start_instance(template_id, entity_type, entity_id, initiator_id)

1. 加载模板节点和边
2. 验证模板状态为 active
3. 创建实例记录
4. 找到 start 节点后的第一个节点
5. 为该节点创建 task 并分配审批人
6. 返回 instance_id

### advance_instance(task_id, action, result)

1. 在事务中执行
2. 标记当前 task 完成/拒绝
3. 查找当前节点的出边
4. 评估条件，确定下一个节点
5. 如果下一个是 end 节点，标记实例完成
6. 如果是 approval/task 节点，创建新 task 并分配
7. 触发 on_approved / on_rejected 回调（如有）

### reject_instance(task_id, reason)

1. 标记 task 为 rejected
2. 根据 config 中的 reject_action 决定行为：
   - terminate: 直接终止实例
   - back_to_previous: 退回到上一个节点，创建新 task

## Assignee Configuration (node.config JSONB)

```json
{
  "assignee_type": "role",
  "assignee_value": "manager",
  "multi_approval": "any",
  "reject_action": "terminate",
  "on_approved": {
    "action": "update_entity_status",
    "params": {"status": "approved"}
  }
}
```

assignee_type 值：`role` | `user` | `department_head` | `initiator_manager`

multi_approval 值：`any`（任一通过即推进）| `all`（全部通过才推进）

## gRPC API

```protobuf
service WorkflowService {
  // Template management
  rpc CreateTemplate(CreateTemplateRequest) returns (Template);
  rpc UpdateTemplate(UpdateTemplateRequest) returns (Template);
  rpc GetTemplate(GetTemplateRequest) returns (Template);
  rpc ListTemplates(ListTemplatesRequest) returns (TemplateList);
  rpc PublishTemplate(PublishTemplateRequest) returns (Template);

  // Instance management
  rpc StartInstance(StartInstanceRequest) returns (Instance);
  rpc GetInstance(GetInstanceRequest) returns (Instance);
  rpc ListInstances(ListInstancesRequest) returns (InstanceList);
  rpc CancelInstance(CancelInstanceRequest) returns (Instance);

  // Task operations
  rpc GetMyTasks(GetMyTasksRequest) returns (TaskList);
  rpc GetTasksByInstance(GetTasksByInstanceRequest) returns (TaskList);
  rpc ApproveTask(TaskActionRequest) returns (Task);
  rpc RejectTask(TaskActionRequest) returns (Task);
  rpc DelegateTask(DelegateTaskRequest) returns (Task);
}
```

### Error Handling

| Status Code | Scenario |
|-------------|----------|
| NOT_FOUND | 模板/实例/任务不存在 |
| FAILED_PRECONDITION | 试图对已完成实例操作、模板 draft 时创建实例 |
| PERMISSION_DENIED | 非审批人试图审批 |
| INVALID_ARGUMENT | 流程定义不合法（缺少 start/end 节点） |

## Implementation Plan

### Step 1: Template CRUD

实现模板、节点、边的创建和管理。Repository + Service + gRPC handler。不涉及运行时逻辑。

### Step 2: Engine Core

实现 WorkflowEngine 的 start_instance 和 advance_instance。先用线性流程（start → approval → end）验证基本通路。

### Step 3: Condition Branches & Parallel

在引擎中加入条件评估和并行网关支持。

### Step 4: gRPC API & Frontend Integration

暴露完整的 gRPC 接口。

## Testing

- Repository 层：sqlx test fixture
- Service 层：mock repository 测试引擎逻辑
- 集成测试：完整的"创建模板 → 发起实例 → 审批 → 完成"流程
- 边界测试：驳回后退回、并行节点部分完成、条件分支选择

## V1 Scope Exclusions

- 拖拽式流程设计器 UI
- 定时器/超时节点
- 子流程嵌套
- 复杂会签规则（V1 只支持 any 和 all）
