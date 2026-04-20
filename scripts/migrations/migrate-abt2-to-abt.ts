/**
 * 数据迁移脚本：从 abt2 迁移到 abt
 *
 * 使用方式：bun run scripts/migrations/migrate-abt2-to-abt.ts
 *
 * 迁移说明：
 * - 保留 abt 中所有用户/权限相关表（users, roles, user_roles, role_permissions,
 *   departments, user_departments, permission_audit_logs）
 * - 清空 abt 中的业务表，然后从 abt2 原样导入
 * - bom 表：abt2 有 5 列，abt 有 7 列（多 process_group_id, bom_category_id），设为 NULL
 * - 整个迁移在事务中执行，失败自动回滚
 */

import { Client } from "pg";

const ABT2_CONFIG = {
  host: "127.0.0.1",
  port: 5432,
  database: "abt2",
  user: "postgres",
  password: "123456",
};

const ABT_CONFIG = {
  host: "127.0.0.1",
  port: 5432,
  database: "abt",
  user: "postgres",
  password: "123456",
};

/**
 * 需要迁移的表及其列（按 abt2 的结构定义）
 *
 * 导入顺序 = 数组顺序（先主表后子表）
 */
const MIGRATION_TABLES = [
  {
    table: "warehouse",
    columns: ["warehouse_id", "warehouse_name", "warehouse_code", "status", "created_at", "updated_at", "deleted_at"],
  },
  {
    table: "products",
    columns: ["product_id", "pdt_name", "meta"],
  },
  {
    table: "terms",
    columns: ["term_id", "term_name", "term_parent", "term_meta", "taxonomy"],
  },
  {
    table: "location",
    columns: ["location_id", "warehouse_id", "location_code", "location_name", "capacity", "created_at", "deleted_at"],
  },
  {
    table: "term_relation",
    columns: ["term_id", "product_id"],
  },
  {
    table: "inventory",
    columns: ["inventory_id", "product_id", "location_id", "quantity", "safety_stock", "batch_no", "created_at", "updated_at"],
  },
  {
    table: "inventory_log",
    columns: ["log_id", "inventory_id", "product_id", "location_id", "change_qty", "before_qty", "after_qty", "operation_type", "ref_order_type", "ref_order_id", "operator", "remark", "created_at"],
  },
  {
    table: "bom",
    columns: ["bom_id", "bom_name", "create_at", "bom_detail", "update_at"],
    // bom 在 abt 中多了 process_group_id 和 bom_category_id，导入时设为 NULL
    targetExtraNulls: ["process_group_id", "bom_category_id"],
  },
] as const;

/**
 * 清空顺序（倒序：先子表后主表，避免外键冲突）
 */
const TRUNCATE_ORDER = [
  "inventory_log",
  "inventory",
  "term_relation",
  "location",
  "bom",
  "products",
  "terms",
  "warehouse",
];

/**
 * 需要重置序列的表及其主键列
 */
const SEQUENCE_TABLES = [
  { table: "warehouse", pk: "warehouse_id" },
  { table: "products", pk: "product_id" },
  { table: "terms", pk: "term_id" },
  { table: "location", pk: "location_id" },
  { table: "inventory", pk: "inventory_id" },
  { table: "inventory_log", pk: "log_id" },
  { table: "bom", pk: "bom_id" },
];

const BATCH_SIZE = 500;

async function migrateTable(
  source: Client,
  target: Client,
  config: (typeof MIGRATION_TABLES)[number],
): Promise<{ table: string; count: number }> {
  const { table, columns, targetExtraNulls } = config;
  const colList = columns.join(", ");
  const placeholders = columns.map((_, i) => `$${i + 1}`).join(", ");

  // 如果目标表有额外的列需要设为 NULL
  let insertCols = colList;
  let insertValues = placeholders;
  if (targetExtraNulls && targetExtraNulls.length > 0) {
    const nullCols = targetExtraNulls.join(", ");
    const nullValues = targetExtraNulls.map(() => "NULL").join(", ");
    insertCols = `${colList}, ${nullCols}`;
    insertValues = `${placeholders}, ${nullValues}`;
  }

  const selectSql = `SELECT ${colList} FROM ${table} ORDER BY ${columns[0]}`;
  const insertSql = `INSERT INTO ${table} (${insertCols}) VALUES (${insertValues})`;

  const result = await source.query(selectSql);
  const rows = result.rows;
  console.log(`  ${table}: 读取 ${rows.length} 行`);

  let inserted = 0;
  for (let i = 0; i < rows.length; i += BATCH_SIZE) {
    const batch = rows.slice(i, i + BATCH_SIZE);
    for (const row of batch) {
      const values = columns.map((col) => row[col]);
      await target.query(insertSql, values);
      inserted++;
    }
  }

  return { table, count: inserted };
}

