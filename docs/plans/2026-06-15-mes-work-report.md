# ④ 报工完善方案

> 对标 Odoo `button_finish` + `mrp.workcenter.productivity` + ERPNext `job_card.on_submit`

## 一、当前问题

### 问题9（P2）：actual_start/actual_end 从未维护

**位置**：`production_batch/implt.rs` confirm_routing_step

设计文档（`04-mes.html:592`）说首次报工设 `actual_start = now()`，但代码完全没实现。work_order 和 batch 的 actual_end 也没设置。

**Odoo 做法**：`button_start` 记录 `date_start`；`button_finish` 记录 `date_finished`。

### 问题10（P2）：工序 Completed 状态缺失

**位置**：`production_batch/implt.rs:326-333`

confirm_routing_step 把 `batch_routing_progress` 从 Pending→InProgress，但**从没设为 Completed**。只有 advance_to_receipt:486-490 才设最后工序 Completed。中间工序永远停在 InProgress。

**Odoo 做法**：workorder 有完整状态机 `pending → ready → progress → done`，button_finish 设为 done。

### 问题12（P2）：suspend/resume/scrap reason 被丢弃

**位置**：`production_batch/implt.rs:506, 531, 557`

```rust
async fn suspend(&self, ..., _reason: String) -> Result<()> { ... }
//                                  ^^^^^^^ 参数名带下划线 = 从未使用
```

暂停/报废原因无审计记录，无法追溯。

### 问题15（P3）：calculate_wage N+1 查询

**位置**：`work_report/implt.rs:91-97, 162-168`

每条报工记录单独查询该工单全部工序。1000 条报工 = 1000 次 DB 往返。

---

## 二、Odoo/ERPNext 参考实现

### Odoo workorder 状态机
```python
# 状态: pending → ready → progress → done / cancel
button_start():  state → progress, date_start = now()
button_finish(): state → done, date_finished = now()
```

### Odoo 工时记录（mrp.workcenter.productivity）
```python
# button_start 时创建 productivity 记录
env['mrp.workcenter.productivity'].create({
    'workorder_id': wo.id,
    'workcenter_id': wo.workcenter_id.id,
    'date_start': now(),
    'loss_id': productive_loss,  # productive / performance / availability
})

# button_finish 时关闭
productivity.write({'date_end': now()})
productivity._check_duration()
```

### ERPNext job_card 校验
```python
def on_submit(self):
    self.validate_inspection()           # 质检门控
    self.validate_transfer_qty()         # 物料校验
    self.validate_job_card()             # 工时校验
        self.validate_time_logs_present()       # 必须有工时记录
        self.validate_completed_qty_matches()   # 完成量校验
```

---

## 三、完善方案

### 3.1 confirm_routing_step 维护时间戳

**文件**：`production_batch/implt.rs`

在 confirm_routing_step 中，首次报工时设 actual_start，工序完成时设 actual_end：

```rust
async fn confirm_routing_step(&self, ctx, db, req: StepConfirmationReq) -> Result<...> {
    // ... 现有逻辑 ...

    let brp = batch_routing_progress_repo.get(db, batch_id, routing_id).await?;

    // 修复：首次报工设 actual_start（对标 Odoo button_start 记录 date_start）
    if brp.started_at.is_none() {
        sqlx::query(
            "UPDATE batch_routing_progress SET started_at = NOW() WHERE id = $1"
        ).bind(brp.id).execute(&mut *db).await?;

        // 同步设 batch.actual_start
        sqlx::query(
            "UPDATE production_batches SET actual_start = NOW() WHERE id = $1 AND actual_start IS NULL"
        ).bind(batch_id).execute(&mut *db).await?;

        // 同步设 work_order 状态 → InProduction + actual_start
        sqlx::query(
            "UPDATE work_orders SET status = 3, actual_start = NOW()
             WHERE id = $1 AND actual_start IS NULL"
        ).bind(work_order_id).execute(&mut *db).await?;
    }

    // ... 累加 completed_qty 等现有逻辑 ...

    // 修复：工序完成时设 Completed + completed_at（对标 Odoo button_finish）
    let new_completed = brp.completed_qty + req.completed_qty;
    let step_standard_qty = /* 从 work_order_routings 查 planned_qty */;
    if new_completed >= step_standard_qty {
        sqlx::query(
            "UPDATE batch_routing_progress
             SET status = 3, completed_at = NOW()  -- 3=Completed
             WHERE id = $1"
        ).bind(brp.id).execute(&mut *db).await?;
    }

    // 最后工序完成 → 设 batch.actual_end + work_order.actual_end
    if is_last_step {
        sqlx::query(
            "UPDATE production_batches SET actual_end = NOW() WHERE id = $1"
        ).bind(batch_id).execute(&mut *db).await?;
    }
}
```

### 3.2 工序完成后设 Completed 状态

**文件**：`production_batch/implt.rs`

