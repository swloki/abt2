# UI-01: 工作中心与工作日历管理

> 核心改动牵涉：`abt-core/src/master_data/work_center/` + `abt-core/src/master_data/work_calendar/`（新模块）
> Odoo 参考：`mrp.workcenter` 模型 + `resource.calendar` 模型

## 1. 目标

为新创建的工作中心 (Work Center) 和工作日历 (Work Calendar) 两个 master_data 模块提供完整的 CRUD UI。这两个模块是排程算法 (schedule_v2) 和成本核算的基础数据。

## 2. Odoo 参考

### Odoo `mrp.workcenter`

```
列表视图: 名称 | 编码 | 产能 | 成本费率/小时 | 效率 | 状态
表单视图: Notebook 制品
  - 基本信息: name, code, resource_type, company_id
  - 成本信息: costs_hour, costs_cycle, costs_hour_account_id, costs_cycle_account_id
  - 产能信息: capacity, capacity_ids (工作日历关联)
  - 信息: note, active
```

### Odoo `resource.calendar`

```
列表视图: 名称 | 公司 | 时区 | 工作时间
表单视图:
  - 通用: name, company_id, tz
  - 工作时间: global_leave_ids, attendance_ids (周/时段)
  - 平均工时: hours_per_week
```

### 我们的适配

SSR 架构不做 Odoo 的日历可视化组件。采用标准 CRUD 页面模式（与现有 `routing_list` / `routing_create` / `routing_detail` 一致），工作日历先实现基础 CRUD + 时段表格，不做拖拽式日历。

## 3. 页面清单

### 3.1 工作中心 (Work Center)

| 页面 | 路径 | 文件 | 说明 |
|------|------|------|------|
| 列表 | `/admin/md/work-centers` | `pages/md_work_center_list.rs` | 数据表 + 状态 Tab + 搜索 |
| 创建 | `/admin/md/work-centers/new` | `pages/md_work_center_create.rs` | 表单页 |
| 详情 | `/admin/md/work-centers/:id` | `pages/md_work_center_detail.rs` | 信息卡 + 关联工艺路线 Tab + 关联日历 Tab |
| 编辑 | `/admin/md/work-centers/:id/edit` | `pages/md_work_center_create.rs` (复用) | 表单页（预填） |

### 3.2 工作日历 (Work Calendar)

| 页面 | 路径 | 文件 | 说明 |
|------|------|------|------|
| 列表 | `/admin/md/work-calendars` | `pages/md_work_calendar_list.rs` | 数据表 + 搜索 |
| 创建 | `/admin/md/work-calendars/new` | `pages/md_work_calendar_create.rs` | 表单页 |
| 详情 | `/admin/md/work-calendars/:id` | `pages/md_work_calendar_detail.rs` | 信息卡 + 可用时段 Tab |
| 编辑 | `/admin/md/work-calendars/:id/edit` | `pages/md_work_calendar_create.rs` (复用) | 表单页（预填） |

## 4. 数据流

### 工作中心

```
浏览器 → HTMX GET /admin/md/work-centers
  → Axum handler (pages/md_work_center_list.rs)
  → state.work_center_service().list(ctx, &mut conn, filter, page)
  → abt-core WorkCenterService::list
  → Maud 渲染 data-table
  → HTMX swap
```

### 工作日历

```
浏览器 → HTMX GET /admin/md/work-calendars
  → Axum handler (pages/md_work_calendar_list.rs)
  → state.work_calendar_service().list(ctx, &mut conn, filter, page)
  → abt-core WorkCalendarService::list
  → Maud 渲染 data-table
  → HTMX swap
```

## 5. 页面详细设计

### 5.1 工作中心列表页

```
┌─────────────────────────────────────────────────────────────┐
│ 工作中心管理                              [+ 新建工作中心]    │
├─────────────────────────────────────────────────────────────┤
│ [全部] [启用] [停用]    🔍 搜索名称/编码                      │
├─────────────────────────────────────────────────────────────┤
│ 编码   │ 名称       │ 产能/小时 │ 成本费率/小时 │ 状态 │ 操作 │
│ WC001  │ 注塑机A    │ 100      │ ¥80.00       │ 启用 │ 👁✏ │
│ WC002  │ 组装线B    │ 50       │ ¥120.00      │ 启用 │ 👁✏ │
│ WC003  │ 检测台C    │ 30       │ ¥60.00       │ 停用 │ 👁✓ │
├─────────────────────────────────────────────────────────────┤
│                    ◀ 1 2 3 ▶                                 │
└─────────────────────────────────────────────────────────────┘
```

