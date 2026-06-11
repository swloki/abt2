/**
 * 统一数据迁移脚本: abt_real → abt2
 *
 * 将旧 abt_real 数据库的所有业务数据迁移到 abt2。
 * 迁移前会清空 abt2 的业务数据（保留运行时/系统表），再按依赖顺序导入。
 *
 * 环境变量：
 *   PGHOST=localhost PGPORT=5432 PGUSER=postgres PGPASSWORD=123456
 *   ABT_DB=abt_real  ABT_V2_DB=abt2
 *
 * 运行：bun run scripts/migrate-all.ts
 */

import pg from "pg";

const HOST = process.env.PGHOST ?? "localhost";
const PORT = parseInt(process.env.PGPORT ?? "5432", 10);
const USER = process.env.PGUSER ?? "postgres";
const PASS = process.env.PGPASSWORD ?? "123456";
const ABT_DB = process.env.ABT_DB ?? "abt_real";
const ABT_V2_DB = process.env.ABT_V2_DB ?? "abt2";

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

/** 检查源表是否包含指定列 */
async function columnExists(pool: pg.Pool, table: string, column: string): Promise<boolean> {
  const { rows } = await pool.query(
    `SELECT 1 FROM information_schema.columns WHERE table_name = $1 AND column_name = $2 LIMIT 1`,
    [table, column],
  );
  return rows.length > 0;
}

// ============================================================================
// Migration
// ============================================================================

