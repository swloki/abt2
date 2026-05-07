import pg from "pg";

const abt2Config = {
  host: "localhost",
  port: 5432,
  user: "postgres",
  password: "123456",
  database: "abt2",
};

const abtConfig = {
  host: "localhost",
  port: 5432,
  user: "postgres",
  password: "123456",
  database: "abt",
};

const abt2 = new pg.Pool(abt2Config);
const abt = new pg.Pool(abtConfig);

// --- helpers ---

async function truncateAbt(client: pg.PoolClient) {
  // Auto-discover all tables in public schema
  const { rows } = await client.query(`
    SELECT tablename FROM pg_tables WHERE schemaname = 'public' ORDER BY tablename
  `);
  const names = rows.map((r) => `"${r.tablename}"`).join(", ");
  await client.query(`TRUNCATE ${names} CASCADE`);
  console.log(`✔ abt ${rows.length} 张表已清空`);
}

async function resetSequence(
  client: pg.PoolClient,
  seqName: string,
  maxVal: number
) {
  if (maxVal > 0) {
    await client.query(
      `SELECT setval('${seqName}', ${maxVal}, true)`
    );
  }
}

// --- migrate functions per table ---

async function migrateUsers(src: pg.PoolClient, dst: pg.PoolClient) {
  const { rows } = await src.query("SELECT * FROM users ORDER BY user_id");
  for (const r of rows) {
    await dst.query(
      `INSERT INTO users (user_id, username, password_hash, display_name, is_active, is_super_admin, created_at, updated_at)
       VALUES ($1,$2,$3,$4,$5,$6,$7,$8)`,
      [
        r.user_id, r.username, r.password_hash, r.display_name,
        r.is_active, r.is_super_admin, r.created_at, r.updated_at,
      ]
    );
  }
  console.log(`  users: ${rows.length} rows`);
}

async function migrateRoles(src: pg.PoolClient, dst: pg.PoolClient) {
  const { rows } = await src.query("SELECT * FROM roles ORDER BY role_id");
  for (const r of rows) {
    await dst.query(
      `INSERT INTO roles (role_id, role_name, role_code, is_system_role, description, created_at, updated_at, parent_role_id)
       VALUES ($1,$2,$3,$4,$5,$6,$7,$8)`,
      [
        r.role_id, r.role_name, r.role_code, r.is_system_role,
        r.description, r.created_at, r.updated_at, r.parent_role_id,
      ]
    );
  }
  console.log(`  roles: ${rows.length} rows`);
}

async function migrateDepartments(src: pg.PoolClient, dst: pg.PoolClient) {
  const { rows } = await src.query("SELECT * FROM departments ORDER BY department_id");
  for (const r of rows) {
    await dst.query(
      `INSERT INTO departments (department_id, department_name, department_code, description, is_active, is_default, created_at, updated_at)
       VALUES ($1,$2,$3,$4,$5,$6,$7,$8)`,
      [
        r.department_id, r.department_name, r.department_code,
        r.description, r.is_active, r.is_default, r.created_at, r.updated_at,
      ]
    );
  }
  console.log(`  departments: ${rows.length} rows`);
}

async function migrateWarehouse(src: pg.PoolClient, dst: pg.PoolClient) {
  const { rows } = await src.query("SELECT * FROM warehouse ORDER BY warehouse_id");
  for (const r of rows) {
    await dst.query(
      `INSERT INTO warehouse (warehouse_id, warehouse_name, warehouse_code, status, created_at, updated_at, deleted_at)
       VALUES ($1,$2,$3,$4,$5,$6,$7)`,
      [
        r.warehouse_id, r.warehouse_name, r.warehouse_code,
        r.status, r.created_at, r.updated_at, r.deleted_at,
      ]
    );
  }
  console.log(`  warehouse: ${rows.length} rows`);
}

async function migrateLaborProcessDict(src: pg.PoolClient, dst: pg.PoolClient) {
  const { rows } = await src.query("SELECT * FROM labor_process_dict ORDER BY id");
  for (const r of rows) {
    await dst.query(
      `INSERT INTO labor_process_dict (id, code, name, description, sort_order, created_at, updated_at)
       VALUES ($1,$2,$3,$4,$5,$6,$7)`,
      [
        r.id, r.code, r.name, r.description,
        r.sort_order, r.created_at, r.updated_at,
      ]
    );
  }
  console.log(`  labor_process_dict: ${rows.length} rows`);
}

