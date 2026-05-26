---
name: workflow-action-defs
description: 工作流 AutoTask Action 定义查询接口，返回已注册 action 的参数元数据
---

# 工作流 AutoTask Action 定义查询接口

## 目标

新增 `ListActionDefs` RPC，返回 ActionRegistry 中已注册 action 的元数据（名称、标签、输入参数、输出字段），前端据此引导用户配置 auto_task 节点。

## Proto

在 `AbtWorkflowService` 新增：

```protobuf
rpc ListActionDefs(ListActionDefsRequest) returns (ActionDefListResponse);

message ListActionDefsRequest {}

message ActionDefListResponse {
  repeated ActionDef items = 1;
}

message ActionDef {
  string name = 1;
  string label = 2;
  string description = 3;
  repeated FieldDef inputs = 4;
  repeated FieldDef outputs = 5;
}

message FieldDef {
  string name = 1;
  string label = 2;
  string field_type = 3;
  bool required = 4;
  string description = 5;
}
```

## Model

在 `abt/src/models/` 中新增 struct：

- `ActionDef` — name, label, description, inputs: Vec\<FieldDef\>, outputs: Vec\<FieldDef\>
- `FieldDef` — name, label, field_type, required, description

实现 model → proto 的转换方法。

## ActionRegistry 改造

`ActionRegistry` 新增 `defs: HashMap<String, ActionDef>`：

- `register()` 签名改为 `register(name, def: ActionDef, action: Arc<dyn AutoAction>)`
- 新增 `list_defs() -> &[ActionDef]`（或迭代器）
- `validate_startup()` 签名不变，只校验 actions map

## gRPC Handler

在 `abt-grpc/src/handlers/workflow.rs` 新增 `ListActionDefs`：

- 从 `WorkflowEngine` 获取 `ActionRegistry`
- 调用 `list_defs()` 转换为 proto `ActionDefListResponse`

## 不做的事

- 不注册具体 action 实现
- 不修改 `AutoAction` trait
- 不修改 `execute()` 执行流程
- 不修改前端组件
