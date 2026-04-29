# BOM 成本报告 RPC + 独立权限 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 新增 `GetBomCostReport` RPC 方法，一次性返回 BOM 成本报告，配置 `BOM_COST:READ` 独立权限，替代前端 5+ 次 gRPC 聚合调用。

**Architecture:** 在现有 BOM 分层架构上扩展。Proto 层新增 `BOM_COST` 资源枚举和成本报告消息类型；Service 层新增 `get_bom_cost_report` 方法，内部组合 BomRepo、ProductRepo、ProductPriceRepo、LaborProcessRepo 查询；Handler 层用 `require_permission(Resource::BomCost, Action::Read)` 鉴权。

**Tech Stack:** Rust, tonic/prost (gRPC), sqlx (PostgreSQL), proto3

---

### Task 1: Proto — 新增 BOM_COST 资源枚举

**Files:**
- Modify: `proto/abt/v1/permission.proto:25`

- [ ] **Step 1: 在 Resource enum 末尾添加 BOM_COST = 15**

在 `proto/abt/v1/permission.proto` 的 `Resource` enum 中，在 `ROUTING = 14;` 之后添加：

```protobuf
  ROUTING = 14;
  BOM_COST = 15;
```

- [ ] **Step 2: 验证文件格式正确**

确认 enum 值从 0 到 15 连续无重复。

---

### Task 2: Proto — 新增 RPC 和消息类型

**Files:**
- Modify: `proto/abt/v1/bom.proto`

- [ ] **Step 1: 在 AbtBomService 中添加 RPC**

在 `proto/abt/v1/bom.proto` 的 `AbtBomService` service 定义中，在最后一行 rpc (`SubstituteProduct`) 之后添加：

```protobuf
  // BOM 成本报告
  rpc GetBomCostReport(GetBomCostReportRequest) returns (BomCostReportResponse);
```

- [ ] **Step 2: 在文件末尾添加消息定义**

在 `bom.proto` 文件末尾（`SubstituteProductResponse` 之后）添加：

```protobuf
// 成本报告请求
message GetBomCostReportRequest {
  int64 bom_id = 1;
}

// 材料成本项
message MaterialCostItem {
  int64 node_id = 1;
  int64 product_id = 2;
  string product_name = 3;
  string product_code = 4;
  double quantity = 5;
  optional string unit_price = 6;
}

// 人工成本项
message LaborCostItem {
  int64 id = 1;
  string name = 2;
  string unit_price = 3;
  string quantity = 4;
  int32 sort_order = 5;
  string remark = 6;
}

// 成本报告响应
message BomCostReportResponse {
  int64 bom_id = 1;
  string bom_name = 2;
  string product_code = 3;
  repeated MaterialCostItem material_costs = 4;
  repeated LaborCostItem labor_costs = 5;
  repeated string warnings = 6;
}
```

---

### Task 3: 资源注册表 — 添加 BOM_COST 定义

**Files:**
- Modify: `abt/src/models/resources.rs:64`

- [ ] **Step 1: 在 RESOURCES 静态数组末尾添加 BOM_COST 条目**

在 `abt/src/models/resources.rs` 的 `RESOURCES` 数组中，最后一个条目 (`DEPARTMENT`) 之后、`];` 之前添加：

```rust
    // BOM 成本
    ResourceActionDef { resource_code: "BOM_COST", resource_name: "BOM成本", description: "BOM成本查看", action: "READ", action_name: "查看" },
```

注意：BOM_COST 只需要 READ 操作，不需要 WRITE 和 DELETE。

---

### Task 4: 数据库迁移 — 权限种子数据

**Files:**
- Create: `abt/migrations/028_add_bom_cost_permission.sql`

- [ ] **Step 1: 创建迁移文件**

创建 `abt/migrations/028_add_bom_cost_permission.sql`，内容：

```sql
-- BOM 成本权限资源
INSERT INTO resources (resource_name, resource_code, group_name, sort_order) VALUES
('BOM_COST', 'BOM_COST', 'BOM管理', 15)
ON CONFLICT (resource_code) DO NOTHING;

-- BOM_COST:READ 权限
INSERT INTO permissions (permission_name, resource_id, action_code, sort_order)
SELECT 'BOM_COST-read', r.resource_id, 'READ', (r.sort_order * 10 + 1)
FROM resources r
WHERE r.resource_code = 'BOM_COST'
ON CONFLICT (resource_id, action_code) DO NOTHING;
```

