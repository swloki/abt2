---
title: "feat: Master Data Module — abt-core implementation"
created: 2026-05-24
plan-id: "2026-05-24-002"
type: feat
depth: deep
status: active
origin: docs/superpowers/specs/2026-05-24-master-data-module-design.md
design-ref: docs/uml-design/09-master-data.html
target-crate: abt-core
target-db: abt_v2
---

# feat: Master Data Module — abt-core Implementation

## Problem Frame

ABT 系统需要一个完整的主数据模块作为所有业务模块（Sales、Purchase、WMS、MES、FMS）的上游基础数据。设计文档 `docs/uml-design/09-master-data.html` v4 已定义了完整的接口、实体和业务规则，现需要在 `abt-core` crate 中实现。

**范围**：model / repo / service trait / impl 四层，不包含 proto、gRPC handler、数据库迁移。

**数据库**：`abt_v2`（`ABT_CORE_DATABASE_URL`），与旧 `abt` crate 独立。

## Output Structure

```
abt-core/src/master_data/
├── mod.rs
├── product/
│   ├── mod.rs, model.rs, repo.rs, service.rs, implt/mod.rs
├── category/
│   ├── mod.rs, model.rs, repo.rs, service.rs, implt/mod.rs
├── price/
│   ├── mod.rs, model.rs, repo.rs, service.rs, implt/mod.rs
├── bom/
│   ├── mod.rs, model.rs, repo.rs, service.rs, implt/mod.rs
├── customer/
│   ├── mod.rs, model.rs, repo.rs, service.rs, implt/mod.rs
└── supplier/
    ├── mod.rs, model.rs, repo.rs, service.rs, implt/mod.rs
```

涉及修改的现有文件：
- `abt-core/src/lib.rs` — 添加 `pub mod master_data;`
- `abt-core/src/shared/enums/document_type.rs` — 追加 `Customer`、`Supplier` 变体
- `abt-core/src/shared/enums/event.rs` — 追加 13 个领域事件变体

## Key Technical Decisions

### KTD-1: 共享服务注入方式

ServiceImpl 通过构造函数注入 `Arc<dyn SharedService>`，与 sales 模块 skeleton 保持一致。

### KTD-2: DocumentType 扩展

`DocumentType` 是强类型枚举（`repr(i16)`），需追加：
- `Customer = 34`（Product=33 之后）
- `Supplier = 35`

BOM 不需要 DocumentSequence 编号（BOM 使用用户指定名称而非自动编号）。

### KTD-3: DomainEventType 扩展

追加 13 个变体，从值 20 开始。需确认与已有枚举值不冲突（当前最大值 19）。

### KTD-4: 事务模式

写操作使用 `TransactionMode::RequiresNew`（自管事务），BomNode/BomCommand.substitute 使用 `InCallerTx`。读操作无事务。与 `abt-core/CLAUDE.md` 中三种事务模式一致。

### KTD-5: 分页查询统一

所有 `list` 方法返回 `PaginatedResult<T>`，参数为 `PageParams`。不做裸 `Vec<T>` 返回。

### KTD-6: 软删除

Product、Customer、Supplier 使用 `deleted_at` 软删除。唯一索引使用 Partial Unique Index（`WHERE deleted_at IS NULL`）。

---

## Implementation Units

### U1. 共享层枚举扩展

**Goal**: 扩展 DocumentType 和 DomainEventType 枚举，为主数据模块提供类型基础。

**Dependencies**: None

**Files:**
- `abt-core/src/shared/enums/document_type.rs` — 追加 Customer=34, Supplier=35
- `abt-core/src/shared/enums/event.rs` — 追加 13 个 DomainEventType 变体

**Approach:**

DocumentType 追加：
- `Customer = 34`
- `Supplier = 35`

