/**
 * 从源数据库导出 labor_process_dict，插入到目标数据库
 *
 * 用法：
 *   # 导出为 JSON
 *   bun run scripts/export-labor-dict.ts --export
 *
 *   # 从 JSON 导入到目标库
 *   bun run scripts/export-labor-dict.ts --import
 *
 * 环境变量（导出时使用源库，导入时使用目标库）：
 *   PGHOST, PGPORT, PGUSER, PGPASSWORD, PGDATABASE
 *   默认: localhost:5432/postgres/123456/abt
 *
 * 也可以指定远程库导出 + 本地库导入：
 *   PGHOST=远程IP bun run scripts/export-labor-dict.ts --export
 *   bun run scripts/export-labor-dict.ts --import
 */

import pg from "pg";
import fs from "fs";
import path from "path";

// 导出连 abt2（服务端），导入连 abt（本地）
const HOST = process.env.PGHOST ?? "localhost";
const PORT = parseInt(process.env.PGPORT ?? "5432", 10);

// 服务端凭据（导出用）
const EXPORT_USER = process.env.EXPORT_USER ?? "user_cC5B3h";
const EXPORT_PASS = process.env.EXPORT_PASS ?? "password_TJWBYK";
const DB_EXPORT = process.env.DB_EXPORT ?? "abt2";

// 本地凭据（导入用）
const IMPORT_USER = process.env.IMPORT_USER ?? "postgres";
const IMPORT_PASS = process.env.IMPORT_PASS ?? "123456";
const DB_IMPORT = process.env.DB_IMPORT ?? "abt";

const OUT_FILE = path.resolve("scripts/labor_process_dict.json");

interface DictRow {
  code: string;
  name: string;
  description: string | null;
  sort_order: number;
}

function getExportPool() {
  return new pg.Pool({ host: HOST, port: PORT, user: EXPORT_USER, password: EXPORT_PASS, database: DB_EXPORT });
}

function getImportPool() {
  return new pg.Pool({ host: HOST, port: PORT, user: IMPORT_USER, password: IMPORT_PASS, database: DB_IMPORT });
}

async function doExport() {
  const pool = getExportPool();
  const { rows } = await pool.query<DictRow>(
    "SELECT code, name, description, sort_order FROM labor_process_dict ORDER BY sort_order, code"
  );

  fs.mkdirSync(path.dirname(OUT_FILE), { recursive: true });
  fs.writeFileSync(OUT_FILE, JSON.stringify(rows, null, 2), "utf-8");
  console.log(`导出 ${rows.length} 条工序字典 → ${OUT_FILE}`);
  console.log(rows.map(r => `  ${r.code} ${r.name}`).join("\n"));
  await pool.end();
}

async function doImport() {
  if (!fs.existsSync(OUT_FILE)) {
    console.error(`文件不存在: ${OUT_FILE}`);
    console.error("请先运行 --export 导出数据");
    process.exit(1);
  }

  const rows: DictRow[] = JSON.parse(fs.readFileSync(OUT_FILE, "utf-8"));
  if (rows.length === 0) {
    console.log("JSON 为空，无数据导入");
    return;
  }

  const pool = getImportPool();
  const client = await pool.connect();

  try {
    await client.query("BEGIN");

    // 清空目标表
    await client.query("DELETE FROM labor_process_dict");
    await client.query("ALTER SEQUENCE labor_process_dict_id_seq RESTART WITH 1");

    let inserted = 0;
    for (const row of rows) {
      await client.query(
        `INSERT INTO labor_process_dict (code, name, description, sort_order)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (code) DO NOTHING`,
        [row.code, row.name, row.description, row.sort_order]
      );
      inserted++;
    }

    // 重置序列到最大 id
    await client.query(
      `SELECT setval('labor_process_dict_id_seq', COALESCE((SELECT MAX(id) FROM labor_process_dict), 1))`
    );

    await client.query("COMMIT");
    console.log(`导入 ${inserted} 条工序字典完成`);
  } catch (e) {
    await client.query("ROLLBACK");
    throw e;
  } finally {
    client.release();
    await pool.end();
  }
}

const mode = process.argv[2];
if (mode === "--export") {
  doExport().catch(e => { console.error("导出失败:", e); process.exit(1); });
} else if (mode === "--import") {
  doImport().catch(e => { console.error("导入失败:", e); process.exit(1); });
} else {
  console.log("用法:");
  console.log("  bun run scripts/export-labor-dict.ts --export   # 从当前库导出到 JSON");
  console.log("  bun run scripts/export-labor-dict.ts --import   # 从 JSON 导入到当前库");
  process.exit(1);
}
