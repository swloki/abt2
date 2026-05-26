/**
 * 创建工作流引擎表结构
 *
 * 4 张表：workflow_templates, workflow_instances, workflow_tasks, workflow_history
 *
 * 用法: npx tsx create-workflow-tables.ts
 * 依赖: npm install pg
 */

import pg from "pg";

const { Pool } = pg;

// ==================== 配置 ====================
const DATABASE_URL = "postgres://user_cC5B3h:password_TJWBYK@127.0.0.1:5432/abt2";
// ==============================================

const MIGRATION_SQL = `
-- ============================================================================
-- workflow_templates: 流程模板定义
-- ============================================================================
CREATE TABLE IF NOT EXISTS workflow_templates (
    id BIGSERIAL PRIMARY KEY,
    entity_type VARCHAR(100) NOT NULL,
    name VARCHAR(255) NOT NULL,
    version INT NOT NULL DEFAULT 1,
    status VARCHAR(20) NOT NULL DEFAULT 'draft',
    graph JSONB,
    graph_checksum VARCHAR(64),
    trigger_event VARCHAR(100),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ
);

-- ============================================================================
-- workflow_instances: 流程实例
-- ============================================================================
CREATE TABLE IF NOT EXISTS workflow_instances (
    id BIGSERIAL PRIMARY KEY,
    template_id BIGINT NOT NULL,
    template_version INT,
    entity_type VARCHAR(100) NOT NULL,
    entity_id BIGINT NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'running',
    frozen_graph JSONB,
    context JSONB,
    suspended_reason JSONB,
    initiator_id BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ,
    last_advanced_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ
);

-- ============================================================================
-- workflow_tasks: 任务
-- ============================================================================
CREATE TABLE IF NOT EXISTS workflow_tasks (
    id BIGSERIAL PRIMARY KEY,
    instance_id BIGINT NOT NULL,
    node_id VARCHAR(100) NOT NULL,
    prev_task_id BIGINT,
    assignee_id BIGINT,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    action VARCHAR(20),
    timeout_action VARCHAR(20),
    due_at TIMESTAMPTZ,
    remind_at TIMESTAMPTZ,
    result JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);

-- ============================================================================
-- workflow_history: 审计历史
-- ============================================================================
CREATE TABLE IF NOT EXISTS workflow_history (
    id BIGSERIAL PRIMARY KEY,
    instance_id BIGINT NOT NULL,
    task_id BIGINT,
    node_id VARCHAR(100),
    event_type VARCHAR(50) NOT NULL,
    actor_id BIGINT,
    payload JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ============================================================================
-- 索引
-- ============================================================================
CREATE INDEX IF NOT EXISTS idx_workflow_templates_entity_status
    ON workflow_templates(entity_type, status)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_workflow_tasks_assignee_status
    ON workflow_tasks(assignee_id, status, due_at);

CREATE INDEX IF NOT EXISTS idx_workflow_tasks_instance_node
    ON workflow_tasks(instance_id, node_id, status);

CREATE INDEX IF NOT EXISTS idx_workflow_tasks_pending_due
    ON workflow_tasks(status, due_at)
    WHERE status = 'pending';

CREATE INDEX IF NOT EXISTS idx_workflow_instances_entity
    ON workflow_instances(entity_type, entity_id, status);

CREATE INDEX IF NOT EXISTS idx_workflow_history_instance_time
    ON workflow_history(instance_id, created_at);

CREATE INDEX IF NOT EXISTS idx_workflow_templates_trigger
    ON workflow_templates(trigger_event)
    WHERE status = 'active' AND deleted_at IS NULL;

-- ============================================================================
-- CHECK 约束
-- ============================================================================
ALTER TABLE workflow_instances ADD CONSTRAINT wf_instance_status_check
    CHECK (status IN ('running', 'completed', 'rejected', 'suspended', 'cancelled', 'terminated'));

ALTER TABLE workflow_tasks ADD CONSTRAINT wf_task_status_check
    CHECK (status IN ('pending', 'completed', 'rejected', 'delegated', 'timed_out', 'cancelled'));

ALTER TABLE workflow_templates ADD CONSTRAINT wf_template_status_check
    CHECK (status IN ('draft', 'active', 'archived'));

-- ============================================================================
-- 注释
-- ============================================================================
COMMENT ON TABLE workflow_templates IS '工作流模板';
COMMENT ON TABLE workflow_instances IS '工作流实例';
COMMENT ON TABLE workflow_tasks IS '工作流任务';
COMMENT ON TABLE workflow_history IS '工作流审计历史';
`;

async function main() {
  const pool = new Pool({ connectionString: DATABASE_URL });

  try {
    console.log("开始创建工作流引擎表...");
    await pool.query(MIGRATION_SQL);
    console.log("工作流引擎表创建完成。");

    // 验证
    const { rows } = await pool.query(`
      SELECT tablename FROM pg_tables
      WHERE tablename LIKE 'workflow_%'
      ORDER BY tablename
    `);
    console.log("已创建的表:", rows.map((r) => r.tablename).join(", "));
  } catch (err) {
    console.error("迁移失败:", err);
    process.exit(1);
  } finally {
    await pool.end();
  }
}

main();