async function migrateTerms(src: pg.PoolClient, dst: pg.PoolClient) {
  const { rows } = await src.query("SELECT * FROM terms ORDER BY term_id");
  for (const r of rows) {
    const termMeta = r.term_meta ? JSON.stringify(r.term_meta) : null;
    await dst.query(
      `INSERT INTO terms (term_id, term_name, term_parent, term_meta, taxonomy)
       VALUES ($1,$2,$3,$4::jsonb,$5)`,
      [r.term_id, r.term_name, r.term_parent, termMeta, r.taxonomy]
    );
  }
  console.log(`  terms: ${rows.length} rows`);
}

async function migrateBomCategory(src: pg.PoolClient, dst: pg.PoolClient) {
  const { rows } = await src.query("SELECT * FROM bom_category ORDER BY bom_category_id");
  for (const r of rows) {
    await dst.query(
      `INSERT INTO bom_category (bom_category_id, bom_category_name, created_at)
       VALUES ($1,$2,$3)`,
      [r.bom_category_id, r.bom_category_name, r.created_at]
    );
  }
  console.log(`  bom_category: ${rows.length} rows`);
}

// products: extract product_code and unit from meta JSONB
async function migrateProducts(src: pg.PoolClient, dst: pg.PoolClient) {
  const { rows } = await src.query("SELECT * FROM products ORDER BY product_id");
  const codeUsage = new Map<string, number>();
  let migrated = 0;
  for (const r of rows) {
    const meta = r.meta ?? {};
    const rawCode = meta.product_code ?? "";
    const unit = meta.unit ?? "pcs";

    // Deduplicate product_code by appending -N suffix
    let productCode = rawCode || `auto_${r.product_id}`;
    const usage = codeUsage.get(productCode) ?? 0;
    if (usage > 0) {
      productCode = `${productCode}-${usage + 1}`;
    }
    codeUsage.set(productCode, (codeUsage.get(productCode) ?? 0) + 1);

    // Strip product_code and unit from meta to match abt's cleaner meta structure
    const cleanMeta = { ...meta };
    delete cleanMeta.product_code;
    delete cleanMeta.unit;

    await dst.query(
      `INSERT INTO products (product_id, pdt_name, meta, product_code, unit)
       VALUES ($1,$2,$3::jsonb,$4,$5)`,
      [
        r.product_id, r.pdt_name,
        JSON.stringify(cleanMeta),
        productCode, unit,
      ]
    );
    migrated++;
  }
  console.log(`  products: ${migrated} rows`);
}

async function migrateLocation(src: pg.PoolClient, dst: pg.PoolClient) {
  const { rows } = await src.query("SELECT * FROM location ORDER BY location_id");
  for (const r of rows) {
    await dst.query(
      `INSERT INTO location (location_id, warehouse_id, location_code, location_name, capacity, created_at, deleted_at, status)
       VALUES ($1,$2,$3,$4,$5,$6,$7,$8)`,
      [
        r.location_id, r.warehouse_id, r.location_code,
        r.location_name, r.capacity, r.created_at, r.deleted_at,
        r.status ?? "active",
      ]
    );
  }
  console.log(`  location: ${rows.length} rows`);
}

async function migrateRouting(src: pg.PoolClient, dst: pg.PoolClient) {
  const { rows } = await src.query("SELECT * FROM routing ORDER BY id");
  for (const r of rows) {
    await dst.query(
      `INSERT INTO routing (id, name, description, created_at, updated_at)
       VALUES ($1,$2,$3,$4,$5)`,
      [r.id, r.name, r.description, r.created_at, r.updated_at]
    );
  }
  console.log(`  routing: ${rows.length} rows`);
}

async function migrateRoutingStep(src: pg.PoolClient, dst: pg.PoolClient) {
  const { rows } = await src.query("SELECT * FROM routing_step ORDER BY id");
  for (const r of rows) {
    await dst.query(
      `INSERT INTO routing_step (id, routing_id, process_code, step_order, is_required, remark, created_at, updated_at)
       VALUES ($1,$2,$3,$4,$5,$6,$7,$8)`,
      [
        r.id, r.routing_id, r.process_code, r.step_order,
        r.is_required, r.remark, r.created_at, r.updated_at,
      ]
    );
  }
  console.log(`  routing_step: ${rows.length} rows`);
}