DomainEventType 追加（值从 20 开始，当前已有 1-19）：
```
BomPublished = 20, BomUnpublished = 21, BomNodeAdded = 22,
BomNodeUpdated = 23, BomNodeDeleted = 24, BomSubstituted = 25,
ProductStatusChanged = 26, CustomerCreated = 27,
CustomerBlacklisted = 28, CustomerTransferred = 29,
SupplierCreated = 30, SupplierBlacklisted = 31,
SupplierBankAccountChanged = 32
```

注意：WMS 领域事件（ArrivalNotice 等）是否已占用 20+ 的值需要检查。如果冲突，则分配到 40+ 区间。

**Patterns to follow:** 现有枚举的 `#[repr(i16)]` + `impl DocumentType` + `From<i16>` 模式。

**Test scenarios:**
- 枚举值与 i16 的双向转换正确
- 新变体的 `as_i16()` 返回预期值
- 枚举值不与现有变体冲突

**Verification:** `cargo clippy -p abt-core` 通过

---

### U2. 模块骨架 + Product Model/Repo

**Goal**: 创建 master_data 模块骨架，实现 Product 子模块的 model 和 repo 层。

**Dependencies:** U1

**Files:**
- `abt-core/src/lib.rs` — 添加 `pub mod master_data;`
- `abt-core/src/master_data/mod.rs` — 模块声明 + pub use
- `abt-core/src/master_data/product/mod.rs`
- `abt-core/src/master_data/product/model.rs` — Product, ProductMeta, ProductStatus, ProductQuery, CreateProductReq, UpdateProductReq, UsageQuery, UsageEntry
- `abt-core/src/master_data/product/repo.rs` — ProductRepo

**Approach:**

**model.rs** 定义（严格遵循 09-master-data.html 类图）：
- `ProductStatus` 枚举：Active/Inactive/Obsolete（`repr(i16)`，与共享层枚举模式一致）
- `ProductMeta` 结构体：specification, acquire_channel, old_code
- `Product` 实体：product_id, pdt_name, product_code, unit, status, external_code, owner_department_id, meta(JSONB)
- `ProductQuery`：name(filter), code(filter), status(filter), term_id(filter)
- `CreateProductReq`：name(必填), unit(必填), status, external_code, owner_department_id, meta
- `UpdateProductReq`：全部 Option 字段
- `UsageQuery` / `UsageEntry`：用于 check_product_usage

**repo.rs** 使用 sqlx 原始 SQL：
- `create(executor, req) -> Result<i64>`
- `update(executor, id, req) -> Result<()>`
- `delete(executor, id) -> Result<()>` — 软删除（SET deleted_at = NOW()）
- `find_by_id(executor, id) -> Result<Option<Product>>`
- `find_by_ids(executor, ids) -> Result<Vec<Product>>`
- `query(executor, filter, page) -> Result<PaginatedResult<Product>>`
- `check_code_unique(executor, code) -> Result<bool>`

Repo 层返回 `anyhow::Result`（与 CLAUDE.md 约定一致），DomainError 转换在 Service impl 层处理。

**Patterns to follow:** 旧 `abt/src/repositories/product_repo.rs` 的 SQL 查询模式，但使用 `PgExecutor` 和 `abt_v2` 表名。

**Test scenarios:**
- ProductStatus 枚举转换正确
- ProductMeta 的 serde JSON 序列化/反序列化
- Product 结构体的 sqlx::FromRow 派生

**Verification:** `cargo clippy -p abt-core` 通过

---

### U3. ProductService Trait + Impl

**Goal**: 定义 ProductService trait 并实现 ProductServiceImpl，集成共享基础设施。

**Dependencies:** U2

**Files:**
- `abt-core/src/master_data/product/service.rs` — ProductService trait
- `abt-core/src/master_data/product/implt/mod.rs` — ProductServiceImpl
- `abt-core/src/master_data/product/mod.rs` — pub use 导出

**Approach:**

**service.rs** — 7 个方法的 async trait（签名见规格文档）

**implt/mod.rs:**
```
pub struct ProductServiceImpl {
    repo: ProductRepo,
    doc_seq: Arc<dyn DocumentSequenceService>,
    audit: Arc<dyn AuditLogService>,
    event_bus: Arc<dyn DomainEventBus>,
    state_machine: Arc<dyn StateMachineService>,
}
```

