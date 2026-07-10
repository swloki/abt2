-- 098: BOM 内联工序重构（推翻 clean break 覆盖层方案）
--
-- 设计文档：docs/uml-design/bom-operation-inline.md
--   - bom_operations：per-BOM-per-step 自洽工序行（工艺+产出+WC），copy-on-write 与 routing 模板解耦
--   - bom_step_prices：per-BOM-per-step 计件单价（价归 IE/成本域，与工艺分表）
--   - bom_step_price_history：单价变更审计（R-15，月度审计 + diff 幅度溯源）
--
-- 数据流（经 2026-07-08 现网审计确认）：
--   - bom_routing_outputs（覆盖层）：3121 行全有价，404 pcodes —— unit_price 真相源，但无 quantity 列
--   - bom_labor_processes（legacy）：6994 行，37% quantity≠1（含 quantity=0「不计件」+ 倍数），534 pcodes —— quantity 唯一来源
--   - (product_code, process_code) 在 bom_labor_processes 完全唯一（0 重复）→ process_code JOIN 安全（R-14）
--   - bro↔blp 经 routing_steps.process_code JOIN 匹配率 99.3%（3100/3121，21 行 unmatched → quantity 默认 1）
--   - 129 个 product_code「价只在 legacy」（bro 无价但 blp 有价）→ 第二步补价
--
-- 幂等：CREATE TABLE IF NOT EXISTS + ON CONFLICT DO NOTHING，可重复执行。
-- 本仓无 migration runner（手动 psql -f，见 memory reference-abt-migration-manual）。

BEGIN;

-- ============================================================
-- M1: 建表
-- ============================================================

-- M1.1 bom_operations —— BOM 内联工序（per-BOM-per-step 自洽行）
CREATE TABLE IF NOT EXISTS bom_operations (
    id                  BIGSERIAL     PRIMARY KEY,
    product_code        VARCHAR(100)  NOT NULL,                  -- 成品编码（与 bom_routings/bom_labor_processes 对齐）
    step_order          INT           NOT NULL,                  -- BOM 内工序序号（BOM 自主，不再对齐 routing_steps）
    process_code        VARCHAR(100)  NOT NULL,                  -- 工序编码（→ labor_process_dicts.code）
    process_name        VARCHAR(200)  NOT NULL,                  -- 工序名（拷贝时 COALESCE(lpd.name, process_code) 物化落库；copy-on-write 后字典改名不自动同步）
    work_center_id      BIGINT,                                  -- 内联工作中心（权威 FK；bom_nodes.work_center VARCHAR 是遗留 free-text）
    standard_time       DECIMAL(18,6),                           -- 标准工时(分钟)
    standard_cost       DECIMAL(18,6),                           -- 标准成本(每小时)
    allowed_loss_rate   DECIMAL(18,6) NOT NULL DEFAULT 0,
    is_outsourced       BOOLEAN       NOT NULL DEFAULT false,
    is_inspection_point BOOLEAN       NOT NULL DEFAULT false,    -- 免检不免工序
    is_required         BOOLEAN       NOT NULL DEFAULT true,
    output_product_id   BIGINT        REFERENCES products(product_id), -- 该工序产出的中间品（须 ∈ 该 product_code 下 BOM 非叶子节点，handler 层校验）
    remark              TEXT,
    source_routing_id   BIGINT        REFERENCES routings(id),   -- 拷贝来源 routing（纯溯源；改 routing 不回流影响本行）；手工建为 NULL
    operator_id         BIGINT,
    created_at          TIMESTAMPTZ   NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ,
    UNIQUE (product_code, step_order)                             -- 一个 BOM 一道工序一行
);
CREATE INDEX IF NOT EXISTS idx_bom_operations_product    ON bom_operations(product_code);
CREATE INDEX IF NOT EXISTS idx_bom_operations_output     ON bom_operations(output_product_id) WHERE output_product_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_bom_operations_source_rt  ON bom_operations(source_routing_id) WHERE source_routing_id IS NOT NULL;
COMMENT ON TABLE bom_operations IS 'BOM 内联工序：per-BOM-per-step 自洽工序行（工艺+产出+WC），copy-on-write 与 routing 模板解耦';