async function migrateInventory(src: pg.PoolClient, dst: pg.PoolClient) {
  const { rows } = await src.query("SELECT * FROM inventory ORDER BY inventory_id");
  for (const r of rows) {
    await dst.query(
      `INSERT INTO inventory (inventory_id, product_id, location_id, quantity, safety_stock, batch_no, created_at, updated_at)
       VALUES ($1,$2,$3,$4,$5,$6,$7,$8)`,
      [
        r.inventory_id, r.product_id, r.location_id,
        r.quantity, r.safety_stock, r.batch_no, r.created_at, r.updated_at,
      ]
    );
  }
  console.log(`  inventory: ${rows.length} rows`);
}

async function migrateInventoryLog(src: pg.PoolClient, dst: pg.PoolClient) {
  const { rows } = await src.query("SELECT * FROM inventory_log ORDER BY log_id");
  for (const r of rows) {
    await dst.query(
      `INSERT INTO inventory_log (log_id, inventory_id, product_id, location_id, change_qty, before_qty, after_qty, operation_type, ref_order_type, ref_order_id, operator, remark, created_at)
       VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)`,
      [
        r.log_id, r.inventory_id, r.product_id, r.location_id,
        r.change_qty, r.before_qty, r.after_qty, r.operation_type,
        r.ref_order_type, r.ref_order_id, r.operator, r.remark, r.created_at,
      ]
    );
  }
  console.log(`  inventory_log: ${rows.length} rows`);
}

async function migrateBom(src: pg.PoolClient, dst: pg.PoolClient) {
  const { rows } = await src.query("SELECT * FROM bom ORDER BY bom_id");
  for (const r of rows) {
    const bomDetail = r.bom_detail ? JSON.stringify(r.bom_detail) : null;
    await dst.query(
      `INSERT INTO bom (bom_id, bom_name, create_at, update_at, bom_category_id, created_by, status, published_at, bom_detail)
       VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9::jsonb)`,
      [
        r.bom_id, r.bom_name, r.create_at, r.update_at,
        r.bom_category_id,
        null,       // created_by — no source
        "published", // status — default
        r.create_at, // published_at — use create_at
        bomDetail,
      ]
    );
  }
  console.log(`  bom: ${rows.length} rows`);
}

async function migrateBomLaborProcess(src: pg.PoolClient, dst: pg.PoolClient) {
  const { rows } = await src.query("SELECT * FROM bom_labor_process ORDER BY id");
  for (const r of rows) {
    await dst.query(
      `INSERT INTO bom_labor_process (id, product_code, name, unit_price, quantity, sort_order, remark, created_at, updated_at, process_code)
       VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)`,
      [
        r.id, r.product_code, r.name, r.unit_price,
        r.quantity, r.sort_order, r.remark, r.created_at, r.updated_at,
        r.process_code,
      ]
    );
  }
  console.log(`  bom_labor_process: ${rows.length} rows`);
}

async function migrateBomNodes(src: pg.PoolClient, dst: pg.PoolClient) {
  const { rows } = await src.query("SELECT bom_id, bom_detail FROM bom WHERE bom_detail IS NOT NULL ORDER BY bom_id");
  let totalNodes = 0;

  for (const bom of rows) {
    const detail = bom.bom_detail;
    // pg returns JSONB as JS object
    const nodes: any[] = detail?.nodes ?? [];
    if (nodes.length === 0) continue;

    // Sort by old id so parents are inserted before children
    nodes.sort((a, b) => (a.id as number) - (b.id as number));

    // old_id → new_id mapping for parent_id resolution
    const idMap = new Map<number, number>();

    for (const node of nodes) {
      const oldId = node.id as number;
      const oldParentId = node.parent_id as number;

      // Insert with parent_id = NULL first, update later
      const insertRes = await dst.query(
        `INSERT INTO bom_nodes (bom_id, product_id, product_code, quantity, parent_id, loss_rate, "order", unit, remark, position, work_center, properties)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12) RETURNING id`,
        [
          bom.bom_id,
          node.product_id,
          node.product_code ?? null,
          node.quantity,
          null, // will update after
          node.loss_rate ?? 0,
          node.order ?? 0,
          node.unit ?? null,
          node.remark || null,
          node.position || null,
          node.work_center || null,
          node.properties ?? null,
        ]
      );
      const newId = insertRes.rows[0].id as number;
      idMap.set(oldId, newId);
      totalNodes++;

      // Update parent_id if this node has a parent that's already mapped
      if (oldParentId !== 0 && idMap.has(oldParentId)) {
        await dst.query(
          `UPDATE bom_nodes SET parent_id = $1 WHERE id = $2`,
          [idMap.get(oldParentId), newId]
        );
      }
    }

    // Second pass: fix any nodes whose parent was inserted after them (shouldn't happen with sorted insert, but just in case)
    for (const node of nodes) {
      const oldParentId = node.parent_id as number;
      if (oldParentId !== 0 && idMap.has(oldParentId)) {
        // Already handled above
      }
    }
  }
  console.log(`  bom_nodes: ${totalNodes} rows (from ${rows.length} BOMs)`);
}

