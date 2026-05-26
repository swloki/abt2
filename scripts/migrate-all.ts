/**
 * 统一数据迁移脚本: abt → abt_v2
 *
 * 将旧 abt 数据库的所有业务数据迁移到 abt_v2。
 * 迁移顺序遵循外键依赖关系。
 *
 * 环境变量：
 *   PGHOST=localhost PGPORT=5432 PGUSER=postgres PGPASSWORD=123456
 *   ABT_DB=abt  ABT_V2_DB=abt_v2
 *
 * 运行：bun run scripts/migrate-all.ts
 */

import pg from "pg";

const HOST = process.env.PGHOST ?? "localhost";
const PORT = parseInt(process.env.PGPORT ?? "5432", 10);
const USER = process.env.PGUSER ?? "postgres";
const PASS = process.env.PGPASSWORD ?? "123456";
const ABT_DB = process.env.ABT_DB ?? "abt";
const ABT_V2_DB = process.env.ABT_V2_DB ?? "abt_v2";

const abtPool = new pg.Pool({ host: HOST, port: PORT, user: USER, password: PASS, database: ABT_DB });
const v2Pool = new pg.Pool({ host: HOST, port: PORT, user: USER, password: PASS, database: ABT_V2_DB });

// ============================================================================
// Helpers
// ============================================================================

const BATCH_SIZE = 500;

async function batchInsert(
  pool: pg.Pool,
  table: string,
  columns: string[],
  rows: any[][],
  conflictCols?: string[],
) {
  if (rows.length === 0) return 0;
  const colList = columns.join(", ");
  const placeholders = columns.map((_, i) => `$${i + 1}`).join(", ");
  const conflict = conflictCols
    ? `ON CONFLICT (${conflictCols.join(", ")}) DO NOTHING`
    : "";
  const sql = `INSERT INTO ${table} (${colList}) VALUES (${placeholders}) ${conflict}`;

  let inserted = 0;
  for (let i = 0; i < rows.length; i += BATCH_SIZE) {
    const batch = rows.slice(i, i + BATCH_SIZE);
    for (const row of batch) {
      try {
        await pool.query(sql, row);
        inserted++;
      } catch (e: any) {
        if (e.code === "23505") continue; // unique violation, skip
        throw e;
      }
    }
  }
  return inserted;
}

async function resetSequence(pool: pg.Pool, table: string, pkCol: string) {
  const seqName = `${table}_${pkCol}_seq`;
  await pool.query(
    `SELECT setval('${seqName}', COALESCE((SELECT MAX(${pkCol}) FROM ${table}), 1))`,
  );
}

// ============================================================================
// Migration
// ============================================================================

