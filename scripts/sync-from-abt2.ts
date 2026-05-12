/**
 * abt2 → abt 增量同步脚本
 *
 * 只同步不存的数据。数据库名通过环境变量配置：
 *   ABT_OLD_DB=abt2  ABT_NEW_DB=abt  PGHOST=localhost PGPORT=5432 PGUSER=postgres PGPASSWORD=123456
 *   bun run scripts/sync-from-abt2.ts
 *
 * 同步范围：产品表、BOM 表（含节点、工序、工艺路线）、分类、价格
 */

import pg from "pg";

// ─── 数据库配置（环境变量） ────────────────────────────────────────

const HOST = process.env.PGHOST ?? "localhost";
const PORT = parseInt(process.env.PGPORT ?? "5432", 10);
const USER = process.env.PGUSER ?? "postgres";
const PASS = process.env.PGPASSWORD ?? "123456";
const ABT_OLD_DB = process.env.ABT_OLD_DB ?? "abt2";
const ABT_NEW_DB = process.env.ABT_NEW_DB ?? "abt";

const abt2 = new pg.Pool({ host: HOST, port: PORT, user: USER, password: PASS, database: ABT_OLD_DB });
const abt  = new pg.Pool({ host: HOST, port: PORT, user: USER, password: PASS, database: ABT_NEW_DB });

// ─── 辅助 ─────────────────────────────────────────────────────────

function log(step: string, msg: string) {
  console.log(`  ${step}: ${msg}`);
}

async function tableExists(client: pg.PoolClient, table: string): Promise<boolean> {
  const { rows } = await client.query(
    `SELECT 1 FROM pg_tables WHERE schemaname = 'public' AND tablename = $1`, [table]
  );
  return rows.length > 0;
}

function extractProductMeta(meta: any) {
  const m = meta ?? {};
  return {
    productCode: m.product_code ?? "",
    unit: m.unit ?? "",
    specification: m.specification ?? "",
    acquireChannel: m.acquire_channel ?? "",
    oldCode: m.old_code ?? null,
  };
}

/** 查找 abt 中已存在的产品的 product_id，按 product_code 优先，其次 pdt_name */
async function findExistingProduct(client: pg.PoolClient, code: string, name: string): Promise<number | null> {
  if (code) {
    const { rows } = await client.query(
      `SELECT product_id FROM products WHERE product_code = $1 LIMIT 1`, [code]
    );
    if (rows.length > 0) return rows[0].product_id as number;
  }
  if (name) {
    const { rows } = await client.query(
      `SELECT product_id FROM products WHERE pdt_name = $1 LIMIT 1`, [name]
    );
    if (rows.length > 0) return rows[0].product_id as number;
  }
  return null;
}

/** 将 BIGSERIAL 序列推进到表当前最大值，防止后续自增冲突 */
async function resetSequence(dst: pg.PoolClient, table: string, column: string) {
  const seqName = `${table}_${column}_seq`;
  const { rows } = await dst.query(
    `SELECT COALESCE(MAX(${column}), 0) AS max_id FROM ${table}`
  );
  const maxId = rows[0].max_id as number;
  await dst.query(`SELECT setval($1, $2)`, [seqName, maxId]);
  log("  sequence", `${seqName} → ${maxId}`);
}

// ─── 同步函数 ──────────────────────────────────────────────────────

async function syncBomCategory(src: pg.PoolClient, dst: pg.PoolClient) {
  const { rows } = await src.query(`SELECT * FROM bom_category ORDER BY bom_category_id`);
  let added = 0;
  for (const r of rows) {
    const { rows: exists } = await dst.query(
      `SELECT 1 FROM bom_category WHERE bom_category_name = $1 LIMIT 1`, [r.bom_category_name]
    );
    if (exists.length > 0) continue;
    // 不传 bom_category_id，让 BIGSERIAL 自动生成，避免序列不同步
    await dst.query(
      `INSERT INTO bom_category (bom_category_name, created_at)
       VALUES ($1,$2)`,
      [r.bom_category_name, r.created_at]
    );
    added++;
  }
  log("bom_category", `共 ${rows.length} 条，新增 ${added} 条`);
}