**组件**：
- `status_tabs_with_param` — 全部/启用/停用（status 参数：None / true / false）
- `filter-bar` + `search-input` — 名称/编码模糊搜索
- `data-table` — 列表
- `pagination` — 分页

**TypedPath**：

```rust
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/md/work-centers")]
pub struct WorkCenterListPath {
    pub status: Option<String>,
    pub keyword: Option<String>,
    pub page: Option<u32>,
}
```

### 5.2 工作中心创建页

```
┌─────────────────────────────────────────────────────────────┐
│ ← 返回列表    新建工作中心                                    │
├─────────────────────────────────────────────────────────────┤
│ ── 基本信息 ──                                               │
│ 编码 *              [___________]                            │
│ 名称 *              [___________]                            │
│ 描述                [_______________________________]        │
│                                                              │
│ ── 产能与成本 ──                                             │
│ 产能/小时 *         [___________]  (Decimal)                 │
│ 成本费率/小时 *     [___________]  (Decimal, ¥)             │
│ 效率系数            [___1.0____]                            │
│                                                              │
│ ── 关联 ──                                                   │
│ 工作日历            [选择日历 ▾]                             │
│ 仓库位置            [选择仓库 ▾]                             │
│                                                              │
│                          [取消]  [保存]                      │
└─────────────────────────────────────────────────────────────┘
```

**表单字段**（对应 `WorkCenter` 模型）：

| 字段 | name | 类型 | 必填 | 说明 |
|------|------|------|------|------|
| 编码 | `code` | text | ✓ | 唯一编码 |
| 名称 | `name` | text | ✓ | 显示名称 |
| 描述 | `description` | textarea | | |
| 产能/小时 | `capacity_hour` | number | ✓ | Decimal |
| 成本费率/小时 | `costs_hour` | number | ✓ | Decimal |
| 效率系数 | `time_efficiency` | number | | 默认 1.0 |
| 工作日历 | `calendar_id` | select | | 关联 work_calendar |
| 仓库 | `warehouse_id` | select | | |
| 启用 | `is_active` | checkbox | | 默认 true |

### 5.3 工作中心详情页

```
┌─────────────────────────────────────────────────────────────┐
│ ← 返回列表    工作中心 WC001 - 注塑机A     [编辑] [停用]      │
├─────────────────────────────────────────────────────────────┤
│  ┌── 基本信息 ──────────────┐  ┌── 产能与成本 ──────────┐    │
│  │ 编码: WC001              │  │ 产能/小时: 100          │    │
│  │ 名称: 注塑机A            │  │ 成本费率: ¥80.00/h      │    │
│  │ 状态: ● 启用             │  │ 效率系数: 1.0           │    │
│  │ 日历: 标准工作日         │  │ 仓库: 主仓库            │    │
│  └──────────────────────────┘  └────────────────────────┘    │
├─────────────────────────────────────────────────────────────┤
│ [使用中的工艺路线]  [排程占用]  [操作日志]                    │
├─────────────────────────────────────────────────────────────┤
│ ── 使用中的工艺路线 ──                                       │
│ 工艺路线          │ 工序     │ 标准工时 │ 标准成本          │
│ RT-电源板-V1      │ 注塑     │ 2.0h    │ ¥160.00          │
│ RT-外壳-V2        │ 注塑     │ 1.5h    │ ¥120.00          │
└─────────────────────────────────────────────────────────────┘
```

**Tab 结构**（Hyperscript 切换）：
1. **使用中的工艺路线** — 查询 routing_steps WHERE work_center_id = :id，关联 routing 名称
2. **排程占用** — 查询 work_orders WHERE current_work_center_id = :id AND status IN (Released, InProduction)
3. **操作日志** — 审计日志

### 5.4 工作日历列表页