-- M1.2 bom_step_prices —— per-BOM-per-step 计件单价（含 quantity，R-1）
-- quantity：单件产品在该工序的计件倍数（legacy bom_labor_processes.quantity 语义，37%≠1）。
--   - BOM 成本报告：单件人工成本 = unit_price × quantity（try_build_labor_from_bom 用 quantity，非 ONE）
--   - 报工 wage_amount：completed_qty × unit_price（production_batch:224 公式不变，quantity 是成本维度非报工维度）
CREATE TABLE IF NOT EXISTS bom_step_prices (
    id            BIGSERIAL     PRIMARY KEY,
    product_code  VARCHAR(100)  NOT NULL,
    step_order    INT           NOT NULL,
    unit_price    NUMERIC(18,6),                          -- 该 BOM 该工序计件单价（空 = 未定价，待工单现场填后回写）
    quantity      NUMERIC(18,6) NOT NULL DEFAULT 1,       -- R-1：单件计件倍数（legacy 语义，影响成本报告；0 = 不计件）
    operator_id   BIGINT,
    created_at    TIMESTAMPTZ   NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ,
    UNIQUE (product_code, step_order)
);
CREATE INDEX IF NOT EXISTS idx_bom_step_prices_product ON bom_step_prices(product_code);
COMMENT ON TABLE bom_step_prices IS 'per-BOM-per-step 计件单价：工单下达时首次填后保存，后续同 BOM 工单自动加载。报工 wage_amount 仍冻结到 work_reports（migration 062 语义不变）';

