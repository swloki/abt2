/**
 * 插入两款产品到 abt（数据来自 abt2）
 *   bun run scripts/sync-two-products.ts
 */

import pg from "pg";

const DATABASE_URL = "postgres://user_cC5B3h:password_TJWBYK@127.0.0.1:5432/abt";
const pool = new pg.Pool({ connectionString: DATABASE_URL });

// ─── 硬编码数据（来自 abt2） ─────────────────────────────────────────

const PRODUCTS = [
  {
    pdt_name: "灯珠/2835/白0.5W铜/3V/13000K/65-70LM/KM12133555",
    unit: "PCS",
    meta: {
      product_code: "x1777873062",
      unit: "PCS",
      specification: "",
      acquire_channel: "采购",
    },
  },
  {
    pdt_name: "灯珠/2835/暖白0.5W铜/3V/3000K/65-70LM/KM11213414",
    unit: "PCS",
    meta: {
      product_code: "x1777873075",
      unit: "PCS",
      specification: "",
      acquire_channel: "采购",
    },
  },
];

// ─── 主流程 ──────────────────────────────────────────────────────────

async function main() {
  const client = await pool.connect();
  try {
    await client.query("BEGIN");

    for (const p of PRODUCTS) {
      const code = p.meta.product_code;

      // 按 meta 里的 product_code 查重
      const { rows: exists } = await client.query(
        `SELECT product_id FROM products WHERE meta->>'product_code' = $1 LIMIT 1`,
        [code]
      );
      if (exists.length > 0) {
        console.log(`已存在: "${p.pdt_name}" (id=${exists[0].product_id})`);
        continue;
      }

      const { rows: inserted } = await client.query(
        `INSERT INTO products (pdt_name, meta, unit) VALUES ($1,$2::jsonb,$3) RETURNING product_id`,
        [p.pdt_name, JSON.stringify(p.meta), p.unit]
      );
      console.log(`新增: "${p.pdt_name}" (id=${inserted[0].product_id})`);
    }

    await client.query("COMMIT");
    console.log("\n=== 完成 ===");
  } catch (err) {
    console.error("失败:", err);
    try { await client.query("ROLLBACK"); } catch { /* ignore */ }
    process.exit(1);
  } finally {
    client.release();
    await pool.end();
  }
}

main();