```
┌─────────────────────────────────────────────────────────────┐
│ 工作日历管理                              [+ 新建日历]        │
├─────────────────────────────────────────────────────────────┤
│ 🔍 搜索名称                                                  │
├─────────────────────────────────────────────────────────────┤
│ 名称          │ 工作中心   │ 有效期          │ 班次 │ 状态   │
│ 标准工作日    │ — (通用)   │ 2026-01~12      │ 3班  │ 启用   │
│ 注塑专用日历  │ WC001     │ 2026-01~06      │ 2班  │ 启用   │
│ 组装弹性排班  │ WC002     │ 2026-03~09      │ 3班  │ 启用   │
└─────────────────────────────────────────────────────────────┘
```

### 5.5 工作日历创建页

**表单字段**（对应 `WorkCalendar` 模型）：

| 字段 | name | 类型 | 必填 | 说明 |
|------|------|------|------|------|
| 名称 | `name` | text | ✓ | |
| 工作中心 | `work_center_id` | select | | 可选，空=通用 |
| 开始日期 | `start_date` | date | ✓ | |
| 结束日期 | `end_date` | date | ✓ | |
| 班次类型 | `shift_type` | select | ✓ | Single/TwoShift/ThreeShift |
| 日产能 | `daily_capacity` | number | | Decimal |
| 描述 | `description` | textarea | | |
| 启用 | `is_active` | checkbox | | 默认 true |

### 5.6 工作日历详情页

**Tab 结构**：
1. **基本信息** — 日历属性
2. **可用时段** — 调用 `find_available_slot` 预览可用时段（调试/验证用）
3. **关联排程** — 使用此日历的排程记录

## 6. 侧边栏导航

在 `layout/sidebar.rs` 的 `md` 模块（工程）中，在"工艺路线"之后新增：

```rust
NavItem {
    name: "工作中心",
    path: "/admin/md/work-centers",
    icon: NavIcon::Wrench,
    permission: Some(("BOM", "read")),
},
NavItem {
    name: "工作日历",
    path: "/admin/md/work-calendars",
    icon: NavIcon::Calendar,
    permission: Some(("BOM", "read")),
},
```

## 7. 路由注册

### `routes/md_work_center.rs`（新建）

```rust
pub fn router() -> axum::Router {
    use axum_extra::routing::TypedPath;
    axum::Router::new()
        .typed_get(pages::md_work_center_list::get_list)
        .typed_get(pages::md_work_center_create::get_create)
        .typed_post(pages::md_work_center_create::post_create)
        .typed_get(pages::md_work_center_detail::get_detail)
        .typed_get(pages::md_work_center_create::get_edit)
        .typed_post(pages::md_work_center_create::post_update)
}
```

### `routes/md_work_calendar.rs`（新建）

同上模式。

### `routes/mod.rs` 注册

```rust
.merge(md_work_center::router())
.merge(md_work_calendar::router())
```

## 8. state.rs 新增

```rust
pub fn work_center_service(&self) -> impl WorkCenterService {
    abt_core::master_data::work_center::new_work_center_service(self.pool.clone())
}
pub fn work_calendar_service(&self) -> impl WorkCalendarService {
    abt_core::master_data::work_calendar::new_work_calendar_service(self.pool.clone())
}
```

## 9. 实现步骤

1. 创建 `routes/md_work_center.rs` + `routes/md_work_calendar.rs`
2. 创建 `pages/md_work_center_list.rs` — 列表页（参照 `routing_list.rs` 模式）
3. 创建 `pages/md_work_center_create.rs` — 创建/编辑页
4. 创建 `pages/md_work_center_detail.rs` — 详情页
5. 创建 `pages/md_work_calendar_list.rs` — 列表页
6. 创建 `pages/md_work_calendar_create.rs` — 创建/编辑页
7. 创建 `pages/md_work_calendar_detail.rs` — 详情页
8. 更新 `layout/sidebar.rs` — 加 2 个导航项
9. 更新 `state.rs` — 加 2 个 service 工厂方法
10. 更新 `routes/mod.rs` + `pages/mod.rs` — 注册路由和页面模块

## 10. 验收标准

- [ ] 工作中心 CRUD 全流程可用（创建→列表→详情→编辑→停用）
- [ ] 工作日历 CRUD 全流程可用
- [ ] 侧边栏显示两个新导航项
- [ ] 列表页支持状态 Tab + 搜索 + 分页
- [ ] 详情页关联数据正确（工艺路线、排程占用）
- [ ] cargo clippy 零错误
