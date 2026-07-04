/**
 * WMS 测试常量与隔离 helper。
 *
 * 常量抄自 abt-web/tests/wms_flow_e2e.rs:21-30（dev DB 硬编码测试数据）。
 * 连共享 dev DB（abt_v2），不回滚 → 靠强唯一标识 + 增量断言隔离，禁绝对值断言。
 */
export const PRODUCT_ID = 565;
export const PRODUCT_ID_ALT = 566;

export const WAREHOUSE_A = 23320; // 备料周转仓
export const BIN_A = 23320000;

export const WAREHOUSE_B = 23327; // 原材料仓
export const BIN_B = 23361;

export const CUSTOMER_ID = 135;
export const SUPPLIER_ID = 129;

/**
 * 临时实体唯一标识，避免共享 dev DB 状态耦合。
 * 范本：abt-web/tests/wms_flow_e2e.rs 用 `T-OCC-{nanos}`。
 */
export function uniqueTag(prefix = 'T-PLAY'): string {
  return `${prefix}-${Date.now()}`;
}

/**
 * 每次新 UUID，防 idempotency 协议吞掉重复提交。
 * 入库/发货等带 idempotency_key 的写操作 spec 必须每次生成新 key
 * （重复 key 第二次会被幂等返回空 200，造成跨 run 假绿）。
 */
export function genIdempotencyKey(): string {
  return crypto.randomUUID();
}
