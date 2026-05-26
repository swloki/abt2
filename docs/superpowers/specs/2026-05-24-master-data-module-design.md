# Master Data Module — abt-core 实现规格

> 基于 `docs/uml-design/09-master-data.html` v4 设计，严格遵循设计文档

## 前提

- **实现位置**：所有 UML 设计中的主数据功能统一在 `abt-core` crate 中实现
- **数据库**：`abt_v2`（通过 `ABT_CORE_DATABASE_URL` 环境变量连接），与旧 `abt` crate 的数据库完全独立
- 表名、字段、索引按设计文档全新定义，不受旧表结构影响

## 范围

在 `abt-core` crate 中实现完整的主数据模块，包含 model / repo / service trait / impl 四层。不涉及 proto 定义、gRPC handler、数据库迁移。

## 模块结构

```
abt-core/src/master_data/
├── mod.rs                     # 模块声明 + pub use 导出
├── product/
│   ├── mod.rs
│   ├── model.rs               # Product, ProductMeta, ProductStatus, ProductQuery
│   ├── repo.rs                # ProductRepo
│   ├── service.rs             # ProductService trait
│   └── implt/
│       └── mod.rs             # ProductServiceImpl
├── category/
│   ├── mod.rs
│   ├── model.rs               # Category, CategoryMeta, CategoryTree, ProductCategory
│   ├── repo.rs                # CategoryRepo
│   ├── service.rs             # CategoryService trait
│   └── implt/
│       └── mod.rs             # CategoryServiceImpl
├── price/
│   ├── mod.rs
│   ├── model.rs               # PriceLogEntry, PriceType, PriceQuery
│   ├── repo.rs                # PriceRepo
│   ├── service.rs             # ProductPriceService trait
│   └── implt/
│       └── mod.rs             # ProductPriceServiceImpl
├── bom/
│   ├── mod.rs
│   ├── model.rs               # Bom, BomDetail, BomNode, BomStatus, BomSnapshot, BomCategory,
│   │                          # BomCostReport, MaterialCostItem, LaborCostItem, BomLaborCostReport,
│   │                          # BomQuery, CreateBomReq, UpdateBomReq, NewBomNode, UpdateBomNodeReq,
│   │                          # SubstituteReq, AttributeOverrides, SubstitutionResult
│   ├── repo.rs                # BomRepo, BomNodeRepo, BomSnapshotRepo, BomCategoryRepo
│   ├── service.rs             # BomQueryService, BomCommandService, BomNodeService,
│   │                          # BomCostService, BomCategoryService traits
│   └── implt/
│       └── mod.rs             # 5 个 ServiceImpl
├── customer/
│   ├── mod.rs
│   ├── model.rs               # Customer, CustomerContact, CustomerAddress,
│   │                          # CustomerCategory, CustomerStatus, CustomerQuery
│   ├── repo.rs                # CustomerRepo, CustomerContactRepo, CustomerAddressRepo
│   ├── service.rs             # CustomerService trait
│   └── implt/
│       └── mod.rs             # CustomerServiceImpl
└── supplier/
    ├── mod.rs
    ├── model.rs               # Supplier, SupplierContact, SupplierBankAccount,
    │                          # SupplierCategory, SupplierStatus, SupplierQuery
    ├── repo.rs                # SupplierRepo, SupplierContactRepo, SupplierBankAccountRepo
    ├── service.rs             # SupplierService trait
    └── implt/
        └── mod.rs             # SupplierServiceImpl
```

## 共享层集成

### 需新增的 DomainEventType 变体

```rust
// 在 shared/enums/event.rs 中追加
BomPublished = 20,
BomUnpublished = 21,
BomNodeAdded = 22,
BomNodeUpdated = 23,
BomNodeDeleted = 24,
BomSubstituted = 25,
ProductStatusChanged = 26,
CustomerCreated = 27,
CustomerBlacklisted = 28,
CustomerTransferred = 29,
SupplierCreated = 30,
SupplierBlacklisted = 31,
SupplierBankAccountChanged = 32,
```

### 需确认的 DocumentType

DocumentSequenceService 需支持 `"PRODUCT"`、`"CUS"`、`"SUP"` 前缀。当前 DocumentType 如果是枚举，需追加对应变体；如果是 String 参数则无需改动。

### 共享服务注入方式

所有 ServiceImpl 构造函数接收共享服务实例：

