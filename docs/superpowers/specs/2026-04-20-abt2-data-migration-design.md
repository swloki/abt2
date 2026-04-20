# abt2 数据迁移到 abt 设计

## 背景

abt2 是上一版本的数据库，包含真实的业务数据。abt 是最新版本的数据库，表结构有部分更新。需要将 abt2 中的业务数据迁移到 abt 中，同时保留 abt 中所有用户/权限相关表的数据不变。

## 数据库概况

- 同一台 PostgreSQL 服务器：`127.0.0.1:5432`
- abt2：10 张表，无用户/权限相关表
- abt：21 张表，包含用户/权限系统

### 需要迁移的表（8 张）

| 表名 | abt2 列数 | abt 列数 | 差异 | abt2 行数 |
|------|-----------|----------|------|-----------|
| products | 3 | 3 | 完全一致 | 11,982 |
| terms | 5 | 5 | 完全一致 | 157 |
| term_relation | 2 | 2 | 完全一致 | 12,552 |
| warehouse | 7 | 7 | 完全一致 | 10 |
| location | 7 | 7 | 完全一致 | 5 |
| inventory | 8 | 8 | 完全一致 | 357 |
| inventory_log | 13 | 13 | 完全一致 | 30 |
| bom | 5 | **7** | abt 多了 `process_group_id`、`bom_category_id` | 510 |

### 保留不动的表（abt 中的用户/权限表）

- users, roles, user_roles, role_permissions
- departments, user_departments
- permission_audit_logs

### abt2 中存在但不迁移的表

- user（abt2 的旧用户表，不需要）
- product_price_log（不需要）

## 方案

用 TypeScript + Bun 编写迁移脚本 `scripts/migrate-abt2-to-abt.ts`。

### 技术选型

- 运行时：Bun
- 数据库驱动：`postgres`（Bun 兼容的 PostgreSQL 客户端）
- 连接方式：两个独立的连接池，分别连接 abt2 和 abt

### 核心流程

```
1. 建立连接：分别连接 abt2（只读）和 abt（读写）
2. 验证：确认两个库都可连接
3. 在 abt 中清空业务表（按外键依赖倒序）：
   - inventory_log → inventory → location → warehouse
   - term_relation → terms
   - bom → products
4. 从 abt2 读取数据，写入 abt（按外键依赖正序）：
   - products → terms → term_relation
   - warehouse → location → inventory → inventory_log
   - bom
5. 重置 abt 中被清空表的序列（sequence）为最大 ID + 1
6. 输出每张表的迁移行数统计
```

### 字段映射

- **bom 表**：abt2 有 5 列（bom_id, bom_name, create_at, bom_detail, update_at），abt 有 7 列（多 process_group_id, bom_category_id）。导入时多出的两列设为 NULL。
- 其他 7 张表完全一致，1:1 直接映射。

### 安全措施

- 整个迁移过程在一个事务中执行，失败自动回滚
- 迁移前验证 abt2 各表数据存在
- 迁移后校验 abt 和 abt2 各表行数一致
- 只操作业务表，不触碰用户/权限相关表
- 使用 TRUNCATE ... CASCADE 清空表（比 DELETE 更快且重置序列）

### 依赖顺序

**清空顺序（倒序，先子表后主表）：**
1. inventory_log
2. inventory
3. term_relation
4. location
5. bom
6. products
7. terms
8. warehouse

**导入顺序（正序，先主表后子表）：**
1. warehouse
2. products
3. terms
4. location
5. term_relation
6. inventory
7. inventory_log
8. bom

## 文件结构

```
scripts/
  migrate-abt2-to-abt.ts    # 迁移脚本
```

## 运行方式

```bash
bun run scripts/migrate-abt2-to-abt.ts
```
