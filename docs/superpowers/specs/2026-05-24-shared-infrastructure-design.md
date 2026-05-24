# 共享基础设施层实现设计

> 基于 `docs/uml-design/00-shared-infrastructure.html` 设计文档
> 实现目标：在 `abt-core` crate 中完整实现 10 个共享基础设施组件

## 范围

在 `abt-core` crate 中实现全部 10 个组件，不修改 `abt` crate 代码。严格遵循 `docs/uml-design/00-shared-infrastructure.html` 设计文档。

## 文件结构

```
abt-core/src/shared/
  types/                         ← 新建：共享基础类型
    mod.rs
    context.rs                   ← ServiceContext, DataScope
    error.rs                     ← DomainError + tonic::Status 映射
    pagination.rs                ← PaginatedResult<T>, PageParams
    batch.rs                     ← BatchResult, BatchMode, BatchFailure
    transaction.rs               ← TransactionMode
  enums/                         ← 新建：共享枚举
    mod.rs
    document_type.rs             ← DocumentType (33 变体)
    link_type.rs                 ← LinkType (7 变体)
    reservation.rs               ← ReservationType, ReservationStatus
    cost.rs                      ← CostType, CostEntityType
    event.rs                     ← DomainEventType (19 变体), EventStatus
    audit.rs                     ← AuditAction
    side_effect.rs               ← SideEffect
  document_sequence/             ← 已有骨架，填充实现
    mod.rs, model.rs, repo.rs, service.rs, implt/mod.rs
  document_link/                 ← 已有骨架，填充实现
    mod.rs, model.rs, repo.rs, service.rs, implt/mod.rs
  inventory_reservation/         ← 已有骨架，填充实现
    mod.rs, model.rs, repo.rs, service.rs, implt/mod.rs
  cost_entry/                    ← 已有骨架，填充实现
    mod.rs, model.rs, repo.rs, service.rs, implt/mod.rs
  event_bus/                     ← 已有骨架，填充实现
    mod.rs, model.rs, service.rs, implt/mod.rs
    registry.rs                  ← EventHandlerRegistry + EventHandler trait
    processor.rs                 ← EventProcessor (LISTEN/NOTIFY + 轮询)
    dead_letter.rs               ← DeadLetterService trait + impl
  state_machine/                 ← 已有骨架，填充实现
    mod.rs, model.rs, service.rs, implt/mod.rs
  audit_log/                     ← 已有骨架，填充实现
    mod.rs, model.rs, repo.rs, service.rs, implt/mod.rs
  idempotency/                   ← 已有骨架，填充实现
    mod.rs, model.rs, repo.rs, service.rs, implt/mod.rs
  identity/                      ← 新建：身份认证与权限
    mod.rs, model.rs, repo.rs
    user_service.rs, role_service.rs, auth_service.rs
    permission_service.rs, department_service.rs
    implt/ (5 个实现文件)
    permission_cache.rs          ← RolePermissionCache
```

## 依赖

需要在 `abt-core/Cargo.toml` 添加：
- `thiserror` — DomainError 派生
- `rust_decimal` — 已有，确保 Decimal 类型可用
- `chrono` — 已有，确保 DateTime 可用

## 基础类型

### DomainError (`shared/types/error.rs`)

```rust
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("{0} not found")]
    NotFound(String),
    #[error("{0} already exists")]
    Duplicate(String),
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    #[error("Business rule: {0}")]
    BusinessRule(String),
    #[error("Validation: {0}")]
    Validation(String),
    #[error("Concurrent conflict")]
    ConcurrentConflict,
    #[error("Invalid state transition: {from} -> {to}")]
    InvalidStateTransition { from: String, to: String },
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}
```

gRPC 映射：
- NotFound → NOT_FOUND
- Duplicate → ALREADY_EXISTS
- PermissionDenied → PERMISSION_DENIED
- BusinessRule → FAILED_PRECONDITION
- Validation → INVALID_ARGUMENT
- ConcurrentConflict → ABORTED
- InvalidStateTransition → FAILED_PRECONDITION
- Internal → INTERNAL（日志记录完整错误链，返回泛化消息）

### ServiceContext (`shared/types/context.rs`)