```rust
pub struct ProductServiceImpl {
    doc_seq: Arc<dyn DocumentSequenceService>,
    audit: Arc<dyn AuditLogService>,
    event_bus: Arc<dyn DomainEventBus>,
    state_machine: Arc<dyn StateMachineService>,
}
```

---

## 批次 1：Product + Category + Price

### ProductService

```rust
#[async_trait]
pub trait ProductService: Send + Sync {
    async fn create(&self, ctx: ServiceContext<'_>, req: CreateProductReq) -> Result<i64, DomainError>;
    async fn update(&self, ctx: ServiceContext<'_>, id: i64, req: UpdateProductReq) -> Result<(), DomainError>;
    async fn delete(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;
    async fn get(&self, ctx: ServiceContext<'_>, id: i64) -> Result<Product, DomainError>;
    async fn get_by_ids(&self, ctx: ServiceContext<'_>, ids: Vec<i64>) -> Result<Vec<Product>, DomainError>;
    async fn list(&self, ctx: ServiceContext<'_>, filter: ProductQuery, page: PageParams) -> Result<PaginatedResult<Product>, DomainError>;
    async fn check_product_usage(&self, ctx: ServiceContext<'_>, id: i64, query: UsageQuery) -> Result<PaginatedResult<UsageEntry>, DomainError>;
}
```

**集成规则：**
- `create` → `DocumentSequenceService.next_number(ctx, "PRODUCT")` 生成编码 + `AuditLogService.record(Create)` + `StateMachineService` 初始化状态
- `update` → `AuditLogService.record(Update, changes=字段diff)`
- `delete` → 先调用 `check_product_usage` 检查引用，再软删除 + `AuditLogService.record(Delete)`
- `get` → 不存在返回 `DomainError::NotFound`

### CategoryService

```rust
#[async_trait]
pub trait CategoryService: Send + Sync {
    async fn create(&self, ctx: ServiceContext<'_>, req: CreateCategoryReq) -> Result<i64, DomainError>;
    async fn update(&self, ctx: ServiceContext<'_>, category_id: i64, req: UpdateCategoryReq) -> Result<(), DomainError>;
    async fn delete(&self, ctx: ServiceContext<'_>, category_id: i64) -> Result<(), DomainError>;
    async fn get(&self, ctx: ServiceContext<'_>, category_id: i64) -> Result<Category, DomainError>;
    async fn list(&self, ctx: ServiceContext<'_>, filter: CategoryQuery, page: PageParams) -> Result<PaginatedResult<Category>, DomainError>;
    async fn get_tree(&self, ctx: ServiceContext<'_>, root_id: Option<i64>, depth_limit: Option<i32>) -> Result<Vec<CategoryTree>, DomainError>;
    async fn move_to(&self, ctx: ServiceContext<'_>, category_id: i64, new_parent_id: i64) -> Result<(), DomainError>;
    async fn assign_products(&self, ctx: ServiceContext<'_>, category_id: i64, product_ids: Vec<i64>) -> Result<(), DomainError>;
    async fn remove_products(&self, ctx: ServiceContext<'_>, category_id: i64, product_ids: Vec<i64>) -> Result<(), DomainError>;
}
```

**关键逻辑：**
- 物化路径 `path`：`create` 时根据 `parent_id` 拼接路径，`move_to` 时 CTE 级联更新子树路径
- `delete`：检查是否有子分类或关联产品，有则拒绝（`DomainError::BusinessRule`）
- `CategoryMeta.count`：`assign_products` / `remove_products` 时更新

### ProductPriceService

```rust
#[async_trait]
pub trait ProductPriceService: Send + Sync {
    async fn update_price(&self, ctx: ServiceContext<'_>, product_id: i64, price_type: PriceType, new_price: Decimal, remark: String) -> Result<(), DomainError>;
    async fn list_price_history(&self, ctx: ServiceContext<'_>, query: PriceQuery) -> Result<PaginatedResult<PriceLogEntry>, DomainError>;
    async fn get_current_price(&self, ctx: ServiceContext<'_>, product_id: i64, price_type: PriceType) -> Result<Option<Decimal>, DomainError>;
    async fn get_price_at(&self, ctx: ServiceContext<'_>, product_id: i64, price_type: PriceType, as_of: DateTime<Utc>) -> Result<Option<Decimal>, DomainError>;
}
```

