# MES 生产模块完善总览

> 基于 ABT MES 全部核心实现代码审查 + Odoo/ERPNext/OFBiz 对标分析。
> 本文档是后续 5 个领域文档的索引和实施路线图。

## 一、核心链路与问题总览

```
需求池 ──→ 排程 ──→ 工单下达 ──→ 领料 ──→ 报工 ──→ 完工入库
  │          │         │           │        │          │
  │     schedule_v1   release     issue   confirm    confirm
  │     ❌非真正排程   ❌工序属性   ❌不消费  ❌无时间戳  ❌成本=数量
  │                   全部丢失     预留     ❌工序状态  ❌FQC放行
  │                                        缺Completed  ❌倒冲脱事务
  │
  └─ MRP无净需求计算
```

## 二、问题分级（共 29 项）

### P0 致命：功能存在但实际无效（3 项）

| # | 问题 | 位置 | 影响 |
|---|---|---|---|
| 1 | release 时工序属性全部丢失 | `work_order/implt.rs:162-178` | 计件工资=0、检验点不触发、委外标记失效 |
| 2 | 成本结转把数量当金额 | `production_receipt/implt.rs:182-194` | 财务数据完全错误 |
| 3 | FQC 门控形同虚设 | `production_receipt/implt.rs:121-153` | 无检验记录直接放行 |

### P1 高危：数据一致性/事务安全（5 项）

| # | 问题 | 位置 |
|---|---|---|
| 4 | 倒冲用独立连接破坏事务 | `production_receipt/implt.rs:200-228` |
| 5 | 物料预检从不执行(bom_snapshot_id永远None) | `production_plan/implt.rs:175` |
| 6 | close 可绕过空批次校验 | `work_order/implt.rs:492-506` |
| 7 | 部分入库释放全部预留 | `production_receipt/implt.rs:230-237` |
| 8 | PlanItem 无条件置 Completed | `production_receipt/implt.rs:280-285` |

### P2 严重：逻辑缺陷/数据不完整（6 项）

| # | 问题 | 位置 |
|---|---|---|
| 9 | actual_start/actual_end 从未维护 | `production_batch/implt.rs` confirm_routing_step |
| 10 | 工序 Completed 状态缺失 | `production_batch/implt.rs:326-333` |
| 11 | cancel 不取消关联领料单 | `work_order/implt.rs:537-601` |
| 12 | suspend/resume/scrap reason 被丢弃 | `production_batch/implt.rs:506,531,557` |
| 13 | schedule_v1 不是真正的排程 | `production_plan/implt.rs:406-434` |
| 14 | 需求转计划无净需求计算 | `demand_handler/implt.rs:104-122` |

### 领料专项（8 项）

| # | 问题 | 位置 |
|---|---|---|
| L1 | 领料不关联工序 | `material_requisition/model.rs` 无 operation_id |
| L2 | 领料不关联批次 | 同上无 batch_id |
| L3 | 快照为空时生成空领料单 | `implt.rs:68-87` |
| L4 | 发料不消费库存预留 | `implt.rs:204-219` |
| L5 | 发料不带单位成本 | `implt.rs:214` unit_cost=None |
| L6 | 发料成本数量当金额 | `implt.rs:237-254` |
| L7 | 无退料功能 | service trait 无 return |
| L8 | 无法部分发料 | issue 直接到终态 |

### P3 中等：性能/健壮性（4 项）

| # | 问题 | 位置 |
|---|---|---|
| 15 | calculate_wage N+1 查询 | `work_report/implt.rs:91-97` |
| 16 | 多处吞错误(unrelease/pre_validate/calculate_wage) | 多处 |
| 17 | BOM 展开只取 leaf_nodes | 多处 |
| 18 | 领料仓库选择简陋 | `implt.rs:54` |

---

## 三、文档索引与实施顺序

按核心链路依赖关系排序，每个文档可独立实现：

| 顺序 | 文档 | 覆盖问题 | 依赖 |
|---|---|---|---|
| ① | `mes-scheduling.md` 排程完善 | #13 + 工作中心/日历基础设施 | 无（最先做） |
| ② | `mes-work-order-release.md` 工单下达 | #1 #5 #6 #11 | 依赖①的 routing_steps 新字段 |
| ③ | `mes-material-requisition.md` 领料 | L1-L8 | 依赖②的工序属性 |
| ④ | `mes-work-report.md` 报工 | #9 #10 #12 #15 | 依赖②的工序单价 |
| ⑤ | `mes-production-receipt.md` 完工入库 | #2 #3 #4 #7 #8 | 依赖③④ |

---

## 四、已完成的数据库变更

### migration 045：routing_steps + requisition_items 加字段
- routing_steps: + work_center_id, standard_time, standard_cost, unit_price, allowed_loss_rate, is_outsourced, is_inspection_point
- material_requisition_items: + operation_id, batch_id

### migration 046：排程基础设施五表
- work_calendars（工作日历）
- work_calendar_lines（日历工作时间）
- work_calendar_exceptions（节假日/特殊工作日）
- work_centers（工作中心，对标 Odoo mrp.workcenter）
- work_center_bookings（时段占用，对标 Odoo resource.calendar.leaves）

---

## 五、Odoo/ERPNext 关键参考算法

### Odoo 排程（_plan_workorder）
```
1. date_start = max(production.date_start, now())
2. 遍历前置工序 blocked_by → 递归排程 → 取前置完成时间
3. 遍历候选工作中心(含替代):
   a. duration = setup + cleanup + cycle_number × time_cycle × 100 / efficiency
   b. _get_first_available_slot(date_start, duration) → 日历找空闲时段
4. 选最早完成的工作中心
5. 创建 leave 占用防冲突
```

### Odoo 成本结转（_cal_price + _post_inventory）
```
成品成本 = Σ(原材料消耗量 × 单位成本) + 工时成本(costs_hour × duration) + 副产品分摊
全部在 stock.move._action_done() 同一事务内
```

### Odoo 工序属性继承
```
mrp.routing.workcenter → workcenter_id, time_cycle_manual, cost_mode
mrp.workorder 从 routing.workcenter 完整继承（不重新查找）
```

### ERPNext 质检门控
```
双重门控: BOM.inspection_required AND operation.quality_inspection_required
Rejected → 可配置 Stop/Warning 策略
```

---

## 六、验收标准

每个文档完成后需满足：
1. `cargo clippy` 无新增警告
2. 设计文档 `docs/uml-design/04-mes.html` 同步更新
3. 涉及的 migration 已在本地数据库执行
4. 核心链路端到端可跑通（工单下达→领料→报工→入库）