注意：`resource_code` 使用大写 `BOM_COST` 以匹配 proto enum 的 `as_str_name()` 返回值。`group_name` 设为 `'BOM管理'` 归入 BOM 管理分组。

---

### Task 5: 批量价格查询 — ProductPriceRepo 扩展

**Files:**
- Modify: `abt/src/repositories/product_price_repo.rs`

- [ ] **Step 1: 在 ProductPriceRepo impl 中添加 get_prices_by_ids 方法**

在 `abt/src/repositories/product_price_repo.rs` 的 `ProductPriceRepo` impl 块末尾（`list_all_price_history` 方法之后）添加：

```rust
    /// 批量获取多个产品的当前价格
    pub async fn get_prices_by_ids(
        pool: &PgPool,
        product_ids: &[i64],
    ) -> Result<std::collections::HashMap<i64, rust_decimal::Decimal>> {
        use std::collections::HashMap;
        use rust_decimal::Decimal;

        if product_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let rows = sqlx::query(
            "SELECT product_id, (meta->>'price')::decimal AS price \
             FROM products \
             WHERE product_id = ANY($1) AND meta->>'price' IS NOT NULL",
        )
        .bind(product_ids)
        .fetch_all(pool)
        .await?;

        let mut map = HashMap::new();
        for row in rows {
            let product_id: i64 = sqlx::Row::try_get(&row, "product_id")?;
            let price: Decimal = sqlx::Row::try_get(&row, "price")?;
            map.insert(product_id, price);
        }
        Ok(map)
    }
```

---

### Task 6: Model — 成本报告模型类型

**Files:**
- Modify: `abt/src/models/bom.rs`

- [ ] **Step 1: 在 bom.rs 文件末尾（`mod tests` 之前）添加模型类型**

在 `abt/src/models/bom.rs` 中，在 `// ============================================================================` + `// 创建/更新请求` 注释块之前（即 `BomQuery` 的 `impl Default` 之后），添加：

```rust
// ============================================================================
// 成本报告
// ============================================================================

/// BOM 成本报告
#[derive(Debug, Clone)]
pub struct BomCostReport {
    pub bom_id: i64,
    pub bom_name: String,
    pub product_code: String,
    pub material_costs: Vec<MaterialCostItem>,
    pub labor_costs: Vec<LaborCostItem>,
    pub warnings: Vec<String>,
}

/// 材料成本项
#[derive(Debug, Clone)]
pub struct MaterialCostItem {
    pub node_id: i64,
    pub product_id: i64,
    pub product_name: String,
    pub product_code: String,
    pub quantity: f64,
    pub unit_price: Option<String>,
}

/// 人工成本项
#[derive(Debug, Clone)]
pub struct LaborCostItem {
    pub id: i64,
    pub name: String,
    pub unit_price: String,
    pub quantity: String,
    pub sort_order: i32,
    pub remark: String,
}
```

---

### Task 7: Service — 添加 get_bom_cost_report 方法

**Files:**
- Modify: `abt/src/service/bom_service.rs`
- Modify: `abt/src/implt/bom_service_impl.rs`

- [ ] **Step 1: 在 BomService trait 中添加方法签名**

在 `abt/src/service/bom_service.rs` 的 `BomService` trait 中，在最后一个方法 (`substitute_product`) 之后、`}` 之前添加：

```rust
    /// 获取 BOM 成本报告
    async fn get_bom_cost_report(&self, bom_id: i64, executor: Executor<'_>) -> Result<crate::models::BomCostReport>;
```

- [ ] **Step 2: 在 BomServiceImpl 中添加 imports**

在 `abt/src/implt/bom_service_impl.rs` 顶部，修改 imports：

```rust
use crate::models::{Bom, BomDetail, BomNode, BomQuery};
```

改为：

```rust
use crate::models::{Bom, BomCostReport, BomDetail, BomNode, BomQuery, LaborCostItem, MaterialCostItem};
```

并添加 import：

```rust
use crate::repositories::{BomRepo, Executor, LaborProcessRepo, ProductPriceRepo, ProductRepo};
```

（在现有 `use crate::repositories::{BomRepo, Executor, ProductRepo};` 基础上添加 `LaborProcessRepo` 和 `ProductPriceRepo`）

- [ ] **Step 3: 在 BomServiceImpl 的 impl 块中实现方法**

