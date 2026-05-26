/**
 * 根据 BOM 的工序自动关联分类
 *
 * 逻辑：
 *   1. 查找每个 BOM 根节点的 product_code
 *   2. 根据 product_code 在 bom_labor_process 中查找工序
 *   3. 如果工序名称包含"电源"→ 分类为"电源"
 *      如果工序名称包含"模组"→ 分类为"模组"
 *   4. 更新 bom.bom_category_id
 *
 * 用法：
 *   PGDATABASE=abt bun run scripts/classify-bom-by-process.ts
 *
 * 环境变量：
 *   PGHOST, PGPORT, PGUSER, PGPASSWORD, PGDATABASE
 */

import pg from "pg";

const HOST = process.env.PGHOST ?? "localhost";
const PORT = parseInt(process.env.PGPORT ?? "5432", 10);
const USER = process.env.PGUSER ?? "user_cC5B3h";
const PASS = process.env.PGPASSWORD ?? "password_TJWBYK";
const DB   = process.env.PGDATABASE ?? "abt2";

const pool = new pg.Pool({ host: HOST, port: PORT, user: USER, password: PASS, database: DB });

interface BomRow {
  bom_id: number;
  bom_name: string;
  product_code: string | null;
  current_category: string | null;
}

interface CategoryRow {
  bom_category_id: number;
  bom_category_name: string;
}

async function main() {
  const client = await pool.connect();

  try {
    // 1. 获取分类 ID
    const { rows: categories } = await client.query<CategoryRow>(
      "SELECT bom_category_id, bom_category_name FROM bom_category"
    );
    const categoryMap = new Map(categories.map((c) => [c.bom_category_name, c.bom_category_id]));

    const powerId = categoryMap.get("电源");
    const moduleId = categoryMap.get("模组");

    if (!powerId) {
      console.error("未找到分类「电源」，请先在 bom_category 表中创建");
      process.exit(1);
    }
    if (!moduleId) {
      console.error("未找到分类「模组」，请先在 bom_category 表中创建");
      process.exit(1);
    }

    console.log(`分类ID — 电源: ${powerId}, 模组: ${moduleId}`);

    // 2. 查找每个 BOM 根节点的 product_code
    //    根节点: parent_id IS NULL
    //    bom_nodes.product_code 大部分为 NULL，需 JOIN products 表获取
    //    用 DISTINCT ON 防止一个 BOM 有多个根节点时重复
    const { rows: boms } = await client.query<BomRow>(`
      SELECT DISTINCT ON (b.bom_id)
        b.bom_id,
        b.bom_name,
        COALESCE(bn.product_code, p.product_code) AS product_code,
        bc.bom_category_name AS current_category
      FROM bom b
      JOIN bom_nodes bn ON bn.bom_id = b.bom_id AND bn.parent_id IS NULL
      JOIN products p ON p.product_id = bn.product_id
      LEFT JOIN bom_category bc ON bc.bom_category_id = b.bom_category_id
      ORDER BY b.bom_id, bn.id
    `);

    console.log(`\n共找到 ${boms.length} 个 BOM\n`);

    let updated = 0;
    let skipped = 0;
    let conflict = 0;

    await client.query("BEGIN");

    for (const bom of boms) {
      if (!bom.product_code) {
        console.log(`  [跳过] BOM#${bom.bom_id} "${bom.bom_name}" — 无根节点 product_code`);
        skipped++;
        continue;
      }

      // 3. 查找该 product_code 的工序
      const { rows: processes } = await client.query<{ name: string }>(
        "SELECT DISTINCT name FROM bom_labor_process WHERE product_code = $1",
        [bom.product_code]
      );

      if (processes.length === 0) {
        console.log(`  [跳过] BOM#${bom.bom_id} "${bom.bom_name}" — 无工序数据`);
        skipped++;
        continue;
      }

      const hasPower = processes.some((p) => p.name.includes("电源"));
      const hasModule = processes.some((p) => p.name.includes("模组"));

      // 4. 判断分类
      let targetCategory: string | null = null;
      let targetId: number | null = null;

      if (hasPower && hasModule) {
        console.log(`  [冲突] BOM#${bom.bom_id} "${bom.bom_name}" — 同时包含电源和模组工序，跳过`);
        conflict++;
        continue;
      } else if (hasPower) {
        targetCategory = "电源";
        targetId = powerId;
      } else if (hasModule) {
        targetCategory = "模组";
        targetId = moduleId;
      }

      if (!targetCategory || !targetId) {
        console.log(`  [跳过] BOM#${bom.bom_id} "${bom.bom_name}" — 工序不匹配电源/模组`);
        skipped++;
        continue;
      }

      // 5. 更新
      if (bom.current_category === targetCategory) {
        console.log(`  [已是] BOM#${bom.bom_id} "${bom.bom_name}" — 已分类为「${targetCategory}」`);
        continue;
      }

      await client.query(
        "UPDATE bom SET bom_category_id = $1 WHERE bom_id = $2",
        [targetId, bom.bom_id]
      );
      console.log(`  [更新] BOM#${bom.bom_id} "${bom.bom_name}" — ${bom.current_category ?? "未分类"} → ${targetCategory}`);
      updated++;
    }

    await client.query("COMMIT");

    console.log(`\n=== 完成 ===`);
    console.log(`更新: ${updated}, 跳过: ${skipped}, 冲突: ${conflict}, 总计: ${boms.length}`);
  } catch (e) {
    await client.query("ROLLBACK");
    throw e;
  } finally {
    client.release();
    await pool.end();
  }
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