当前代码只有最后工序的 brp 设为 Completed（在 advance_to_receipt 里）。中间工序的 brp 永远停在 InProgress。

修复：在 confirm_routing_step 中，当工序累计完成量 >= 标准量时，设该工序 brp 为 Completed：

```rust
// 工序完成判定：累计完成量 >= 工序标准量(planned_qty)
let routing = WorkOrderRoutingRepo::get_by_id(db, routing_id).await?;
if new_completed >= routing.planned_qty {
    // 设 batch_routing_progress 为 Completed
    BatchRoutingProgressRepo::update_status(db, brp.id, RoutingStatus::Completed).await?;

    // 自动推进到下一工序
    let next_step = current_step + 1;
    ProductionBatchRepo::update_current_step(db, batch_id, next_step).await?;
}
```

### 3.3 suspend/resume/scrap reason 写审计日志

**文件**：`production_batch/implt.rs`

```rust
async fn suspend(&self, ctx, db, batch_id, routing_id, reason: String) -> Result<()> {
    // ... 现有状态转换逻辑 ...

    // 修复：记录暂停原因到审计日志
    new_audit_log_service(self.pool.clone())
        .record(ctx, db, RecordAuditLogReq {
            entity_type: "ProductionBatch",
            entity_id: batch_id,
            action: AuditAction::Update,
            changes: Some(serde_json::json!({
                "action": "suspend",
                "routing_id": routing_id,
                "reason": reason,
            })),
            context: None,
        }).await?;

    Ok(())
}
// resume 和 scrap 同理
```

### 3.4 calculate_wage N+1 修复

**文件**：`work_report/implt.rs`

```rust
async fn calculate_wage(&self, ctx, db, worker_id, date_range) -> Result<WageSummary> {
    let reports = WorkReportRepo::list_by_worker_and_date_range(
        db, worker_id, date_range.from, date_range.to
    ).await?;

    // 修复 N+1：批量预加载所有相关工单的工序
    let wo_ids: Vec<i64> = reports.iter().map(|r| r.work_order_id).collect::<HashSet<_>>().into_iter().collect();
    let all_routings = WorkOrderRoutingRepo::get_by_work_order_ids(db, &wo_ids).await?;

    // 构建 HashMap<work_order_id, Vec<WorkOrderRouting>>
    let routing_map: HashMap<i64, Vec<&WorkOrderRouting>> =
        all_routings.iter().fold(HashMap::new(), |mut acc, r| {
            acc.entry(r.work_order_id).or_default().push(r);
            acc
        });

    let mut total_amount = Decimal::ZERO;
    let mut details = Vec::new();

    for report in &reports {
        // O(1) 查找，不再 N+1
        let routings = routing_map.get(&report.work_order_id);
        let routing_info = routings.and_then(|rs| rs.iter().find(|r| r.id == report.routing_id));

        let (process_name, unit_price) = routing_info
            .as_ref()
            .map(|r| (r.process_name.clone(), r.unit_price.unwrap_or(Decimal::ZERO)))
            .unwrap_or_else(|| (String::new(), Decimal::ZERO));

        // 工资计算（现有公式不变）
        let non_operator_defect_qty = match report.defect_reason {
            Some(reason) if reason.affect_wage() => report.defect_qty,
            _ => Decimal::ZERO,
        };
        let wage_amount = (report.completed_qty + non_operator_defect_qty) * unit_price;
        total_amount += wage_amount;
        // ... 构建 details ...
    }

    Ok(WageSummary { ... })
}
```

需要新增 repo 方法 `get_by_work_order_ids`：
```rust
// WorkOrderRoutingRepo
pub async fn get_by_work_order_ids(
    executor: impl sqlx::Executor<'_, Database = sqlx::Postgres>,
    wo_ids: &[i64],
) -> Result<Vec<WorkOrderRouting>> {
    // SELECT ... FROM work_order_routings WHERE work_order_id = ANY($1)
}
```

---

## 四、实现步骤

1. 更新 `production_batch/implt.rs` confirm_routing_step：首次报工设 actual_start，工序完成设 Completed + actual_end
2. 更新 `production_batch/implt.rs` suspend/resume/scrap：reason 写审计日志
3. 新增 `production_batch/repo.rs`：WorkOrderRoutingRepo::get_by_work_order_ids
4. 更新 `work_report/implt.rs` calculate_wage + list_all_wage_summaries：批量预加载消除 N+1
5. `cargo clippy` 验证

## 五、验收标准

- [ ] 首次报工后 batch.actual_start 和 work_order.actual_start 有值
- [ ] 工序完成后 brp 状态为 Completed（不是永远 InProgress）
- [ ] 最后工序完成后 batch.actual_end 有值
- [ ] suspend/resume/scrap 的 reason 记录在审计日志中可查
- [ ] calculate_wage 1000 条报工只产生 2 次 DB 查询（reports + routings）
- [ ] 计件工资能取到正确的 unit_price（依赖文档②的工序属性修复）