async function syncTerms(src: pg.PoolClient, dst: pg.PoolClient): Promise<Map<number, number>> {
  const { rows } = await src.query(`SELECT * FROM terms WHERE taxonomy = 'category' ORDER BY term_id`);
  let added = 0;
  const idMap = new Map<number, number>(); // abt2.term_id → abt.term_id

  for (const r of rows) {
    const { rows: exists } = await dst.query(
      `SELECT term_id FROM terms WHERE term_name = $1 AND taxonomy = $2 LIMIT 1`,
      [r.term_name, r.taxonomy ?? "category"]
    );
    if (exists.length > 0) {
      idMap.set(r.term_id, exists[0].term_id as number);
      continue;
    }
    const termMeta = r.term_meta ? JSON.stringify(r.term_meta) : "{}";
    const { rows: inserted } = await dst.query(
      `INSERT INTO terms (term_name, term_parent, term_meta, taxonomy)
       VALUES ($1,$2,$3::jsonb,$4) RETURNING term_id`,
      [r.term_name, r.term_parent ?? 0, termMeta, r.taxonomy ?? "category"]
    );
    idMap.set(r.term_id, inserted[0].term_id as number);
    added++;
  }
  log("terms", `共 ${rows.length} 条，新增 ${added} 条`);
  return idMap;
}

async function syncProducts(src: pg.PoolClient, dst: pg.PoolClient): Promise<Map<number, number>> {
  const { rows } = await src.query(`SELECT * FROM products ORDER BY product_id`);
  let added = 0, existed = 0;
  const idMap = new Map<number, number>(); // abt2.product_id → abt.product_id

  for (const r of rows) {
    const { productCode, unit, specification, acquireChannel, oldCode } = extractProductMeta(r.meta);

    // 无论是否有 productCode，都先尝试在目标库查找已存在的产品
    const existingId = await findExistingProduct(dst, productCode, r.pdt_name);
    if (existingId !== null) {
      idMap.set(r.product_id, existingId);
      existed++;
      continue;
    }

    // 没有 productCode 时生成唯一占位码，加上时间戳防冲突
    const code = productCode || `SYNC-${r.product_id}-${Date.now()}`;

    const cleanMeta: Record<string, any> = {};
    if (specification) cleanMeta.specification = specification;
    if (acquireChannel) cleanMeta.acquire_channel = acquireChannel;
    if (oldCode) cleanMeta.old_code = oldCode;

    const { rows: inserted } = await dst.query(
      `INSERT INTO products (pdt_name, meta, product_code, unit)
       VALUES ($1,$2::jsonb,$3,$4) RETURNING product_id`,
      [r.pdt_name, JSON.stringify(cleanMeta), code, unit || "pcs"]
    );
    idMap.set(r.product_id, inserted[0].product_id);
    added++;
  }
  log("products", `共 ${rows.length} 条，新增 ${added} 条，已存在 ${existed} 条`);
  return idMap;
}

async function syncTermRelation(src: pg.PoolClient, dst: pg.PoolClient, productIdMap: Map<number, number>, termIdMap: Map<number, number>) {
  const { rows } = await src.query(`
    SELECT tr.term_id, tr.product_id
    FROM term_relation tr
    JOIN products p ON p.product_id = tr.product_id
    ORDER BY tr.term_id, tr.product_id
  `);
  let added = 0, skipped = 0;
  for (const r of rows) {
    const abtProductId = productIdMap.get(r.product_id);
    if (!abtProductId) { skipped++; continue; }

    const abtTermId = termIdMap.get(r.term_id) ?? r.term_id;

    const { rows: termOk } = await dst.query(
      `SELECT 1 FROM terms WHERE term_id = $1 LIMIT 1`, [abtTermId]
    );
    if (termOk.length === 0) { skipped++; continue; }

    const { rows: exists } = await dst.query(
      `SELECT 1 FROM term_relation WHERE term_id = $1 AND product_id = $2 LIMIT 1`,
      [abtTermId, abtProductId]
    );
    if (exists.length > 0) continue;

    await dst.query(
      `INSERT INTO term_relation (term_id, product_id) VALUES ($1,$2)`,
      [abtTermId, abtProductId]
    );
    added++;
  }
  log("term_relation", `新增 ${added} 条，无映射 ${skipped} 条`);
}

async function syncBom(src: pg.PoolClient, dst: pg.PoolClient, catIdMap: Map<number, number>): Promise<Map<number, number>> {
  const { rows } = await src.query(`SELECT * FROM bom ORDER BY bom_id`);
  let added = 0;
  const idMap = new Map<number, number>();

  for (const r of rows) {
    if (!r.bom_name) continue;
    const { rows: existing } = await dst.query(
      `SELECT bom_id FROM bom WHERE bom_name = $1 LIMIT 1`, [r.bom_name]
    );
    if (existing.length > 0) {
      idMap.set(r.bom_id, existing[0].bom_id);
      continue;
    }

    // 映射 bom_category_id（通过 name 中转：abt2.id → name → abt.id），映射不到则置空
    const mappedCategoryId = r.bom_category_id ? (catIdMap.get(r.bom_category_id) ?? null) : null;

    const bomDetail = r.bom_detail ? JSON.stringify(r.bom_detail) : null;
    const { rows: inserted } = await dst.query(
      `INSERT INTO bom (bom_name, create_at, update_at, bom_category_id, created_by, status, published_at, bom_detail)
       VALUES ($1,$2,$3,$4,$5,$6,$7,$8::jsonb) RETURNING bom_id`,
      [r.bom_name, r.create_at, r.update_at, mappedCategoryId, r.created_by, r.status ?? "published", r.published_at ?? r.create_at, bomDetail]
    );
    idMap.set(r.bom_id, inserted[0].bom_id);
    added++;
  }
  log("bom", `共 ${rows.length} 条，新增 ${added} 条`);
  return idMap;
}

