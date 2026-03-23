/**
 * 数据迁移脚本：从 abt_real 迁移到 abt
 *
 * 使用方式：bun run migrate-from-abt-real.ts
 *
 * 迁移说明：
 * 1. warehouse: 从 abt_real terms 表中 taxonomy='warehouse' 且 term_parent=0 的记录
 * 2. location: 从 abt_real terms 表中 taxonomy='warehouse' 且 term_parent!=0 的记录
 * 3. products: 需要转换 meta 结构（移除 warehouses, price, quantity, storage_location）
 * 4. inventory: 从 products.meta.warehouses 转换
 * 5. terms: 直接迁移（分类）
 * 6. bom: 直接迁移
 * 7. term_relation: 直接迁移
 */

import { Client } from "pg";

// 数据库配置
const ABT_REAL_CONFIG = {
  host: "localhost",
  port: 5432,
  database: "abt_real",
  user: "postgres",
  password: "123456",
};

const ABT_CONFIG = {
  host: "localhost",
  port: 5432,
  database: "abt",
  user: "postgres",
  password: "123456",
};

// ============================================================================
// 类型定义
// ============================================================================

// 旧版 products.meta 结构 (abt_real)
interface OldProductMeta {
  unit?: string;
  price?: number;
  category?: string;
  old_code?: string;
  quantity?: number;
  loss_rate?: number;
  warehouses?: WarehouseStock[];
  subcategory?: string;
  product_code?: string;
  specification?: string;
  acquire_channel?: string;
  storage_location?: string;
}

// 仓库库存项
interface WarehouseStock {
  term_id: number;
  quantity: number;
  term_name: string;
  update_at: string;
}

// 新版 products.meta 结构 (abt)
interface NewProductMeta {
  category: string;
  subcategory: string;
  product_code: string;
  specification: string;
  unit: string;
  acquire_channel: string;
  loss_rate: number;
  old_code: string | null;
}

// term 结构
interface Term {
  term_id: number;
  term_name: string;
  term_parent: number;
  term_meta: { count: number };
  taxonomy: string;
}

// 仓库 term (abt_real)
interface WarehouseTerm {
  term_id: number;
  term_name: string;
  term_parent: number;
}

// 库存记录
interface InventoryItem {
  product_id: number;
  location_id: number;
  quantity: number;
  term_name: string;
}

// ============================================================================
// 工具函数
// ============================================================================

/**
 * 转换旧版 meta 为新版格式（移除 warehouses, price, quantity, storage_location）
 */
function transformProductMeta(oldMeta: OldProductMeta): NewProductMeta {
  return {
    category: oldMeta.category || "",
    subcategory: oldMeta.subcategory || "",
    product_code: oldMeta.product_code || "",
    specification: oldMeta.specification || "",
    unit: oldMeta.unit || "",
    acquire_channel: oldMeta.acquire_channel || "",
    loss_rate: oldMeta.loss_rate || 0,
    old_code: oldMeta.old_code || null,
  };
}

/**
 * 生成仓库编码
 */
function generateWarehouseCode(termId: number): string {
  return `WH${termId}`;
}

/**
 * 生成库位编码
 */
function generateLocationCode(termName: string, termId: number): string {
  const cleaned = termName.replace(/[^a-zA-Z0-9\u4e00-\u9fa5]/g, "");
  return cleaned.substring(0, 20) || `LOC${termId}`;
}

// ============================================================================
// 迁移函数
// ============================================================================

/**
 * 迁移 warehouse 表
 */
async function migrateWarehouse(
  sourceClient: Client,
  targetClient: Client
): Promise<Map<number, number>> {
  console.log("\n🏭 开始迁移 warehouse 表...");

  const termToWarehouseId = new Map<number, number>();

  const result = await sourceClient.query<WarehouseTerm>(
    "SELECT term_id, term_name, term_parent FROM terms WHERE taxonomy = 'warehouse' AND term_parent = 0 ORDER BY term_id"
  );

  const warehouses = result.rows;
  console.log(`   发现 ${warehouses.length} 个顶级仓库`);

  let migrated = 0;
  let skipped = 0;

  for (const wh of warehouses) {
    try {
      const warehouseId = wh.term_id;
      const warehouseCode = generateWarehouseCode(wh.term_id);

      await targetClient.query(
        `INSERT INTO warehouse (warehouse_id, warehouse_name, warehouse_code, status)
         VALUES ($1, $2, $3, 'active')
         ON CONFLICT (warehouse_id) DO NOTHING`,
        [warehouseId, wh.term_name, warehouseCode]
      );

      termToWarehouseId.set(wh.term_id, warehouseId);
      migrated++;
    } catch (err) {
      console.error(`   ❌ 迁移仓库 ${wh.term_id} (${wh.term_name}) 失败:`, err);
      skipped++;
    }
  }

  console.log(`   ✅ warehouse 迁移完成: ${migrated} 成功, ${skipped} 跳过`);
  return termToWarehouseId;
}

