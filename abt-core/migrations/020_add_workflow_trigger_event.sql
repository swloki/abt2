-- 044: 工作流触发器 — 模板表添加 trigger_event 列

ALTER TABLE workflow_templates ADD COLUMN IF NOT EXISTS trigger_event VARCHAR(100);

CREATE INDEX IF NOT EXISTS idx_workflow_templates_trigger
    ON workflow_templates(trigger_event)
    WHERE status = 'active' AND deleted_at IS NULL;
