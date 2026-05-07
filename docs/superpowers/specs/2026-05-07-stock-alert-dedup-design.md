# 库存告警去重策略设计

## 问题

StockAlertTask 每 5 分钟扫描一次低库存产品。当前去重逻辑只检查未读通知 (`is_read = false`)，用户标记已读后下次扫描会立即重新创建通知。实际补货周期数天，反复通知造成干扰。

## 方案

在 `product_watchers` 表加两个字段，用状态机跟踪告警生命周期。

### 数据库变更

```sql
ALTER TABLE product_watchers
  ADD COLUMN alert_active BOOLEAN NOT NULL DEFAULT false,
  ADD COLUMN last_notified_at TIMESTAMPTZ;
```

### 状态机

每个 (user_id, product_id) 独立跟踪：

```
正常 → 库存低于阈值 → 发通知 → alert_active=true
                              ↓
         用户标记已读（不影响 alert_active）
                              ↓
         库存回升到安全线以上 → alert_active=false
                              ↓
         库存再次低于阈值 → 发通知 → alert_active=true
```

**核心规则：`alert_active = true` 时跳过，不管已读未读。**

### 去重逻辑

扫描流程（批量，4 次查询）：

1. **查低库存产品** — 现有 `find_watched_low_stock_products`
2. **批量查关注者** — 现有 `find_watchers_by_products`
3. **批量查 alert_active 状态** — 查 `product_watchers` 中所有相关 watcher 的 `alert_active`
4. **决策 + 批量更新：**
   - `alert_active = false` 且库存低 → 发通知，设 `alert_active = true, last_notified_at = now()`
   - `alert_active = true` 且产品不在低库存集合 → 已回升，设 `alert_active = false`
   - `alert_active = true` 且产品仍低库存 → 跳过

### Repo 变更

- `ProductWatcherRepo::batch_get_alert_status(product_ids)` — 批量查 alert_active 状态
- `ProductWatcherRepo::batch_set_alert_active(user_id, product_id_pairs, active)` — 批量设置 alert_active
- `NotificationRepo::batch_has_unread_alerts_multi` — 不再需要（去重不再依赖 is_read）

### StockAlertTask 变更

- 移除 `batch_has_unread_alerts_multi` 调用
- 新增 `batch_get_alert_status` 调用
- 用 alert_active 状态判断是否发送
- 发送后批量更新 alert_active + last_notified_at
- 扫描结束后批量重置回升产品的 alert_active

## 不做的事

- 不修改 notifications 表
- 不加独立状态表（用 product_watchers 现有行）
- 不改 gRPC proto 定义
- 不改通知中心的已读/未读逻辑
