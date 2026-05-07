ALTER TABLE product_watchers
  ADD COLUMN IF NOT EXISTS alert_active BOOLEAN NOT NULL DEFAULT false,
  ADD COLUMN IF NOT EXISTS last_notified_at TIMESTAMPTZ;

COMMENT ON COLUMN product_watchers.alert_active IS '当前是否处于活跃告警状态（库存低于阈值且已发送通知）';
COMMENT ON COLUMN product_watchers.last_notified_at IS '上次发送库存告警通知的时间';
