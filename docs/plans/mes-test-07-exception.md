# 生产异常实现计划

**路径**: `/admin/mes/exceptions`
**复杂度**: ★★★（最复杂，需新建完整子模块 + 数据库表）

## Context

当前 `mes_exception_list.rs` 是 stub，路由硬编码在 `routes/mod.rs` 中。原型 `04-exception-list.html` + `04-exception-detail.html` 定义了完整的异常管理功能。

**需要从零创建**：
- 数据库表 `production_exceptions`
- MES 子模块 `mes/production_exception/`（model/repo/service/implt/mod）
- MES 枚举（ExceptionType、ExceptionStatus、ExceptionSeverity、ReasonCategory）
- 独立路由模块 `routes/mes_exception.rs`
- 列表页 + 详情页

## 原型设计要点

### 列表页
- **4 个统计卡**：本月异常总数、批次暂停、报废批次、报检不合格
- **6 个状态 Tab**：全部 / 批次暂停 / 批次报废 / 不良异常 / 报检不合格 / 设备故障
- **筛选栏**：搜索框 + 异常类型下拉 + 原因分类下拉 + 日期范围
- **数据表**：异常编号 / 类型标签 / 关联(工单+批次) / 描述 / 影响数量 / 原因 / 发现时间 / 状态
- **分页**

### 详情页
- **头部**：异常编号 + 状态标签 + 操作按钮（转交/关闭）
- **信息网格**（12字段）：类型、原因分类、关联工单、关联批次、产品、当前工序、影响数量、发现时间、发现人、负责人、处置方式、优先级
- **异常描述**：详细文本
- **处理时间线**：垂直时间轴（异常上报→批次暂停→提交维修→维修进行中）
- **关联信息表**：关联类型/单号/说明/状态

## 实现步骤

### Step 1: Migration — 新建 `production_exceptions` 表

**`abt-core/migrations/027_create_production_exceptions.sql`**：

```sql
CREATE TABLE production_exceptions (
    id BIGSERIAL PRIMARY KEY,
    doc_number VARCHAR(64) NOT NULL,         -- 异常编号 EX-YYYY-MM-NNN
    exception_type SMALLINT NOT NULL,         -- 类型：1=批次暂停 2=批次报废 3=不良异常 4=报检不合格 5=设备故障
    status SMALLINT NOT NULL DEFAULT 1,       -- 状态：1=待处理 2=处理中 3=已关闭 4=条件放行 5=已恢复
    severity SMALLINT NOT NULL DEFAULT 2,     -- 优先级：1=紧急 2=一般 3=低
    reason_category SMALLINT,                 -- 原因分类：1=物料不良 2=设备故障 3=操作失误 4=工艺问题
    work_order_id BIGINT,                     -- 关联工单
    batch_id BIGINT,                          -- 关联批次
    product_id BIGINT,                        -- 产品
    current_step INTEGER,                     -- 当前工序序号
    impact_qty DECIMAL(10,6),                 -- 影响数量
    description TEXT,                         -- 异常描述
    disposition VARCHAR(255),                 -- 处置方式
    found_at TIMESTAMPTZ NOT NULL,            -- 发现时间
    finder_id BIGINT,                         -- 发现人
    owner_id BIGINT,                          -- 负责人
    operator_id BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ
);

CREATE INDEX idx_exceptions_type ON production_exceptions(exception_type);
CREATE INDEX idx_exceptions_status ON production_exceptions(status);
CREATE INDEX idx_exceptions_work_order ON production_exceptions(work_order_id);
CREATE INDEX idx_exceptions_batch ON production_exceptions(batch_id);
CREATE INDEX idx_exceptions_found_at ON production_exceptions(found_at);
```

**`abt-core/migrations/028_create_exception_events.sql`**：

```sql
CREATE TABLE production_exception_events (
    id BIGSERIAL PRIMARY KEY,
    exception_id BIGINT NOT NULL REFERENCES production_exceptions(id),
    event_type VARCHAR(64) NOT NULL,          -- reported / suspended / repair_submitted / repair_in_progress / resolved / closed
    description TEXT,
    operator_id BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_exception_events_exception ON production_exception_events(exception_id);
```

### Step 2: 枚举 — 新增异常相关枚举

**`abt-core/src/mes/enums.rs`** — 新增：

```rust
mes_enum! {
    /// 异常类型
    pub enum ExceptionType {
        BatchSuspended = 1,      // 批次暂停
        BatchScrapped = 2,       // 批次报废
        DefectAnomaly = 3,       // 不良异常
        InspectionFailed = 4,    // 报检不合格
        EquipmentFault = 5,      // 设备故障
    }
}

mes_enum! {
    /// 异常状态
    pub enum ExceptionStatus {
        Pending = 1,       // 待处理
        Processing = 2,    // 处理中
        Closed = 3,        // 已关闭
        ConditionalRelease = 4, // 条件放行
        Resolved = 5,      // 已恢复
    }
}

mes_enum! {
    /// 异常严重度
    pub enum ExceptionSeverity {
        Urgent = 1,  // 紧急
        Normal = 2,  // 一般
        Low = 3,     // 低
    }
}

mes_enum! {
    /// 原因分类
    pub enum ReasonCategory {
        MaterialDefect = 1,     // 物料不良
        EquipmentFault = 2,     // 设备故障
        OperatorError = 3,      // 操作失误
        ProcessIssue = 4,       // 工艺问题
    }
}
```