async function main() {
  console.log("=== 统一数据迁移: abt_real → abt2 ===\n");

  // ── Phase -1: 清空 abt2 业务数据（保留运行时/系统表）────────────────
  console.log("── Phase -1: 清空 abt2 业务数据 ──");
  {
    // 按外键依赖反序分批 TRUNCATE，使用 CASCADE 处理依赖
    const truncateGroups = [
      // 组1: 末端业务表（无被依赖）
      [
        "sales_return_items", "sales_returns", "sales_order_items", "sales_orders",
        "shipping_request_items", "shipping_requests",
        "quotation_items", "quotations",
        "reconciliation_items", "reconciliations",
        "purchase_return_items", "purchase_returns",
        "purchase_order_items", "purchase_orders",
        "purchase_quotation_items", "purchase_quotations",
        "purchase_recon_items", "purchase_reconciliations",
        "work_order_routings", "work_orders", "work_reports",
        "production_plan_items", "production_plans",
        "production_batches", "production_inspections", "production_receipts",
        "production_exception_events", "production_exceptions",
        "material_requisition_items", "material_requisitions",
        "arrival_notice_items", "arrival_notices",
        "backflush_items", "backflush_records",
        "cycle_count_items", "cycle_counts",
        "transfer_items", "inventory_transfers",
        "conversion_items", "form_conversions",
        "cash_journal_lines", "cash_journals",
        "write_offs",
        "expense_reimbursement_items", "expense_reimbursements",
        "cost_entries", "payment_requests",
        "inspection_results", "inspection_specifications",
        "mrbs", "rmas",
        "outsourcing_materials", "outsourcing_orders", "outsourcing_trackings",
        "misc_request_items", "miscellaneous_requests",
        "inventory_reservations", "inventory_locks",
        "bom_snapshots",
        "product_categories", "categories",
        "supplier_bank_accounts", "supplier_contacts", "suppliers",
        "customer_addresses", "customer_contacts", "customers",
      ],
      // 组2: 库存交易
      [
        "inventory_transactions", "stock_ledger",
      ],
      // 组3: BOM + 工艺
      [
        "bom_nodes", "bom_routings", "bom_labor_processes",
        "boms", "bom_categories",
        "routing_steps", "routings", "labor_process_dicts",
      ],
      // 组4: 主数据 + 库位
      [
        "products", "bins", "zones", "warehouses",
      ],
      // 组5: 身份层 + 工作流 + 其他
      [
        "user_roles", "user_departments", "role_permissions",
        "roles", "users", "departments",
        "workflow_history", "workflow_tasks", "workflow_instances", "workflow_templates",
        "notifications", "product_watchers", "price_log",
        "document_links", "entity_state_logs",
      ],
    ];

    for (let i = 0; i < truncateGroups.length; i++) {
      const tables = truncateGroups[i];
      const tableList = tables.join(", ");
      try {
        await v2Pool.query(`TRUNCATE TABLE ${tableList} CASCADE`);
        console.log(`  组${i + 1} 已清空: ${tables.length} 张表`);
      } catch (e: any) {
        console.error(`  组${i + 1} 清空失败: ${e.message}`);
        throw e;
      }
    }
    console.log("  业务数据清空完成\n");
  }

  // ── 0. Identity 数据（用户、角色、部门、权限）──────────────────────
  console.log("── 0. identity (users, roles, departments, permissions) ──");

  // 0a. departments
  {
    const { rows } = await abtPool.query(
      "SELECT department_id, department_name, department_code, description, is_active, is_default, created_at, updated_at FROM departments ORDER BY department_id",
    );
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

  // 0b. users
  {
    const { rows } = await abtPool.query(
      "SELECT user_id, username, password_hash, display_name, is_active, is_super_admin, created_at, updated_at FROM users ORDER BY user_id",
    );
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

  // 0c. roles
  {
    const hasParentRole = await columnExists(abtPool, "roles", "parent_role_id");
    const selectCols = hasParentRole
      ? "role_id, role_name, role_code, is_system_role, parent_role_id, description, created_at, updated_at"
      : "role_id, role_name, role_code, is_system_role, description, created_at, updated_at";
    const { rows } = await abtPool.query(
      `SELECT ${selectCols} FROM roles ORDER BY role_id`,
    );
    await v2Pool.query("DELETE FROM roles WHERE role_code IN ('super_admin', 'admin', 'viewer')");
    const data = rows.map((r) => [
      Number(r.role_id),
      r.role_name,
      r.role_code,
      r.is_system_role ?? false,
      hasParentRole && r.parent_role_id ? Number(r.parent_role_id) : null,
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
  const warehouseMapping = new Map<number, number>();
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
        r.status === "active" ? 1 : 2,
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
    const hasLocUpdatedAt = await columnExists(abtPool, "location", "updated_at");
    const locSelectCols = hasLocUpdatedAt
      ? "location_id, warehouse_id, location_code, location_name, capacity, created_at, updated_at, deleted_at, status"
      : "location_id, warehouse_id, location_code, location_name, capacity, created_at, deleted_at, status";
    const { rows } = await abtPool.query(
      `SELECT ${locSelectCols} FROM location ORDER BY location_id`,
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
        hasLocUpdatedAt && r.updated_at ? r.updated_at : r.created_at,
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
      const binId = zoneId;
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
        hasLocUpdatedAt && r.updated_at ? r.updated_at : r.created_at,
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
    // 动态检测 bom 表列（029 添加了 status/published_at/created_by，030 回滚了它们）
    const hasStatus = await columnExists(abtPool, "bom", "status");
    const hasPublishedAt = await columnExists(abtPool, "bom", "published_at");
    const hasCreatedBy = await columnExists(abtPool, "bom", "created_by");

    const extraCols: string[] = [];
    if (hasStatus) extraCols.push("status");
    if (hasPublishedAt) extraCols.push("published_at");
    if (hasCreatedBy) extraCols.push("created_by");

    const selectSql = `SELECT bom_id, bom_name, create_at, update_at, bom_detail, bom_category_id${extraCols.length > 0 ? ", " + extraCols.join(", ") : ""} FROM bom ORDER BY bom_id`;
    const { rows } = await abtPool.query(selectSql);

    const data = rows.map((r) => [
      Number(r.bom_id),
      r.bom_name,
      r.create_at,
      r.update_at,
      r.bom_detail ? JSON.stringify(r.bom_detail) : '{"nodes":[]}',
      r.bom_category_id ? Number(r.bom_category_id) : null,
      hasStatus && r.status === "published" ? 2 : 1,
      1, // version
      hasPublishedAt ? r.published_at : null,
      hasCreatedBy && r.created_by ? Number(r.created_by) : null,
    ]);
    const n = await batchInsert(
      v2Pool,
      "boms",
      ["bom_id", "bom_name", "create_at", "update_at", "bom_detail", "bom_category_id", "status", "version", "published_at", "created_by"],
      data,
      ["bom_id"],
    );
    await resetSequence(v2Pool, "boms", "bom_id");
    console.log(`  ${n}/${rows.length} 条${extraCols.length < 3 ? ` (注意: 缺少列 ${["status", "published_at", "created_by"].filter(c => !extraCols.includes(c)).join(", ")})` : ""}\n`);
  }

  // ── 8. BOM 节点 ──────────────────────────────────────────────────
  console.log("── 8. bom_nodes ──");
  {
    const { rows } = await abtPool.query(
      `SELECT id, bom_id, product_id, product_code, quantity, parent_id, loss_rate, "order", unit, remark, position, work_center, properties FROM bom_nodes ORDER BY id`,
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

    const hasProcessCode = await columnExists(abtPool, "bom_labor_process", "process_code");
    const selectSql = hasProcessCode
      ? "SELECT id, product_code, name, unit_price, quantity, sort_order, remark, created_at, updated_at, process_code FROM bom_labor_process ORDER BY id"
      : "SELECT id, product_code, name, unit_price, quantity, sort_order, remark, created_at, updated_at FROM bom_labor_process ORDER BY id";

    const { rows } = await abtPool.query(selectSql);
    let skipped = 0;
    const data: any[][] = [];
    for (const r of rows) {
      const processCode = hasProcessCode ? r.process_code : null;
      const dictId = processCode ? codeToDictId.get(processCode) : null;
      // 跳过 process_code 存在但无法匹配字典的记录（而非插入 dict_id=0）
      if (processCode && !dictId) {
        skipped++;
        continue;
      }
      data.push([
        Number(r.id),
        r.product_code,
        dictId ?? 0, // labor_process_dict_id
        processCode ?? null,
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
        loc.binId,
        r.batch_no ?? null, // 保持 NULL 而非空字符串
        r.safety_stock ?? 0, // safety_stock
        r.quantity,          // quantity
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
      ["id", "product_id", "warehouse_id", "zone_id", "bin_id", "batch_no", "safety_stock", "quantity", "reserved_qty", "available_qty", "unit_cost", "received_date", "expiry_date", "updated_at"],
      data,
      ["id"],
    );
    await resetSequence(v2Pool, "stock_ledger", "id");
    console.log(`  ${n}/${rows.length} 条${skipped > 0 ? ` (跳过 ${skipped} 条无法匹配 location)` : ""}\n`);
  }

  // ── 13. 库存日志 → inventory_transactions ────────────────────────
  console.log("── 13. inventory_log → inventory_transactions ──");
  {
    // 修复: 正确映射 operation_type → transaction_type
    // 目标枚举: 1=PurchaseReceipt, 2=ProductionReceipt, 3=SalesShipment, 4=MaterialIssue,
    //           5=MaterialReturn, 6=Backflush, 7=Transfer, 8=FormConversion, 9=Adjustment
    const opTypeMap: Record<string, number> = {
      in: 1,         // 入库 → PurchaseReceipt
      out: 4,        // 出库 → MaterialIssue
      transfer: 7,   // 调拨 → Transfer
      adjustment: 9, // 调整 → Adjustment
    };

    // 构建 username → user_id 映射，用于还原 operator 信息
    const { rows: userRows } = await v2Pool.query("SELECT user_id, username FROM users");
    const usernameToId = new Map<string, number>();
    for (const u of userRows) {
      usernameToId.set(u.username, Number(u.user_id));
    }

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
      const txnType = opTypeMap[r.operation_type] ?? 9; // 默认 Adjustment

      // 尝试通过 username 查找 operator_id
      let operatorId = 0;
      if (r.operator) {
        const found = usernameToId.get(r.operator);
        if (found) operatorId = found;
      }

      // 安全解析 ref_order_id: VARCHAR → BIGINT
      let sourceId = 0;
      if (r.ref_order_id) {
        const parsed = parseInt(r.ref_order_id, 10);
        if (!isNaN(parsed)) sourceId = parsed;
      }

      // 在 remark 中保留原始操作人信息（如果无法匹配 user_id）
      let remark = r.remark ?? "";
      if (r.operator && operatorId === 0) {
        remark = remark ? `${remark} [原始操作人: ${r.operator}]` : `[原始操作人: ${r.operator}]`;
      }

      data.push([
        Number(r.log_id),
        null, // doc_number
        txnType,
        Number(r.product_id),
        loc.warehouseId,
        loc.zoneId,
        loc.binId,
        null, // batch_no
        r.change_qty,
        null, // unit_cost
        r.ref_order_type ?? "",
        sourceId,
        remark,
        operatorId,
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

  // ── 15. 产品监听 ────────────────────────────────────────────────
  console.log("── 15. product_watchers ──");
  {
    const { rows } = await abtPool.query(
      "SELECT user_id, product_id, safety_stock_override, alert_active, last_notified_at, created_at, updated_at FROM product_watchers ORDER BY user_id, product_id",
    );
    const data = rows.map((r) => [
      Number(r.user_id),
      Number(r.product_id),
      r.safety_stock_override ?? null,
      r.alert_active ?? false,
      r.last_notified_at ?? null,
      r.created_at,
      r.updated_at,
    ]);
    const n = await batchInsert(
      v2Pool,
      "product_watchers",
      ["user_id", "product_id", "safety_stock_override", "alert_active", "last_notified_at", "created_at", "updated_at"],
      data,
      ["user_id", "product_id"],
    );
    console.log(`  ${n}/${rows.length} 条\n`);
  }

  // ── 16. H3云同步状态 — abt2 无此表，跳过 ──

  // ── 17. 工作流 ──────────────────────────────────────────────────
  console.log("── 17. workflow ──");
  {
    // 17a. workflow_templates
    const hasTriggerEvent = await columnExists(abtPool, "workflow_templates", "trigger_event");
    const tmplSelect = hasTriggerEvent
      ? "SELECT id, entity_type, name, version, status, graph, graph_checksum, trigger_event, created_at, updated_at, deleted_at FROM workflow_templates ORDER BY id"
      : "SELECT id, entity_type, name, version, status, graph, graph_checksum, created_at, updated_at, deleted_at FROM workflow_templates ORDER BY id";
    const { rows: tmpls } = await abtPool.query(tmplSelect);
    const tmplData = tmpls.map((r: any) => [
      Number(r.id),
      r.entity_type,
      r.name,
      Number(r.version ?? 1),
      r.status,
      r.graph ? JSON.stringify(r.graph) : null,
      r.graph_checksum ?? null,
      hasTriggerEvent ? r.trigger_event ?? null : null,
      r.created_at,
      r.updated_at,
      r.deleted_at,
    ]);
    const tn = await batchInsert(
      v2Pool,
      "workflow_templates",
      ["id", "entity_type", "name", "version", "status", "graph", "graph_checksum", "trigger_event", "created_at", "updated_at", "deleted_at"],
      tmplData,
      ["id"],
    );
    await resetSequence(v2Pool, "workflow_templates", "id");
    console.log(`  workflow_templates: ${tn}/${tmpls.length} 条`);

    // 17b. workflow_instances
    const { rows: insts } = await abtPool.query(
      "SELECT id, template_id, template_version, entity_type, entity_id, status, frozen_graph, context, suspended_reason, initiator_id, created_at, updated_at, last_advanced_at, completed_at FROM workflow_instances ORDER BY id",
    );
    const instData = insts.map((r: any) => [
      Number(r.id),
      Number(r.template_id),
      r.template_version ? Number(r.template_version) : null,
      r.entity_type,
      Number(r.entity_id),
      r.status,
      r.frozen_graph ? JSON.stringify(r.frozen_graph) : null,
      r.context ? JSON.stringify(r.context) : null,
      r.suspended_reason ? JSON.stringify(r.suspended_reason) : null,
      Number(r.initiator_id),
      r.created_at,
      r.updated_at,
      r.last_advanced_at,
      r.completed_at,
    ]);
    const in_ = await batchInsert(
      v2Pool,
      "workflow_instances",
      ["id", "template_id", "template_version", "entity_type", "entity_id", "status", "frozen_graph", "context", "suspended_reason", "initiator_id", "created_at", "updated_at", "last_advanced_at", "completed_at"],
      instData,
      ["id"],
    );
    await resetSequence(v2Pool, "workflow_instances", "id");
    console.log(`  workflow_instances: ${in_}/${insts.length} 条`);

    // 17c. workflow_tasks
    const { rows: tasks } = await abtPool.query(
      "SELECT id, instance_id, node_id, prev_task_id, assignee_id, status, action, timeout_action, due_at, remind_at, result, created_at, completed_at FROM workflow_tasks ORDER BY id",
    );
    const taskData = tasks.map((r: any) => [
      Number(r.id),
      Number(r.instance_id),
      r.node_id,
      r.prev_task_id ? Number(r.prev_task_id) : null,
      r.assignee_id ? Number(r.assignee_id) : null,
      r.status,
      r.action ?? null,
      r.timeout_action ?? null,
      r.due_at,
      r.remind_at,
      r.result ? JSON.stringify(r.result) : null,
      r.created_at,
      r.completed_at,
    ]);
    const tkn = await batchInsert(
      v2Pool,
      "workflow_tasks",
      ["id", "instance_id", "node_id", "prev_task_id", "assignee_id", "status", "action", "timeout_action", "due_at", "remind_at", "result", "created_at", "completed_at"],
      taskData,
      ["id"],
    );
    await resetSequence(v2Pool, "workflow_tasks", "id");
    console.log(`  workflow_tasks: ${tkn}/${tasks.length} 条`);

    // 17d. workflow_history
    const { rows: hist } = await abtPool.query(
      "SELECT id, instance_id, task_id, node_id, event_type, actor_id, payload, created_at FROM workflow_history ORDER BY id",
    );
    const histData = hist.map((r: any) => [
      Number(r.id),
      Number(r.instance_id),
      r.task_id ? Number(r.task_id) : null,
      r.node_id ?? null,
      r.event_type,
      r.actor_id ? Number(r.actor_id) : null,
      r.payload ? JSON.stringify(r.payload) : null,
      r.created_at,
    ]);
    const hn = await batchInsert(
      v2Pool,
      "workflow_history",
      ["id", "instance_id", "task_id", "node_id", "event_type", "actor_id", "payload", "created_at"],
      histData,
      ["id"],
    );
    await resetSequence(v2Pool, "workflow_history", "id");
    console.log(`  workflow_history: ${hn}/${hist.length} 条\n`);
  }

  // ── BOM 状态转换规则 + 状态日志回填 ──────────────────────────────
  console.log("── BomStatus state transitions ──");
  {
    await v2Pool.query(`
      INSERT INTO state_transition_defs (entity_type, from_state, to_state, trigger_event, sort_order) VALUES
        ('BomStatus', '', 'Draft', NULL, 1),
        ('BomStatus', 'Draft', 'Published', NULL, 2),
        ('BomStatus', 'Published', 'Draft', NULL, 3)
      ON CONFLICT DO NOTHING
    `);

    // backfill: '' -> Draft for all Draft BOMs
    const { rowCount: draftCount } = await v2Pool.query(`
      INSERT INTO entity_state_logs (entity_type, entity_id, from_state, to_state, transition_id, operator_id, remark)
      SELECT 'BomStatus', b.bom_id, '', 'Draft', t.id, COALESCE(b.created_by, 0), 'backfill'
      FROM boms b
      JOIN state_transition_defs t ON t.entity_type = 'BomStatus' AND t.from_state = '' AND t.to_state = 'Draft'
      WHERE b.status = 1 AND b.deleted_at IS NULL
      ON CONFLICT DO NOTHING
    `);

    // backfill: '' -> Draft for all Published BOMs
    await v2Pool.query(`
      INSERT INTO entity_state_logs (entity_type, entity_id, from_state, to_state, transition_id, operator_id, remark)
      SELECT 'BomStatus', b.bom_id, '', 'Draft', t.id, COALESCE(b.created_by, 0), 'backfill'
      FROM boms b
      JOIN state_transition_defs t ON t.entity_type = 'BomStatus' AND t.from_state = '' AND t.to_state = 'Draft'
      WHERE b.status = 2 AND b.deleted_at IS NULL
      ON CONFLICT DO NOTHING
    `);

    // backfill: Draft -> Published for all Published BOMs
    const { rowCount: pubCount } = await v2Pool.query(`
      INSERT INTO entity_state_logs (entity_type, entity_id, from_state, to_state, transition_id, operator_id, remark)
      SELECT 'BomStatus', b.bom_id, 'Draft', 'Published', t.id, COALESCE(b.created_by, 0), 'backfill'
      FROM boms b
      JOIN state_transition_defs t ON t.entity_type = 'BomStatus' AND t.from_state = 'Draft' AND t.to_state = 'Published'
      WHERE b.status = 2 AND b.deleted_at IS NULL
      ON CONFLICT DO NOTHING
    `);

    console.log(`  transitions seeded, backfill: ${draftCount} Draft + ${pubCount} Published\n`);
  }

  // ── 注意：以下 abt_real 表因无对应 abt2 目标表或 schema 差异过大，跳过 ──
  // - terms / term_relation: 旧分类体系，abt2 用 categories + product_categories（结构完全不同）
  // - product_price_log_archived: 归档表，无对应目标
  // - permission_audit_logs: 结构与 abt2 的 audit_logs 分区表不同

  // ── 验证 ─────────────────────────────────────────────────────────
  console.log("=== 验证：abt_real 源表 vs abt2 目标表行数对比 ===\n");
  const checks: { srcTable: string; dstTable: string; srcPk: string; dstPk: string }[] = [
    { srcTable: "users", dstTable: "users", srcPk: "user_id", dstPk: "user_id" },
    { srcTable: "roles", dstTable: "roles", srcPk: "role_id", dstPk: "role_id" },
    { srcTable: "departments", dstTable: "departments", srcPk: "department_id", dstPk: "department_id" },
    { srcTable: "user_roles", dstTable: "user_roles", srcPk: "user_id", dstPk: "user_id" },
    { srcTable: "role_permissions", dstTable: "role_permissions", srcPk: "role_id", dstPk: "role_id" },
    { srcTable: "user_departments", dstTable: "user_departments", srcPk: "user_id", dstPk: "user_id" },
    { srcTable: "products", dstTable: "products", srcPk: "product_id", dstPk: "product_id" },
    { srcTable: "product_price", dstTable: "price_log", srcPk: "id", dstPk: "log_id" },
    { srcTable: "labor_process_dict", dstTable: "labor_process_dicts", srcPk: "id", dstPk: "id" },
    { srcTable: "routing", dstTable: "routings", srcPk: "id", dstPk: "id" },
    { srcTable: "routing_step", dstTable: "routing_steps", srcPk: "id", dstPk: "id" },
    { srcTable: "bom_category", dstTable: "bom_categories", srcPk: "bom_category_id", dstPk: "bom_category_id" },
    { srcTable: "bom", dstTable: "boms", srcPk: "bom_id", dstPk: "bom_id" },
    { srcTable: "bom_nodes", dstTable: "bom_nodes", srcPk: "id", dstPk: "node_id" },
    { srcTable: "bom_routing", dstTable: "bom_routings", srcPk: "id", dstPk: "id" },
    { srcTable: "bom_labor_process", dstTable: "bom_labor_processes", srcPk: "id", dstPk: "id" },
    { srcTable: "warehouse", dstTable: "warehouses", srcPk: "warehouse_id", dstPk: "id" },
    { srcTable: "location", dstTable: "zones", srcPk: "location_id", dstPk: "id" },
    { srcTable: "inventory", dstTable: "stock_ledger", srcPk: "inventory_id", dstPk: "id" },
    { srcTable: "inventory_log", dstTable: "inventory_transactions", srcPk: "log_id", dstPk: "id" },
    { srcTable: "notifications", dstTable: "notifications", srcPk: "notification_id", dstPk: "notification_id" },
    { srcTable: "product_watchers", dstTable: "product_watchers", srcPk: "user_id", dstPk: "user_id" },
    { srcTable: "workflow_templates", dstTable: "workflow_templates", srcPk: "id", dstPk: "id" },
    { srcTable: "workflow_instances", dstTable: "workflow_instances", srcPk: "id", dstPk: "id" },
    { srcTable: "workflow_tasks", dstTable: "workflow_tasks", srcPk: "id", dstPk: "id" },
    { srcTable: "workflow_history", dstTable: "workflow_history", srcPk: "id", dstPk: "id" },
  ];

  let allOk = true;
  for (const { srcTable, dstTable } of checks) {
    const { rows: srcCnt } = await abtPool.query(`SELECT COUNT(*)::int AS c FROM ${srcTable}`);
    const { rows: dstCnt } = await v2Pool.query(`SELECT COUNT(*)::int AS c FROM ${dstTable}`);
    const src = srcCnt[0].c;
    const dst = dstCnt[0].c;
    const ok = src === dst ? "✓" : "✗";
    if (src !== dst) allOk = false;
    console.log(`  ${ok} abt_real.${srcTable} → abt2.${dstTable}: ${src} → ${dst}`);
  }

  console.log(`\n${allOk ? "全部验证通过" : "存在差异，请检查上方标记为 ✗ 的表"}。\n`);
  console.log("迁移完成。");
  await abtPool.end();
  await v2Pool.end();
}

main().catch((err) => {
  console.error("迁移失败:", err);
  process.exit(1);
});
