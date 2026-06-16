# ① 排程完善方案

> 对标 Odoo `_plan_workorder` + ERPNext Workstation Working Hour
> 前置条件：migration 046（已写）已执行

## 一、当前问题

### schedule_v1 只做排序，不是排程
**位置**：`production_plan/implt.rs:406-434`

```rust
async fn schedule_v1(&self, ..., plan_id: i64) -> Result<()> {
    let mut items = get_items_by_plan_id(...).await?;
    items.sort_by(|a, b| a.priority.cmp(&b.priority)
        .then_with(|| a.scheduled_end.cmp(&b.scheduled_end)));
    for item in &items {
        if item.scheduled_start < today && item.priority > 0 {
            update_item_priority(..., 0).await?;  // 标记紧急
        }
    }
    Ok(())
}
```

**缺失**：无工序时长计算、无工作日历、无产能负荷、无冲突检查、无倒推排程。

### 没有 work_centers 实体
`work_center_id` 在 work_orders/routing_steps/work_order_routings 中全部悬空（无外键表）。

---

## 二、Odoo 排程算法详解

### 2.1 数据模型（对标）

| Odoo 模型 | 字段 | 我们的表(migration 046) |
|---|---|---|
| mrp.workcenter | costs_hour, time_efficiency, setup/cleanup, resource_calendar_id | work_centers |
| resource.calendar | name | work_calendars |
| resource.calendar.attendance | dayofweek, hour_from, hour_to | work_calendar_lines |
| resource.calendar.leaves | date_from, date_to | work_center_bookings |

### 2.2 排程流程（_plan_workorder）

```
输入: 工单 + 工序列表 + 起始时间 date_start

FOR EACH 工序（按依赖顺序）:
    1. 计算理论时长:
       capacity = workcenter.default_capacity
       cycle_number = ceil(planned_qty / capacity)
       duration = setup_time + cleanup_time
                + cycle_number × standard_time × 100 / time_efficiency

    2. 在工作中心日历上找第一个可用时段:
       _get_first_available_slot(date_start, duration, work_center_id)
       → 遍历工作日历的每天工作时段
       → 排除已有 bookings（重叠时段）
       → 返回 (from_datetime, to_datetime)

    3. 创建 booking 占用该时段:
       INSERT INTO work_center_bookings(work_center_id, date_from, date_to, ...)

    4. 更新工序的 date_planned_start / date_planned_finished

    5. date_start = 本工序完成时间（下一工序从这开始）
```

### 2.3 _get_first_available_slot 算法

```
输入: date_from, duration_minutes, work_center_id
输出: (slot_start, slot_end) 或 None

1. 查工作中心的日历 calendar_id
2. 从 date_from 开始，逐日扫描:
   a. 检查是否有 exception (节假日=跳过, 特殊工作日=用特殊时间)
   b. 无 exception → 查 weekday 对应的 work_calendar_lines
   c. 对每个工作时段 [from_time, to_time]:
      - 计算该时段可用时长
      - 减去已有 bookings 占用的重叠部分
      - 累积可用时长，直到 >= duration_minutes
3. 返回满足时长的连续时段
```

---

## 三、完善方案

### 3.1 新建模块

#### work_center 模块（`master_data/work_center/`）

**service.rs**:
```rust
#[async_trait]
pub trait WorkCenterService: Send + Sync {
    async fn create(&self, ctx, db, req: CreateWorkCenterReq) -> Result<i64>;
    async fn get(&self, ctx, db, id: i64) -> Result<WorkCenter>;
    async fn get_by_code(&self, ctx, db, code: &str) -> Result<Option<WorkCenter>>;
    async fn list(&self, ctx, db, filter: WorkCenterFilter, page, page_size) -> Result<PaginatedResult<WorkCenter>>;
    async fn update(&self, ctx, db, id: i64, req: UpdateWorkCenterReq) -> Result<()>;
    async fn delete(&self, ctx, db, id: i64) -> Result<()>;
    async fn list_active(&self, ctx, db) -> Result<Vec<WorkCenter>>;
}
```

**model.rs**:
```rust
pub struct WorkCenter {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub work_center_type: i16,      // 1=机器 2=人工 3=委外
    pub costs_hour: Decimal,         // 每小时成本
    pub time_efficiency: Decimal,    // 效率系数(100=正常)
    pub setup_time: Decimal,         // 准备时间(分钟)
    pub cleanup_time: Decimal,       // 清理时间(分钟)
    pub default_capacity: Decimal,   // 默认并行产能
    pub calendar_id: Option<i64>,
    pub location: Option<String>,
    pub is_active: bool,
}
```

#### work_calendar 模块（`master_data/work_calendar/`）

**service.rs** — 核心是可用时段查找:
```rust
#[async_trait]
pub trait WorkCalendarService: Send + Sync {
    // CRUD
    async fn create_calendar(&self, ctx, db, req) -> Result<i64>;
    async fn get_calendar(&self, ctx, db, id: i64) -> Result<WorkCalendar>;
    async fn set_lines(&self, ctx, db, calendar_id, lines: Vec<CalendarLineReq>) -> Result<()>;
    async fn add_exception(&self, ctx, db, req: ExceptionReq) -> Result<()>;

    /// 核心算法：在工作中心日历上找第一个可用时段
    /// 对标 Odoo _get_first_available_slot
    async fn find_available_slot(
        &self, db, work_center_id: i64,
        from: DateTime<Utc>, duration_minutes: Decimal,
    ) -> Result<Option<(DateTime<Utc>, DateTime<Utc>)>>;

    /// 查询工作中心在指定日期范围内的已有占用
    async fn list_bookings(&self, db, work_center_id, from, to) -> Result<Vec<WorkCenterBooking>>;

    /// 创建时段占用（排程时调用）
    async fn create_booking(&self, ctx, db, req: CreateBookingReq) -> Result<i64>;

    /// 取消占用（反下达/取消工单时调用）
    async fn cancel_bookings_by_work_order(&self, ctx, db, work_order_id: i64) -> Result<()>;
}
```