async function syncBomNodes(src: pg.PoolClient, dst: pg.PoolClient, productIdMap: Map<number, number>, bomIdMap: Map<number, number>) {
  const { rows } = await src.query(
    `SELECT bom_id, bom_name, bom_detail FROM bom WHERE bom_detail IS NOT NULL ORDER BY bom_id`
  );
  let totalNodes = 0;
  let totalBoms = 0;

  for (const bom of rows) {
    const mappedBomId = bomIdMap.get(bom.bom_id);
    if (!mappedBomId) continue;

    const { rows: existingNodes } = await dst.query(
      `SELECT 1 FROM bom_nodes WHERE bom_id = $1 LIMIT 1`, [mappedBomId]
    );
    if (existingNodes.length > 0) continue;

    const detail = bom.bom_detail;
    const nodes: any[] = detail?.nodes ?? [];
    if (nodes.length === 0) continue;

    // 两阶段插入：先插入所有节点收集 ID 映射，再统一更新 parent_id
    const idMap = new Map<number, number>(); // old node id → new node id
    const pendingParents: { newId: number; oldParentId: number }[] = [];

    for (const node of nodes) {
      const oldId = node.id as number;
      const oldParentId = node.parent_id as number;
      const mappedProductId = productIdMap.get(node.product_id) ?? node.product_id;

      const insertRes = await dst.query(
        `INSERT INTO bom_nodes (bom_id, product_id, product_code, quantity, parent_id, loss_rate, "order", unit, remark, position, work_center, properties)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12) RETURNING id`,
        [
          mappedBomId, mappedProductId, node.product_code ?? null,
          node.quantity, null, node.loss_rate ?? 0,
          node.order ?? 0, node.unit ?? null,
          node.remark || null, node.position || null,
          node.work_center || null, node.properties ?? null,
        ]
      );
      const newId = insertRes.rows[0].id as number;
      idMap.set(oldId, newId);
      totalNodes++;

      if (oldParentId && oldParentId !== 0) {
        pendingParents.push({ newId, oldParentId });
      }
    }

    // 统一更新 parent_id（此时所有节点都已插入，idMap 完整）
    for (const { newId, oldParentId } of pendingParents) {
      const mappedParentId = idMap.get(oldParentId);
      if (mappedParentId) {
        await dst.query(
          `UPDATE bom_nodes SET parent_id = $1 WHERE id = $2`,
          [mappedParentId, newId]
        );
      }
    }

    totalBoms++;
  }
  log("bom_nodes", `${totalNodes} 条（来自 ${totalBoms} 个 BOM）`);
}

async function syncBomLaborProcess(src: pg.PoolClient, dst: pg.PoolClient) {
  const { rows } = await src.query(`SELECT * FROM bom_labor_process ORDER BY id`);
  let added = 0;
  for (const r of rows) {
    const { rows: exists } = await dst.query(
      `SELECT 1 FROM bom_labor_process WHERE product_code = $1 AND name = $2 LIMIT 1`,
      [r.product_code, r.name]
    );
    if (exists.length > 0) continue;

    // 不传 id，让 BIGSERIAL 自动生成
    await dst.query(
      `INSERT INTO bom_labor_process (product_code, name, unit_price, quantity, sort_order, remark, created_at, updated_at, process_code)
       VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)`,
      [
        r.product_code, r.name, r.unit_price,
        r.quantity, r.sort_order, r.remark, r.created_at, r.updated_at,
        r.process_code,
      ]
    );
    added++;
  }
  log("bom_labor_process", `共 ${rows.length} 条，新增 ${added} 条`);
}

async function syncBomRouting(src: pg.PoolClient, dst: pg.PoolClient) {
  const { rows } = await src.query(`SELECT * FROM bom_routing ORDER BY id`);
  let added = 0, updated = 0;
  for (const r of rows) {
    // 用 UPSERT 处理 UNIQUE(product_code) 约束
    const { rows: result } = await dst.query(
      `INSERT INTO bom_routing (product_code, routing_id, created_at, updated_at)
       VALUES ($1,$2,$3,$4)
       ON CONFLICT (product_code) DO UPDATE SET routing_id = EXCLUDED.routing_id, updated_at = EXCLUDED.updated_at
       RETURNING (xmax = 0) AS is_insert`,
      [r.product_code, r.routing_id, r.created_at, r.updated_at]
    );
    if (result[0].is_insert) {
      added++;
    } else {
      updated++;
    }
  }
  log("bom_routing", `共 ${rows.length} 条，新增 ${added} 条，更新 ${updated} 条`);
}

