/**
 * 清除工艺路线 + 人工成本数据
 *
 * 涉及表：
 *   bom_labor_process  — BOM 人工工序（人工成本）
 *   bom_routing        — BOM → 工艺路线映射
 *   routing_step       — 工艺路线工序明细
 *   routing            — 工艺路线
 *   labor_process_dict — 工序字典（不删除，保留字典数据）
 *
 * 用法：
 *   PGDATABASE=abt bun run scripts/cleanup-routing-labor.ts
 *
 * 环境变量：
 *   PGHOST, PGPORT, PGUSER, PGPASSWORD, PGDATABASE
 */

import pg from "pg";

// ─── 数据库配置 ─────────────────────────────────────────────────────

const HOST = process.env.PGHOST ?? "localhost";
const PORT = parseInt(process.env.PGPORT ?? "5432", 10);
const USER = process.env.PGUSER ?? "user_cC5B3h";
const PASS = process.env.PGPASSWORD ?? "password_TJWBYK";
const DB   = process.env.PGDATABASE ?? "abt";

const pool = new pg.Pool({ host: HOST, port: PORT, user: USER, password: PASS, database: DB });

// ─── 主流程 ─────────────────────────────────────────────────────────

async function main() {
  const client = await pool.connect();

  try {
    // 检查表是否存在
    const tables = ["bom_labor_process", "bom_routing", "routing_step", "routing"];
    const existing: string[] = [];
    for (const t of tables) {
      const { rows } = await client.query(
        `SELECT 1 FROM pg_tables WHERE schemaname = 'public' AND tablename = $1`, [t]
      );
      if (rows.length > 0) existing.push(t);
    }
    console.log(`已存在的表: ${existing.join(", ")}`);

    // 统计
    console.log("\n=== 清除前统计 ===");
    for (const t of existing) {
      const { rows } = await client.query(`SELECT count(*)::int AS n FROM ${t}`);
      console.log(`  ${t}: ${rows[0].n} 行`);
    }

    // 清除（按外键依赖顺序）
    console.log("\n=== 清除 ===");
    const clearOrder = ["bom_labor_process", "bom_routing", "routing_step", "routing"];
    for (const t of clearOrder) {
      if (!existing.includes(t)) continue;
      const { rowCount } = await client.query(`DELETE FROM ${t}`);
      console.log(`  ${t}: 删除 ${rowCount} 行`);
    }

    // 重置序列
    console.log("\n=== 重置序列 ===");
    for (const t of clearOrder) {
      if (!existing.includes(t)) continue;
      await client.query(`ALTER SEQUENCE ${t}_id_seq RESTART WITH 1`);
      console.log(`  ${t}_id_seq → 1`);
    }

    console.log("\n完成。工序字典 (labor_process_dict) 未清除。");
  } finally {
    client.release();
  }
}

main()
  .then(() => process.exit(0))
  .catch((e) => {
    console.error("错误:", e);
    process.exit(1);
  })
  .finally(() => pool.end());
