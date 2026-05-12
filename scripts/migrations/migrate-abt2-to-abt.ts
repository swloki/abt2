/**
 * 增量数据迁移：从 abt2 迁移到 abt
 *
 * 使用方式：bun run scripts/migrations/migrate-abt2-to-abt.ts
 *
 * 迁移策略（增量，不破坏已有数据）：
 * - products：按 product_code 匹配，只迁移 abt 中不存在的产品
 * - bom：按 bom_name 匹配，只迁移 abt 中不存在的 BOM
 * - bom_nodes：从 abt2 的 bom.bom_detail JSONB 解析写入 abt 的 bom_nodes 表
 * - parent_id 转换：abt2 用 0 表示根节点，abt 用 NULL
 * - 整个迁移在事务中执行，失败自动回滚
 */

import { Client } from "pg";

const OLD_DB = {
  host: "127.0.0.1",
  port: 5432,
  database: "abt2",
  user: "postgres",
  password: "123456",
};

const NEW_DB = {
  host: "127.0.0.1",
  port: 5432,
  database: "abt",
  user: "postgres",
  password: "123456",
};

// abt2 bom_detail JSONB 中单个 node 的结构
interface BomNode {
  id: number;
  product_id: number;
  product_code?: string;
  quantity: number;
  parent_id: number;
  loss_rate: number;
  order: number;
  unit?: string;
  remark?: string;
  position?: string;
  work_center?: string;
  properties?: string;
}

// ============================================================================
// 迁移 products（增量）
// ============================================================================

async function migrateProducts(
  source: Client,
  target: Client,
): Promise<{ inserted: number; skipped: number }> {
  console.log("\n📦 迁移 products（增量）...");

  const result = await source.query(
    "SELECT product_id, pdt_name, meta FROM products ORDER BY product_id",
  );
  console.log(`  abt2 共 ${result.rows.length} 行`);

  // 获取 abt 中已有的 product_code 和 pdt_name 集合
  const existing = await target.query(
    "SELECT product_code, pdt_name FROM products WHERE product_code IS NOT NULL AND product_code != ''",
  );
  const existingCodes = new Set(existing.rows.map((r: { product_code: string }) => r.product_code));
  const existingNames = new Set(existing.rows.map((r: { pdt_name: string }) => r.pdt_name));
  console.log(`  abt 已有 ${existingCodes.size} 个编码`);

  let inserted = 0;
  let skipped = 0;

  for (const row of result.rows) {
    const meta = row.meta || {};
    const productCode = (meta.product_code || "").trim();
    const unit = meta.unit || "pcs";

    // 没有编码的产品不迁移
    if (!productCode) {
      skipped++;
      continue;
    }

    // 编码或名称已存在则跳过
    if (existingCodes.has(productCode) || existingNames.has(row.pdt_name)) {
      skipped++;
      continue;
    }

    await target.query(
      "INSERT INTO products (pdt_name, meta, product_code, unit) VALUES ($1, $2, $3, $4)",
      [row.pdt_name, JSON.stringify(meta), productCode, unit],
    );
    existingCodes.add(productCode);
    inserted++;
  }

  console.log(`  ✅ 新增 ${inserted}，跳过 ${skipped}`);
  return { inserted, skipped };
}

// ============================================================================
// product_id 转换辅助
// ============================================================================

interface ProductIdMapping {
  /** abt2 product_id → product_code */
  abt2IdToCode: Map<number, string>;
  /** product_code → abt product_id */
  codeToAbtId: Map<string, number>;
}

/** 构建双向映射：abt2 的 product_id → product_code → abt 的 product_id */
async function buildProductMapping(
  source: Client,
  target: Client,
): Promise<ProductIdMapping> {
  // abt2: product_id → product_code
  const abt2Rows = await source.query(
    "SELECT product_id, meta->>'product_code' AS code FROM products WHERE meta->>'product_code' IS NOT NULL AND meta->>'product_code' != ''",
  );
  const abt2IdToCode = new Map<number, string>();
  for (const r of abt2Rows.rows) {
    abt2IdToCode.set(Number(r.product_id), r.code);
  }

  // abt: product_code → product_id
  const abtRows = await target.query(
    "SELECT product_id, product_code FROM products WHERE product_code IS NOT NULL AND product_code != ''",
  );
  const codeToAbtId = new Map<string, number>();
  for (const r of abtRows.rows) {
    codeToAbtId.set(r.product_code, Number(r.product_id));
  }

  return { abt2IdToCode, codeToAbtId };
}

/**
 * 过滤节点：通过 product_code 桥接，product_id 在 abt 中找不到对应产品的节点
 * 及其所有子节点，全部丢弃
 */
function filterAndResolveNodes(
  nodes: BomNode[],
  mapping: ProductIdMapping,
): BomNode[] {
  // 递归标记无效节点
  const invalidIds = new Set<number>();
  function markInvalid(id: number) {
    invalidIds.add(id);
    for (const n of nodes) {
      if (n.parent_id === id) markInvalid(n.id);
    }
  }

  for (const n of nodes) {
    const code = mapping.abt2IdToCode.get(n.product_id);
    if (!code || !mapping.codeToAbtId.has(code)) {
      markInvalid(n.id);
    }
  }

  return nodes.filter((n) => !invalidIds.has(n.id));
}