```rust
pub struct ServiceContext<'a> {
    pub executor: PgExecutor<'a>,
    pub operator_id: i64,
    pub department_id: Option<i64>,
    pub data_scope: DataScope,
    pub trace_id: Option<String>,
    pub request_id: Option<String>,
}

pub enum DataScope { All, Department, Self }
```

### PaginatedResult (`shared/types/pagination.rs`)

```rust
pub struct PaginatedResult<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub page: u32,
    pub page_size: u32,
    pub total_pages: u32,
}

pub struct PageParams {
    pub page: u32,
    pub page_size: u32,
}
```

### BatchResult (`shared/types/batch.rs`)

```rust
pub struct BatchResult {
    pub success_count: i32,
    pub failed_items: Vec<BatchFailure>,
    pub total: i32,
    pub mode: BatchMode,
}

pub enum BatchMode { Atomic, ContinueOnError }

pub struct BatchFailure {
    pub index: i32,
    pub error: DomainError,
}
```

### TransactionMode (`shared/types/transaction.rs`)

```rust
pub enum TransactionMode {
    InCallerTx,
    IndependentTx,
    AsyncOutbox,
}
```

## 枚举定义

### DocumentType (33 变体)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(type_name = "smallint", rename_all = "snake_case")]
pub enum DocumentType {
    // Sales CRM
    Quotation = 1, SalesOrder = 2, ShippingRequest = 3, SalesReturn = 4, Reconciliation = 5,
    // Purchase SRM
    PurchaseQuotation = 6, PurchaseOrder = 7, PurchaseReturn = 8, MiscellaneousRequest = 9,
    // MES
    WorkOrder = 10, OutsourcingOrder = 11, ProductionPlan = 12, WorkReport = 13,
    ProductionInspection = 14, ProductionReceipt = 15,
    // WMS
    ArrivalNotice = 16, MaterialRequisition = 17, Backflush = 18, CycleCount = 19,
    InventoryTransfer = 20, FormConversion = 21, InventoryLock = 22,
    PaymentRequest = 23, Invoice = 24,
    // OM
    OutsourcingTracking = 25,
    // QMS
    InspectionSpecification = 26, InspectionResult = 27, MRB = 28, RMA = 29,
    // FMS
    CashJournal = 30, WriteOff = 31, ExpenseReimbursement = 32,
    // Master Data
    Product = 33,
}
```

### 其他枚举

- **LinkType**: DerivedFrom(1), Triggers(2), References(3), Reconciles(4), Inspects(5), Fulfills(6), Allocates(7)
- **ReservationType**: Hard(1), Soft(2), SafetyStock(3)
- **ReservationStatus**: Active(1), Fulfilled(2), Cancelled(3), Expired(4)
- **CostType**: Material(1), Labor(2), Overhead(3), Outsource(4), Rework(5), Scrap(6)
- **CostEntityType**: Product(1), WorkOrder(2), SalesOrder(3), PurchaseOrder(4), Inspection(5)
- **DomainEventType**: 19 变体（SalesOrderConfirmed, SalesOrderShipped, SalesReturnReceived, PurchaseOrderConfirmed, ArrivalNoticeReceived, POConfirmed, PaymentPaid, PlanReleased, WOReleased, WOCompleted, ReceiptConfirmed, OutsourcingSent, OutsourcingReceived, ConvertedToInternal, InspectionPassed, InspectionFailed, MRBDispositioned, RMACreated, CashJournalConfirmed, WriteOffCompleted）
- **EventStatus**: Pending(1), Processing(2), Processed(3), Failed(4), DeadLetter(5)
- **AuditAction**: Create(1), Update(2), Delete(3), Transition(4)
- **SideEffect**: PublishEvent, Notify, TriggerWorkflow, UpdateField（JSONB 存储）
- **SequenceStrategy**: Sequential(1), Timestamp(2)

## 共享服务接口

### DocumentSequenceService

```rust
#[async_trait]
pub trait DocumentSequenceService: Send + Sync {
    /// 生成下一个单据编号
    /// Sequential: PREFIX-YYYY-MM-SEQ (如 SO-2026-05-00142)
    /// Timestamp: x{unix_timestamp} (如 x1747891200)
    async fn next_number(
        ctx: &ServiceContext<'_>,
        doc_type: DocumentType,
    ) -> Result<String, DomainError>;
}
```

实现要点：
- Sequential 策略：`INSERT ... ON CONFLICT (prefix, seq_date) DO UPDATE SET current_value = document_sequences.current_value + 1 RETURNING current_value`
- 按 prefix + seq_date 唯一约束
- 每月自动从 001 开始

### DocumentLinkService

```rust
#[async_trait]
pub trait DocumentLinkService: Send + Sync {
    /// 批量创建关联（Atomic 模式）
    async fn create_links(
        ctx: &ServiceContext<'_>,
        requests: Vec<LinkRequest>,
    ) -> Result<BatchResult, DomainError>;

