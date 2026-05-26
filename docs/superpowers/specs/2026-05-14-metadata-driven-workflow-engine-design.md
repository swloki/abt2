---
name: Metadata-Driven Workflow Engine Design
date: 2026-05-14
status: draft
---

# 元数据驱动的 Rust 工作流引擎设计

## 1. 设计目标

构建一个大型工作流系统中可独立部署的微服务引擎，支持：

- **任务节点的无状态与可复用**：同一业务任务可在不同流程中复用。
- **流程的灵活编排**：通过元数据（JSON/YAML）定义流程结构与数据流，无需修改任务代码即可调整组合方式。
- **类型安全的数据传递**：利用 Rust 类型系统，在运行时通过统一上下文和表达式映射保证数据正确性。
- **与业务解耦**：引擎只负责调度与数据适配，不感知具体业务逻辑。

## 2. 核心架构组件

| 组件 | 职责 | 关键点 |
|------|------|--------|
| Task Trait | 所有业务任务的统一接口 | 只定义 `execute(inputs) -> Result<TaskOutput>`，不感知流程 |
| WorkflowContext | 全局数据总线 | 承载流程变量，任务间通过 key 读写数据 |
| WorkflowDefinition | 流程的元数据模型 | 定义有哪些节点、转移规则、输入输出映射 |
| WorkflowEngine | 运行时引擎 | 解析元数据，调度任务，执行数据映射，管理状态转移 |
| Task Registry | 任务类型注册表 | 将任务类型名映射到具体的 Trait 实现实例 |

## 3. 关键设计原则

### 3.1 任务与流程彻底解耦

- **任务实现**：只关心"我需要哪些参数，产出哪些结果"，不决定下一步去哪。
- **流程定义**：描述任务节点的连接方式、转移条件、数据映射关系。
- **任务接口固定为 `execute(ctx)`**，通过上下文交互，而非直接调用下一个任务。

### 3.2 元数据驱动的编排

所有流程结构均以声明式配置描述。示例：

```json
{
  "name": "order_fulfillment",
  "startTask": "lock_inventory",
  "endTasks": ["order_closed"],
  "tasks": {
    "lock_inventory": {
      "type": "LockInventoryTask",
      "inputMapping": { "orderId": "${order.id}" },
      "outputMapping": { "lockId": "inventory_lock_id" }
    },
    "create_shipment": {
      "type": "CreateShipmentTask",
      "inputMapping": { "lockId": "${inventory_lock_id}" }
    }
  },
  "transitions": [
    { "from": "lock_inventory", "to": "create_shipment", "condition": "success" }
  ]
}
```

### 3.3 三步数据适配机制

1. **注册时**：任务只声明其类型名和实现，不绑定任何数据源。
2. **设计时**：在流程定义中为每个节点实例配置 `inputMapping` / `outputMapping`。
3. **运行时**：引擎通过表达式引擎解析映射，从上下文中提取数据注入任务，收集结果写回上下文。

这种"定义与调用分离"的模式实现了同一任务的多场景复用。

## 4. 核心类型定义（Rust 伪代码）

### 4.1 任务接口

```rust
#[async_trait]
pub trait Task: Send + Sync {
    /// 接收经过映射的输入参数，返回执行结果
    async fn execute(&self, inputs: HashMap<String, Value>) -> Result<TaskOutput>;
}

pub struct TaskOutput {
    pub status: TaskStatus,              // Success / Failed
    pub data: HashMap<String, Value>,    // 输出字段
}
```

### 4.2 上下文

```rust
#[derive(Default)]
pub struct WorkflowContext {
    variables: HashMap<String, Value>,
}
```

### 4.3 流程元数据（反序列化用）

```rust
struct WorkflowDefinition {
    name: String,
    start_task: String,
    end_tasks: Vec<String>,
    tasks: HashMap<String, TaskDefinition>,
    transitions: Vec<Transition>,
}

struct TaskDefinition {
    type_name: String,
    input_mapping: HashMap<String, String>,    // 参数名 -> 表达式
    output_mapping: HashMap<String, String>,   // 输出字段名 -> 上下文变量名
}

struct Transition {
    from: String,
    to: String,
    condition: String,    // "success" | "failed" | "always" | 表达式
}
```

## 5. 引擎执行流程

1. **加载定义**：从持久化存储读取 `WorkflowDefinition`。
2. **初始化上下文**：注入业务初始数据（如订单信息）。
3. **从 `start_task` 开始循环**：
   - 查定义，获取当前节点的 `TaskDefinition`。
   - 调用表达式引擎，解析 `inputMapping`，从上下文提取值。
   - 从注册表获取对应类型的 `Task` 实例，调用 `execute(inputs)`。
   - 根据 `outputMapping` 将结果写回上下文。
   - 根据任务状态和 `transitions` 决定下一个任务。
4. 当进入 `end_tasks` 集合中的某个任务时，流程结束。

## 6. 设计要点总结

| 要点 | 说明 |
|------|------|
| 任务是无状态的 | 不持有流程状态，不指定后继 |
| 编排是集中的 | 所有路由、数据映射规则全部在元数据中定义 |
| 数据传递是显式的 | 通过映射配置连接上下游，避免隐式耦合 |
| 引擎是通用的 | 不包含任何业务代码，可复用于各种业务场景 |
| Rust 实现选择 | `async_trait` 定义任务接口，`serde` 处理元数据，`regex` 或自定义解析器处理表达式 |

此设计可直接映射到独立微服务架构中，工作流引擎服务仅需依赖任务注册表、元数据存储和表达式解析器，即可成为完整的流程编排中心。