// ============================================================================
// 迁移 bom（增量）+ 解析 bom_detail → bom_nodes
// ============================================================================

async function migrateBom(
  source: Client,
  target: Client,
  productMapping: ProductIdMapping,
): Promise<{ inserted: number; skipped: number; nodesInserted: number }> {
  console.log("\n📦 迁移 bom（增量）...");

  // 获取 abt 中已有的 bom_name 集合
  const existing = await target.query("SELECT bom_name FROM bom");
  const existingNames = new Set(existing.rows.map((r: { bom_name: string }) => r.bom_name));
  console.log(`  abt 已有 ${existingNames.size} 个 BOM`);

  const result = await source.query(
    "SELECT bom_id, bom_name, create_at, bom_detail FROM bom ORDER BY bom_id",
  );
  console.log(`  abt2 共 ${result.rows.length} 行`);

  let inserted = 0;
  let skipped = 0;
  let nodesInserted = 0;

  for (const row of result.rows) {
    if (existingNames.has(row.bom_name)) {
      skipped++;
      continue;
    }

    // 插入 bom
    const bomResult = await target.query(
      `INSERT INTO bom (bom_name, create_at, status, published_at)
       VALUES ($1, $2, 'published', $3)
       RETURNING bom_id`,
      [row.bom_name, row.create_at, row.create_at],
    );
    const newBomId: number = bomResult.rows[0].bom_id;
    inserted++;

    // 解析 bom_detail.nodes → bom_nodes
    const detail = row.bom_detail;
    const nodes: BomNode[] = detail?.nodes;
    if (!nodes || nodes.length === 0) continue;

    // 过滤：product_id 在 abt 中找不到对应产品的节点 + 子节点全部丢弃
    const filtered = filterAndResolveNodes(nodes, productMapping);
    if (filtered.length === 0) {
      console.log(`  ⚠️ BOM "${row.bom_name}" 所有节点被过滤（产品不存在）`);
      continue;
    }
    if (filtered.length < nodes.length) {
      console.log(`  ⚠️ BOM "${row.bom_name}": ${nodes.length - filtered.length}/${nodes.length} 节点被过滤`);
    }

    // 旧 node id → 新 node id 映射
    const idMap = new Map<number, number>();

    for (const node of filtered) {
      const parentId = node.parent_id === 0 ? null : (idMap.get(node.parent_id) ?? null);

      // abt2 product_id → product_code → abt product_id
      const code = productMapping.abt2IdToCode.get(node.product_id)!;
      const abtProductId = productMapping.codeToAbtId.get(code)!;

      const nodeResult = await target.query(
        `INSERT INTO bom_nodes (bom_id, product_id, product_code, quantity, parent_id, loss_rate, "order", unit, remark, position, work_center, properties)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
         RETURNING id`,
        [
          newBomId,
          abtProductId,
          code,
          Number(node.quantity) || 0,
          parentId,
          Number(node.loss_rate) || 0,
          Number(node.order) || 0,
          node.unit || null,
          node.remark || null,
          node.position || null,
          node.work_center || null,
          node.properties || null,
        ],
      );

      idMap.set(node.id, nodeResult.rows[0].id);
      nodesInserted++;
    }
  }

  console.log(`  ✅ BOM 新增 ${inserted}，跳过 ${skipped}，节点 ${nodesInserted}`);
  return { inserted, skipped, nodesInserted };
}

// ============================================================================
// 主流程
// ============================================================================

async function main() {
  console.log("=".repeat(60));
  console.log("🚀 增量迁移：abt2 → abt");
  console.log("=".repeat(60));

  const source = new Client(OLD_DB);
  const target = new Client(NEW_DB);

  try {
    await source.connect();
    console.log("✅ 已连接 abt2（只读）");
    await target.connect();
    console.log("✅ 已连接 abt（读写）");

    const startTime = Date.now();

    await target.query("BEGIN");

    try {
      const productResult = await migrateProducts(source, target);

      // 先同步产品，再构建 product_id 映射（包含新增的产品）
      const productMapping = await buildProductMapping(source, target);

      const bomResult = await migrateBom(source, target, productMapping);

      await target.query("COMMIT");
      console.log("\n✅ 事务已提交");

      const duration = ((Date.now() - startTime) / 1000).toFixed(2);
      console.log("\n" + "=".repeat(60));
      console.log(`✅ 迁移完成！耗时: ${duration}s`);
      console.log("=".repeat(60));
      console.log("\n📊 汇总:");
      console.log(`  products: 新增 ${productResult.inserted}，跳过 ${productResult.skipped}`);
      console.log(`  bom: 新增 ${bomResult.inserted}，跳过 ${bomResult.skipped}`);
      console.log(`  bom_nodes: ${bomResult.nodesInserted} 个节点`);
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