关键实现逻辑：
- `create`: 调用 `doc_seq.next_number(ctx, DocumentType::Product)` 生成编码 → repo.create → state_machine 初始化 Active 状态 → audit.record(Create) → 返回 id
- `update`: repo.update → audit.record(Update, changes=字段diff)
- `delete`: repo.check_product_usage 检查引用 → 有引用则返回 BusinessRule 错误 → repo.delete(软删除) → audit.record(Delete)
- `get`: repo.find_by_id → None 则 NotFound
- `get_by_ids`: repo.find_by_ids
- `list`: repo.query
- `check_product_usage`: 查询 BOM/MES 等下游引用

状态变更时额外发布 `ProductStatusChanged` 事件。

**Test scenarios:**
- create 成功生成编码并返回 id
- create 时编码唯一性冲突返回 Duplicate
- get 不存在返回 NotFound
- delete 有 BOM 引用时返回 BusinessRule
- update 时字段级 diff 正确记录到 AuditLog
- 状态变更触发 ProductStatusChanged 事件

**Verification:** `cargo clippy -p abt-core` 通过

---

### U4. Category Model/Repo/Service/Impl

**Goal**: 实现产品分类模块，包含树形结构、物化路径、产品关联。

**Dependencies:** U2（Product model 定义，因为 CategoryService 操作 ProductCategory 关联）

**Files:**
- `abt-core/src/master_data/category/mod.rs`
- `abt-core/src/master_data/category/model.rs` — Category, CategoryMeta, CategoryTree, ProductCategory, CategoryQuery, CreateCategoryReq, UpdateCategoryReq
- `abt-core/src/master_data/category/repo.rs` — CategoryRepo
- `abt-core/src/master_data/category/service.rs` — CategoryService trait
- `abt-core/src/master_data/category/implt/mod.rs` — CategoryServiceImpl

**Approach:**

**model.rs:**
- `Category`：category_id, category_name, parent_id, path(物化路径如"/1/5/12/"), meta(JSONB→CategoryMeta), created_at, updated_at
- `CategoryMeta`：count(关联产品数量)
- `CategoryTree`：递归结构，包含 children: Vec<CategoryTree>
- `ProductCategory`：product_id, category_id 关联表实体

**关键逻辑：**
- `create`: 根据 parent_id 查询父分类 path，拼接新路径 `parent_path + new_id + "/"`
- `move_to`: PostgreSQL CTE 递归更新子树路径 `UPDATE categories SET path = $new_prefix || substring(path, $old_prefix_len + 1) WHERE path LIKE $old_prefix || '%'`
- `delete`: 检查子分类（`SELECT count FROM categories WHERE parent_id = $1`）和关联产品，有则拒绝
- `get_tree`: 查询所有分类后内存中构建树（depth_limit 控制层级）
- `assign_products` / `remove_products`: 操作 product_categories 关联表 + 更新 CategoryMeta.count

**Test scenarios:**
- create 正确生成物化路径
- create 顶级分类 parent_id=0 → path="/{id}/"
- move_to 级联更新子树路径
- delete 有子分类时拒绝
- delete 有关联产品时拒绝
- get_tree 正确构建树形结构
- assign_products/remove_products 正确更新 count

**Verification:** `cargo clippy -p abt-core` 通过

---

### U5. ProductPriceService Model/Repo/Service/Impl

**Goal**: 实现价格日志模块，支持多价格类型和时间点查询。

**Dependencies:** U2（Product model）

**Files:**
- `abt-core/src/master_data/price/mod.rs`
- `abt-core/src/master_data/price/model.rs` — PriceLogEntry, PriceType, PriceQuery
- `abt-core/src/master_data/price/repo.rs` — PriceRepo
- `abt-core/src/master_data/price/service.rs` — ProductPriceService trait
- `abt-core/src/master_data/price/implt/mod.rs` — ProductPriceServiceImpl