**关键逻辑：**
- `update_price`：查询当前最新价格作为 `old_price`，插入新记录 + `AuditLogService.record(Update)`
- `remark` 必填，变更原因
- `get_price_at`：`SELECT new_price FROM price_log WHERE product_id = $1 AND price_type = $2 AND created_at <= $3 ORDER BY created_at DESC LIMIT 1`

---

## 批次 2：BOM（CQRS 拆分）

### BomQueryService（只读）

```rust
#[async_trait]
pub trait BomQueryService: Send + Sync {
    async fn get(&self, ctx: ServiceContext<'_>, bom_id: i64) -> Result<Bom, DomainError>;
    async fn list(&self, ctx: ServiceContext<'_>, query: BomQuery) -> Result<PaginatedResult<Bom>, DomainError>;
    async fn get_leaf_nodes(&self, ctx: ServiceContext<'_>, bom_id: i64) -> Result<Vec<BomNode>, DomainError>;
    async fn get_snapshots(&self, ctx: ServiceContext<'_>, bom_id: i64, version: Option<i32>, limit: Option<i32>) -> Result<Vec<BomSnapshot>, DomainError>;
    async fn exists_name(&self, ctx: ServiceContext<'_>, name: &str, caller_id: Option<i64>) -> Result<bool, DomainError>;
}
```

### BomCommandService（生命周期写操作）

```rust
#[async_trait]
pub trait BomCommandService: Send + Sync {
    async fn create(&self, ctx: ServiceContext<'_>, req: CreateBomReq) -> Result<i64, DomainError>;
    async fn update(&self, ctx: ServiceContext<'_>, id: i64, req: UpdateBomReq, expected_version: i32) -> Result<(), DomainError>;
    async fn delete(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;
    async fn publish(&self, ctx: ServiceContext<'_>, id: i64) -> Result<BomSnapshot, DomainError>;
    async fn unpublish(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;
    async fn save_as(&self, ctx: ServiceContext<'_>, source_id: i64, new_name: String) -> Result<i64, DomainError>;
    async fn substitute_product(&self, ctx: ServiceContext<'_>, req: SubstituteReq) -> Result<SubstitutionResult, DomainError>;
    async fn validate_cycle(&self, ctx: ServiceContext<'_>, bom_id: i64) -> Result<(), DomainError>;
}
```

**乐观锁机制：**
- `update`：`UPDATE boms SET ..., version = version + 1 WHERE bom_id = $1 AND version = $2`，affected_rows == 0 → `DomainError::ConcurrentConflict`
- `publish`：version 自增 + 生成 BomSnapshot + `AuditLog.record(Transition)` + `EventBus.publish(BomPublished)`
- `substitute_product`：事务内 `SELECT ... FOR UPDATE` 锁定受影响节点

### BomNodeService（节点树操作）

```rust
#[async_trait]
pub trait BomNodeService: Send + Sync {
    async fn add_node(&self, ctx: ServiceContext<'_>, bom_id: i64, node: NewBomNode) -> Result<i64, DomainError>;
    async fn update_node(&self, ctx: ServiceContext<'_>, bom_id: i64, node_id: i64, req: UpdateBomNodeReq, expected_version: i32) -> Result<(), DomainError>;
    async fn delete_node(&self, ctx: ServiceContext<'_>, bom_id: i64, node_id: i64) -> Result<i64, DomainError>;
    async fn move_node(&self, ctx: ServiceContext<'_>, bom_id: i64, node_id: i64, new_parent_id: i64, before_sibling_id: Option<i64>) -> Result<(), DomainError>;
}
```

**节点操作规则：**
- `add_node`：若 order 未指定或冲突，自动按同级最大 order + 1 处理
- `move_node`：跨父节点移动，含兄弟排序 swap；插入中间位置时自动后移后续兄弟
- 所有写操作：`AuditLog.record` + `EventBus.publish(BomNodeAdded/Updated/Deleted)`
- 事务模式：`InCallerTx`

### BomCostService（成本核算）

```rust
#[async_trait]
pub trait BomCostService: Send + Sync {
    async fn get_cost_report(&self, ctx: ServiceContext<'_>, bom_id: i64, as_of_date: Option<DateTime<Utc>>) -> Result<BomCostReport, DomainError>;
}
```

- 遍历 BomNode，查询每个物料的当前价格（或 `as_of_date` 时间点价格）
- `warnings`：收集无价格数据的物料节点

### BomCategoryService