async function syncProductPrice(src: pg.PoolClient, dst: pg.PoolClient, productIdMap: Map<number, number>) {
  const { rows } = await src.query(`
    SELECT DISTINCT ON (product_id)
      log_id, product_id, new_price, operator_id, remark, created_at
    FROM product_price_log
    ORDER BY product_id, created_at DESC
  `);
  let added = 0, skipped = 0;
  for (const r of rows) {
    const dstProductId = productIdMap.get(r.product_id);
    if (!dstProductId) { skipped++; continue; }

    const { rows: exists } = await dst.query(
      `SELECT 1 FROM product_price WHERE product_id = $1 LIMIT 1`, [dstProductId]
    );
    if (exists.length > 0) continue;

    // 不传 id，让 BIGSERIAL 自动生成
    await dst.query(
      `INSERT INTO product_price (product_id, price, operator_id, remark, created_at)
       VALUES ($1,$2,$3,$4,$5)`,
      [dstProductId, r.new_price, r.operator_id, r.remark, r.created_at]
    );
    added++;
  }
  log("product_price", `新增 ${added} 条，无映射 ${skipped} 条`);
}

// ─── 主流程 ──────────────────────────────────────────────────────

async function main() {
  const src = await abt2.connect();
  const dst = await abt.connect();

  try {
    console.log(`\n=== 增量同步 ${ABT_OLD_DB} → ${ABT_NEW_DB} ===\n`);

    // 开启目标库事务，失败时整体回滚
    await dst.query("BEGIN");

    // 1. 字典表 / 基础数据
    console.log("[1] 基础数据...");
    const bomCategoryIdMap = new Map<number, number>(); // abt2.category_id → abt.category_id
    if (await tableExists(src, "bom_category")) {
      await syncBomCategory(src, dst);
      // 同步完成后构建 id 映射（通过 name 中转：abt2.id → name → abt.id）
      const { rows: srcCats } = await src.query(`SELECT bom_category_id, bom_category_name FROM bom_category`);
      const { rows: dstCats } = await dst.query(`SELECT bom_category_id, bom_category_name FROM bom_category`);
      const dstNameToId = new Map<string, number>();
      for (const c of dstCats) dstNameToId.set(c.bom_category_name, c.bom_category_id);
      for (const c of srcCats) {
        const dstId = dstNameToId.get(c.bom_category_name);
        if (dstId !== undefined) bomCategoryIdMap.set(c.bom_category_id, dstId);
      }
    } else {
      log("bom_category", "源库无此表，跳过");
    }
    let termIdMap = new Map<number, number>();
    if (await tableExists(src, "terms")) {
      termIdMap = await syncTerms(src, dst);
    } else {
      log("terms", "源库无此表，跳过");
    }
    console.log();

    // 2. 产品
    console.log("[2] 产品...");
    const productIdMap = await syncProducts(src, dst);
    if (await tableExists(src, "term_relation")) {
      await syncTermRelation(src, dst, productIdMap, termIdMap);
    } else {
      log("term_relation", "源库无此表，跳过");
    }
    if (await tableExists(src, "product_price_log")) {
      await syncProductPrice(src, dst, productIdMap);
    } else {
      log("product_price", "源库无 product_price_log，跳过");
    }
    console.log();

    // 3. BOM
    console.log("[3] BOM...");
    const bomIdMap = await syncBom(src, dst, bomCategoryIdMap);
    await syncBomNodes(src, dst, productIdMap, bomIdMap);
    if (await tableExists(src, "bom_labor_process")) {
      await syncBomLaborProcess(src, dst);
    } else {
      log("bom_labor_process", "源库无此表，跳过");
    }
    if (await tableExists(src, "bom_routing")) {
      await syncBomRouting(src, dst);
    } else {
      log("bom_routing", "源库无此表，跳过");
    }
    console.log();

    // 4. 重置所有 BIGSERIAL 序列到当前最大值
    console.log("[4] 重置序列...");
    await resetSequence(dst, "bom_category", "bom_category_id");
    await resetSequence(dst, "bom_labor_process", "id");
    await resetSequence(dst, "bom_routing", "id");
    await resetSequence(dst, "product_price", "id");
    console.log();

    await dst.query("COMMIT");
    console.log("=== 同步完成 ===\n");
  } catch (err) {
    console.error("同步失败:", err);
    try { await dst.query("ROLLBACK"); } catch { /* ignore */ }
    process.exit(1);
  } finally {
    src.release();
    dst.release();
    await abt2.end();
    await abt.end();
  }
}

main();