在 `abt/src/implt/bom_service_impl.rs` 的 `#[async_trait] impl BomService for BomServiceImpl` 块末尾，在最后一个方法 (`substitute_product`) 之后、最后一个 `}` 之前添加：

```rust
    async fn get_bom_cost_report(&self, bom_id: i64, executor: Executor<'_>) -> Result<crate::models::BomCostReport> {
        // 1. 获取 BOM
        let bom = BomRepo::find_by_id(executor, bom_id).await?
            .ok_or_else(|| anyhow::anyhow!("BOM not found"))?;

        // 2. 获取根节点 product_code（第一个节点 = 成品）
        let root_node = bom.bom_detail.nodes.first()
            .ok_or_else(|| anyhow::anyhow!("BOM has no nodes"))?;
        let product_code = if let Some(ref code) = root_node.product_code {
            code.clone()
        } else {
            let products = ProductRepo::find_by_ids(&self.pool, &[root_node.product_id]).await?;
            products.first()
                .map(|p| p.meta.product_code.clone())
                .ok_or_else(|| anyhow::anyhow!("Root product not found"))?
        };

        // 3. 获取所有节点的产品信息（批量）
        let all_product_ids: Vec<i64> = bom.bom_detail.nodes.iter().map(|n| n.product_id).collect();
        let products = ProductRepo::find_by_ids(&self.pool, &all_product_ids).await?;
        let valid_ids: HashSet<i64> = products.iter().map(|p| p.product_id).collect();
        let name_map: HashMap<i64, String> = products.iter()
            .map(|p| (p.product_id, p.pdt_name.clone())).collect();
        let code_map: HashMap<i64, String> = products.iter()
            .map(|p| (p.product_id, p.meta.product_code.clone())).collect();

        // 4. 计算叶子节点（非任何节点的 parent，且产品有效）
        let parent_ids: HashSet<i64> = bom.bom_detail.nodes.iter()
            .filter(|n| n.parent_id != 0)
            .map(|n| n.parent_id)
            .collect();

        // 构建无效节点集合（产品不存在的节点及其所有后代）
        let mut children_map: HashMap<i64, Vec<i64>> = HashMap::new();
        for node in &bom.bom_detail.nodes {
            children_map.entry(node.parent_id).or_default().push(node.id);
        }
        fn get_descendants(id: i64, cm: &HashMap<i64, Vec<i64>>) -> Vec<i64> {
            let mut out = Vec::new();
            if let Some(children) = cm.get(&id) {
                for &c in children {
                    out.push(c);
                    out.extend(get_descendants(c, cm));
                }
            }
            out
        }
        let mut invalid_ids: HashSet<i64> = HashSet::new();
        for node in &bom.bom_detail.nodes {
            if !valid_ids.contains(&node.product_id) {
                invalid_ids.insert(node.id);
                for d in get_descendants(node.id, &children_map) {
                    invalid_ids.insert(d);
                }
            }
        }

        let mut leaf_nodes: Vec<&BomNode> = bom.bom_detail.nodes.iter()
            .filter(|n| !parent_ids.contains(&n.id))
            .filter(|n| !invalid_ids.contains(&n.id))
            .collect();
        leaf_nodes.sort_by_key(|n| n.order);

        // 5. 批量获取价格
        let leaf_product_ids: Vec<i64> = leaf_nodes.iter().map(|n| n.product_id).collect();
        let prices = ProductPriceRepo::get_prices_by_ids(&self.pool, &leaf_product_ids).await?;

        // 6. 获取人工工序
        let labor_processes = LaborProcessRepo::list_all_by_product_code(&self.pool, &product_code).await?;

        // 7. 组装结果
        let material_costs: Vec<MaterialCostItem> = leaf_nodes.iter().map(|node| {
            let price = prices.get(&node.product_id);
            MaterialCostItem {
                node_id: node.id,
                product_id: node.product_id,
                product_name: name_map.get(&node.product_id).cloned().unwrap_or_default(),
                product_code: code_map.get(&node.product_id).cloned().unwrap_or_default(),
                quantity: node.quantity,
                unit_price: price.map(|p| p.to_string()),
            }
        }).collect();

        let warnings: Vec<String> = material_costs.iter()
            .filter(|m| m.unit_price.is_none())
            .map(|m| m.product_name.clone())
            .collect();

        let labor_costs: Vec<LaborCostItem> = labor_processes.iter().map(|lp| {
            LaborCostItem {
                id: lp.id,
                name: lp.name.clone(),
                unit_price: lp.unit_price.to_string(),
                quantity: lp.quantity.to_string(),
                sort_order: lp.sort_order,
                remark: lp.remark.clone().unwrap_or_default(),
            }
        }).collect();

        Ok(BomCostReport {
            bom_id,
            bom_name: bom.bom_name,
            product_code,
            material_costs,
            labor_costs,
            warnings,
        })
    }
```