```rust
#[async_trait]
pub trait BomCategoryService: Send + Sync {
    async fn create(&self, ctx: ServiceContext<'_>, req: CreateBomCategoryReq) -> Result<i64, DomainError>;
    async fn update(&self, ctx: ServiceContext<'_>, id: i64, req: UpdateBomCategoryReq) -> Result<(), DomainError>;
    async fn delete(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;
    async fn list(&self, ctx: ServiceContext<'_>, query: BomCategoryQuery) -> Result<PaginatedResult<BomCategory>, DomainError>;
}
```

---

## 批次 3：Customer + Supplier

### CustomerService

```rust
#[async_trait]
pub trait CustomerService: Send + Sync {
    async fn create(&self, ctx: ServiceContext<'_>, req: CreateCustomerReq) -> Result<CreateCustomerResult, DomainError>;
    async fn get(&self, ctx: ServiceContext<'_>, id: i64) -> Result<Customer, DomainError>;
    async fn update(&self, ctx: ServiceContext<'_>, id: i64, req: UpdateCustomerReq) -> Result<(), DomainError>;
    async fn list(&self, ctx: ServiceContext<'_>, filter: CustomerQuery, page: PageParams) -> Result<PaginatedResult<Customer>, DomainError>;
    async fn add_contact(&self, ctx: ServiceContext<'_>, cid: i64, req: CreateContactReq) -> Result<i64, DomainError>;
    async fn update_contact(&self, ctx: ServiceContext<'_>, cid: i64, contact_id: i64, req: UpdateContactReq) -> Result<(), DomainError>;
    async fn delete_contact(&self, ctx: ServiceContext<'_>, cid: i64, contact_id: i64) -> Result<(), DomainError>;
    async fn list_contacts(&self, ctx: ServiceContext<'_>, cid: i64) -> Result<Vec<CustomerContact>, DomainError>;
    async fn add_address(&self, ctx: ServiceContext<'_>, cid: i64, req: CreateAddressReq) -> Result<i64, DomainError>;
    async fn update_address(&self, ctx: ServiceContext<'_>, cid: i64, address_id: i64, req: UpdateAddressReq) -> Result<(), DomainError>;
    async fn delete_address(&self, ctx: ServiceContext<'_>, cid: i64, address_id: i64) -> Result<(), DomainError>;
    async fn list_addresses(&self, ctx: ServiceContext<'_>, cid: i64) -> Result<Vec<CustomerAddress>, DomainError>;
    async fn validate_contact_ownership(&self, ctx: ServiceContext<'_>, cid: i64, contact_id: i64) -> Result<bool, DomainError>;
    async fn claim(&self, ctx: ServiceContext<'_>, id: i64) -> Result<(), DomainError>;
    async fn transfer(&self, ctx: ServiceContext<'_>, id: i64, new_owner_id: i64, new_department_id: Option<i64>) -> Result<(), DomainError>;
}
```

**关键逻辑：**
- `create`：`DocumentSequenceService.next_number(ctx, "CUS")` 生成编码 + `AuditLog.record(Create)` + `EventBus.publish(CustomerCreated)`
- `create` 校验 `tax_number`：跨表检索 Customer + Supplier，若已存在**不阻断但返回 Warning**（`CreateCustomerResult` 包含 `id` 和 `warnings: Vec<String>`）
- `list`：行级数据过滤，根据 `ctx.data_scope` 强制拼接 WHERE（`DataScope::All` 无过滤，`DataScope::Department` 过滤 department_id，`DataScope::SelfOnly` 过滤 owner_id），公海客户（owner_id IS NULL）对所有人可见
- `claim`：owner_id=null → owner_id=ctx.operator_id, department_id=ctx.department_id；校验客户必须为公海状态且 status=Active
- `transfer`：高权限操作，发布 `CustomerTransferred` 事件

### SupplierService

