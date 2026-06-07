# MES-08 跨模块集成测试 + 端到端流程

## 1. 端到端生产流程测试

以下测试覆盖从创建计划到完工入库的完整业务流程。

### 1.1 标准生产流程 (MTO)

```
创建计划 → 确认计划 → 下达计划(生成工单) → 工单下达 → 批次报工 → 检验 → 完工入库 → 确认入库(触发倒冲)
```

#### 流程步骤

| 步骤 | 操作 | URL | 预期结果 |
|------|------|-----|---------|
| E2E-01 | 创建 MTO 生产计划 | POST /admin/mes/plans/create | 成功，跳转列表 |
| E2E-02 | 查看计划详情 | GET /admin/mes/plans/{plan_id} | 状态=草稿，显示明细行 |
| E2E-03 | 确认计划 | POST /admin/mes/plans/{id}/confirm | 状态变=已确认 |
| E2E-04 | 下达计划 | POST /admin/mes/plans/{id}/release | 状态变=进行中，工单列表出现新工单 |
| E2E-05 | 查看生成的工单 | GET /admin/mes/orders | 工单数量=计划行数量，状态=Planned |
| E2E-06 | 下达工单 | POST /admin/mes/orders/{id}/release | 工单状态变=已下达 |
| E2E-07 | 查看批次 | GET /admin/mes/batches | (需验证是否有自动创建的批次) |
| E2E-08 | 批次报工 | POST /admin/mes/batches/{id}/confirm-step | 报工成功 |
| E2E-09 | 创建检验 | POST /admin/mes/inspections/create | 检验记录创建成功 |
| E2E-10 | 记录检验结果 | POST /admin/mes/inspections/{id}/record-result | 结果=合格 |
| E2E-11 | 创建入库单 | POST /admin/mes/receipts/create | 入库单创建，状态=草稿 |
| E2E-12 | 确认入库 | POST /admin/mes/receipts/{id}/confirm | 状态=已确认，倒冲触发 |
| E2E-13 | 关闭工单 | POST /admin/mes/orders/{id}/close | 工单关闭 |

### 1.2 验证检查点

| ID | 检查点 | 验证方式 |
|----|--------|---------|
| E2E-20 | 计划下达后工单数量正确 | 查看工单列表，对比计划行数 |
| E2E-21 | 工单关联了正确的计划行 | 工单的 plan_item_id 不为空 |
| E2E-22 | 报工后批次 completed_qty 更新 | 查看批次详情 |
| E2E-23 | 检验结果与工单关联 | 检验的 work_order_id 正确 |
| E2E-24 | 入库确认后 WMS 库存增加 | 查看库存查询页 |
| E2E-25 | 入库确认触发倒冲 | backflush_triggered=true |
| E2E-26 | 倒冲记录生成 | 查看倒冲记录列表 |
| E2E-27 | 计划状态最终正确 | 计划所有行完成后状态=已完成 |

### 1.3 工单取消流程

| 步骤 | 操作 | 预期结果 |
|------|------|---------|
| E2E-30 | 创建工单 | 状态=Planned |
| E2E-31 | 取消工单 | 状态=Cancelled |
| E2E-32 | 已取消工单不可操作 | 无操作按钮 |

### 1.4 批次暂停/恢复流程

| 步骤 | 操作 | 预期结果 |
|------|------|---------|
| E2E-40 | 批次进行中 | 状态=InProgress |
| E2E-41 | 暂停批次 | 状态=Suspended |
| E2E-42 | 恢复批次 | 状态=InProgress |
| E2E-43 | 推进入库 | 状态=Completed，创建入库单 |

---

## 2. 跨模块交互测试

### 2.1 MES ↔ WMS

| ID | 测试项 | 操作 | 预期结果 |
|----|--------|------|---------|
| INT-01 | 完工入库确认后库存增加 | 确认入库单 → 查看 WMS 库存查询 | 产品库存数量增加 |
| INT-02 | 倒冲扣减原材料 | 确认入库单 → 查看倒冲记录 | 原材料库存扣减 |
| INT-03 | 入库到指定仓库 | 创建入库单选择仓库 → 确认 | 库存出现在正确仓库 |
| INT-04 | 入库到指定储位 | 创建入库单选择储位 → 确认 | 库存出现在正确储位 |

### 2.2 MES ↔ 主数据

| ID | 测试项 | 操作 | 预期结果 |
|----|--------|------|---------|
| INT-10 | 计划中使用产品ID | 创建计划行使用产品ID | 产品名称正确解析显示 |
| INT-11 | 工单中使用不存在的产品ID | 输入不存在的ID | 服务端返回错误或显示"—" |
| INT-12 | BOM 展开 | 工单下达时 | 是否自动展开 BOM（待验证） |
| INT-13 | 工艺路线关联 | 创建工单时关联工艺路线 | 工序表是否自动填充 |

### 2.3 MES ↔ 身份认证