**Approach:**

**model.rs:**
- `PriceType` 枚举：Purchase/Sales/StandardCost（repr i16）
- `PriceLogEntry`：log_id, product_id, price_type, old_price, new_price, operator_id, remark, created_at

**关键逻辑：**
- `update_price`: 先查当前最新价格（`get_current_price`），作为 old_price 插入新记录。operator_id 从 ctx.operator_id 自动获取。强制 audit.record(Update)
- `get_price_at`: `WHERE created_at <= $as_of ORDER BY created_at DESC LIMIT 1`
- remark 必填，变更原因

ServiceImpl 注入：`repo`, `audit`, `event_bus`（价格变更可能不需要发事件，看设计文档未定义专门事件）。

**Test scenarios:**
- update_price 首次设价 old_price 为 None
- update_price 非首次设价 old_price 为上次 new_price
- get_current_price 无记录返回 None
- get_price_at 正确返回指定时间点的价格
- list_price_history 分页正确

**Verification:** `cargo clippy -p abt-core` 通过

---

### U6. BOM Model + BomCategory + BomQueryService

**Goal**: 实现 BOM 模块的数据模型、分类管理、只读查询服务。

**Dependencies:** U2（Product model，BomNode 引用 product_id）

**Files:**
- `abt-core/src/master_data/bom/mod.rs`
- `abt-core/src/master_data/bom/model.rs` — Bom, BomDetail, BomNode, BomStatus, BomSnapshot, BomCategory, BomCostReport, MaterialCostItem, LaborCostItem, BomLaborCostReport, BomQuery, CreateBomReq, UpdateBomReq, NewBomNode, UpdateBomNodeReq, SubstituteReq, AttributeOverrides, SubstitutionResult
- `abt-core/src/master_data/bom/repo.rs` — BomRepo, BomNodeRepo, BomSnapshotRepo, BomCategoryRepo
- `abt-core/src/master_data/bom/service.rs` — 5 个 trait
- `abt-core/src/master_data/bom/implt/mod.rs` — BomQueryServiceImpl, BomCategoryServiceImpl

**Approach:**

**model.rs** 是最大的 model 文件，包含 17+ 个类型：
- `BomStatus`：Draft/Published（repr i16）
- `Bom`：bom_id, bom_name, create_at, update_at, bom_detail(从 bom_nodes 加载), bom_category_id, status, version, published_at, created_by
- `BomDetail`：nodes: Vec<BomNode>
- `BomNode`：id, bom_id, product_id, product_code, quantity(Decimal), parent_id, loss_rate(Decimal), order, unit, remark, position, work_center, properties
- `BomSnapshot`：snapshot_id, bom_id, version, bom_name, bom_detail, published_at, published_by
- `BomCategory`：bom_category_id, bom_category_name, created_at
- 成本报告相关：BomCostReport, MaterialCostItem, LaborCostItem, BomLaborCostReport
- 请求/响应：CreateBomReq, UpdateBomReq, NewBomNode, UpdateBomNodeReq, SubstituteReq, AttributeOverrides, SubstitutionResult

**repo.rs** 分 4 个 struct：
- `BomRepo`：BOM 表 CRUD + 查询
- `BomNodeRepo`：bom_nodes 表 CRUD + 叶节点查询
- `BomSnapshotRepo`：bom_snapshots 表查询
- `BomCategoryRepo`：bom_categories 表 CRUD

**BomQueryServiceImpl:**
- `get`: 查 BOM + 加载所有 BomNode 组装 BomDetail
- `list`: 查询过滤（name, date, product, status, bom_category_id, 权限）
- `get_leaf_nodes`: 无子节点的 BomNode
- `get_snapshots`: 按 version DESC 排序，指定 version 则返回单条
- `exists_name`: 名称唯一性检查（published 或 own drafts）

**BomCategoryServiceImpl:** 标准 CRUD + 名称唯一性检查

