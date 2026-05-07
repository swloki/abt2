# Product Watchlist + Notification Center Design

## Overview

用户可以关注产品（个人级别 CRUD），后台 Worker 每 5 分钟扫描关注列表，检测库存低于安全线时推送告警到通用通知中心。用户可为单个产品设置独立的告警阈值（safety_stock_override）。通知中心设计为通用基础设施，`stock_alert` 是第一种通知类型，后续工作流审批、系统公告等可复用同一张 `notifications` 表。

## Requirements

- R1. 关注 CRUD：用户关注/取消关注产品，查询自己的关注列表
- R2. 关注列表带实时库存：查询时 JOIN inventory，展示当前库存、安全库存、告警状态
- R3. 自定义阈值：用户可为单个关注产品设置 safety_stock_override，覆盖默认 safety_stock
- R4. 通用通知中心：通知表支持多种 type（stock_alert / system / 后续 approval 等）
- R5. 通知 CRUD：分页查询（支持 type/is_read/时间范围 filter）、标记已读、全部已读、未读计数（按 type 分组）
- R6. 告警 Worker：每 5 分钟扫描，库存低于阈值时创建通知，含 metadata（当前库存、阈值、产品名）
- R7. 告警去重：库存曾回升到安全线以上再次低于阈值时才重新告警，防止频繁抖动

## Data Model

### product_watchers

```sql
CREATE TABLE product_watchers (
    user_id BIGINT NOT NULL,
    product_id BIGINT NOT NULL,
    safety_stock_override BIGINT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, product_id)
);
CREATE INDEX IF NOT EXISTS idx_product_watchers_product ON product_watchers(product_id);
```

- 联合主键 (user_id, product_id)，无独立 ID
- safety_stock_override：用户自定义告警阈值，NULL 表示使用 inventory.safety_stock
- 不加外键（遵循代码库约定）
- product_id 索引用于 Worker 按 product 批量扫描

### notifications

```sql
CREATE TABLE notifications (
    notification_id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL,
    type VARCHAR(32) NOT NULL DEFAULT 'system',
    title VARCHAR(256) NOT NULL,
    content TEXT,
    related_type VARCHAR(64),
    related_id BIGINT,
    is_read BOOLEAN NOT NULL DEFAULT false,
    read_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    metadata JSONB NULL
);
CREATE INDEX IF NOT EXISTS idx_notifications_user_unread
    ON notifications(user_id, is_read, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_notifications_user_created
    ON notifications(user_id, created_at DESC);
```

- type 字段区分通知类型：`stock_alert`、`system`、后续可扩展 `approval` 等
- related_type + related_id 可选关联实体（如 related_type='product', related_id=123）
- read_at 记录具体已读时间
- metadata JSONB 存放额外数据，如 `{"current_quantity": 12, "safety_stock": 50, "product_name": "XXX"}`，前端展示告警时无需额外查询
- 两个索引分别支持"未读列表"和"时间排序"查询

## API

### 关注列表（追加到 product.proto 的 AbtProductService）

```protobuf
rpc WatchProduct(WatchProductRequest) returns (WatchProductResponse);
rpc UnwatchProduct(UnwatchProductRequest) returns (google.protobuf.Empty);
rpc ListWatchedProducts(ListWatchedProductsRequest) returns (ListWatchedProductsResponse);

message WatchProductResponse {
    bool is_new = 1;  // true=新增关注, false=已关注过(更新了override)
}

message WatchedProduct {
    int64 product_id = 1;
    string product_code = 2;
    string product_name = 3;
    string current_quantity = 4;      // Decimal → string
    string effective_safety_stock = 5; // COALESCE(override, safety_stock)
    bool is_alerting = 6;             // current_quantity < effective_safety_stock
}
```

- WatchProduct — 关注，支持可选 safety_stock_override；INSERT ON CONFLICT DO UPDATE，返回 is_new
- UnwatchProduct — 取消关注，幂等（DELETE WHERE，无匹配不报错）
- ListWatchedProducts — 分页，返回 WatchedProduct 列表（含实时库存和告警状态）

### 通知中心（新 notification.proto）