async function migrateBomRouting(src: pg.PoolClient, dst: pg.PoolClient) {
  const { rows } = await src.query("SELECT * FROM bom_routing ORDER BY id");
  for (const r of rows) {
    await dst.query(
      `INSERT INTO bom_routing (id, product_code, routing_id, created_at, updated_at)
       VALUES ($1,$2,$3,$4,$5)`,
      [r.id, r.product_code, r.routing_id, r.created_at, r.updated_at]
    );
  }
  console.log(`  bom_routing: ${rows.length} rows`);
}

async function migrateTermRelation(src: pg.PoolClient, dst: pg.PoolClient) {
  const { rows } = await src.query("SELECT * FROM term_relation ORDER BY term_id, product_id");
  for (const r of rows) {
    await dst.query(
      `INSERT INTO term_relation (term_id, product_id)
       VALUES ($1,$2)`,
      [r.term_id, r.product_id]
    );
  }
  console.log(`  term_relation: ${rows.length} rows`);
}

async function migratePermissionAuditLogs(src: pg.PoolClient, dst: pg.PoolClient) {
  const { rows } = await src.query("SELECT * FROM permission_audit_logs ORDER BY log_id");
  for (const r of rows) {
    const oldValue = r.old_value === "" || r.old_value === null
      ? null
      : JSON.stringify(r.old_value);
    const newValue = r.new_value === "" || r.new_value === null
      ? null
      : JSON.stringify(r.new_value);
    await dst.query(
      `INSERT INTO permission_audit_logs (log_id, operator_id, target_type, target_id, action, old_value, new_value, created_at)
       VALUES ($1,$2,$3,$4,$5,$6::jsonb,$7::jsonb,$8)`,
      [
        r.log_id, r.operator_id, r.target_type, r.target_id,
        r.action, oldValue, newValue, r.created_at,
      ]
    );
  }
  console.log(`  permission_audit_logs: ${rows.length} rows`);
}

async function migrateUserRoles(src: pg.PoolClient, dst: pg.PoolClient) {
  const { rows } = await src.query("SELECT * FROM user_roles ORDER BY user_id, role_id");
  for (const r of rows) {
    await dst.query(
      `INSERT INTO user_roles (user_id, role_id, assigned_at)
       VALUES ($1,$2,$3)`,
      [r.user_id, r.role_id, r.assigned_at]
    );
  }
  console.log(`  user_roles: ${rows.length} rows`);
}

async function migrateUserDepartments(src: pg.PoolClient, dst: pg.PoolClient) {
  const { rows } = await src.query("SELECT * FROM user_departments ORDER BY id");
  for (const r of rows) {
    await dst.query(
      `INSERT INTO user_departments (id, user_id, department_id, created_at)
       VALUES ($1,$2,$3,$4)`,
      [r.id, r.user_id, r.department_id, r.created_at]
    );
  }
  console.log(`  user_departments: ${rows.length} rows`);
}

async function migrateRolePermissions(src: pg.PoolClient, dst: pg.PoolClient) {
  const { rows } = await src.query("SELECT * FROM role_permissions ORDER BY role_id, resource_code, action_code");
  for (const r of rows) {
    await dst.query(
      `INSERT INTO role_permissions (role_id, resource_code, action_code, assigned_at)
       VALUES ($1,$2,$3,$4)`,
      [r.role_id, r.resource_code, r.action_code, r.assigned_at]
    );
  }
  console.log(`  role_permissions: ${rows.length} rows`);
}

// product_price_log → product_price_log_archived (same schema)
async function migrateProductPriceLogArchived(src: pg.PoolClient, dst: pg.PoolClient) {
  const { rows } = await src.query("SELECT * FROM product_price_log ORDER BY log_id");
  for (const r of rows) {
    await dst.query(
      `INSERT INTO product_price_log_archived (log_id, product_id, old_price, new_price, operator_id, remark, created_at)
       VALUES ($1,$2,$3,$4,$5,$6,$7)`,
      [
        r.log_id, r.product_id, r.old_price, r.new_price,
        r.operator_id, r.remark, r.created_at,
      ]
    );
  }
  console.log(`  product_price_log_archived: ${rows.length} rows`);
}