async function main() {
  console.log("=== 统一数据迁移: abt → abt_v2 ===\n");

  // ── 0. Identity 数据（用户、角色、部门、权限）──────────────────────
  console.log("── 0. identity (users, roles, departments, permissions) ──");

  // 0a. departments
  {
    const { rows } = await abtPool.query(
      "SELECT department_id, department_name, department_code, description, is_active, is_default, created_at, updated_at FROM departments ORDER BY department_id",
    );
    // 先删除 seed 的默认部门，用旧库的真实数据替代
    await v2Pool.query("DELETE FROM departments WHERE department_code = 'DEFAULT'");
    const data = rows.map((r) => [
      Number(r.department_id),
      r.department_name,
      r.department_code,
      r.description ?? null,
      r.is_active ?? true,
      r.is_default ?? false,
      r.created_at,
      r.updated_at,
    ]);
    const n = await batchInsert(
      v2Pool,
      "departments",
      ["department_id", "department_name", "department_code", "description", "is_active", "is_default", "created_at", "updated_at"],
      data,
      ["department_id"],
    );
    await resetSequence(v2Pool, "departments", "department_id");
    console.log(`  departments: ${n}/${rows.length} 条`);
  }

  // 0b. users（用旧库的 password_hash 替代 seed 的 placeholder）
  {
    const { rows } = await abtPool.query(
      "SELECT user_id, username, password_hash, display_name, is_active, is_super_admin, created_at, updated_at FROM users ORDER BY user_id",
    );
    // 删除 seed 的 placeholder admin
    await v2Pool.query("DELETE FROM users WHERE username = 'admin'");
    const data = rows.map((r) => [
      Number(r.user_id),
      r.username,
      r.password_hash,
      r.display_name ?? null,
      r.is_active ?? true,
      r.is_super_admin ?? false,
      r.created_at,
      r.updated_at,
    ]);
    const n = await batchInsert(
      v2Pool,
      "users",
      ["user_id", "username", "password_hash", "display_name", "is_active", "is_super_admin", "created_at", "updated_at"],
      data,
      ["user_id"],
    );
    await resetSequence(v2Pool, "users", "user_id");
    console.log(`  users: ${n}/${rows.length} 条`);
  }

  // 0c. roles（合并旧库角色和 seed 的系统角色）
  {
    const { rows } = await abtPool.query(
      "SELECT role_id, role_name, role_code, is_system_role, parent_role_id, description, created_at, updated_at FROM roles ORDER BY role_id",
    );
    // 删除 seed 的系统角色，用旧库数据替代
    await v2Pool.query("DELETE FROM roles WHERE role_code IN ('super_admin', 'admin', 'viewer')");
    const data = rows.map((r) => [
      Number(r.role_id),
      r.role_name,
      r.role_code,
      r.is_system_role ?? false,
      r.parent_role_id ? Number(r.parent_role_id) : null,
      r.description ?? null,
      r.created_at,
      r.updated_at,
    ]);
    const n = await batchInsert(
      v2Pool,
      "roles",
      ["role_id", "role_name", "role_code", "is_system_role", "parent_role_id", "description", "created_at", "updated_at"],
      data,
      ["role_id"],
    );
    await resetSequence(v2Pool, "roles", "role_id");
    console.log(`  roles: ${n}/${rows.length} 条`);
  }

  // 0d. user_roles
  {
    const { rows } = await abtPool.query(
      "SELECT user_id, role_id FROM user_roles ORDER BY user_id",
    );
    const data = rows.map((r) => [
      Number(r.user_id),
      Number(r.role_id),
    ]);
    const n = await batchInsert(
      v2Pool,
      "user_roles",
      ["user_id", "role_id"],
      data,
      ["user_id", "role_id"],
    );
    console.log(`  user_roles: ${n}/${rows.length} 条`);
  }

  // 0e. role_permissions（旧库 action_code → abt_v2 action）
  {
    const { rows } = await abtPool.query(
      "SELECT role_id, resource_code, action_code FROM role_permissions ORDER BY role_id",
    );
    const data = rows.map((r) => [
      Number(r.role_id),
      r.resource_code,
      r.action_code,
    ]);
    const n = await batchInsert(
      v2Pool,
      "role_permissions",
      ["role_id", "resource_code", "action"],
      data,
      ["role_id", "resource_code", "action"],
    );
    console.log(`  role_permissions: ${n}/${rows.length} 条`);
  }

  // 0f. user_departments
  {
    const { rows } = await abtPool.query(
      "SELECT user_id, department_id FROM user_departments ORDER BY user_id",
    );
    const data = rows.map((r) => [
      Number(r.user_id),
      Number(r.department_id),
    ]);
    const n = await batchInsert(
      v2Pool,
      "user_departments",
      ["user_id", "department_id"],
      data,
      ["user_id", "department_id"],
    );
    console.log(`  user_departments: ${n}/${rows.length} 条\n`);
  }

  // ── 1. 工序字典 ──────────────────────────────────────────────────
  console.log("── 1. labor_process_dicts ──");
  {
    const { rows } = await abtPool.query(
      "SELECT id, code, name, description, sort_order, created_at, updated_at FROM labor_process_dict ORDER BY id",
    );
    const data = rows.map((r) => [
      Number(r.id),
      r.code,
      r.name,
      r.description ?? null,
      Number(r.sort_order),
      null, // operator_id
      r.created_at,
      r.updated_at,
      null, // deleted_at
    ]);
    const n = await batchInsert(
      v2Pool,
      "labor_process_dicts",
      ["id", "code", "name", "description", "sort_order", "operator_id", "created_at", "updated_at", "deleted_at"],
      data,
      ["id"],
    );
    await resetSequence(v2Pool, "labor_process_dicts", "id");
    console.log(`  ${n}/${rows.length} 条\n`);
  }

  // ── 2. 工艺路线 ──────────────────────────────────────────────────
  console.log("── 2. routings + routing_steps ──");
  {
    const { rows } = await abtPool.query(
      "SELECT id, name, description, created_at, updated_at FROM routing ORDER BY id",
    );
    const data = rows.map((r) => [
      Number(r.id),
      r.name,
      r.description ?? null,
      null, // operator_id
      r.created_at,
      r.updated_at,
      null, // deleted_at
    ]);
    const n = await batchInsert(
      v2Pool,
      "routings",
      ["id", "name", "description", "operator_id", "created_at", "updated_at", "deleted_at"],
      data,
      ["id"],
    );
    await resetSequence(v2Pool, "routings", "id");
    console.log(`  routings: ${n}/${rows.length} 条`);

    // routing_steps
    const { rows: steps } = await abtPool.query(
      "SELECT id, routing_id, process_code, step_order, is_required, remark, created_at FROM routing_step ORDER BY id",
    );
    const stepData = steps.map((r) => [
      Number(r.id),
      Number(r.routing_id),
      r.process_code,
      Number(r.step_order),
      r.is_required,
      r.remark ?? null,
      r.created_at,
    ]);
    const sn = await batchInsert(
      v2Pool,
      "routing_steps",
      ["id", "routing_id", "process_code", "step_order", "is_required", "remark", "created_at"],
      stepData,
      ["id"],
    );
    await resetSequence(v2Pool, "routing_steps", "id");
    console.log(`  routing_steps: ${sn}/${steps.length} 条\n`);
  }

  // ── 3. BOM 分类 ──────────────────────────────────────────────────
  console.log("── 3. bom_categories ──");
  {
    const { rows } = await abtPool.query(
      "SELECT bom_category_id, bom_category_name, created_at FROM bom_category ORDER BY bom_category_id",
    );
    const data = rows.map((r) => [
      Number(r.bom_category_id),
      r.bom_category_name,
      r.created_at,
    ]);
    const n = await batchInsert(
      v2Pool,
      "bom_categories",
      ["bom_category_id", "bom_category_name", "created_at"],
      data,
      ["bom_category_id"],
    );
    await resetSequence(v2Pool, "bom_categories", "bom_category_id");
    console.log(`  ${n}/${rows.length} 条\n`);
  }

  // ── 4. 产品 ──────────────────────────────────────────────────────
  console.log("── 4. products ──");
  {
    const { rows } = await abtPool.query(
      "SELECT product_id, pdt_name, product_code, unit, meta FROM products ORDER BY product_id",
    );
    const data = rows.map((r) => {
      const meta = r.meta ?? { specification: "", acquire_channel: "" };
      return [
        Number(r.product_id),
        r.pdt_name,
        r.product_code,
        r.unit,
        1, // status = Active
        meta.old_code ?? null, // external_code
        null, // owner_department_id
        JSON.stringify(meta),
      ];
    });
    const n = await batchInsert(
      v2Pool,
      "products",
      ["product_id", "pdt_name", "product_code", "unit", "status", "external_code", "owner_department_id", "meta"],
      data,
      ["product_id"],
    );
    await resetSequence(v2Pool, "products", "product_id");
    console.log(`  ${n}/${rows.length} 条\n`);
  }

  // ── 5. 仓库 ──────────────────────────────────────────────────────
  console.log("── 5. warehouses ──");
  const warehouseMapping = new Map<number, number>(); // old_id → new_id (1:1)
  {
    const { rows } = await abtPool.query(
      "SELECT warehouse_id, warehouse_name, warehouse_code, status, created_at, updated_at, deleted_at FROM warehouse ORDER BY warehouse_id",
    );
    const data = rows.map((r) => {
      const id = Number(r.warehouse_id);
      warehouseMapping.set(id, id);
      return [
        id,
        r.warehouse_code,
        r.warehouse_name,
        1, // warehouse_type = General
        r.status === "active" ? 1 : 2, // status
        null, // address
        null, // manager_id
        false, // is_virtual
        "", // remark
        0, // operator_id
        r.created_at,
        r.updated_at ?? r.created_at,
        r.deleted_at,
      ];
    });
    const n = await batchInsert(
      v2Pool,
      "warehouses",
      ["id", "code", "name", "warehouse_type", "status", "address", "manager_id", "is_virtual", "remark", "operator_id", "created_at", "updated_at", "deleted_at"],
      data,
      ["id"],
    );
    await resetSequence(v2Pool, "warehouses", "id");
    console.log(`  ${n}/${rows.length} 条\n`);
  }

  // ── 6. 库位 → zones + 默认 bin ───────────────────────────────────
  console.log("── 6. locations → zones + bins ──");
  const locationMapping = new Map<number, { warehouseId: number; zoneId: number; binId: number }>();
  {
    const { rows } = await abtPool.query(
      "SELECT location_id, warehouse_id, location_code, location_name, capacity, created_at, deleted_at, status FROM location ORDER BY location_id",
    );
    // 先插 zones
    const zoneData = rows.map((r) => {
      const id = Number(r.location_id);
      const whId = Number(r.warehouse_id);
      return [
        id,
        whId,
        r.location_code,
        r.location_name ?? r.location_code,
        1, // zone_type = Storage
        0, // sort_order
        null, // remark
        r.created_at,
        r.updated_at ?? r.created_at,
        r.deleted_at,
      ];
    });
    const zn = await batchInsert(
      v2Pool,
      "zones",
      ["id", "warehouse_id", "code", "name", "zone_type", "sort_order", "remark", "created_at", "updated_at", "deleted_at"],
      zoneData,
      ["id"],
    );
    await resetSequence(v2Pool, "zones", "id");

    // 为每个 zone 创建一个默认 bin（stock_ledger.bin_id NOT NULL）
    const binData = rows.map((r) => {
      const zoneId = Number(r.location_id);
      const binId = zoneId; // 用同一个 ID 作为 bin_id
      locationMapping.set(zoneId, {
        warehouseId: Number(r.warehouse_id),
        zoneId,
        binId,
      });
      return [
        binId,
        zoneId,
        "DEFAULT", // code
        "默认库位",
        null, null, null, // row_no, column_no, layer_no
        null, // capacity_limit
        null, // allowed_product_types
        null, // temperature_req
        1, // status = Active
        r.created_at,
        r.updated_at ?? r.created_at,
        r.deleted_at,
      ];
    });
    const bn = await batchInsert(
      v2Pool,
      "bins",
      ["id", "zone_id", "code", "name", "row_no", "column_no", "layer_no", "capacity_limit", "allowed_product_types", "temperature_req", "status", "created_at", "updated_at", "deleted_at"],
      binData,
      ["id"],
    );
    await resetSequence(v2Pool, "bins", "id");
    console.log(`  zones: ${zn}/${rows.length} 条, bins: ${bn}/${rows.length} 条\n`);
  }

  // ── 7. BOM ───────────────────────────────────────────────────────
  console.log("── 7. boms ──");
  {
    const { rows } = await abtPool.query(
      "SELECT bom_id, bom_name, create_at, update_at, bom_detail, bom_category_id, status, published_at, created_by FROM bom ORDER BY bom_id",
    );
    const data = rows.map((r) => [
      Number(r.bom_id),
      r.bom_name,
      r.create_at,
      r.update_at,
      r.bom_detail ? JSON.stringify(r.bom_detail) : '{"nodes":[]}',
      r.bom_category_id ? Number(r.bom_category_id) : null,
      r.status === "published" ? 2 : 1, // Draft=1, Published=2
      1, // version
      r.published_at,
      r.created_by ? Number(r.created_by) : null,
    ]);
    const n = await batchInsert(
      v2Pool,
      "boms",
      ["bom_id", "bom_name", "create_at", "update_at", "bom_detail", "bom_category_id", "status", "version", "published_at", "created_by"],
      data,
      ["bom_id"],
    );
    await resetSequence(v2Pool, "boms", "bom_id");
    console.log(`  ${n}/${rows.length} 条\n`);
  }

  // ── 8. BOM 节点 ──────────────────────────────────────────────────
  console.log("── 8. bom_nodes ──");
  {
    const { rows } = await abtPool.query(
      "SELECT id, bom_id, product_id, product_code, quantity, parent_id, loss_rate, \"order\", unit, remark, position, work_center, properties FROM bom_nodes ORDER BY id",
    );
    const data = rows.map((r) => [
      Number(r.id),
      Number(r.bom_id),
      Number(r.product_id),
      r.product_code ?? null,
      r.quantity,
      r.parent_id ? Number(r.parent_id) : 0,
      r.loss_rate ?? 0,
      Number(r.order ?? r.order_num ?? 0),
      r.unit ?? null,
      r.remark ?? null,
      r.position ?? null,
      r.work_center ?? null,
      r.properties ?? null,
    ]);
    const n = await batchInsert(
      v2Pool,
      "bom_nodes",
      ["node_id", "bom_id", "product_id", "product_code", "quantity", "parent_id", "loss_rate", "order_num", "unit", "remark", "position", "work_center", "properties"],
      data,
      ["node_id"],
    );
    await resetSequence(v2Pool, "bom_nodes", "node_id");
    console.log(`  ${n}/${rows.length} 条\n`);
  }

  // ── 9. BOM 工艺路线关联 ──────────────────────────────────────────
  console.log("── 9. bom_routings ──");
  {
    const { rows } = await abtPool.query(
      "SELECT id, product_code, routing_id, created_at, updated_at FROM bom_routing ORDER BY id",
    );
    const data = rows.map((r) => [
      Number(r.id),
      r.product_code,
      Number(r.routing_id),
      null, // operator_id
      r.created_at,
      r.updated_at,
    ]);
    const n = await batchInsert(
      v2Pool,
      "bom_routings",
      ["id", "product_code", "routing_id", "operator_id", "created_at", "updated_at"],
      data,
      ["id"],
    );
    await resetSequence(v2Pool, "bom_routings", "id");
    console.log(`  ${n}/${rows.length} 条\n`);
  }

  // ── 10. BOM 劳务工序 ─────────────────────────────────────────────
  console.log("── 10. bom_labor_processes ──");
  {
    // 先构建 process_code → labor_process_dict_id 映射
    const { rows: dicts } = await v2Pool.query("SELECT id, code FROM labor_process_dicts");
    const codeToDictId = new Map<string, number>();
    for (const d of dicts) {
      codeToDictId.set(d.code, Number(d.id));
    }

    const { rows } = await abtPool.query(
      "SELECT id, product_code, name, unit_price, quantity, sort_order, remark, created_at, updated_at, process_code FROM bom_labor_process ORDER BY id",
    );
    let skipped = 0;
    const data: any[][] = [];
    for (const r of rows) {
      const dictId = r.process_code ? codeToDictId.get(r.process_code) : null;
      if (!dictId && r.process_code) {
        skipped++;
      }
      data.push([
        Number(r.id),
        r.product_code,
        dictId ?? 0, // labor_process_dict_id
        r.process_code ?? null,
        r.name,
        r.unit_price,
        r.quantity,
        Number(r.sort_order),
        r.remark ?? null,
        null, // operator_id
        r.created_at,
        r.updated_at,
        null, // deleted_at
      ]);
    }
    const n = await batchInsert(
      v2Pool,
      "bom_labor_processes",
      ["id", "product_code", "labor_process_dict_id", "process_code", "name", "unit_price", "quantity", "sort_order", "remark", "operator_id", "created_at", "updated_at", "deleted_at"],
      data,
      ["id"],
    );
    await resetSequence(v2Pool, "bom_labor_processes", "id");
    console.log(`  ${n}/${rows.length} 条${skipped > 0 ? ` (跳过 ${skipped} 条无法匹配 process_code)` : ""}\n`);
  }

  // ── 11. 价格日志 ─────────────────────────────────────────────────
  console.log("── 11. price_log ──");
  {
    const { rows } = await abtPool.query(
      "SELECT id, product_id, price, operator_id, remark, created_at FROM product_price ORDER BY id",
    );
    const data = rows.map((r) => [
      Number(r.id),
      Number(r.product_id),
      1, // price_type = Purchase
      null, // old_price
      r.price,
      r.operator_id ? Number(r.operator_id) : null,
      r.remark ?? "",
      r.created_at,
    ]);
    const n = await batchInsert(
      v2Pool,
      "price_log",
      ["log_id", "product_id", "price_type", "old_price", "new_price", "operator_id", "remark", "created_at"],
      data,
      ["log_id"],
    );
    await resetSequence(v2Pool, "price_log", "log_id");
    console.log(`  ${n}/${rows.length} 条\n`);
  }

  // ── 12. 库存 → stock_ledger ──────────────────────────────────────
  console.log("── 12. inventory → stock_ledger ──");
  {
    const { rows } = await abtPool.query(
      "SELECT inventory_id, product_id, location_id, quantity, safety_stock, batch_no, created_at, updated_at FROM inventory ORDER BY inventory_id",
    );
    let skipped = 0;
    const data: any[][] = [];
    for (const r of rows) {
      const loc = locationMapping.get(Number(r.location_id));
      if (!loc) {
        skipped++;
        continue;
      }
      data.push([
        Number(r.inventory_id),
        Number(r.product_id),
        loc.warehouseId,
        loc.zoneId,
        loc.binId, // 使用默认 bin_id
        r.batch_no ?? "",
        r.quantity,
        0, // reserved_qty
        r.quantity, // available_qty
        null, // unit_cost
        null, // received_date
        null, // expiry_date
        r.updated_at ?? r.created_at,
      ]);
    }
    const n = await batchInsert(
      v2Pool,
      "stock_ledger",
      ["id", "product_id", "warehouse_id", "zone_id", "bin_id", "batch_no", "quantity", "reserved_qty", "available_qty", "unit_cost", "received_date", "expiry_date", "updated_at"],
      data,
      ["id"],
    );
    await resetSequence(v2Pool, "stock_ledger", "id");
    console.log(`  ${n}/${rows.length} 条${skipped > 0 ? ` (跳过 ${skipped} 条无法匹配 location)` : ""}\n`);
  }

  // ── 13. 库存日志 → inventory_transactions ────────────────────────
  console.log("── 13. inventory_log → inventory_transactions ──");
  {
    const opTypeMap: Record<string, number> = {
      in: 1, out: 2, transfer: 3, adjustment: 4,
    };
    const { rows } = await abtPool.query(
      "SELECT log_id, inventory_id, product_id, location_id, change_qty, before_qty, after_qty, operation_type, ref_order_type, ref_order_id, operator, remark, created_at FROM inventory_log ORDER BY log_id",
    );
    let skipped = 0;
    const data: any[][] = [];
    for (const r of rows) {
      const loc = locationMapping.get(Number(r.location_id));
      if (!loc) {
        skipped++;
        continue;
      }
      const txnType = opTypeMap[r.operation_type] ?? 4; // default adjustment
      data.push([
        Number(r.log_id),
        null, // doc_number
        txnType,
        Number(r.product_id),
        loc.warehouseId,
        loc.zoneId,
        loc.binId, // 使用默认 bin_id
        null, // batch_no
        r.change_qty,
        null, // unit_cost
        r.ref_order_type ?? "",
        Number(r.ref_order_id) || 0, // source_id
        r.remark ?? "",
        0, // operator_id
        r.created_at,
      ]);
    }
    const n = await batchInsert(
      v2Pool,
      "inventory_transactions",
      ["id", "doc_number", "transaction_type", "product_id", "warehouse_id", "zone_id", "bin_id", "batch_no", "quantity", "unit_cost", "source_type", "source_id", "remark", "operator_id", "created_at"],
      data,
      ["id"],
    );
    await resetSequence(v2Pool, "inventory_transactions", "id");
    console.log(`  ${n}/${rows.length} 条${skipped > 0 ? ` (跳过 ${skipped} 条)` : ""}\n`);
  }

  // ── 14. 通知 ─────────────────────────────────────────────────────
  console.log("── 14. notifications ──");
  {
    const typeMap: Record<string, number> = {
      system: 1, business: 2, alert: 3,
    };
    const { rows } = await abtPool.query(
      "SELECT notification_id, user_id, type, title, content, related_type, related_id, is_read, read_at, created_at FROM notifications ORDER BY notification_id",
    );
    const data = rows.map((r) => [
      Number(r.notification_id),
      Number(r.user_id),
      typeMap[r.type] ?? 1,
      r.title,
      r.content ?? null,
      r.related_type ?? null,
      r.related_id ? Number(r.related_id) : null,
      r.is_read,
      r.read_at,
      r.created_at,
    ]);
    const n = await batchInsert(
      v2Pool,
      "notifications",
      ["notification_id", "user_id", "notification_type", "title", "content", "related_type", "related_id", "is_read", "read_at", "created_at"],
      data,
      ["notification_id"],
    );
    await resetSequence(v2Pool, "notifications", "notification_id");
    console.log(`  ${n}/${rows.length} 条\n`);
  }

  // ── 验证 ─────────────────────────────────────────────────────────
  console.log("=== 验证 ===\n");
  const checks: [string, string][] = [
    ["users", "user_id"],
    ["roles", "role_id"],
    ["departments", "department_id"],
    ["user_roles", "user_id"],
    ["role_permissions", "role_id"],
    ["products", "product_id"],
    ["price_log", "log_id"],
    ["labor_process_dicts", "id"],
    ["routings", "id"],
    ["routing_steps", "id"],
    ["bom_categories", "bom_category_id"],
    ["boms", "bom_id"],
    ["bom_nodes", "node_id"],
    ["bom_routings", "id"],
    ["bom_labor_processes", "id"],
    ["warehouses", "id"],
    ["zones", "id"],
    ["bins", "id"],
    ["stock_ledger", "id"],
    ["notifications", "notification_id"],
  ];
  for (const [table, _pk] of checks) {
    const { rows: cnt } = await v2Pool.query(`SELECT COUNT(*)::int AS c FROM ${table}`);
    console.log(`  abt_v2.${table}: ${cnt[0].c} 条`);
  }

  console.log("\n迁移完成。");
  await abtPool.end();
  await v2Pool.end();
}

main().catch((err) => {
  console.error("迁移失败:", err);
  process.exit(1);
});
