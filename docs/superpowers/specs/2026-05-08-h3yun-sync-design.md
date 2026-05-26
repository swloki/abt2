---
name: H3Yun ERP Sync
date: 2026-05-08
status: approved
---

# H3Yun ERP 同步功能设计

## 概述

将 ABT 系统的产品信息和库存数据单向同步到 H3Yun ERP。复用旧系统（abt_废弃）的 H3Yun API 接口和凭证，采用独立 Sync 模块架构。通过 tokio channel 统一三种触发方式（定时、手动、实时），使用独立映射表跟踪同步状态，支持逐记录错误隔离和定期对账。

同步功能计划后续从后端删除，因此不修改现有表结构，所有同步相关状态存储在独立的 `h3yun_sync_state` 表中，清理时直接删除。

## 同步范围

| 实体 | 方向 | H3Yun Schema | 状态 |
|------|------|-------------|------|
| 产品信息 | ABT → H3Yun | `D000119Product_sale` | 第一期实现 |
| 库存记录 | ABT → H3Yun | `D000119warehouse` | 第一期实现 |
| BOM 数据 | ABT → H3Yun | `D000119BomNodes` | 暂不实现 |

## 架构：独立 Sync 模块

在 `abt` crate 内新增 `h3yun` 模块，遵循现有分层模式：

```
abt/src/
  h3yun/
    mod.rs              # 模块入口，re-export
    client.rs           # H3Yun REST API 客户端
    models.rs           # 请求/响应数据结构 + SyncError 分类
    sync_state.rs       # 映射表读写（h3yun_sync_state）
    product_sync.rs     # 产品同步逻辑（含删除）
    inventory_sync.rs   # 库存同步逻辑
    sync_worker.rs      # channel 消费者，统一同步执行入口

proto/abt/v1/
  sync.proto            # AbtSyncService 定义

abt-grpc/src/handlers/
  sync_handler.rs       # gRPC handler
```

## 数据库表：h3yun_sync_state

新建独立映射表，存储 ABT 实体与 H3Yun 的同步关系。不修改现有表结构，删除同步功能时直接 DROP TABLE。

```sql
CREATE TABLE h3yun_sync_state (
    id              SERIAL PRIMARY KEY,
    entity_type     VARCHAR(32) NOT NULL,  -- 'product' | 'inventory'
    entity_id       UUID NOT NULL,         -- ABT 中的 product_id / inventory_id
    h3yun_object_id VARCHAR(64),           -- H3Yun 返回的 ObjectId（首次同步后填充）
    last_synced_at  TIMESTAMPTZ,           -- 上次成功同步时间
    content_hash    VARCHAR(64),           -- 上次同步的内容哈希（用于去重）
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(entity_type, entity_id)
);
```

**用途：**
- **消除存在性查询**：有 `h3yun_object_id` → 直接 UpdateBizObject；无 → CreateBizObject 并回填 ObjectId。将 API 调用从 2N 降至 N。
- **持久化水印**：`last_synced_at` 跨重启保留，定时任务增量查询 `WHERE last_synced_at IS NULL OR last_synced_at < updated_at`。
- **清理便利**：同步功能删除时 `DROP TABLE h3yun_sync_state`，零残留。

## H3Yun API Client

**端点**: `https://www.h3yun.com/OpenApi/Invoke`

**认证**: 请求头带 `EngineCode` + `EngineSecret`，从环境变量读取：
- `H3YUN_ENGINE_CODE`（默认: `wkcmav3emlzu0l1smysmopu85`）
- `H3YUN_ENGINE_SECRET`（默认: `PO+ZqVdtElYtTteED8z0wPUs5QBP/3WoXzGj4PEYYyKl0riiEhB8Rw==`）

**方法**:

| 方法 | H3Yun Action | 用途 |
|------|-------------|------|
| `query_list` | `LoadBizObjects` | 查询对象列表（对账用） |
| `create` | `CreateBizObject` | 创建新对象 |
| `update` | `UpdateBizObject` | 更新已有对象 |
| `delete` | `RemoveBizObject` | 删除对象 |

**请求结构**:
```json
{
  "ActionName": "LoadBizObjects",
  "SchemaCode": "D000119Product_sale",
  "BizObject": "{...}",
  "IsSubmit": true
}
```

## 产品同步

### 流程

1. 从 ABT 查询产品（ProductService）
2. 查询 `h3yun_sync_state` 获取映射：
   - 有 `h3yun_object_id` → `UpdateBizObject`
   - 无映射 → `CreateBizObject`，成功后写入映射表
3. 同步完成后更新 `last_synced_at` 和 `content_hash`

### 删除流程

当 ABT 产品被删除时：
1. 从 `h3yun_sync_state` 查询该产品的 `h3yun_object_id`
2. 有 ObjectId → 调用 H3Yun `RemoveBizObject`，然后删除映射行
3. 无映射 → 跳过（未同步过的产品）
4. H3Yun 删除失败 → warn 日志，不阻塞 ABT 删除流程

### 字段映射

| ABT 字段 | H3Yun 字段 | 说明 |
|----------|-----------|------|
| `product_code` | `Procode` | 产品编码 |
| `pdt_name` | `Proname` | 产品名称 |
| `meta.specification` | `Prospec` | 规格 |
| `unit` | `Unit` | 单位 |
| `meta.acquire_channel` | `huoqu` | 获取渠道 |
| 大分类 | `Pgroup` | 从 terms 读取一级分类 |
| 中分类 | `PgroupM` | 从 terms 读取二级分类 |
| 小分类 | `PgroupS` | 从 terms 读取三级分类 |
| 固定值 | `Fa5124b...` | 交付方式 = "系统倒冲" |

