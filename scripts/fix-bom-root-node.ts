/**
 * 修复 BOM 根结点脚本
 *
 * 清空服务端 bom_id=1000821 的节点，从内嵌数据逐条插入
 *
 * 用法: npx tsx fix-bom-root-node.ts
 * 依赖: npm install pg
 */

import pg from "pg";

const { Pool } = pg;

// ==================== 配置 ====================
const DATABASE_URL = "postgres://user_cC5B3h:password_TJWBYK@127.0.0.1:5432/abt2";
const SERVER_BOM_ID = 1000821;
// ==============================================

// 本地 bom_id=1000838 的节点数据 (parent_id: null=根结点, 数字=对应上面的本地id)
const NODES = [
  { id: 22148, product_id: 12941, product_code: "x1774508938", quantity: "1.000000", parent_id: null, loss_rate: "0.000000", order: 0, unit: "pcs", remark: null, position: null, work_center: null, properties: null },
  { id: 22149, product_id: 9014, product_code: "x1735880589", quantity: "0.005000", parent_id: 22148, loss_rate: "0.000000", order: 15, unit: "pcs", remark: "200装", position: null, work_center: null, properties: null },
  { id: 22150, product_id: 12079, product_code: "x1760496044", quantity: "0.000340", parent_id: 22148, loss_rate: "0.000000", order: 16, unit: "pcs", remark: "3000", position: null, work_center: null, properties: null },
  { id: 22151, product_id: 12942, product_code: "x1774508955", quantity: "1.000000", parent_id: 22148, loss_rate: "0.000000", order: 17, unit: "pcs", remark: null, position: null, work_center: null, properties: null },
  { id: 22152, product_id: 12946, product_code: "x1774509626", quantity: "0.007700", parent_id: 22151, loss_rate: "0.000000", order: 18, unit: "m²", remark: null, position: null, work_center: null, properties: null },
  { id: 22153, product_id: 12945, product_code: "x1774508996", quantity: "1.000000", parent_id: 22151, loss_rate: "0.000000", order: 19, unit: "pcs", remark: null, position: null, work_center: null, properties: null },
  { id: 22154, product_id: 8693, product_code: "x1735139565", quantity: "4.000000", parent_id: 22153, loss_rate: "0.000000", order: 20, unit: "g", remark: null, position: null, work_center: null, properties: null },
  { id: 22155, product_id: 12947, product_code: "x1774509686", quantity: "1.000000", parent_id: 22153, loss_rate: "0.000000", order: 21, unit: "pcs", remark: null, position: null, work_center: null, properties: null },
  { id: 22156, product_id: 12943, product_code: "x1774508967", quantity: "1.000000", parent_id: 22153, loss_rate: "0.000000", order: 29, unit: "pcs", remark: null, position: null, work_center: null, properties: null },
  { id: 22157, product_id: 12948, product_code: "x1774510141", quantity: "0.113000", parent_id: 22156, loss_rate: "0.000000", order: 31, unit: "m", remark: null, position: null, work_center: null, properties: null },
  { id: 22158, product_id: 12949, product_code: "x1774510155", quantity: "0.113000", parent_id: 22156, loss_rate: "0.000000", order: 32, unit: "m", remark: null, position: null, work_center: null, properties: null },
  { id: 22159, product_id: 8694, product_code: "x1735139656", quantity: "0.100000", parent_id: 22156, loss_rate: "0.000000", order: 34, unit: "g", remark: null, position: null, work_center: null, properties: null },
  { id: 22160, product_id: 12944, product_code: "x1774508978", quantity: "1.000000", parent_id: 22156, loss_rate: "0.000000", order: 35, unit: "pcs", remark: null, position: null, work_center: null, properties: null },
  { id: 22161, product_id: 12060, product_code: "x1759806599", quantity: "1.000000", parent_id: 22160, loss_rate: "0.000000", order: 36, unit: "pcs", remark: null, position: null, work_center: null, properties: null },
  { id: 22162, product_id: 11050, product_code: "x1750322879", quantity: "3.000000", parent_id: 22160, loss_rate: "0.000000", order: 37, unit: "pcs", remark: null, position: null, work_center: null, properties: null },
  { id: 22163, product_id: 5933, product_code: "x1738752822", quantity: "2.000000", parent_id: 22160, loss_rate: "0.000000", order: 38, unit: "pcs", remark: null, position: null, work_center: null, properties: null },
] as const;