// product_price: derive from latest product_price_log per product
async function migrateProductPrice(src: pg.PoolClient, dst: pg.PoolClient) {
  const { rows } = await src.query(`
    SELECT DISTINCT ON (product_id)
      log_id, product_id, new_price, operator_id, remark, created_at
    FROM product_price_log
    ORDER BY product_id, created_at DESC
  `);
  let id = 1;
  for (const r of rows) {
    await dst.query(
      `INSERT INTO product_price (id, product_id, price, operator_id, remark, created_at)
       VALUES ($1,$2,$3,$4,$5,$6)`,
      [id++, r.product_id, r.new_price, r.operator_id, r.remark, r.created_at]
    );
  }
  console.log(`  product_price: ${rows.length} rows (derived from latest price_log)`);
}

// --- reset all sequences to match max(id) ---

async function resetAllSequences(dst: pg.PoolClient) {
  // Auto-discover all sequences and their owning table/column from pg_depend
  const { rows } = await dst.query(`
    SELECT
      s.relname AS seq_name,
      t.relname AS table_name,
      a.attname AS col_name
    FROM pg_class s
    JOIN pg_depend d ON d.objid = s.oid
    JOIN pg_class t ON d.refobjid = t.oid
    JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = d.refobjsubid
    WHERE s.relkind = 'S'
      AND s.relnamespace = (SELECT oid FROM pg_namespace WHERE nspname = 'public')
  `);

  for (const r of rows) {
    await dst.query(
      `SELECT setval($1, COALESCE((SELECT MAX("${r.col_name}") FROM "${r.table_name}"), 1), true)`,
      [r.seq_name]
    );
  }
  console.log(`✔ ${rows.length} 个序列已重置`);
}

// --- main ---

async function main() {
  const srcClient = await abt2.connect();
  const dstClient = await abt.connect();

  try {
    console.log("=== 开始迁移 abt2 → abt ===\n");

    // Step 1: Truncate
    console.log("[1] 清空 abt 所有表...");
    await truncateAbt(dstClient);
    console.log();

    // Step 2: Migrate base tables (no FK deps)
    console.log("[2] 迁移基础表...");
    await migrateUsers(srcClient, dstClient);
    await migrateRoles(srcClient, dstClient);
    await migrateDepartments(srcClient, dstClient);
    await migrateWarehouse(srcClient, dstClient);
    await migrateLaborProcessDict(srcClient, dstClient);
    await migrateTerms(srcClient, dstClient);
    await migrateBomCategory(srcClient, dstClient);
    console.log();

    // Step 3: Migrate products (extracts product_code/unit from meta)
    console.log("[3] 迁移产品表...");
    await migrateProducts(srcClient, dstClient);
    console.log();

    // Step 4: Migrate tables with FK deps
    console.log("[4] 迁移关联表...");
    await migrateLocation(srcClient, dstClient);
    await migrateRouting(srcClient, dstClient);
    await migrateRoutingStep(srcClient, dstClient);
    await migrateInventory(srcClient, dstClient);
    await migrateInventoryLog(srcClient, dstClient);
    await migrateBom(srcClient, dstClient);
    await migrateBomLaborProcess(srcClient, dstClient);
    await migrateBomRouting(srcClient, dstClient);
    await migrateBomNodes(srcClient, dstClient);
    await migrateTermRelation(srcClient, dstClient);
    await migratePermissionAuditLogs(srcClient, dstClient);
    await migrateUserRoles(srcClient, dstClient);
    await migrateUserDepartments(srcClient, dstClient);
    await migrateRolePermissions(srcClient, dstClient);
    console.log();

    // Step 5: Derived tables
    console.log("[5] 迁移派生表...");
    await migrateProductPriceLogArchived(srcClient, dstClient);
    await migrateProductPrice(srcClient, dstClient);
    console.log();

    // Step 6: Reset sequences
    console.log("[6] 重置序列...");
    await resetAllSequences(dstClient);
    console.log();

    console.log("=== 迁移完成 ===");
  } catch (err) {
    console.error("迁移失败:", err);
    process.exit(1);
  } finally {
    srcClient.release();
    dstClient.release();
    await abt2.end();
    await abt.end();
  }
}

main();
