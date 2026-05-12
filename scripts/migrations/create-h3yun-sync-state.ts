/**
 * H3Yun 同步相关表结构迁移脚本
 *
 * 使用方式：DB_PASSWORD=xxx bun run scripts/migrations/create-h3yun-sync-state.ts
 *
 * 在服务器上创建 h3yun_sync_state 表（幂等，已存在则跳过）
 */

import { Client } from "pg";

const DB = {
  host: process.env.DB_HOST || "localhost",
  port: parseInt(process.env.DB_PORT || "5432"),
  database: process.env.DB_NAME || "abt",
  user: process.env.DB_USER || "postgres",
  password: process.env.DB_PASSWORD || "123456",
};

const MIGRATIONS = [
  {
    name: "h3yun_sync_state",
    sql: `
      CREATE TABLE IF NOT EXISTS h3yun_sync_state (
          id              SERIAL PRIMARY KEY,
          entity_type     VARCHAR(32) NOT NULL,
          entity_id       BIGINT NOT NULL,
          h3yun_object_id VARCHAR(64),
          last_synced_at  TIMESTAMPTZ,
          content_hash    VARCHAR(64),
          created_at      TIMESTAMPTZ DEFAULT NOW(),
          UNIQUE(entity_type, entity_id)
      );

      COMMENT ON TABLE h3yun_sync_state IS 'H3Yun 同步映射表：ABT 实体与 H3Yun ObjectId 的映射关系';
      COMMENT ON COLUMN h3yun_sync_state.entity_type IS '实体类型：product | inventory';
      COMMENT ON COLUMN h3yun_sync_state.entity_id IS 'ABT 中的 product_id / inventory_id';
      COMMENT ON COLUMN h3yun_sync_state.h3yun_object_id IS 'H3Yun 返回的 ObjectId（首次同步后填充）';
      COMMENT ON COLUMN h3yun_sync_state.last_synced_at IS '上次成功同步时间';
      COMMENT ON COLUMN h3yun_sync_state.content_hash IS '上次同步的内容哈希（用于去重）';
    `,
  },
];

async function main() {
  console.log("=".repeat(60));
  console.log("🚀 H3Yun 同步表结构迁移");
  console.log("=".repeat(60));

  const client = new Client(DB);

  try {
    await client.connect();
    console.log(`✅ 已连接 ${DB.database}@${DB.host}:${DB.port}`);

    for (const { name, sql } of MIGRATIONS) {
      console.log(`\n📦 创建 ${name}...`);
      await client.query(sql);
      console.log(`  ✅ ${name} 完成`);
    }

    console.log("\n" + "=".repeat(60));
    console.log("✅ 迁移完成");
    console.log("=".repeat(60));
  } catch (err) {
    console.error("\n❌ 迁移失败:", err);
    process.exit(1);
  } finally {
    await client.end();
  }
}

main();