```rust
#[async_trait]
pub trait SupplierService: Send + Sync {
    async fn create(&self, ctx: ServiceContext<'_>, req: CreateSupplierReq) -> Result<CreateSupplierResult, DomainError>;
    async fn get(&self, ctx: ServiceContext<'_>, id: i64) -> Result<Supplier, DomainError>;
    async fn update(&self, ctx: ServiceContext<'_>, id: i64, req: UpdateSupplierReq) -> Result<(), DomainError>;
    async fn list(&self, ctx: ServiceContext<'_>, filter: SupplierQuery, page: PageParams) -> Result<PaginatedResult<Supplier>, DomainError>;
    async fn add_contact(&self, ctx: ServiceContext<'_>, sid: i64, req: CreateContactReq) -> Result<i64, DomainError>;
    async fn update_contact(&self, ctx: ServiceContext<'_>, sid: i64, contact_id: i64, req: UpdateContactReq) -> Result<(), DomainError>;
    async fn delete_contact(&self, ctx: ServiceContext<'_>, sid: i64, contact_id: i64) -> Result<(), DomainError>;
    async fn list_contacts(&self, ctx: ServiceContext<'_>, sid: i64) -> Result<Vec<SupplierContact>, DomainError>;
    async fn add_bank_account(&self, ctx: ServiceContext<'_>, sid: i64, req: CreateBankAccountReq) -> Result<i64, DomainError>;
    async fn update_bank_account(&self, ctx: ServiceContext<'_>, sid: i64, account_id: i64, req: UpdateBankAccountReq) -> Result<(), DomainError>;
    async fn delete_bank_account(&self, ctx: ServiceContext<'_>, sid: i64, account_id: i64) -> Result<(), DomainError>;
    async fn list_bank_accounts(&self, ctx: ServiceContext<'_>, sid: i64) -> Result<Vec<SupplierBankAccount>, DomainError>;
}
```

**关键逻辑：**
- `create`：`DocumentSequenceService.next_number(ctx, "SUP")` 生成编码 + `AuditLog.record(Create)` + `EventBus.publish(SupplierCreated)`
- `create` 校验 `tax_number`：同 Customer，不阻断但返回 Warning
- `add_bank_account` / `update_bank_account`：P0 高危操作 → 强制 `AuditLog.record`（含字段级 diff + 操作人 IP）+ `EventBus.publish(SupplierBankAccountChanged)`

---

## 状态转换规则（通过 StateMachineService）

| 实体 | 转换 |
|------|------|
| ProductStatus | Active ↔ Inactive（双向），Active/Inactive → Obsolete（不可逆），Obsolete → *（禁止） |
| BomStatus | Draft → Published（发布，生成快照），Published → Draft（取消发布，不删快照） |
| CustomerStatus | Prospective → Active，Active ↔ Inactive，Active → Blacklisted（发布事件），Blacklisted → *（禁止自动恢复） |
| SupplierStatus | Prospective → Qualified，Qualified ↔ Probation，Qualified → Disqualified，* → Blacklisted（发布事件），Blacklisted → *（禁止自动恢复） |

## 领域事件

| 事件 | 触发点 | 载荷 |
|------|--------|------|
| BomPublished | BomCommandService.publish | { bom_id, version, snapshot_id, operator_id } |
| BomUnpublished | BomCommandService.unpublish | { bom_id, version, operator_id } |
| BomNodeAdded | BomNodeService.add_node | { bom_id, node_id, product_id } |
| BomNodeUpdated | BomNodeService.update_node | { bom_id, node_id, changes } |
| BomNodeDeleted | BomNodeService.delete_node | { bom_id, node_id } |
| BomSubstituted | BomCommandService.substitute_product | { bom_id, old_product_id, new_product_id, affected_nodes } |
| ProductStatusChanged | ProductService.update (状态变更时) | { product_id, old_status, new_status } |
| CustomerCreated | CustomerService.create | { customer_id, code, category } |
| CustomerBlacklisted | CustomerService.update (拉黑时) | { customer_id } |
| CustomerTransferred | CustomerService.transfer | { customer_id, old_owner_id, new_owner_id } |
| SupplierCreated | SupplierService.create | { supplier_id, code, category } |
| SupplierBlacklisted | SupplierService.update (拉黑时) | { supplier_id } |
| SupplierBankAccountChanged | SupplierService.add/update_bank_account | { supplier_id, old_account?, new_account } |

## 事务模式

| Service 方法 | 事务模式 |
|-------------|---------|
| BomCommandService: create, update, delete, publish, unpublish, save_as | RequiresNew |
| BomCommandService: substitute_product | InCallerTx (FOR UPDATE) |
| BomNodeService: add_node, update_node, delete_node, move_node | InCallerTx |
| BomCostService: get_cost_report | None |
| BomQueryService: get, list, get_leaf_nodes, get_snapshots, exists_name | None |
| 其他所有写操作 | RequiresNew |
| 其他所有读操作 | None |