### 3.2 find_available_slot 实现算法

```rust
async fn find_available_slot(
    &self, db, work_center_id, from, duration_minutes
) -> Result<Option<(DateTime<Utc>, DateTime<Utc>)>> {
    // 1. 查工作中心 → calendar_id
    let wc = work_center_repo.get_by_id(db, work_center_id).await?;
    let calendar_id = wc.calendar_id.ok_or(no_calendar)?;

    // 2. 查已有 bookings（未来 90 天范围）
    let bookings = booking_repo.list_by_wc_and_range(
        db, work_center_id, from, from + 90.days()
    ).await?;

    // 3. 逐日扫描
    let mut remaining = duration_minutes;
    let mut current_date = from.date();
    let mut slot_start: Option<DateTime<Utc>> = None;

    while remaining > 0 && current_date < from.date() + 90.days() {
        // 3a. 检查例外日
        if let Some(exc) = exception_repo.get_by_date(db, calendar_id, current_date).await? {
            if !exc.is_workday { current_date += 1.day(); continue; }
            // 特殊工作日: 用 exc.from_time/to_time
        }

        // 3b. 查当日工作时段
        let weekday = current_date.weekday().num_days_from_sunday() as i16;
        let lines = line_repo.get_by_calendar_and_weekday(db, calendar_id, weekday).await?;

        // 3c. 对每个时段，扣除已有 booking 重叠
        for line in lines {
            let period_start = current_date.and_time(line.from_time);
            let period_end = current_date.and_time(line.to_time);
            let period_minutes = (period_end - period_start).num_minutes();

            // 扣除重叠的 bookings
            let occupied: i64 = bookings.iter()
                .filter(|b| b.overlaps(period_start, period_end))
                .map(|b| b.overlap_minutes(period_start, period_end))
                .sum();
            let available = period_minutes - occupied;

            if available >= remaining {
                if slot_start.is_none() { slot_start = Some(period_start); }
                let slot_end = slot_start.unwrap() + remaining.minutes();
                return Ok(Some((slot_start.unwrap(), slot_end)));
            } else if available > 0 {
                if slot_start.is_none() { slot_start = Some(period_start); }
                remaining -= available;
            }
        }
        current_date += 1.day();
    }
    Ok(None) // 90天内无可用时段
}
```

### 3.3 重写 schedule（production_plan service）

```rust
async fn schedule(&self, ctx, db, plan_id: i64) -> Result<ScheduleResult> {
    let items = get_items_by_plan_id(db, plan_id).await?;
    let wc_svc = new_work_calendar_service(self.pool.clone());
    let mut results = Vec::new();

    for item in &items {
        // 获取产品的工艺路线步骤
        let routing = routing_svc.get_bom_routing(ctx, db, &product_code).await?;
        let steps = routing.steps;

        // 计算总工序时长 + 逐工序排程
        let mut date_cursor = item.scheduled_end.and_hms(18, 0, 0); // 从交期当天结束倒推

        for step in steps.rev() {
            let wc_id = step.work_center_id.unwrap_or(default_wc);
            let wc = work_center_svc.get(ctx, db, wc_id).await?;

            // Odoo 时长公式
            let cycle_number = ceil(item.planned_qty / wc.default_capacity);
            let duration = wc.setup_time + wc.cleanup_time
                + cycle_number * step.standard_time * 100 / wc.time_efficiency;

            // 倒推：从 date_cursor 往前找可用时段
            let slot = wc_svc.find_available_slot_reverse(db, wc_id, date_cursor, duration).await?;

            if let Some((start, end)) = slot {
                wc_svc.create_booking(ctx, db, wc_id, work_order_id, start, end, duration).await?;
                date_cursor = start;
                results.push(ScheduledStep { step_id: step.id, planned_start: start, planned_end: end });
            } else {
                results.push(ScheduledStep { step_id: step.id, error: "无可用时段".into() });
            }
        }

        // 更新 plan_item 的 scheduled_start
        update_item_scheduled_start(db, item.id, date_cursor.date()).await?;
    }
    Ok(ScheduleResult { scheduled: results })
}
```

---

## 四、实现步骤

1. ✅ migration 046（已写）
2. 创建 `master_data/work_center/` 模块（model + repo + service + implt + mod）
3. 创建 `master_data/work_calendar/` 模块（含 find_available_slot 算法）
4. 在 `master_data/mod.rs` 注册两个新模块
5. 在 `abt-web/src/state.rs` 加 factory 方法
6. 创建 web 路由（工作中心/日历的 CRUD 页面）
7. 重写 `production_plan/service.rs` 的 schedule 方法
8. 更新 `routing/service.rs` 的 RoutingStep model 加新字段
9. `cargo clippy` 验证

## 五、验收标准

- [ ] 工作中心可 CRUD（含成本费率、效率、日历关联）
- [ ] 工作日历可定义每天工作时段（如周一至周五 8:00-17:00）
- [ ] 工作日历可定义节假日例外
- [ ] schedule 对每个计划项按工序倒推排程
- [ ] 排程在工作中心日历上创建 booking 占用时段
- [ ] 同一工作中心同一时段不会重复排程
- [ ] 排程结果更新 plan_item.scheduled_start