**Test scenarios:**
- BomStatus 枚举转换
- BomDetail 从 BomNode 列表正确组装（parent_id 构建树）
- get_leaf_nodes 正确过滤叶节点
- get_snapshots 分页和 version 过滤
- BomCategory 唯一性校验

**Verification:** `cargo clippy -p abt-core` 通过

---

### U7. BomCommandService + BomNodeService

**Goal**: 实现 BOM 的生命周期写操作和节点树操作，包含乐观锁和物料替换。

**Dependencies:** U6（Bom model/repo）

**Files:**
- `abt-core/src/master_data/bom/implt/mod.rs` — 追加 BomCommandServiceImpl, BomNodeServiceImpl

**Approach:**

**BomCommandServiceImpl** 注入：`bom_repo`, `node_repo`, `snapshot_repo`, `doc_seq`(不需要，BOM 无自动编号), `audit`, `event_bus`, `state_machine`

关键实现：
- `create`: 检查 exists_name → repo.create(version=1) → audit.record(Create)
- `update`: 乐观锁 `WHERE bom_id=$1 AND version=$2` → affected_rows=0 返回 ConcurrentConflict → audit.record(Update)
- `publish`: 验证 status=Draft → 创建 BomSnapshot（当前 BomDetail 快照） → 更新 status=Published, version++ → audit.record(Transition) → event_bus.publish(BomPublished) → 返回 BomSnapshot
- `unpublish`: 验证 status=Published → 更新 status=Draft → 不删除快照 → audit.record(Transition) → event_bus.publish(BomUnpublished)
- `save_as`: 复制 BOM + 所有节点（新 version=1, status=Draft）
- `substitute_product`: 事务内 `SELECT ... FOR UPDATE` 锁定匹配节点 → 批量更新 product_id + 应用 AttributeOverrides → 返回 affected_boms/affected_nodes
- `validate_cycle`: DFS 环检测（通过 parent_id → product_id → 查找该 product_id 对应的 Bom → 递归检查）

**BomNodeServiceImpl** 注入：`node_repo`, `bom_repo`, `audit`, `event_bus`

关键实现：
- `add_node`: 自动 order = max(sibling order) + 1（若未指定） → repo.insert → audit + event(BomNodeAdded)
- `update_node`: 乐观锁验证 bom version → repo.update → audit + event(BomNodeUpdated)
- `delete_node`: repo.delete → 返回被删节点的 parent_id（设计文档返回 i64） → audit + event(BomNodeDeleted)
- `move_node`: 验证 bom version → 跨父节点移动 + 调整兄弟排序 → 事务内原子操作

**Test scenarios:**
- create 成功且 version=1
- update 乐观锁冲突返回 ConcurrentConflict
- publish 生成 BomSnapshot 且 version 自增
- unpublish 不删除快照
- save_as 深拷贝所有节点
- substitute_product 全局替换和指定 bom 替换
- validate_cycle 检测到环返回错误
- add_node 自动 order 分配
- move_node 跨父节点移动正确

**Verification:** `cargo clippy -p abt-core` 通过

---

### U8. BomCostService

**Goal**: 实现 BOM 成本核算服务。

**Dependencies:** U6（Bom model）, U5（PriceService — 需要查询物料价格）

**Files:**
- `abt-core/src/master_data/bom/implt/mod.rs` — 追加 BomCostServiceImpl

**Approach:**

**BomCostServiceImpl** 注入：`bom_query_service`(Arc<dyn BomQueryService>), `price_service`(Arc<dyn ProductPriceService>)

关键实现：
- `get_cost_report`: 查询 Bom → 获取所有 BomNode → 对每个节点查询 material price（get_current_price 或 get_price_at） → 计算 MaterialCostItem 列表 → 查询 labor costs（BomLaborCostReport，从 bom_labor_costs 表或 BomNode 的 work_center 关联） → 收集 warnings（无价格的物料） → 组装 BomCostReport