```protobuf
service AbtNotificationService {
    rpc ListNotifications(ListNotificationsRequest) returns (ListNotificationsResponse);
    rpc MarkAsRead(MarkAsReadRequest) returns (google.protobuf.Empty);
    rpc MarkAllAsRead(MarkAllAsReadRequest) returns (google.protobuf.Empty);
    rpc GetUnreadCount(GetUnreadCountRequest) returns (GetUnreadCountResponse);
}

message GetUnreadCountResponse {
    int64 total = 1;
    map<string, int64> by_type = 2;  // {"stock_alert": 3, "system": 1}
}
```

- ListNotifications — 分页，支持 filter by type、is_read、start_time、end_time
- MarkAsRead — 单条标记已读，同时设置 read_at
- MarkAllAsRead — 全部标记已读，支持可选 type 参数（只标记某类通知已读）
- GetUnreadCount — 返回总数 + 按 type 分组数量

### 权限

- 所有操作校验 user_id = 当前登录用户，不能操作别人的数据
- 无需额外权限配置

## Alert Worker

### 扫描逻辑（每 5 分钟）

1. 查询被关注的低库存产品：
```sql
SELECT DISTINCT pw.product_id
FROM product_watchers pw
JOIN inventory i ON i.product_id = pw.product_id
WHERE i.quantity < COALESCE(pw.safety_stock_override, i.safety_stock)
```
2. 对每个低库存产品，找出其关注者
3. 去重检查（回升再跌策略）：查询同一用户+同一产品的最近一条 stock_alert，检查自该告警后库存是否曾回升到安全线以上
4. 为需要通知的关注者写入 notifications（type='stock_alert'），metadata 含 current_quantity、safety_stock、product_name

### 去重策略（回升再跌）

防止抖动：同一用户+同一产品，只有当库存曾回升到安全线以上再次低于阈值时才创建新告警。实现方式：
- 在 product_watchers 中增加内存标记（Worker 内部 HashMap），记录产品是否曾回升
- 每次扫描时：如果当前库存 >= 阈值，标记"已回升"；如果库存 < 阈值 且 有"已回升"标记，创建告警并清除标记
- 无"已回升"标记的低库存产品，检查是否已有未读 stock_alert，有则跳过

### Worker Metrics

每次循环结束记录日志：扫描产品数、低库存数、发送告警数、耗时。

### 实现

- `abt/src/implt/stock_alert_worker.rs`
- `tokio_util::sync::CancellationToken` 控制生命周期
- 扫描间隔：环境变量 `STOCK_ALERT_SCAN_INTERVAL_SECS`，默认 300
- 在 `server.rs` 中 `tokio::spawn` 启动

## File Structure

```
proto/abt/v1/notification.proto                    ← 通知 gRPC 服务
abt/migrations/039_create_notifications.sql         ← notifications 表
abt/migrations/040_create_product_watchers.sql      ← product_watchers 表
abt/src/models/notification.rs                      ← Notification 模型
abt/src/repositories/notification_repo.rs            ← 通知 CRUD + 未读计数
abt/src/repositories/product_watcher_repo.rs         ← 关注 CRUD
abt/src/service/notification_service.rs              ← NotificationService trait
abt/src/service/product_watcher_service.rs           ← ProductWatcherService trait
abt/src/implt/notification_service_impl.rs           ← 通知服务实现
abt/src/implt/product_watcher_service_impl.rs        ← 关注服务实现
abt/src/implt/stock_alert_worker.rs                  ← 告警 Worker
abt-grpc/src/handlers/notification.rs                ← 通知 gRPC handler
```

关注相关 3 个 RPC（Watch/Unwatch/ListWatched）追加到 `product.proto` 和 `ProductHandler`。

## Scope Boundaries

### In Scope

- 关注产品 CRUD（含 safety_stock_override）
- 通用通知中心 CRUD
- 库存告警 Worker（回升再跌去重）

### Out of Scope (V2)

- 通知推送（邮件/消息）— 后续集成外部通知服务
- 通知偏好设置（用户选择接收哪些类型）
- 批量关注/取消关注 — 前端逐个调用
- 具体业务 Action 的通知（如工作流审批通知）— 各业务模块独立实现
- 扫描分片 / Redis 缓存 — 关注量小时不需要