/**
 * 迁移 location 表
 */
async function migrateLocation(
  sourceClient: Client,
  targetClient: Client,
  termToWarehouseId: Map<number, number>
): Promise<Map<number, number>> {
  console.log("\n📍 开始迁移 location 表...");

  const termToLocationId = new Map<number, number>();

  // 只查询库位（term_parent != 0）
  const result = await sourceClient.query<WarehouseTerm>(
    "SELECT term_id, term_name, term_parent FROM terms WHERE taxonomy = 'warehouse' AND term_parent > 0 ORDER BY term_id"
  );

  const locations = result.rows;
  console.log(`   发现 ${locations.length} 个库位`);

  let migrated = 0;
  let skipped = 0;

  for (const loc of locations) {
    try {
      const locationId = loc.term_id;
      const warehouseId = termToWarehouseId.get(loc.term_parent);

      if (!warehouseId) {
        console.error(`   ⚠️ 库位 ${loc.term_id} 的父仓库 ${loc.term_parent} 不存在，跳过`);
        skipped++;
        continue;
      }

      const locationCode = generateLocationCode(loc.term_name, loc.term_id);

      await targetClient.query(
        `INSERT INTO location (location_id, warehouse_id, location_code, location_name)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (location_id) DO NOTHING`,
        [locationId, warehouseId, locationCode, loc.term_name]
      );

      termToLocationId.set(loc.term_id, locationId);
      migrated++;
    } catch (err) {
      console.error(`   ❌ 迁移库位 ${loc.term_id} (${loc.term_name}) 失败:`, err);
      skipped++;
    }
  }

  console.log(`   ✅ location 迁移完成: ${migrated} 成功, ${skipped} 跳过`);

  // 为顶级仓库创建默认库位（如果该仓库没有子库位）
  console.log("\n   🔧 检查并创建默认库位...");

  const topLevelResult = await sourceClient.query<WarehouseTerm>(
    "SELECT term_id, term_name FROM terms WHERE taxonomy = 'warehouse' AND term_parent = 0 ORDER BY term_id"
  );

  for (const wh of topLevelResult.rows) {
    // 检查该仓库是否已经有子库位
    const childCheck = await sourceClient.query<{ count: number }>(
      "SELECT COUNT(*) as count FROM terms WHERE taxonomy = 'warehouse' AND term_parent = $1",
      [wh.term_id]
    );

    const hasChildLocations = parseInt(childCheck.rows[0].count) > 0;

    if (!hasChildLocations) {
      // 没有子库位，创建默认库位
      const defaultLocationId = wh.term_id * 1000;
      const warehouseId = termToWarehouseId.get(wh.term_id);

      if (warehouseId) {
        try {
          await targetClient.query(
            `INSERT INTO location (location_id, warehouse_id, location_code, location_name)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (location_id) DO NOTHING`,
            [defaultLocationId, warehouseId, `DEFAULT-${wh.term_id}`, `${wh.term_name}-默认库位`]
          );

          termToLocationId.set(wh.term_id, defaultLocationId);
          console.log(`   ✅ 为仓库 ${wh.term_name} 创建默认库位 (ID: ${defaultLocationId})`);
        } catch (err) {
          console.error(`   ❌ 为仓库 ${wh.term_id} 创建默认库位失败:`, err);
        }
      }
    }
  }

  return termToLocationId;
}

/**
 * 迁移 products 表
 */
async function migrateProducts(
  sourceClient: Client,
  targetClient: Client
): Promise<number> {
  console.log("\n📦 开始迁移 products 表...");

  const result = await sourceClient.query<{
    product_id: number;
    pdt_name: string;
    meta: OldProductMeta;
  }>(
    "SELECT product_id, pdt_name, meta FROM products ORDER BY product_id"
  );

  const products = result.rows;
  console.log(`   发现 ${products.length} 条记录`);

  let migrated = 0;
  let errors = 0;

  for (const product of products) {
    try {
      const newMeta = transformProductMeta(product.meta);

      await targetClient.query(
        `INSERT INTO products (product_id, pdt_name, meta)
         VALUES ($1, $2, $3)
         ON CONFLICT DO NOTHING`,
        [product.product_id, product.pdt_name, JSON.stringify(newMeta)]
      );
      migrated++;
    } catch (err) {
      console.error(`   ❌ 迁移 product ${product.product_id} 失败:`, err);
      errors++;
    }
  }

  console.log(`   ✅ products 迁移完成: ${migrated} 成功, ${errors} 错误`);
  return migrated;
}

/**
 * 迁移 inventory 表
 */
