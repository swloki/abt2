/**
 * 分类数据迁移脚本: abt → abt_v2.categories
 *
 * 迁移两个来源：
 *   1. abt.terms (taxonomy='category') — 产品分类（层级结构）
 *   2. abt.bom_category — BOM 分类（扁平结构，作为顶级节点）
 *
 * 环境变量：
 *   PGHOST=localhost PGPORT=5432 PGUSER=postgres PGPASSWORD=123456
 *   ABT_DB=abt  ABT_V2_DB=abt_v2
 *
 * 运行：bun run scripts/migrate-categories.ts
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

interface Term {
  term_id: number;
  term_name: string;
  term_parent: number;
  taxonomy: string;
  term_meta: { count: number };
}

interface BomCategory {
  bom_category_id: number;
  bom_category_name: string;
  created_at: Date;
}

async function insertCategory(
  name: string,
  parentId: number,
  meta: { count: number },
): Promise<number> {
  const result = await v2Pool.query<{ category_id: number }>(
    `INSERT INTO categories (category_name, parent_id, path, meta)
     VALUES ($1, $2, '/', $3::jsonb)
     RETURNING category_id`,
    [name, parentId, JSON.stringify(meta)]
  );
  const id = result.rows[0].category_id;
  await v2Pool.query("UPDATE categories SET path = $1 WHERE category_id = $2", [`/${id}/`, id]);
  return id;
}

async function main() {
  console.log("=== 分类数据迁移: abt → abt_v2.categories ===\n");

  // 检查目标表
  const { rows: tables } = await v2Pool.query(
    "SELECT 1 FROM pg_tables WHERE schemaname = 'public' AND tablename = 'categories'"
  );
  if (tables.length === 0) {
    console.error("错误: abt_v2 中不存在 categories 表，请先执行 008_create_categories.sql");
    process.exit(1);
  }

  // ── Part 1: 迁移 abt.terms (taxonomy='category') ─────────────────
  console.log("── Part 1: abt.terms (taxonomy='category') ──\n");

  const { rows: terms } = await abtPool.query<Term>(
    "SELECT term_id, term_name, term_parent, taxonomy, term_meta FROM terms WHERE taxonomy = 'category' ORDER BY term_id"
  );
  console.log(`源数据: ${terms.length} 条 terms 记录\n`);

  const termMapping = new Map<number, number>(); // old term_id → new category_id

  // 按 parent 排序：先插入顶级节点 (term_parent=0)，再逐级插入
  const topLevel = terms.filter(t => Number(t.term_parent) === 0);
  const childTerms = terms.filter(t => Number(t.term_parent) !== 0);

  for (const term of topLevel) {
    const newId = await insertCategory(term.term_name, Number(term.term_parent), term.term_meta || { count: 0 });
    termMapping.set(Number(term.term_id), newId);
    console.log(`  [term:${term.term_id}] ${term.term_name} → category_id=${newId}`);
  }

  // 逐级处理子节点（最多支持 10 层，防止死循环）
  let remaining = childTerms;
  for (let depth = 0; depth < 10 && remaining.length > 0; depth++) {
    const nextRound: typeof remaining = [];
    for (const term of remaining) {
      const newParentId = termMapping.get(Number(term.term_parent));
      if (newParentId === undefined) {
        nextRound.push(term);
        continue;
      }
      const newId = await insertCategory(term.term_name, newParentId, term.term_meta || { count: 0 });
      termMapping.set(Number(term.term_id), newId);
      console.log(`  [term:${term.term_id}] ${term.term_name} (parent=${newParentId}) → category_id=${newId}`);
    }
    if (nextRound.length === remaining.length) {
      console.log(`  警告: ${nextRound.length} 条 terms 的 parent_id 无法解析，跳过`);
      for (const t of nextRound) {
        console.log(`    未解析: term_id=${t.term_id} term_parent=${t.term_parent}`);
      }
      break;
    }
    remaining = nextRound;
  }

  // 迁移 term_relations → product_categories
  const { rows: relations } = await abtPool.query(
    "SELECT term_id, product_id FROM term_relation"
  );
  if (relations.length > 0) {
    let migrated = 0;
    for (const rel of relations) {
      const newCategoryId = termMapping.get(Number(rel.term_id));
      if (newCategoryId === undefined) continue;
      await v2Pool.query(
        "INSERT INTO product_categories (product_id, category_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        [rel.product_id, newCategoryId]
      );
      migrated++;
    }
    console.log(`\n  产品-分类关联: ${migrated}/${relations.length} 条迁移成功`);
  }

  // ── Part 2: 迁移 abt.bom_category ────────────────────────────────
  console.log("\n── Part 2: abt.bom_category ──\n");

  const { rows: bomCategories } = await abtPool.query<BomCategory>(
    "SELECT bom_category_id, bom_category_name, created_at FROM bom_category ORDER BY bom_category_id"
  );
  console.log(`源数据: ${bomCategories.length} 条 bom_category 记录\n`);

  const bomMapping = new Map<number, number>();
  for (const row of bomCategories) {
    const newId = await insertCategory(row.bom_category_name, 0, { count: 0 });
    bomMapping.set(row.bom_category_id, newId);
    console.log(`  [bom:${row.bom_category_id}] ${row.bom_category_name} → category_id=${newId}`);
  }

  // ── 验证 ─────────────────────────────────────────────────────────
  const { rows: checkRows } = await v2Pool.query("SELECT COUNT(*) as cnt FROM categories");
  console.log(`\n验证: abt_v2.categories 现有 ${checkRows[0].cnt} 条记录`);

  const { rows: pcRows } = await v2Pool.query("SELECT COUNT(*) as cnt FROM product_categories");
  console.log(`验证: abt_v2.product_categories 现有 ${pcRows[0].cnt} 条记录`);

  console.log("\n迁移完成。");
  await abtPool.end();
  await v2Pool.end();
}

main().catch((err) => {
  console.error("迁移失败:", err);
  process.exit(1);
});