### Step 3: 后端 — 新建 `mes/production_exception/` 子模块

完整模块结构：

```
abt-core/src/mes/production_exception/
├── mod.rs       — 导出 + 工厂函数
├── service.rs   — Service trait
├── implt.rs     — Service 实现
├── model.rs     — 数据模型
└── repo.rs      — SQL 查询
```

**model.rs**：
- `ProductionException` — 主表映射
- `ExceptionEvent` — 事件表映射
- `ExceptionListItem` — 列表视图（关联 WO/批次/产品名）
- `ExceptionListFilter` — `{ keyword, exception_type, status, reason_category, date_from, date_to }`
- `ExceptionStats` — 统计卡数据

**service.rs** — `ProductionExceptionService` trait：
```rust
async fn create(...) -> Result<i64>;
async fn find_by_id(...) -> Result<ProductionException>;
async fn list(...) -> Result<PaginatedResult<ExceptionListItem>>;
async fn get_stats(...) -> Result<ExceptionStats>;
async fn add_event(...) -> Result<i64>;
async fn list_events(...) -> Result<Vec<ExceptionEvent>>;
async fn update_status(...) -> Result<()>;
async fn get_detail_lookups(...) -> Result<ExceptionDetailLookups>;
```

**repo.rs** — SQL 查询：
- `insert` — INSERT 生产异常 + 事件
- `get_by_id` — JOIN work_orders, production_batches, products 获取名称
- `list` — 动态 WHERE 过滤 + 分页 + JOIN 显示名称
- `get_stats` — COUNT 按类型分组
- `insert_event` — INSERT 事件
- `list_events` — 按时间倒序
- `update_status` — UPDATE status

### Step 4: 路由 — 新建 `routes/mes_exception.rs`

```rust
#[derive(TypedPath, Deserialize)]
#[typed_path("/admin/mes/exceptions")]
pub struct ExceptionListPath;

#[derive(TypedPath, Deserialize)]
#[typed_path("/admin/mes/exceptions/table")]
pub struct ExceptionTablePath;

#[derive(TypedPath, Deserialize)]
#[typed_path("/admin/mes/exceptions/:id")]
pub struct ExceptionDetailPath { pub id: i64 }
```

### Step 5: 前端 — 列表页 + 详情页

**`mes_exception_list.rs`** — 重写：
- 统计卡（4个）
- Tab 栏（HTMX 点击切换）
- 筛选栏 + 搜索框
- 数据表（HTMX 分页加载）

**`mes_exception_detail.rs`** — 新增：
- 异常信息网格（12字段，lookup 名称）
- 异常描述
- 处理时间线（垂直步骤）
- 关联信息表

### Step 6: 路由注册

**`routes/mod.rs`**：
- 移除硬编码 `/admin/mes/exceptions` 路由
- 添加 `mes_exception::router()` 合并

**`mes/mod.rs`**：
- 添加 `pub mod production_exception;`
- 在 `new_mes_router()` 中注册 service

### Step 7: CSS — 时间线 + 异常标签

**`uno.config.ts`** — 新增：
- `.timeline` / `.timeline-item` / `.timeline-dot` / `.timeline-time` / `.timeline-action` — 垂直时间线
- `.diff-indicator` — 差异标签
- 异常类型 pill 颜色

### 涉及文件

| 文件 | 改动 |
|------|------|
| `abt-core/migrations/027_*.sql` | 新建 `production_exceptions` 表 |
| `abt-core/migrations/028_*.sql` | 新建 `production_exception_events` 表 |
| `abt-core/src/mes/enums.rs` | 新增 4 个枚举 |
| `abt-core/src/mes/production_exception/mod.rs` | **新建** — 导出 + 工厂 |
| `abt-core/src/mes/production_exception/model.rs` | **新建** — 数据模型 |
| `abt-core/src/mes/production_exception/service.rs` | **新建** — trait 定义 |
| `abt-core/src/mes/production_exception/implt.rs` | **新建** — trait 实现 |
| `abt-core/src/mes/production_exception/repo.rs` | **新建** — SQL 查询 |
| `abt-core/src/mes/mod.rs` | 注册新模块 |
| `abt-web/src/routes/mes_exception.rs` | **新建** — 独立路由模块 |
| `abt-web/src/routes/mod.rs` | 移除硬编码，添加 mes_exception::router() |
| `abt-web/src/pages/mes_exception_list.rs` | 完整重写 |
| `abt-web/src/pages/mes_exception_detail.rs` | **新建** — 详情页 |
| `uno.config.ts` | 新增时间线 + 异常样式 |