async function resetSequences(target: Client): Promise<void> {
  console.log("\n🔄 重置序列...");
  for (const { table, pk } of SEQUENCE_TABLES) {
    const seqName = `${table}_${pk}_seq`;
    const result = await target.query<{ max_id: string | null }>(
      `SELECT MAX(${pk}) as max_id FROM ${table}`,
    );
    const maxId = result.rows[0]?.max_id;
    if (maxId !== null) {
      await target.query(
        `SELECT setval(pg_get_serial_sequence('${table}', '${pk}'), ${maxId})`,
      );
      console.log(`  ${seqName} -> ${maxId}`);
    }
  }
}

async function verifyMigration(
  source: Client,
  target: Client,
): Promise<void> {
  console.log("\n✅ 校验行数...");
  let allMatch = true;

  for (const config of MIGRATION_TABLES) {
    const { table } = config;

    const srcResult = await source.query<{ count: string }>(
      `SELECT COUNT(*) as count FROM ${table}`,
    );
    const tgtResult = await target.query<{ count: string }>(
      `SELECT COUNT(*) as count FROM ${table}`,
    );

    const srcCount = parseInt(srcResult.rows[0].count);
    const tgtCount = parseInt(tgtResult.rows[0].count);
    const match = srcCount === tgtCount;
    const icon = match ? "✅" : "❌";
    console.log(`  ${icon} ${table}: abt2=${srcCount}, abt=${tgtCount}`);

    if (!match) allMatch = false;
  }

  if (!allMatch) {
    throw new Error("行数校验不通过，请检查数据");
  }
}

async function main() {
  console.log("=".repeat(60));
  console.log("🚀 数据迁移：abt2 → abt");
  console.log("=".repeat(60));

  const source = new Client(ABT2_CONFIG);
  const target = new Client(ABT_CONFIG);

  try {
    await source.connect();
    console.log("✅ 已连接 abt2（只读）");
    await target.connect();
    console.log("✅ 已连接 abt（读写）");

    const startTime = Date.now();

    // 在 abt 中执行整个迁移（事务）
    await target.query("BEGIN");

    try {
      // 1. 清空业务表
      console.log("\n🗑️ 清空 abt 业务表...");
      for (const table of TRUNCATE_ORDER) {
        await target.query(`TRUNCATE TABLE ${table} CASCADE`);
        console.log(`  ✅ TRUNCATE ${table}`);
      }

      // 2. 按顺序导入数据
      console.log("\n📦 导入数据...");
      const results: { table: string; count: number }[] = [];
      for (const config of MIGRATION_TABLES) {
        const result = await migrateTable(source, target, config);
        results.push(result);
      }

      // 3. 重置序列
      await resetSequences(target);

      await target.query("COMMIT");
      console.log("\n✅ 事务已提交");

      // 4. 校验
      await verifyMigration(source, target);

      const duration = ((Date.now() - startTime) / 1000).toFixed(2);
      console.log("\n" + "=".repeat(60));
      console.log(`✅ 迁移完成！耗时: ${duration}s`);
      console.log("=".repeat(60));

      // 汇总
      console.log("\n📊 迁移汇总:");
      for (const r of results) {
        console.log(`  ${r.table}: ${r.count} 行`);
      }
    } catch (err) {
      await target.query("ROLLBACK");
      console.error("\n❌ 迁移失败，已回滚:", err);
      process.exit(1);
    }
  } catch (error) {
    console.error("\n❌ 连接失败:", error);
    process.exit(1);
  } finally {
    await source.end();
    await target.end();
  }
}

main();