注意：labor_costs 的数据来源需要确认——设计文档有 `BomLaborCostReport` 结构体，可能来自独立的工时成本表或 BomNode 的 properties 字段。

**Test scenarios:**
- 所有物料有价格时正确计算总成本
- 部分物料无价格时 warnings 非空
- as_of_date 参数正确传递给价格查询
- 空 BOM 返回空报告

**Verification:** `cargo clippy -p abt-core` 通过

---

### U9. Customer Model/Repo/Service/Impl

**Goal**: 实现客户主数据模块，包含联系人、地址、公海/私海管理。

**Dependencies:** U1（DomainEventType.CustomerCreated 等）

**Files:**
- `abt-core/src/master_data/customer/mod.rs`
- `abt-core/src/master_data/customer/model.rs` — Customer, CustomerContact, CustomerAddress, CustomerCategory, CustomerStatus, CustomerQuery, CreateCustomerReq, UpdateCustomerReq, CreateContactReq, UpdateContactReq, CreateAddressReq, UpdateAddressReq, CreateCustomerResult
- `abt-core/src/master_data/customer/repo.rs` — CustomerRepo, CustomerContactRepo, CustomerAddressRepo
- `abt-core/src/master_data/customer/service.rs` — CustomerService trait
- `abt-core/src/master_data/customer/implt/mod.rs` — CustomerServiceImpl

**Approach:**

**model.rs:**
- `CustomerCategory` 枚举：Distributor/DirectCustomer/OEM/Retailer
- `CustomerStatus` 枚举：Prospective/Active/Inactive/Blacklisted
- `Customer`：id, code, name, short_name, category, status, tax_number, invoice_title, credit_limit, payment_terms, receivable_account, owner_id, department_id, remark, operator_id, created_at, updated_at, deleted_at
- `CustomerContact`：id, customer_id, name, position, phone, email, is_primary
- `CustomerAddress`：id, customer_id, address_type, province, city, district, detail, contact_name, contact_phone, is_default
- `CreateCustomerResult`：id + warnings: Vec<String>（tax_number 查重结果）

**repo.rs** 三个 struct：
- `CustomerRepo`：CRUD + list（支持 DataScope 过滤） + code 唯一性 + tax_number 跨表查询
- `CustomerContactRepo`：CRUD + 按 customer_id 查询
- `CustomerAddressRepo`：CRUD + 按 customer_id 查询

**CustomerServiceImpl** 关键逻辑：
- `create`: doc_seq.next_number(Customer) → repo.create → state_machine 初始化 Prospective → tax_number 跨表查重（不阻断） → audit.record → event_bus.publish(CustomerCreated) → 返回 CreateCustomerResult{id, warnings}
- `list`: **行级数据过滤** — 根据 ctx.data_scope 在 repo 层强制拼接 WHERE。公海客户（owner_id IS NULL）对所有人可见
- `claim`: 验证 owner_id IS NULL + status=Active → SET owner_id=ctx.operator_id, department_id=ctx.department_id
- `transfer`: 验证权限（需高权限角色） → SET owner_id, department_id → event_bus.publish(CustomerTransferred)
- `validate_contact_ownership`: list_contacts → 内存比对 contact_id

**Test scenarios:**
- create 自动生成 CUS 编码
- create 时 tax_number 已存在返回 warnings 但不阻断
- list 按 DataScope::SelfOnly 过滤
- list 公海客户对所有人可见
- claim 非公海客户返回错误
- claim status 非 Active 返回错误
- transfer 发布 CustomerTransferred 事件
- validate_contact_ownership 正确校验归属
- contact/address 的 CRUD 操作正确

**Verification:** `cargo clippy -p abt-core` 通过

---

### U10. Supplier Model/Repo/Service/Impl

**Goal:** 实现供应商主数据模块，包含联系人、银行账户（P0 风控）。

**Dependencies:** U1（DomainEventType.SupplierCreated 等）