    /// 查询关联文档（分页）
    async fn find_linked(
        ctx: &ServiceContext<'_>,
        source_type: DocumentType,
        source_id: i64,
        page: PageParams,
    ) -> Result<PaginatedResult<DocumentLink>, DomainError>;
}
```

实现要点：
- create_links 固定 Atomic 模式
- path 物化路径：`parent_path + "." + LINK_TYPE:source_id-TARGET_TYPE:target_id`
- 支持 LIKE 查询整条链路

### InventoryReservationService

```rust
#[async_trait]
pub trait InventoryReservationService: Send + Sync {
    /// 批量预留（ContinueOnError 模式）
    async fn reserve(
        ctx: &ServiceContext<'_>,
        requests: Vec<ReserveRequest>,
    ) -> Result<BatchResult, DomainError>;

    /// 完成预留（消耗库存）
    async fn fulfill(ctx: &ServiceContext<'_>, id: i64) -> Result<(), DomainError>;

    /// 取消预留
    async fn cancel(ctx: &ServiceContext<'_>, id: i64) -> Result<(), DomainError>;

    /// 查询已预留总量
    async fn total_reserved(
        ctx: &ServiceContext<'_>,
        product_id: i64,
        warehouse_id: Option<i64>,
    ) -> Result<Decimal, DomainError>;
}
```

实现要点：
- reserve 固定 ContinueOnError 模式
- 并发控制：`SELECT ... FOR UPDATE` 锁定行
- 可用量 = OnHand - total_reserved

### CostEntryService

```rust
#[async_trait]
pub trait CostEntryService: Send + Sync {
    /// 批量写入成本分录（Atomic 模式）
    async fn create_entries(
        ctx: &ServiceContext<'_>,
        entries: Vec<EntryRequest>,
    ) -> Result<BatchResult, DomainError>;

    /// 按实体查询成本（分页）
    async fn find_by_entity(
        ctx: &ServiceContext<'_>,
        entity_type: CostEntityType,
        entity_id: i64,
        page: PageParams,
    ) -> Result<PaginatedResult<CostEntry>, DomainError>;
}
```

实现要点：
- create_entries 固定 Atomic 模式（双层记账必须完整）
- period 格式 "YYYY-MM"

### DomainEventBus

```rust
#[async_trait]
pub trait DomainEventBus: Send + Sync {
    /// 发布事件（写入 Outbox + NOTIFY）
    async fn publish(
        ctx: &ServiceContext<'_>,
        req: EventPublishRequest,
    ) -> Result<i64, DomainError>;

    /// 批量标记已处理
    async fn mark_processed(
        ctx: &ServiceContext<'_>,
        ids: Vec<i64>,
    ) -> Result<u64, DomainError>;

    /// 标记失败
    async fn mark_failed(
        ctx: &ServiceContext<'_>,
        id: i64,
        reason: &str,
    ) -> Result<(), DomainError>;

    /// 统一查询事件
    async fn find_events(
        ctx: &ServiceContext<'_>,
        query: EventQuery,
    ) -> Result<PaginatedResult<DomainEvent>, DomainError>;
}
```

EventPublishRequest：
```rust
pub struct EventPublishRequest {
    pub event_type: DomainEventType,
    pub aggregate_type: String,
    pub aggregate_id: i64,
    pub payload: Value,
    pub idempotency_key: Option<String>,
}
```

实现要点：
- publish 写入 domain_events 表 + `NOTIFY domain_event, '{id}'`
- idempotency_key: None 时自动生成 `{aggregate_type}:{aggregate_id}:{event_type}`
- INSERT ON CONFLICT (idempotency_key) DO NOTHING

### EventHandlerRegistry + EventHandler

```rust
#[async_trait]
pub trait EventHandler: Send + Sync {
    async fn handle(&self, ctx: &ServiceContext<'_>, event: &DomainEvent) -> Result<(), DomainError>;
    fn name(&self) -> &str;
}

pub trait EventHandlerRegistry: Send + Sync {
    fn register(&self, event_type: DomainEventType, handler: Arc<dyn EventHandler>);
    async fn dispatch(&self, ctx: &ServiceContext<'_>, event: &DomainEvent) -> Result<(), DomainError>;
}
```

### EventProcessor

```rust
pub struct EventProcessor {
    pool: Arc<PgPool>,
    registry: Arc<dyn EventHandlerRegistry>,
    idempotency: Arc<dyn IdempotencyService>,
    dead_letter: Arc<dyn DeadLetterService>,
    max_retries: i32,  // default 3
    running: Arc<AtomicBool>,
}
```

方法：
- `start()` → spawn 后台 tokio task
- `stop()` → 优雅关闭
- `is_running()` → bool
- `last_processed_at()` → Option<DateTime>
- `retry_failed()` → Result<u64>

工作流：
1. LISTEN domain_event → 收到通知
2. FETCH FOR UPDATE SKIP LOCKED
3. check_and_mark (idempotency)
4. registry.dispatch()
5. 成功 → mark_processed | 失败 → mark_failed + 指数退避
6. retry_count > max_retries → DEAD_LETTER
7. 30s 轮询兜底：扫描 status=PENDING 且 created_at < now()-30s

### DeadLetterService

```rust
#[async_trait]
pub trait DeadLetterService: Send + Sync {
    async fn list_dead_letters(ctx: &ServiceContext<'_>, page: PageParams) -> Result<PaginatedResult<DomainEvent>, DomainError>;
    async fn retry_one(ctx: &ServiceContext<'_>, event_id: i64) -> Result<(), DomainError>;
    async fn archive(ctx: &ServiceContext<'_>, before: DateTime<Utc>) -> Result<u64, DomainError>;
}
```

### StateMachineService

```rust
#[async_trait]
pub trait StateMachineService: Send + Sync {
    async fn configure(
        ctx: &ServiceContext<'_>,
        entity_type: &str,
        states: Vec<StateDef>,
        transitions: Vec<TransitionDef>,
    ) -> Result<(), DomainError>;

    async fn transition(
        ctx: &ServiceContext<'_>,
        entity_type: &str,
        entity_id: i64,
        to_state: &str,
        remark: Option<String>,
    ) -> Result<EntityStateLog, DomainError>;

    async fn get_current_state(
        ctx: &ServiceContext<'_>,
        entity_type: &str,
        entity_id: i64,
    ) -> Result<String, DomainError>;

    async fn get_allowed_transitions(
        ctx: &ServiceContext<'_>,
        entity_type: &str,
        state: &str,
    ) -> Result<Vec<String>, DomainError>;

    async fn get_state_history(
        ctx: &ServiceContext<'_>,
        entity_type: &str,
        entity_id: i64,
        page: PageParams,
    ) -> Result<PaginatedResult<EntityStateLog>, DomainError>;
}
```

transition() 校验流程：
1. 查询当前状态（from_state）
2. 匹配转换规则（from_state → to_state）
3. 校验 guard_condition（如果存在）
4. 插入 EntityStateLog（追加写）
5. 执行 side_effects（发布事件等）

SideEffect 枚举存为 JSONB：
```rust
pub enum SideEffect {
    PublishEvent { event_type: DomainEventType, payload_template: Value },
    Notify { role_ids: Vec<i64>, template: String },
    TriggerWorkflow { definition_id: i64 },
    UpdateField { field: String, value_template: Value },
}
```

### AuditLogService

```rust
#[async_trait]
pub trait AuditLogService: Send + Sync {
    async fn record(
        ctx: &ServiceContext<'_>,
        entity_type: &str,
        entity_id: i64,
        action: AuditAction,
        changes: Option<Value>,
        context: Option<Value>,
    ) -> Result<i64, DomainError>;

    async fn query_logs(
        ctx: &ServiceContext<'_>,
        query: AuditLogQuery,
    ) -> Result<PaginatedResult<AuditLog>, DomainError>;
}
```

实现要点：
- 同事务内写入（InCallerTx）
- Append-only 不可变
- changes 支持敏感字段标记 `sensitive: true`，record 内部自动脱敏
- 查询支持 entity_type, operator_id, action, time_range 过滤

### IdempotencyService

```rust
#[async_trait]
pub trait IdempotencyService: Send + Sync {
    async fn check_and_mark(
        ctx: &ServiceContext<'_>,
        event_id: i64,
        handler_name: &str,
    ) -> Result<bool, DomainError>;

    async fn mark_processed(
        ctx: &ServiceContext<'_>,
        event_id: i64,
        handler_name: &str,
        result: Option<Value>,
    ) -> Result<(), DomainError>;

    async fn cleanup_expired(
        ctx: &ServiceContext<'_>,
        before: DateTime<Utc>,
    ) -> Result<u64, DomainError>;
}
```

实现要点：
- idempotency_key = `{event_id}:{handler_name}` UNIQUE
- INSERT ON CONFLICT DO NOTHING
- 三级幂等：API 防重复提交 + Handler 防重复消费 + Saga 补偿防重复执行

## Identity & Access（⑩）

### 模块结构

```
shared/identity/
  mod.rs
  model.rs                  ← User, Role, Department, Claims, AuthContext, ResourceActionDef
  repo.rs                   ← SQL queries
  permission_cache.rs       ← RolePermissionCache (RwLock<HashMap>)
  user_service.rs           ← UserService trait
  role_service.rs           ← RoleService trait
  auth_service.rs           ← AuthService trait
  permission_service.rs     ← PermissionService trait
  department_service.rs     ← DepartmentService trait
  implt/
    user_service_impl.rs
    role_service_impl.rs
    auth_service_impl.rs
    permission_service_impl.rs
    department_service_impl.rs
```

### Service Traits

```rust
// UserService
#[async_trait]
pub trait UserService: Send + Sync {
    async fn create_user(ctx: &ServiceContext<'_>, req: CreateUserRequest) -> Result<User, DomainError>;
    async fn update_user(ctx: &ServiceContext<'_>, req: UpdateUserRequest) -> Result<User, DomainError>;
    async fn delete_user(ctx: &ServiceContext<'_>, user_id: i64) -> Result<(), DomainError>;
    async fn get_user(user_id: i64) -> Result<User, DomainError>;
    async fn list_users(query: UserQuery) -> Result<PaginatedResult<User>, DomainError>;
    async fn batch_assign_roles(ctx: &ServiceContext<'_>, user_id: i64, role_ids: Vec<i64>) -> Result<(), DomainError>;
}

// RoleService
#[async_trait]
pub trait RoleService: Send + Sync {
    async fn create_role(ctx: &ServiceContext<'_>, req: CreateRoleRequest) -> Result<Role, DomainError>;
    async fn update_role(ctx: &ServiceContext<'_>, req: UpdateRoleRequest) -> Result<Role, DomainError>;
    async fn delete_role(ctx: &ServiceContext<'_>, role_id: i64) -> Result<(), DomainError>;
    async fn list_roles() -> Result<Vec<Role>, DomainError>;
    async fn assign_permissions(ctx: &ServiceContext<'_>, role_id: i64, permissions: Vec<(String, String)>) -> Result<(), DomainError>;
    async fn remove_permissions(ctx: &ServiceContext<'_>, role_id: i64, permissions: Vec<(String, String)>) -> Result<(), DomainError>;
}

// AuthService
#[async_trait]
pub trait AuthService: Send + Sync {
    async fn login(username: &str, password: &str) -> Result<AuthResponse, DomainError>;
    async fn refresh_token(token: &str) -> Result<AuthResponse, DomainError>;
    async fn get_user_claims(user_id: i64) -> Result<Claims, DomainError>;
    async fn list_resources() -> Vec<ResourceActionDef>;
}

// PermissionService
#[async_trait]
pub trait PermissionService: Send + Sync {
    async fn check_permission(user_id: i64, resource: &str, action: &str) -> Result<bool, DomainError>;
    async fn batch_check_permissions(user_id: i64, pairs: Vec<(&str, &str)>) -> Result<Vec<bool>, DomainError>;
    async fn get_user_permissions(user_id: i64) -> Result<Vec<String>, DomainError>;
}

// DepartmentService
#[async_trait]
pub trait DepartmentService: Send + Sync {
    async fn create_department(ctx: &ServiceContext<'_>, req: CreateDepartmentRequest) -> Result<Department, DomainError>;
    async fn update_department(ctx: &ServiceContext<'_>, req: UpdateDepartmentRequest) -> Result<Department, DomainError>;
    async fn delete_department(ctx: &ServiceContext<'_>, department_id: i64) -> Result<(), DomainError>;
    async fn list_departments() -> Result<Vec<Department>, DomainError>;
    async fn assign_departments(ctx: &ServiceContext<'_>, user_id: i64, department_ids: Vec<i64>) -> Result<(), DomainError>;
    async fn remove_departments(ctx: &ServiceContext<'_>, user_id: i64, department_ids: Vec<i64>) -> Result<(), DomainError>;
}
```

### 关键设计

- **JWT Claims 只存 ID**：sub, role_ids, department_ids 等轻量标识
- **RolePermissionCache**：进程内 RwLock<HashMap<i64, HashSet<String>>>，启动全量加载，角色变更时 reload
- **密码哈希**：argon2
- **16 种资源定义**：PRODUCT, CATEGORY, BOM, BOM_CATEGORY, WAREHOUSE, LOCATION, INVENTORY, PRICE, SALES_ORDER, PURCHASE_ORDER, WORK_ORDER, INSPECTION, COST, USER, ROLE, DEPARTMENT
- **super_admin** 绕过所有权限检查

## 数据库迁移

新增迁移文件 `abt/migrations/055_create_shared_infrastructure.sql`：

### document_links
```sql
CREATE TABLE document_links (
    id BIGSERIAL PRIMARY KEY,
    source_type SMALLINT NOT NULL,
    source_id BIGINT NOT NULL,
    target_type SMALLINT NOT NULL,
    target_id BIGINT NOT NULL,
    link_type SMALLINT NOT NULL,
    path TEXT NOT NULL,
    depth INT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by BIGINT
);
CREATE INDEX idx_doc_links_source ON document_links(source_type, source_id);
CREATE INDEX idx_doc_links_target ON document_links(target_type, target_id);
CREATE INDEX idx_doc_links_path ON document_links USING gin(path gin_trgm_ops);
```

### inventory_reservations
```sql
CREATE TABLE inventory_reservations (
    id BIGSERIAL PRIMARY KEY,
    product_id BIGINT NOT NULL REFERENCES products(id),
    warehouse_id BIGINT NOT NULL REFERENCES warehouses(id),
    reserved_qty DECIMAL(18,6) NOT NULL,
    reservation_type SMALLINT NOT NULL DEFAULT 1,
    source_type SMALLINT NOT NULL,
    source_id BIGINT NOT NULL,
    source_line_id BIGINT,
    status SMALLINT NOT NULL DEFAULT 1,
    priority INT NOT NULL DEFAULT 5,
    expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_reservations_product_warehouse ON inventory_reservations(product_id, warehouse_id, status);
CREATE INDEX idx_reservations_source ON inventory_reservations(source_type, source_id);
```

### cost_entries
```sql
CREATE TABLE cost_entries (
    id BIGSERIAL PRIMARY KEY,
    entity_type SMALLINT NOT NULL,
    entity_id BIGINT NOT NULL,
    cost_type SMALLINT NOT NULL,
    debit_amount DECIMAL(20,4) NOT NULL DEFAULT 0,
    credit_amount DECIMAL(20,4) NOT NULL DEFAULT 0,
    cost_center BIGINT,
    profit_center BIGINT,
    period VARCHAR(7) NOT NULL,
    source_type SMALLINT NOT NULL,
    source_id BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_cost_entries_entity ON cost_entries(entity_type, entity_id);
CREATE INDEX idx_cost_entries_period ON cost_entries(period);
```

### domain_events (Outbox)
```sql
CREATE TABLE domain_events (
    id BIGSERIAL PRIMARY KEY,
    event_type SMALLINT NOT NULL,
    event_version INT NOT NULL DEFAULT 1,
    aggregate_type VARCHAR(64) NOT NULL,
    aggregate_id BIGINT NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}',
    operator_id BIGINT NOT NULL,
    idempotency_key VARCHAR(255) NOT NULL UNIQUE,
    trace_id VARCHAR(64),
    request_id VARCHAR(64),
    status SMALLINT NOT NULL DEFAULT 1,
    retry_count INT NOT NULL DEFAULT 0,
    processed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_events_status ON domain_events(status) WHERE status IN (1, 2, 4);
CREATE INDEX idx_events_aggregate ON domain_events(aggregate_type, aggregate_id);
```

### state_definitions
```sql
CREATE TABLE state_definitions (
    id BIGSERIAL PRIMARY KEY,
    entity_type VARCHAR(64) NOT NULL,
    state_name VARCHAR(64) NOT NULL,
    label VARCHAR(128) NOT NULL,
    is_initial BOOLEAN NOT NULL DEFAULT false,
    is_final BOOLEAN NOT NULL DEFAULT false,
    UNIQUE(entity_type, state_name)
);

CREATE TABLE state_transition_defs (
    id BIGSERIAL PRIMARY KEY,
    entity_type VARCHAR(64) NOT NULL,
    from_state VARCHAR(64) NOT NULL,
    to_state VARCHAR(64) NOT NULL,
    trigger_event SMALLINT,
    guard_condition JSONB,
    side_effects JSONB NOT NULL DEFAULT '[]',
    sort_order INT NOT NULL DEFAULT 0,
    UNIQUE(entity_type, from_state, to_state)
);

CREATE TABLE entity_state_logs (
    id BIGSERIAL PRIMARY KEY,
    entity_type VARCHAR(64) NOT NULL,
    entity_id BIGINT NOT NULL,
    from_state VARCHAR(64),
    to_state VARCHAR(64) NOT NULL,
    transition_id BIGINT NOT NULL REFERENCES state_transition_defs(id),
    operator_id BIGINT NOT NULL,
    remark TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_state_logs_entity ON entity_state_logs(entity_type, entity_id);
```

### audit_logs
```sql
CREATE TABLE audit_logs (
    id BIGSERIAL PRIMARY KEY,
    entity_type VARCHAR(64) NOT NULL,
    entity_id BIGINT NOT NULL,
    action SMALLINT NOT NULL,
    changes JSONB,
    operator_id BIGINT NOT NULL,
    context JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_audit_entity ON audit_logs(entity_type, entity_id);
CREATE INDEX idx_audit_operator ON audit_logs(operator_id);
CREATE INDEX idx_audit_created ON audit_logs(created_at);
```

### idempotency_records
```sql
CREATE TABLE idempotency_records (
    id BIGSERIAL PRIMARY KEY,
    idempotency_key VARCHAR(255) NOT NULL UNIQUE,
    event_id BIGINT NOT NULL REFERENCES domain_events(id),
    handler_name VARCHAR(128) NOT NULL,
    status VARCHAR(16) NOT NULL DEFAULT 'Processing',
    result JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ
);
CREATE INDEX idx_idempotency_event_handler ON idempotency_records(event_id, handler_name);
```

## 实现批次

| 批次 | 内容 | 文件数 | 依赖 |
|------|------|--------|------|
| 1 | types/ (5 文件) + enums/ (9 文件) | 14 | 无 |
| 2 | DocumentSequence + AuditLog + Idempotency | ~15 | 批次 1 |
| 3 | DocumentLink + InventoryReservation + CostEntry | ~15 | 批次 1 |
| 4 | DomainEventBus + EventProcessor + DeadLetter + Registry | ~12 | 批次 1-2 |
| 5 | StateMachineService | ~8 | 批次 1-4 |
| 6 | Identity & Access | ~15 | 批次 1-2 |
| 7 | 数据库迁移 | 1 | 批次 1-6 |

总计约 80 个文件（含 mod.rs），实际新写约 60 个有内容的文件。

## 约束

- **不修改 abt crate 代码** — 所有实现只在 abt-core
- **严格遵循设计文档** — 接口签名、数据模型、组件关系均按 `00-shared-infrastructure.html`
- **ServiceError 保持 deprecated 不删** — abt crate 老代码继续使用
- **验证手段** — `cargo clippy -p abt-core` + `cargo test -p abt-core`