---

### Task 8: Proto 转换 — convert.rs

**Files:**
- Modify: `abt-grpc/src/handlers/convert.rs`

- [ ] **Step 1: 添加 From 转换实现**

在 `abt-grpc/src/handlers/convert.rs` 文件末尾添加：

```rust
// ========== BOM Cost Report conversions ==========

use crate::generated::abt::v1::{
    BomCostReportResponse, LaborCostItem as ProtoLaborCostItem,
    MaterialCostItem as ProtoMaterialCostItem,
};

impl From<abt::BomCostReport> for BomCostReportResponse {
    fn from(report: abt::BomCostReport) -> Self {
        BomCostReportResponse {
            bom_id: report.bom_id,
            bom_name: report.bom_name,
            product_code: report.product_code,
            material_costs: report.material_costs.into_iter().map(|m| m.into()).collect(),
            labor_costs: report.labor_costs.into_iter().map(|l| l.into()).collect(),
            warnings: report.warnings,
        }
    }
}

impl From<abt::MaterialCostItem> for ProtoMaterialCostItem {
    fn from(item: abt::MaterialCostItem) -> Self {
        ProtoMaterialCostItem {
            node_id: item.node_id,
            product_id: item.product_id,
            product_name: item.product_name,
            product_code: item.product_code,
            quantity: item.quantity,
            unit_price: item.unit_price,
        }
    }
}

impl From<abt::LaborCostItem> for ProtoLaborCostItem {
    fn from(item: abt::LaborCostItem) -> Self {
        ProtoLaborCostItem {
            id: item.id,
            name: item.name,
            unit_price: item.unit_price,
            quantity: item.quantity,
            sort_order: item.sort_order,
            remark: item.remark,
        }
    }
}
```

---

### Task 9: Handler — 添加 get_bom_cost_report 方法

**Files:**
- Modify: `abt-grpc/src/handlers/bom.rs`

- [ ] **Step 1: 在 BomHandler impl 中添加 handler 方法**

在 `abt-grpc/src/handlers/bom.rs` 的 `impl GrpcBomService for BomHandler` 块末尾，在 `substitute_product` 方法之后、最后的 `}` 之前添加：

```rust
    #[require_permission(Resource::BomCost, Action::Read)]
    async fn get_bom_cost_report(
        &self,
        request: Request<GetBomCostReportRequest>,
    ) -> GrpcResult<BomCostReportResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.bom_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let report = srv
            .get_bom_cost_report(req.bom_id, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(report.into()))
    }
```

注意：不需要 `tx.commit()`，因为这是只读操作。事务会在离开作用域时自动回滚（无副作用）。

---

### Task 10: 构建验证

- [ ] **Step 1: 运行 cargo build**

```bash
cargo build
```

预期：编译成功。`build.rs` 会自动从 `.proto` 文件重新生成 Rust 类型（包括 `GetBomCostReportRequest`、`BomCostReportResponse`、`MaterialCostItem`、`LaborCostItem`、`Resource::BomCost`）。

如果编译失败：
- 检查 proto 生成的类型名是否匹配（`cargo build` 后查看 `abt-grpc/src/generated/` 下的文件）
- 检查 `Resource::BomCost` 是否被正确生成（proto enum 中 `BOM_COST` 会生成 Rust 的 `BomCost` 变体）

- [ ] **Step 2: 提交**

```bash
git add proto/abt/v1/permission.proto proto/abt/v1/bom.proto abt/src/models/resources.rs abt/src/models/bom.rs abt/src/service/bom_service.rs abt/src/implt/bom_service_impl.rs abt/src/repositories/product_price_repo.rs abt/migrations/028_add_bom_cost_permission.sql abt-grpc/src/handlers/convert.rs abt-grpc/src/handlers/bom.rs
git commit -m "feat: add GetBomCostReport RPC with BOM_COST:READ permission"
```
