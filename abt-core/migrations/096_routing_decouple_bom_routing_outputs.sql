-- 096: 工艺路线解耦 —— 产出品/计件价从 routing_steps 下沉到 per-BOM 覆盖层
-- 设计文档: docs/uml-design/routing-decouple.md
-- 本 migration 含 M1(建表) + M2(回填)。M3(DROP routing_steps.product_id/unit_price 列)
--   在所有代码改读覆盖层后，由后续 migration 执行（grep 确认无下游读后）。
--
-- 背景：045/063 迁移把 unit_price/product_id 复制到 routing_steps，导致每个 BOM 配一条
--   专属 routing。本解耦新增 per-BOM 覆盖层，恢复 routing 为纯工艺模板（可跨产品复用）。

-- M1: per-BOM 工艺产出覆盖层
CREATE TABLE IF NOT EXISTS bom_routing_outputs (
    id                BIGSERIAL    PRIMARY KEY,
    product_code      VARCHAR(100) NOT NULL,                       -- 成品编码（与 bom_routings.product_code 对齐）
    routing_id        BIGINT       NOT NULL REFERENCES routings(id),
    step_order        INT          NOT NULL,                        -- 对齐 routing_steps.step_order
    output_product_id BIGINT       REFERENCES products(product_id), -- 该工序产出的中间品（∈ 该 BOM 非叶子节点）
    unit_price        NUMERIC(18,6),                                -- 该 BOM 该工序计件单价（可空）
    work_center_id    BIGINT,                                       -- 工作中心覆盖（空 → 用模板 routing_steps.work_center_id）
    operator_id       BIGINT,
    created_at        TIMESTAMPTZ  NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ,
    UNIQUE (product_code, step_order)                               -- 一个 BOM 的一道工序最多一个产出映射
);
CREATE INDEX IF NOT EXISTS idx_bom_routing_outputs_routing ON bom_routing_outputs(routing_id);
COMMENT ON TABLE bom_routing_outputs IS 'BOM 工艺产出覆盖：per-BOM-per-step 的产出品+计件价+工作中心覆盖（替代 routing_steps.product_id/unit_price，恢复 routing 可复用）';

-- M2(a): 从模板回填覆盖行 —— 每个绑定的 BOM × 该 routing 的 steps
--   RT000001 同构家族：388 BOM × 8 step；product_id 为空的 step（如 RT000002 空壳）跳过
INSERT INTO bom_routing_outputs (product_code, routing_id, step_order, output_product_id, unit_price, work_center_id, created_at)
SELECT br.product_code, br.routing_id, rs.step_order, rs.product_id, rs.unit_price, rs.work_center_id, now()
FROM bom_routings br
JOIN routing_steps rs ON rs.routing_id = br.routing_id
WHERE rs.product_id IS NOT NULL
ON CONFLICT (product_code, step_order) DO NOTHING;

-- M2(b): 回填历史工单快照 product_id 断裂
--   migration 063（给 routing_steps 加 product_id）前生成的 work_order_routings 快照 product_id 永久 NULL，
--   导致 OM 委外 / 工序级领料读不到产出品（现网 bug）。按 work_orders.routing_id + step_no 回填。
UPDATE work_order_routings wor
SET product_id = rs.product_id
FROM work_orders wo
JOIN routing_steps rs ON rs.routing_id = wo.routing_id AND rs.step_order = wor.step_no
WHERE wor.work_order_id = wo.id
  AND wor.product_id IS NULL
  AND rs.product_id IS NOT NULL;