async function migrateInventory(
  sourceClient: Client,
  targetClient: Client,
  termToWarehouseId: Map<number, number>
): Promise<number> {
  console.log("\n📊 开始迁移 inventory 表...");

  const result = await sourceClient.query<{
    product_id: number;
    pdt_name: string;
    meta: OldProductMeta;
  }>(
    "SELECT product_id, pdt_name, meta FROM products WHERE jsonb_array_length(meta->'warehouses') > 0 ORDER BY product_id"
  );

  const products = result.rows;
  console.log(`   发现 ${products.length} 个产品有库存数据`);

  const inventoryItems: InventoryItem[] = [];
  let unmapped = 0;

  // 收集所有库存项
  for (const product of products) {
    if (!product.meta.warehouses) continue;

    for (const wh of product.meta.warehouses) {
      // 直接从数据库查询 location_id
      const locationResult = await targetClient.query<{ location_id: number }>(
        "SELECT location_id FROM location WHERE location_id = $1",
        [wh.term_id]
      );

      if (locationResult.rows.length > 0) {
        // 库位已存在
        inventoryItems.push({
          product_id: product.product_id,
          location_id: locationResult.rows[0].location_id,
          quantity: wh.quantity,
          term_name: wh.term_name,
        });
      } else if (termToWarehouseId.has(wh.term_id)) {
        // term_id 是顶级仓库，使用默认库位 ID
        const defaultLocId = wh.term_id * 1000;
        inventoryItems.push({
          product_id: product.product_id,
          location_id: defaultLocId,
          quantity: wh.quantity,
          term_name: wh.term_name,
        });
      } else {
        // 无法映射
        console.warn(`   ⚠️ 产品 ${product.product_id} 的仓库 term_id=${wh.term_id} (${wh.term_name}) 无法映射`);
        unmapped++;
      }
    }
  }

  console.log(`   共 ${inventoryItems.length} 条库存记录, ${unmapped} 条无法映射`);

  // 批量插入库存
  let migrated = 0;
  let skipped = 0;

  for (const item of inventoryItems) {
    try {
      await targetClient.query(
        `INSERT INTO inventory (product_id, location_id, quantity)
         VALUES ($1, $2, $3)
         ON CONFLICT (product_id, location_id) DO UPDATE SET quantity = EXCLUDED.quantity`,
        [item.product_id, item.location_id, item.quantity]
      );
      migrated++;
    } catch (err) {
      console.error(`   ❌ 迁移库存 (product=${item.product_id}, location=${item.location_id}) 失败:`, err);
      skipped++;
    }
  }

  console.log(`   ✅ inventory 迁移完成: ${migrated} 成功, ${skipped} 跳过`);
  return migrated;
}

/**
 * 迁移 terms 表（仅分类，不包含 warehouse）
 */
async function migrateTerms(
  sourceClient: Client,
  targetClient: Client
): Promise<number> {
  console.log("\n🏷️ 开始迁移 terms 表（分类）...");

  const result = await sourceClient.query<Term>(
    "SELECT term_id, term_name, term_parent, term_meta, taxonomy FROM terms WHERE taxonomy != 'warehouse' ORDER BY term_id"
  );

  const terms = result.rows;
  console.log(`   发现 ${terms.length} 条分类记录`);

  let migrated = 0;
  let skipped = 0;

  for (const term of terms) {
    try {
      await targetClient.query(
        `INSERT INTO terms (term_id, term_name, term_parent, term_meta, taxonomy)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT DO NOTHING`,
        [
          term.term_id,
          term.term_name,
          term.term_parent,
          JSON.stringify(term.term_meta),
          term.taxonomy,
        ]
      );
      migrated++;
    } catch (err) {
      console.error(`   ❌ 迁移 term ${term.term_id} 失败:`, err);
      skipped++;
    }
  }

  console.log(`   ✅ terms 迁移完成: ${migrated} 成功, ${skipped} 跳过`);
  return migrated;
}

/**
 * 迁移 bom 表
 */
async function migrateBom(
  sourceClient: Client,
  targetClient: Client
): Promise<number> {
  console.log("\n🔧 开始迁移 bom 表...");

  const result = await sourceClient.query<{
    bom_id: number;
    bom_name: string;
    create_at: Date;
    bom_detail: unknown;
  }>(
    "SELECT bom_id, bom_name, create_at, bom_detail FROM bom ORDER BY bom_id"
  );

  const boms = result.rows;
  console.log(`   发现 ${boms.length} 条记录`);

  let migrated = 0;
  let skipped = 0;

  for (const bom of boms) {
    try {
      await targetClient.query(
        `INSERT INTO bom (bom_id, bom_name, create_at, bom_detail, update_at)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT DO NOTHING`,
        [
          bom.bom_id,
          bom.bom_name,
          bom.create_at,
          JSON.stringify(bom.bom_detail),
          null,
        ]
      );
      migrated++;
    } catch (err) {
      console.error(`   ❌ 迁移 bom ${bom.bom_id} 失败:`, err);
      skipped++;
    }
  }

  console.log(`   ✅ bom 迁移完成: ${migrated} 成功, ${skipped} 跳过`);
  return migrated;
}