**Files:**
- `abt-core/src/master_data/supplier/mod.rs`
- `abt-core/src/master_data/supplier/model.rs` — Supplier, SupplierContact, SupplierBankAccount, SupplierCategory, SupplierStatus, SupplierQuery, CreateSupplierReq, UpdateSupplierReq, CreateSupplierResult, CreateContactReq, UpdateContactReq, CreateBankAccountReq, UpdateBankAccountReq
- `abt-core/src/master_data/supplier/repo.rs` — SupplierRepo, SupplierContactRepo, SupplierBankAccountRepo
- `abt-core/src/master_data/supplier/service.rs` — SupplierService trait
- `abt-core/src/master_data/supplier/implt/mod.rs` — SupplierServiceImpl

**Approach:**

**model.rs:**
- `SupplierCategory` 枚举：RawMaterial/Packaging/Outsourcing/Consumable/Service
- `SupplierStatus` 枚举：Prospective/Qualified/Probation/Disqualified/Blacklisted
- `Supplier`：id, code, name, short_name, category, status, tax_number, lead_time_days, payment_terms, remark, operator_id, created_at, updated_at, deleted_at
- `SupplierContact`：id, supplier_id, name, position, phone, email, is_primary
- `SupplierBankAccount`：id, supplier_id, bank_name, account_name, account_number, is_default
- `CreateSupplierResult`：id + warnings（同 Customer tax_number 查重）

**P0 银行账户风控**（设计文档明确标注）：
- `add_bank_account` / `update_bank_account`：强制 audit.record（字段级 diff + 操作人 IP） → event_bus.publish(SupplierBankAccountChanged)
- audit context 中记录完整变更前后对比

**SupplierServiceImpl** 关键逻辑：
- `create`: doc_seq.next_number(Supplier) → repo.create → state_machine 初始化 Prospective → tax_number 查重（不阻断） → audit → event(SupplierCreated) → 返回 CreateSupplierResult{id, warnings}
- `list`: 供应商不需要 DataScope 过滤（无公海/私海概念）
- `update`: 状态变更到 Blacklisted 时发布 SupplierBlacklisted 事件

**Test scenarios:**
- create 自动生成 SUP 编码
- create 时 tax_number 查重 warnings
- add_bank_account 发布 SupplierBankAccountChanged 事件
- update_bank_account 字段级 diff 正确记录
- 状态变更到 Blacklisted 发布事件
- contact/bank_account 的 CRUD 正确

**Verification:** `cargo clippy -p abt-core` 通过

---

## Scope Boundaries

### In Scope
- 6 个子模块的 model/repo/service trait/impl
- 共享层枚举扩展（DocumentType、DomainEventType）
- lib.rs 模块注册
- 所有共享基础设施集成点（AuditLog、EventBus、StateMachine、DocumentSequence）

### Deferred to Follow-Up Work
- 数据库迁移文件（`abt-core/migrations/` 中新增 002_create_master_data.sql）
- Proto 定义（`proto/abt/v1/` 中新增 master_data.proto 或拆分为 product.proto/bom.proto/customer.proto/supplier.proto）
- gRPC Handler（`abt-grpc/src/handlers/` 中新增对应 handler）
- `abt-grpc/src/server.rs` 中注册新 handler
- 工厂函数（`abt-core/src/lib.rs` 或独立文件中的 service 工厂）
- ServiceImpl 的单元测试和集成测试

---

## Dependency Graph

```
U1 (枚举扩展)
├── U2 (Product Model/Repo)
│   ├── U3 (ProductService)
│   ├── U4 (Category) ← depends on Product model
│   ├── U5 (PriceService) ← depends on Product model
│   └── U6 (BOM Model/BomQuery/BomCategory) ← depends on Product model
│       ├── U7 (BomCommand/BomNode) ← depends on U6
│       └── U8 (BomCost) ← depends on U6 + U5
├── U9 (Customer) ← depends on U1 only
└── U10 (Supplier) ← depends on U1 only
```

可并行：U3+U4+U5（互不依赖），U9+U10（互不依赖），U7+U8（U8 依赖 U5 但可延迟）