| ID | 测试项 | 操作 | 预期结果 |
|----|--------|------|---------|
| INT-20 | 创建人记录 | 创建计划/工单 | operator_id 正确记录当前用户 |
| INT-21 | 创建人显示 | 列表/详情中 | 显示用户 display_name，非 ID |
| INT-22 | Session 过期 | 操作过程中 session 过期 | 重定向到登录页 |

---

## 3. 数据一致性测试

### 3.1 乐观锁

| ID | 测试项 | 操作 | 预期结果 |
|----|--------|------|---------|
| CON-01 | 工单并发操作 | 两个窗口同时对同一工单操作 | 后提交的应失败（版本冲突） |
| CON-02 | 工单 version 字段 | 详情页显示 version | 每次 release/close/cancel 后 version+1 |

### 3.2 数据完整性

| ID | 测试项 | 预期结果 |
|----|--------|---------|
| CON-10 | 计划删除后工单 | 计划软删除后，关联工单应保持 | — |
| CON-11 | 工单删除后批次 | 工单删除后，关联批次应保持 | — |
| CON-12 | 批次完成数量≤计划数量 | 报工时 completed_qty 不超过 batch_qty | — |
| CON-13 | 入库数量≤完工数量 | 入库 received_qty 合理 | — |

---

## 4. 边界条件测试

| ID | 测试项 | 操作 | 预期结果 |
|----|--------|------|---------|
| EDGE-01 | 空计划（无明细行） | 创建计划不添加明细 | 创建成功，明细表显示"暂无计划明细" |
| EDGE-02 | 计划日期为过去 | 设置 plan_date 为昨天 | 是否允许（待确认） |
| EDGE-03 | 工单日期范围 | 开始日期 > 结束日期 | 是否允许（待确认） |
| EDGE-04 | 报工数量为 0 | completed_qty=0 | 是否允许 |
| EDGE-05 | 报工数量为负数 | completed_qty=-1 | 应被拒绝 |
| EDGE-06 | 报工数量超过计划 | completed_qty > planned_qty | 应被拒绝或警告 |
| EDGE-07 | 检验样本为 0 | sample_qty=0 | 是否允许 |
| EDGE-08 | 入库数量为 0 | received_qty=0 | 是否允许 |
| EDGE-09 | 不存在的批次 ID | 报工时输入不存在批次 | 服务端返回错误 |
| EDGE-10 | 不存在的工单 ID | 创建检验/入库时输入不存在工单 | 服务端返回错误 |
| EDGE-11 | 超长备注 | remark 输入 10000 字符 | 是否截断或报错 |
| EDGE-12 | 特殊字符备注 | remark 输入 `<script>alert(1)</script>` | XSS 防护，不执行脚本 |

---

## 5. 性能 / 稳定性测试

| ID | 测试项 | 预期结果 |
|----|--------|---------|
| PERF-01 | 计划列表分页 | 100+ 条计划时翻页流畅 |
| PERF-02 | 工单列表搜索 | 关键词搜索响应 < 1s |
| PERF-03 | 创建计划含 50 行明细 | 提交成功，items_json 正确序列化 |
| PERF-04 | HTMX 请求并发 | 多个 Tab 快速切换不报错 |

---

## 6. 已知缺陷汇总

根据代码审查发现的问题：

| ID | 问题 | 严重度 | 位置 |
|----|------|--------|------|
| BUG-01 | 批次列表页是 stub，无数据查询 | 高 | mes_batch_list.rs |
| BUG-02 | 报工列表页是 stub | 高 | mes_report_list.rs |
| BUG-03 | 工资列表页是 stub | 高 | mes_wage_list.rs |
| BUG-04 | 检验列表页是 stub | 高 | mes_inspection_list.rs |
| BUG-05 | 入库列表页是 stub | 高 | mes_receipt_list.rs |
| BUG-06 | Dashboard 统计卡片全部显示"—" | 中 | mes_dashboard.rs |
| BUG-07 | 批次报工表单缺少 defect_reason 和 remark UI | 中 | mes_batch_detail.rs:141-158 |
| BUG-08 | 检验创建表单缺少 remark UI | 低 | mes_inspection_create.rs |
| BUG-09 | 入库创建表单缺少 remark UI | 低 | mes_receipt_create.rs |
| BUG-10 | 流转卡查询无查询逻辑 | 中 | mes_card_query.rs |
| BUG-11 | "生产异常"快捷入口指向 404 | 低 | mes_dashboard.rs:106 |
| BUG-12 | 工单详情无工序列表和批次列表 | 中 | mes_order_detail.rs |
| BUG-13 | 批次详情无工序进度条 | 低 | mes_batch_detail.rs |
| BUG-14 | 排程看板页面未实现 | 中 | sidebar.rs:169 |
| BUG-15 | 物料消耗页面未实现 | 中 | sidebar.rs:174 |
| BUG-16 | 创建工单/计划时产品选择用手动 ID 输入 | 中 | 多处 |