-- M1.3 bom_step_price_history —— 单价变更审计（R-15，月度审计 + diff 幅度溯源）
CREATE TABLE IF NOT EXISTS bom_step_price_history (
    id            BIGSERIAL     PRIMARY KEY,
    product_code  VARCHAR(100)  NOT NULL,
    step_order    INT           NOT NULL,
    old_price     NUMERIC(18,6),
    new_price     NUMERIC(18,6),
    quantity      NUMERIC(18,6) NOT NULL DEFAULT 1,
    source_type   VARCHAR(50)   NOT NULL,                  -- 'work_order_release' / 'bom_editor' / 'migration'
    source_wo_id  BIGINT,                                  -- 工单填价时记录 wo_id；BOM 编辑器/migration 为 NULL
    operator_id   BIGINT,
    created_at    TIMESTAMPTZ   NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_bom_step_price_history_product ON bom_step_price_history(product_code);
CREATE INDEX IF NOT EXISTS idx_bom_step_price_history_created ON bom_step_price_history(created_at);
COMMENT ON TABLE bom_step_price_history IS '计件单价变更历史：upsert_price 追加一行，支持月度审计报告 + diff 幅度溯源（R-15）';

-- M1.4 BOM_STEP_PRICE 权限种子（R-13：定价影响全员工资，独立闸门）
-- 为 admin 角色（role_code='admin'）授予全部 BOM_STEP_PRICE 权限。
-- super_admin 走 is_super_admin bypass，无需 seed（见 058 模式）。
INSERT INTO role_permissions (role_id, resource_code, action)
SELECT r.role_id, 'BOM_STEP_PRICE', actions.action
FROM roles r
CROSS JOIN (VALUES ('create'), ('read'), ('update'), ('delete')) AS actions(action)
WHERE r.role_code = 'admin'
ON CONFLICT (role_id, resource_code, action) DO NOTHING;

-- ============================================================
-- M2(a): 回填 bom_operations
--   工艺属性从 routing_steps 模板；产出/WC 从 bom_routing_outputs 覆盖层（COALESCE 覆盖优先回退模板）
--   source_routing_id = bom_routings.routing_id（拷贝来源）
-- ============================================================
INSERT INTO bom_operations (
    product_code, step_order, process_code, process_name,
    work_center_id, standard_time, standard_cost, allowed_loss_rate,
    is_outsourced, is_inspection_point, is_required,
    output_product_id, source_routing_id, remark, created_at
)
SELECT
    br.product_code, rs.step_order, rs.process_code,
    COALESCE(lpd.name, rs.process_code),
    COALESCE(bro.work_center_id, rs.work_center_id),    -- 覆盖层优先，回退模板
    rs.standard_time, rs.standard_cost, COALESCE(rs.allowed_loss_rate, 0),
    rs.is_outsourced, rs.is_inspection_point, rs.is_required,
    bro.output_product_id,                               -- 产出仅来自覆盖层（模板 097 已 DROP）
    br.routing_id,                                       -- source_routing_id = 拷贝来源
    rs.remark, now()
FROM bom_routings br
JOIN routing_steps rs ON rs.routing_id = br.routing_id
LEFT JOIN labor_process_dicts lpd ON lpd.code = rs.process_code AND lpd.deleted_at IS NULL
LEFT JOIN bom_routing_outputs bro ON bro.product_code = br.product_code AND bro.step_order = rs.step_order
ON CONFLICT (product_code, step_order) DO NOTHING;
-- 注：INSERT...SELECT 的 JOIN ON 可引用任意表（无 UPDATE...FROM 目标表坑，对比 096:38-39）

-- ============================================================
-- M2(b): 回填 bom_step_prices（含 quantity，R-1 + R-14）
-- ============================================================

-- M2(b)-1：从 bom_routing_outputs（覆盖层 unit_price 真相源，3121 行）
--          quantity 从 bom_labor_processes 按 process_code 对齐补（99.3% 匹配；unmatched 默认 1）
INSERT INTO bom_step_prices (product_code, step_order, unit_price, quantity, operator_id, created_at)
SELECT
    bro.product_code, bro.step_order, bro.unit_price,
    COALESCE(blp.quantity, 1),                           -- R-1/R-14：从 legacy 按 process_code 对齐补 quantity
    bro.operator_id, now()
FROM bom_routing_outputs bro
JOIN bom_routings br ON br.product_code = bro.product_code
JOIN routing_steps rs ON rs.routing_id = br.routing_id AND rs.step_order = bro.step_order
LEFT JOIN bom_labor_processes blp
       ON blp.product_code = bro.product_code
      AND blp.process_code = rs.process_code
      AND blp.deleted_at IS NULL
WHERE bro.unit_price IS NOT NULL
ON CONFLICT (product_code, step_order) DO NOTHING;

-- M2(b)-2：补「价只在 legacy」129 pcodes（bro 无价但 blp 有价，§7.4）
--          step_order 对齐 routing_steps（与 bom_operations 一致）
INSERT INTO bom_step_prices (product_code, step_order, unit_price, quantity, operator_id, created_at)
SELECT
    br.product_code, rs.step_order, blp.unit_price, blp.quantity, NULL, now()
FROM bom_routings br
JOIN routing_steps rs ON rs.routing_id = br.routing_id
JOIN bom_labor_processes blp
       ON blp.product_code = br.product_code
      AND blp.process_code = rs.process_code
      AND blp.deleted_at IS NULL
      AND blp.unit_price IS NOT NULL
      AND blp.unit_price > 0
LEFT JOIN bom_routing_outputs bro
       ON bro.product_code = br.product_code
      AND bro.step_order = rs.step_order
      AND bro.unit_price IS NOT NULL
WHERE bro.product_code IS NULL                           -- 仅覆盖层无价的（避免与 M2(b)-1 冲突）
ON CONFLICT (product_code, step_order) DO NOTHING;

-- 记录迁移基线到 history（source_type='migration'，便于审计溯源）
INSERT INTO bom_step_price_history (product_code, step_order, old_price, new_price, quantity, source_type, operator_id, created_at)
SELECT product_code, step_order, NULL, unit_price, quantity, 'migration', operator_id, now()
FROM bom_step_prices
ON CONFLICT DO NOTHING;

COMMIT;

-- ============================================================
-- M3: 门禁校验（部署前人工核对，不等则排查后重跑 M2）
--   本仓无 migration runner，M3 为 SELECT 审计输出（非 RAISE EXCEPTION 阻断）
-- ============================================================
-- 校验 1：bom_operations 行数应 = 绑定 BOM × routing_steps 行数
SELECT
    (SELECT COUNT(*) FROM bom_routings br JOIN routing_steps rs ON rs.routing_id = br.routing_id) AS expected_ops,
    (SELECT COUNT(*) FROM bom_operations) AS actual_ops;
-- expected_ops 与 actual_ops 应相等；不等则人工排查后重跑 M2(a)

-- 校验 2：bom_step_prices 行数应 ≥ bom_routing_outputs 有价行数（3121）+ legacy 补价
SELECT
    (SELECT COUNT(*) FROM bom_routing_outputs WHERE unit_price IS NOT NULL) AS bro_priced,
    (SELECT COUNT(*) FROM bom_step_prices) AS actual_prices;
-- actual_prices 应 ≥ bro_priced（3121）；含 legacy 补价应更多

-- 校验 3：quantity 审计（R-1）—— 确认 quantity 维度已迁移
SELECT
    COUNT(*) AS total,
    COUNT(*) FILTER (WHERE quantity <> 1) AS qty_ne1,
    ROUND(100.0 * COUNT(*) FILTER (WHERE quantity <> 1) / NULLIF(COUNT(*), 0), 1) AS pct_ne1
FROM bom_step_prices;
-- qty_ne1 应接近现网 bom_labor_processes 的 37% 量级（迁移后含 bro 行 quantity 默认 1，比例会下降）

-- 校验 4：bom_operations 行数 ≥ bom_routing_outputs 行数（含孤儿，见设计 §7.2）
SELECT
    (SELECT COUNT(*) FROM bom_routing_outputs) AS bro_total,
    (SELECT COUNT(*) FROM bom_operations) AS bo_total;
-- bo_total 应 ≥ bro_total；若 bo_total < bro_total 说明有孤儿 bro 行（routing_step 被删但覆盖行残留）