## 库存同步

### 流程

1. 查询 ABT inventory 记录（关联 location + warehouse + product）
2. 查询 `h3yun_sync_state` 获取映射：
   - 有 `h3yun_object_id` → `UpdateBizObject`
   - 无映射 → `CreateBizObject`，成功后写入映射表
3. 同步完成后更新 `last_synced_at`

### 字段映射

| ABT 字段 | H3Yun 字段 | 说明 |
|----------|-----------|------|
| `location.location_code` | `KW20201118` | 库位编码 |
| `warehouse.warehouse_name` | `WH20201118` | 仓库名称 |
| `product.product_code` | `Pcode20201118` | 产品编码 |
| `product.product_code` | `Name` | 记录名称 |
| `product.pdt_name` | `pname` | 产品名称 |
| 固定值"期初导入" | `Size` | 导入方式 |
| `inventory.quantity` | `stockqty` | 库存数量 |
| `product.unit` | `unit` | 单位 |

## 触发机制：统一事件管道

三种触发方式统一通过 `tokio::sync::mpsc` channel 合并为单一执行路径：

### SyncEvent 定义

```rust
struct SyncEvent {
    entity_type: EntityType,  // Product | Inventory
    entity_id: Uuid,
    priority: Priority,       // High | Normal | Low
}
```

### 触发源

| 触发方式 | Priority | 说明 |
|---------|----------|------|
| 实时（CRUD 变更） | High | 产品创建/更新/删除时发送事件到 channel |
| 手动 gRPC | Normal | `SyncProduct` / `SyncAllProducts` 发送事件 |
| 定时任务（5分钟） | Low | 批量扫描 `h3yun_sync_state` 中需要同步的实体 |

### Channel 消费者（sync_worker）

单个 worker task 消费 channel：
- 去重：相同 `(entity_type, entity_id)` 只保留最新事件
- 按优先级排序执行（High 优先）
- 每条记录独立错误隔离，单条失败不阻塞后续记录
- H3Yun 宕机时 channel 提供背压，ABT CRUD 响应不受影响（~5ms）

### 定时任务增量逻辑

利用已有的 `ScheduledTaskService` 注册定时任务：
- 查询需要同步的实体：`WHERE last_synced_at IS NULL OR last_synced_at < entity.updated_at`
- 批量发送 Low priority SyncEvent 到 channel
- 同步产品后自动发送关联库存的同步事件

### 手动触发 gRPC

```protobuf
service AbtSyncService {
  // 同步单个产品（含库存）
  rpc SyncProduct(SyncProductRequest) returns (SyncResponse);
  // 全量同步所有产品和库存
  rpc SyncAllProducts(SyncAllRequest) returns (SyncResponse);
  // 同步指定产品的库存
  rpc SyncInventory(SyncInventoryRequest) returns (SyncResponse);
  // 对账：比较 ABT 与 H3Yun 状态差异
  rpc Reconcile(ReconcileRequest) returns (ReconcileResponse);
}
```

## 错误处理：逐记录隔离 + SyncError 分类

每条记录的同步包裹在独立错误边界中，不使用 `?` 传播跨记录错误。

### SyncError 分类

```rust
enum SyncError {
    Transient { backoff_hint: Duration },
    ValidationError { record_id: String, fields: Vec<String> },
    FatalError { reason: String },
}
```

| 错误类型 | 示例 | 处理策略 |
|---------|------|---------|
| `Transient` | 网络超时、429 rate limit | 自动重试（带退避），最多 3 次 |
| `ValidationError` | 字段格式错误、必填字段缺失 | 记录日志，跳过该记录，继续后续 |
| `FatalError` | 认证失败、schema 不匹配 | 中止当前批次，上报 FatalError |

### 结果上报

通过 `TaskRunResult { processed, succeeded, message }` 上报同步结果，`message` 中包含失败记录的详细信息。

### 与现有错误约定对齐

- H3Yun HTTP 连接/超时错误 → 基础设施错误（`err_to_status()`）
- 单条记录被 H3Yun 拒绝 → 业务错误（不记日志，结构化返回）
- 凭证缺失/无效 → FatalError，启动时 fail-closed（`.expect()` 而非静默继续）

## 对账：读回验证

定期从 H3Yun 读取所有记录，与 ABT 本地状态对比，检测漂移。

### 流程

1. 调用 H3Yun `LoadBizObjects` 获取远程所有记录（使用读取 API，不消耗写入配额）
2. 对每个 H3Yun 记录，查 `h3yun_sync_state` 找到对应 ABT 实体
3. 比较关键字段，标记差异：
   - ABT 存在但 H3Yun 缺失 → "同步丢失"
   - H3Yun 存在但 ABT 已删除（映射表无对应） → "幽灵记录"
   - 字段不一致 → "数据漂移"
4. 结果通过 gRPC `Reconcile` RPC 返回，不自动修复

### 触发

通过 gRPC `Reconcile` 手动触发，或作为低频定时任务（每小时/每天）运行。

## 工厂函数

在 `abt/src/lib.rs` 新增：
- `get_h3yun_client()` — 获取 H3Yun API 客户端实例（OnceLock，启动时 fail-closed）
- `get_sync_event_sender()` — 获取 SyncEvent channel 的 sender（用于各触发源发送事件）

## 注册

在 `abt-grpc/src/server.rs` 注册 `AbtSyncService` handler。

在 `abt-grpc/src/server.rs` 启动时 spawn sync_worker task 消费 channel。

在产品/库存的 CRUD service 中注入 `SyncEvent` sender，变更时发送事件。
