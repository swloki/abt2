/**
 * BOM 节点数据迁移：从 bom.bom_detail JSONB → bom_nodes 表
 *
 * 使用方式：bun run migrate-bom-detail-to-nodes.ts
 *
 * 迁移说明：
 * 1. 读取 bom 表中每条记录的 bom_detail JSONB 字段
 * 2. 解析 nodes 数组，每个节点转为 bom_nodes 表的一行
 * 3. 使用两阶段插入：先插入所有节点（parent_id=NULL），再更新 parent_id 映射
 * 4. 支持重跑（ON CONFLICT DO NOTHING）
 *
 * 前置条件：bom_nodes 表已由 migration 029 创建
 */

import { Client } from "pg";

const DB_CONFIG = {
  host: process.env.DB_HOST || "localhost",
  port: parseInt(process.env.DB_PORT || "5432"),
  database: process.env.DB_NAME || "abt",
  user: process.env.DB_USER || "postgres",
  password: process.env.DB_PASSWORD || (() => { throw new Error("DB_PASSWORD env var is required"); })(),
};

interface BomNode {
  id: number;
  product_id: number;
  product_code: string | null;
  quantity: number;
  parent_id: number;
  loss_rate: number;
  order: number;
  unit: string | null;
  remark: string | null;
  position: string | null;
  work_center: string | null;
  properties: string | null;
}

interface BomDetail {
  nodes: BomNode[];
  created_by?: number;
}

interface BomRow {
  bom_id: number;
  bom_detail: string;
}

async function migrate() {
  const client = new Client(DB_CONFIG);
  await client.connect();

  console.log("开始迁移 BOM 节点数据...");

  try {
    await client.query("BEGIN");

    // 读取所有 BOM
    const { rows }: { rows: BomRow[] } = await client.query(
      "SELECT bom_id, bom_detail::text FROM bom ORDER BY bom_id"
    );

    console.log(`共 ${rows.length} 个 BOM 需要迁移`);

    let totalNodes = 0;
    let skippedBoms = 0;

    for (const bom of rows) {
      let detail: BomDetail;
      try {
        detail = JSON.parse(bom.bom_detail);
      } catch {
        console.warn(`  BOM ${bom.bom_id}: bom_detail 解析失败，跳过`);
        skippedBoms++;
        continue;
      }

      if (!detail.nodes || detail.nodes.length === 0) {
        console.log(`  BOM ${bom.bom_id}: 无节点，跳过`);
        skippedBoms++;
        continue;
      }

      // 检查是否已迁移
      const { rows: existing } = await client.query(
        "SELECT COUNT(*) as cnt FROM bom_nodes WHERE bom_id = $1",
        [bom.bom_id]
      );
      if (existing[0].cnt > 0) {
        console.log(`  BOM ${bom.bom_id}: 已有 ${existing[0].cnt} 个节点，跳过`);
        skippedBoms++;
        continue;
      }

      console.log(`  BOM ${bom.bom_id}: 迁移 ${detail.nodes.length} 个节点`);

      // 阶段1：插入所有节点，暂存 old_id → new_id 映射，parent_id 暂设 NULL
      const idMap = new Map<number, number>();

      for (const node of detail.nodes) {
        // 类型安全转换：JSON 中的数值字段可能是字符串
        const productId = typeof node.product_id === "number" ? node.product_id : parseInt(String(node.product_id), 10);
        const quantity = typeof node.quantity === "number" ? node.quantity : parseFloat(String(node.quantity));
        const lossRate = typeof node.loss_rate === "number" ? node.loss_rate : parseFloat(String(node.loss_rate)) || 0;
        const order = typeof node.order === "number" ? node.order : parseInt(String(node.order), 10) || 0;

        if (isNaN(productId)) {
          console.warn(`    跳过节点 id=${node.id}: product_id 无效 (${node.product_id})`);
          continue;
        }

        const result = await client.query(
          `INSERT INTO bom_nodes (bom_id, product_id, product_code, quantity, parent_id, loss_rate, "order", unit, remark, position, work_center, properties)
           VALUES ($1, $2, $3, $4, NULL, $5, $6, $7, $8, $9, $10, $11)
           RETURNING id`,
          [
            bom.bom_id,
            productId,
            node.product_code || null,
            quantity,
            lossRate,
            order,
            node.unit || null,
            node.remark || null,
            node.position || null,
            node.work_center || null,
            node.properties || null,
          ]
        );

        const newId = result.rows[0].id;
        idMap.set(node.id, newId);
        totalNodes++;
      }

      // 阶段2：更新 parent_id（将旧 ID 映射为新 ID）
      for (const node of detail.nodes) {
        if (node.parent_id && node.parent_id !== 0) {
          const newId = idMap.get(node.id);
          const newParentId = idMap.get(node.parent_id);
          if (newId && newParentId) {
            await client.query(
              `UPDATE bom_nodes SET parent_id = $1 WHERE id = $2`,
              [newParentId, newId]
            );
          }
        }
      }

      // 同步 created_by 到 bom 表（仅当值为有效数字时）
      if (detail.created_by != null) {
        const createdBy = typeof detail.created_by === "number"
          ? detail.created_by
          : parseInt(String(detail.created_by), 10);
        if (!isNaN(createdBy)) {
          await client.query(
            "UPDATE bom SET created_by = $1 WHERE bom_id = $2",
            [createdBy, bom.bom_id]
          );
        } else {
          console.warn(`    created_by 值无效 (${detail.created_by})，跳过`);
        }
      }
    }

    await client.query("COMMIT");
    console.log(`\n迁移完成！`);
    console.log(`  总计: ${totalNodes} 个节点已迁移`);
    console.log(`  跳过: ${skippedBoms} 个 BOM`);
  } catch (err) {
    await client.query("ROLLBACK");
    console.error("迁移失败，已回滚:", err);
    throw err;
  } finally {
    await client.end();
  }
}

migrate().catch((err) => {
  console.error(err);
  process.exit(1);
});