/**
 * 迁移 term_relation 表
 */
async function migrateTermRelation(
  sourceClient: Client,
  targetClient: Client
): Promise<number> {
  console.log("\n🔗 开始迁移 term_relation 表...");

  const result = await sourceClient.query<{
    term_id: number;
    product_id: number;
  }>(
    "SELECT term_id, product_id FROM term_relation ORDER BY term_id"
  );

  const relations = result.rows;
  console.log(`   发现 ${relations.length} 条记录`);

  let migrated = 0;
  let skipped = 0;

  for (const relation of relations) {
    try {
      await targetClient.query(
        `INSERT INTO term_relation (term_id, product_id)
         VALUES ($1, $2)
         ON CONFLICT DO NOTHING`,
        [relation.term_id, relation.product_id]
      );
      migrated++;
    } catch (err) {
      console.error(
        `   ❌ 迁移 term_relation (${relation.term_id}, ${relation.product_id}) 失败:`,
        err
      );
      skipped++;
    }
  }

  console.log(`   ✅ term_relation 迁移完成: ${migrated} 成功, ${skipped} 跳过`);
  return migrated;
}

/**
 * 验证迁移结果
 */
async function verifyMigration(targetClient: Client): Promise<void> {
  console.log("\n🔍 验证迁移结果...");

  const tables = [
    "warehouse",
    "location",
    "products",
    "inventory",
    "terms",
    "bom",
    "term_relation",
  ];

  for (const table of tables) {
    const result = await targetClient.query<{ count: string }>(
      `SELECT COUNT(*) as count FROM ${table}`
    );
    console.log(`   ${table}: ${result.rows[0].count} 条记录`);
  }

  // 检查 inventory 数据
  console.log("\n📋 库存分布示例:");
  const inventorySample = await targetClient.query(`
    SELECT i.product_id, p.pdt_name, l.location_name, w.warehouse_name, i.quantity
    FROM inventory i
    JOIN products p ON i.product_id = p.product_id
    JOIN location l ON i.location_id = l.location_id
    JOIN warehouse w ON l.warehouse_id = w.warehouse_id
    LIMIT 10
  `);

  if (inventorySample.rows.length === 0) {
    console.log("   (无库存数据)");
  } else {
    for (const row of inventorySample.rows) {
      console.log(`   产品: ${row.pdt_name}, 仓库: ${row.warehouse_name}, 库位: ${row.location_name}, 数量: ${row.quantity}`);
    }
  }
}

// ============================================================================
// 主流程
// ============================================================================

async function main() {
  console.log("=".repeat(60));
  console.log("🚀 数据迁移脚本：从 abt_real 迁移到 abt");
  console.log("=".repeat(60));

  const sourceClient = new Client(ABT_REAL_CONFIG);
  const targetClient = new Client(ABT_CONFIG);

  try {
    // 连接数据库
    console.log("\n📡 连接数据库...");
    await sourceClient.connect();
    console.log("   ✅ 已连接 abt_real");

    await targetClient.connect();
    console.log("   ✅ 已连接 abt");

    // 开始迁移
    const startTime = Date.now();

    // 1. 迁移 warehouse（顶级仓库）
    const termToWarehouseId = await migrateWarehouse(sourceClient, targetClient);

    // 2. 迁移 location（库位）
    await migrateLocation(sourceClient, targetClient, termToWarehouseId);

    // 3. 迁移 products
    await migrateProducts(sourceClient, targetClient);

    // 4. 迁移 inventory（依赖 warehouse）
    await migrateInventory(sourceClient, targetClient, termToWarehouseId);

    // 5. 迁移 terms（分类，不包含 warehouse）
    await migrateTerms(sourceClient, targetClient);

    // 6. 迁移 bom
    await migrateBom(sourceClient, targetClient);

    // 7. 迁移 term_relation
    await migrateTermRelation(sourceClient, targetClient);

    const duration = ((Date.now() - startTime) / 1000).toFixed(2);

    // 验证
    await verifyMigration(targetClient);

    console.log("\n" + "=".repeat(60));
    console.log(`✅ 迁移完成！耗时: ${duration} 秒`);
    console.log("=".repeat(60));
  } catch (error) {
    console.error("\n❌ 迁移失败:", error);
    process.exit(1);
  } finally {
    await sourceClient.end();
    await targetClient.end();
  }
}

// 运行迁移
main();