// 需要补充插入的产品 (根结点产品在服务端被删除了)
const MISSING_PRODUCTS = [
  {
    pdt_name: "注塑模组/7114/白",
    product_code: "x1774508938",
    unit: "pcs",
    meta: { specification: "--", acquire_channel: "自制", old_code: "" },
  },
];

async function main() {
  const pool = new Pool({ connectionString: DATABASE_URL });
  const client = await pool.connect();

  // 根结点先插
  const sorted = [...NODES].sort((a, b) => {
    if (a.parent_id === null && b.parent_id !== null) return -1;
    if (a.parent_id !== null && b.parent_id === null) return 1;
    return a.order - b.order;
  });

  try {
    // 1. 用 product_code 在服务端查出对应的 product_id
    const codes = [...new Set(NODES.map((n) => n.product_code))];
    console.log(`查询 ${codes.length} 个产品的 product_id...`);
    const { rows: products } = await client.query(
      `SELECT product_id, product_code FROM products WHERE product_code = ANY($1::text[])`,
      [codes]
    );
    const codeToId = new Map(products.map((p: any) => [p.product_code, p.product_id]));

    // 2. 插入服务端缺失的产品
    for (const p of MISSING_PRODUCTS) {
      if (codeToId.has(p.product_code)) continue;

      // 用 pdt_name 查一下是否已存在
      const { rows: existing } = await client.query(
        `SELECT product_id, product_code FROM products WHERE pdt_name = $1`,
        [p.pdt_name]
      );

      if (existing.length > 0) {
        const row = existing[0];
        if (row.product_code === p.product_code) {
          // 同名同 code，直接用
          console.log(`  产品已存在: ${p.pdt_name} (${p.product_code}) -> product_id=${row.product_id}`);
          codeToId.set(p.product_code, row.product_id);
        } else {
          // 同名不同 code，更新 code
          console.log(`  更新产品 code: ${p.pdt_name} (${row.product_code} -> ${p.product_code})`);
          await client.query(
            `UPDATE products SET product_code = $1, unit = $2, meta = $3::jsonb WHERE product_id = $4`,
            [p.product_code, p.unit, JSON.stringify(p.meta), row.product_id]
          );
          codeToId.set(p.product_code, row.product_id);
        }
      } else {
        // 不存在，插入
        console.log(`  插入缺失产品: ${p.pdt_name} (${p.product_code})`);
        const res = await client.query(
          `INSERT INTO products (pdt_name, product_code, unit, meta) VALUES ($1, $2, $3, $4::jsonb) RETURNING product_id`,
          [p.pdt_name, p.product_code, p.unit, JSON.stringify(p.meta)]
        );
        codeToId.set(p.product_code, res.rows[0].product_id);
      }
    }

    for (const code of codes) {
      if (!codeToId.has(code)) {
        console.error(`  产品 ${code} 在服务端不存在且无法插入!`);
        process.exit(1);
      }
      console.log(`  ${code} -> product_id=${codeToId.get(code)}`);
    }

    // 本地 node id -> 服务端新 node id
    const idMap = new Map<number, number>();

    await client.query("BEGIN");

    // 3. 清空服务端该 BOM 节点
    console.log(`\n清空 bom_id=${SERVER_BOM_ID}...`);
    const del = await client.query("DELETE FROM bom_nodes WHERE bom_id = $1", [SERVER_BOM_ID]);
    console.log(`  删除 ${del.rowCount} 个节点\n`);

    // 4. 逐条插入
    for (const node of sorted) {
      const parentId = node.parent_id ? idMap.get(node.parent_id)! : null;
      const productId = codeToId.get(node.product_code);
      const res = await client.query(
        `INSERT INTO bom_nodes (bom_id, product_id, product_code, quantity, parent_id, loss_rate, "order", unit, remark, position, work_center, properties) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12) RETURNING id`,
        [SERVER_BOM_ID, productId, node.product_code, node.quantity, parentId, node.loss_rate, node.order, node.unit, node.remark, node.position, node.work_center, node.properties]
      );
      idMap.set(node.id, res.rows[0].id);
      console.log(`  ${node.product_code} (product_id=${productId}) -> node_id=${res.rows[0].id}`);
    }

    await client.query("COMMIT");
    console.log(`\n完成! 共插入 ${sorted.length} 个节点`);
  } catch (err) {
    await client.query("ROLLBACK");
    console.error("失败，已回滚:", err);
    process.exit(1);
  } finally {
    client.release();
    await pool.end();
  }
}

main();
