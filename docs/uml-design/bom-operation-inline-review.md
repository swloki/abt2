# BOM 内联工序重构 — feature-review 评审报告

> 评审对象：`docs/uml-design/bom-operation-inline.md`（已定稿设计 D1-D11 + Q1-Q7）
> 评审方式：6 角色独立压力测试（ERP 架构师 / 高级后端工程师 / 制造业业务顾问 / 产品经理 / 前端 UIUX 工程师 / 最终使用者）+ 整合
> 评审基准：已读取 `abt-core/src/master_data/{bom,routing,bom_routing_output,bom_labor_process,product}/` + `abt-core/src/mes/{work_order,production_batch,work_report}/implt.rs` + `abt-web/src/{pages,components}/` + 相关 migration（013/003/096），核对所有 file:行 引用
> 标注规则：「新发现」= 前轮 phase 3 四角色评审未覆盖；「否-确认」= 前轮已覆盖，本次复核确认

---

## 一、Ground Truth 对齐（设计假设 vs 代码现状差异）

逐条核对 6 角色 findings 中引用的 file:行。**结论：全部引用准确**，设计文档与代码现状一致；下列差异/陈旧点需在设计文档中校正。

### 1.1 验证通过的关键引用

| 设计引用 | 代码现状 | 一致性 |
|---|---|---|
| `bom/implt.rs:817-829` 新路径 `try_build_labor_from_routing` 的 `quantity: Decimal::ONE` | 实读 `:821 quantity: rust_decimal::Decimal::ONE` | ✅ 准确 |
| `bom/implt.rs:843-855` legacy `build_labor_from_legacy` 的 `quantity: r.quantity` | 实读 `:847 quantity: r.quantity` | ✅ 准确 |
| `bom_labor_process/model.rs:13 BomLaborProcess.quantity: Decimal` + `013:88 NUMERIC(18,6) NOT NULL DEFAULT 0` | 实读一致；**注意 DEFAULT 0 非 1**（迁移审计须重点查 `quantity <> 1`） | ✅ 准确（含 DEFAULT 0 补充） |
| `bom/repo.rs:361-378 list_non_leaf_product_ids_by_codes WHERE b.status IN (1,2) DISTINCT` | 实读 `:365 WHERE b.status IN (1,2)` + `:361 SELECT DISTINCT` | ✅ 准确（跨 Draft+Published 并集） |
| `bom_routing_output/repo.rs:9-37 upsert 同行级联`（operation 与 unit_price 同行） | 实读 `:15-25 INSERT...ON CONFLICT DO UPDATE` 同行 | ✅ 准确 |
| `work_order/implt.rs:51-75 try_load_routings_from_bom`（经 get_bom_routing 取 routing_id 的间接路径） | 实读 `:62-68 get_bom_routing → load_routings_from_template` | ✅ 准确 |
| `work_order/implt.rs:196 release 兜底仅 routing_id.is_none 触发` | 实读 `:196 if work_order.routing_id.is_none()` | ✅ 准确 |
| `work_order/implt.rs:274 has_routing: routing_id.is_some()` | 实读 `:274 "has_routing": work_order.routing_id.is_some()` | ✅ 准确 |
| `production_batch/implt.rs:204 防跳序 guard current_step != step_no-1` | 实读 `:204 if batch.current_step != step_no - 1` | ✅ 准确 |
| `production_batch/implt.rs:224 unit_price.unwrap_or(Decimal::ZERO)` | 实读 `:224 let unit_price = routing.unit_price.unwrap_or(Decimal::ZERO)` | ✅ 准确（NULL 价报工冻结 0 工资的实证） |
| `production_batch/implt.rs:591 reload 状态门 Draft/Planned/Released/InProduction` | 实读 `:601 if !matches!(wo.status, Draft\|Planned\|Released\|InProduction)`（`Planned` 为下达 drawer「从 BOM 更新」入口放宽） | ✅ 准确 |
| `production_batch/implt.rs:603-615 per-step lock（删未报工 step + 锁已报工 step_no）` | 实读 `:607-614 has_report(r.id) → locked_step_nos.insert / DELETE` | ✅ 准确 |
| `production_batch/repo.rs:378-395 has_any_report` | 实读 `:378-394 SELECT EXISTS JOIN work_reports` | ✅ 准确 |
| `product/model.rs:230-237 UpdateProductReq 无 product_code 字段` | 实读 `:230-237` 仅 name/unit/acquire_channel/external_code/owner_department_id/meta | ✅ 准确（Q1 守卫落点为空的实证） |
| `work_report/implt.rs:177-185 list_all_wage_summaries 重算` vs `:112 get_wage_summary 冻结` | 实读 `:185 wage_amount = (completed_qty + non_operator_defect_qty) * unit_price` vs `:112 let wage_amount = report.wage_amount` | ✅ 准确（D6 latent bug 实证） |
| `work_report/implt.rs:154-157 .ok().unwrap_or_default()` | 实读 `:154-157 get_by_work_order_ids(...).await.ok().unwrap_or_default()` | ✅ 准确（静默吞 DB 错误实证） |
| `mes_work_center.rs:2291 #[require_permission("WORK_ORDER", "update")]` | 实读一致 | ✅ 准确（计件价定价权限粒度缺口实证） |
| `mes_work_center.rs:2319 r.unit_price.is_none() || r.unit_price == Some(Decimal::ZERO)` | 实读一致（release drawer 已校验 None+Zero） | ✅ 准确 |
| `mes_work_center.rs:2329-2332 校验失败重渲染 render_release_drawer_body(&data, Some(&errors))` | 实读一致 | ✅ 准确（outerHTML 重建会清空未 blur 输入实证） |
| `mes_work_center.rs:1775/1785 ReleaseErrors::summary 文案` | 实读 `:1775 "请到「工艺路线管理」关联后重新创建工单"` / `:1785 "请在工艺路线模板补全产出品/单价后重新创建工单"` | ✅ 准确（与 D7 反向实证） |
| `overlay.rs:17-21 overlay_hs Esc 守卫` + `:45-55 drawer_shell 硬编码 z-[90] 无 z_class 形参` | 实读一致；drawer_shell 签名 `(id, width_class, inner)` 无 z_class；modal_shell `(id, z_class, inner)` 有 | ✅ 准确（参数不对称实证） |
| `routing_create.rs:706 toggleOpCombo 内联 + position:fixed + getBoundingClientRect` | 实读 `:706-726` 内联 JS | ✅ 准确 |
| `static/app.js:543 window.filterCatOptions` + `:554 window.selectCat` 已全局 | 实读 `window.filterCatOptions = function(...)` / `window.selectCat = function(...)` | ✅ 准确（M7 措辞误述实证） |
| `category_select.rs:64/72 call filterCatOptions/selectCat 复用全局` | 实读 `:64 _="on input call filterCatOptions(me)"` / `:72 _="on click call selectCat(me)"` | ✅ 准确 |
| `routing_detail.rs:588-594 「维护产出」按钮（overlay drawer 链唯一入口）` | 实读 `:588-594 button hx-get=RoutingOutputEditPath` | ✅ 准确 |
| `routing_create.rs:548-557 警告条 @if is_edit && bound_count > 0` | 实读 `:548-557` 含「删除或重排已有工序将被拒绝」 | ✅ 准确 |
| `bom_detail.rs:1003-1007 计件单价链接 → RoutingListPath` | 实读 `:1003 "（所有工序单价为0）"` / `:1005-1008 href=RoutingListPath::PATH` 「去工艺路径配置计件单价 →」 | ✅ 准确 |
| `013:87 unit_price NUMERIC(20,4)` vs `096:16 bom_routing_outputs.unit_price NUMERIC(18,6)` | 实读一致（精度迁移链验证） | ✅ 准确 |

### 1.2 设计文档需校正的措辞/陈旧点

| 位置 | 现状 | 校正建议 |
|---|---|---|
| §3.5 / §4.1 / Q2 | 「产出品 `output_product_id` 须 ∈ **该 BOM** 非叶子节点」 | 措辞误导。实际是「∈ **该 product_code 下 Draft+Published 所有 BOM 版本的非叶子节点并集**」（`bom/repo.rs:365 WHERE b.status IN (1,2)` + DISTINCT），与既有 overlay 行为一致。措辞与实现不符会误导实现者去找「精确到单 BOM」的入口 |
| §10-Q1 | 「在 products 改名路径加守卫（改名时 grep 命中 bom_operations/bom_step_prices 则警告或级联更新）」 | 守卫落点为空：`UpdateProductReq`（`product/model.rs:230-237`）不含 `product_code` 字段，应用层无改名入口。真实风险仅限直改 DB（无法用应用层守卫拦截）。校正为「Q1 本次的『加守卫』动作 = 无操作；技术债保留；若未来 `UpdateProductReq` 开放 `product_code` 字段，须同步加四表级联」 |
| §10-M7 / §9 步骤 2 | 「抽 `cat-select` 到 `static/app.js` 全局函数（`toggleOpCombo` / `selectCat` / `filterCatOptions`）」 | 误述现状。`selectCat`（`app.js:554`）+ `filterCatOptions`（`app.js:543`）早已是 `window.*` 全局，`category_select.rs:64/72` 已 `call` 复用；**仅 `toggleOpCombo`/`closeOpCombo` 是 `routing_create.rs:706/728` 内联**。校正为「抽 `toggleOpCombo`/`closeOpCombo` 到 app.js，`selectCat`/`filterCatOptions` 直接复用」 |
| §6.4 routing_create 警告条「直接删除」 | clean break 期护栏移除后直接删警告条 | routing 降级为 copy-on-write 模板后，用户对「改模板是否影响已绑 BOM」的心智疑问更突出，应**改写为正向信息条**而非删除（见修改项 R-19） |
| §8/M6 bom_detail 链接「改指向 `BomEditPath#bom-ops-card`」 | 按 §9 步骤 2 BOM ops card 无单价输入字段（价按 D7 只在 release drawer 填） | 链接落点与 D7 冲突，操作员点过去扑空。须二选一钉死（见修改项 R-17） |

---

## 二、6 角色评审发言

### 🏛️ ERP 系统设计师（架构视角）

**summary**：设计整体方向正确——`bom_operations` 内联工序 + routing 降级为 copy-on-write 模板，根治了 `step_order` 错位与覆盖层脱节，符合 ERPNext/Odoo 范式。但发现 **1 个 P0 数据正确性红线 + 3 个 P1 架构级硬伤**：

- **P0（新发现）**：Q5 拍板「`bom_labor_processes` 本期迁入 `bom_step_prices`」，但后者 schema（§3.2:118-127）**无 `quantity` 列**。legacy 成本 = `unit_price × quantity`（`build_labor_from_legacy:847` 用 `r.quantity`），新路径恒为 `×1`（`try_build_labor_from_routing:821` `quantity: Decimal::ONE`）。现网若有 `quantity<>1` 的工序行（如标准工时制残留），迁入后成本腰斩。§7.4 审计只查了「价只在 legacy」的 BOM 清单，完全没审计 `quantity<>1` 分布。
- **P1（新发现）**：产出品「∈ 该 BOM 非叶子节点」措辞与实现不符——实际是 `product_code` 下 Draft+Published 所有版本的非叶子节点并集（`bom/repo.rs:365 WHERE b.status IN (1,2)`）。跨版本物料结构变更时产出品会引用到错误节点。
- **P1（否-确认）**：`source_routing_id` 与 `bom_routings.routing_id` 双溯源在 force 重拷后分叉——load 信 `bom_routings`、展示信 `source_routing_id`、`routing_detail` 关联反查又换一源，三个地方三种答案。§6.3 标 review minor 严重度被低估。
- **P1（新发现）**：Released 且全无报工的工单，create 时已 load 过一次工序，release 时若 `routing_id` 已 Some 则不再兜底重载（`:196` 仅 `is_none` 触发）。之后用户改 BOM 工序（加 step / 改工作中心 / 改计件价），没有任何路径触发该工单 reload——计件价尤其敏感。
- **P2（否-确认）**：D9 `work_orders.routing_id` 停写但保留作溯源。`WOReleased` payload `has_routing: routing_id.is_some()`（`:274`）仍读它。「有 bom_operations 但无 routing_id」成为合法状态后，下游消费 `has_routing` 会误判为「无工艺工单」。
- **P2（新发现）**：`bom_labor_processes.unit_price NUMERIC(20,4)`（013:87）→ `bom_step_prices.unit_price NUMERIC(18,6)`（§3.2:122）是精度方向变化（整数位 16→12、小数位 4→6）。现网单价绝对值不大方向 OK，但迁移 SQL（§7.1 M2(b)）未显式 CAST，异常大值会溢出。

### 💻 高级后端工程师（实现与性能视角）

**summary**：设计整体扎实——D6 wage 口径 bug 真实存在（`:112` 冻结 vs `:185` 重算）、§4.4 `has_report` 两步解析准确、`work_reports.routing_id` 的 FK（003:150）护住了 reload 期数据完整性、`load_operations_from_bom` 单源 + `price_map` HashMap 比旧 N 次 `has_report` + 逐行 DELETE 性能更好。但发现 **3 个 P1 实现级问题 + 4 个 P2**：

- **P1（新发现，建议升 P0）**：`bom_operations` / `bom_step_prices` 分表后，`replace_operations`（BOM 保存）与 `apply_routing_to_bom(force=true)` 都不级联清理 `bom_step_prices`。旧 `bom_routing_outputs`（`repo.rs:9-37`）operation 与 unit_price 同行天然级联；新分表后 BOM 编辑器做工序增删/上下移（§9 swap step_order）保存时，旧 step_order 上的 `bom_step_prices` 残留 → `price_map.get(step_order)` 命中旧价 → 「焊接 step2 的价」错误套到「测试 step2」→ 工单 load 写错价 → 报工冻结错工资。**计件工资资产错配的真实数据正确性红线**。
- **P1（新发现）**：per-order lock 的 `has_any_report(wo_id)`（`production_batch/repo.rs:378`）检查与后续 reload 的 DELETE+INSERT 非原子。并发报工下 Req1 走 `has_any_report→false`，Req2 在检查后 DELETE 前插入 `work_report`（`routing_id=wor.id`），Req1 随后 DELETE `work_order_routings` 撞 `work_reports.routing_id` 的 FK（003:150 NOT NULL REFERENCES）→ 事务回滚，数据完整性保住，但用户看到 opaque 500 而非「工单已有报工，不可重载」友好提示。READ COMMITTED 下 SELECT 不加锁，窗口非零。
- **P1（新发现）**：`list_all_wage_summaries` 预取 routings 时 `.ok().unwrap_or_default()`（`:154-157`）静默吞 DB 错误：`get_by_work_order_ids` 返回 Err 被丢成空 Vec → routing_map 空 → 每条 detail 的 `unit_price`/`process_name` 全落 `unwrap_or(Decimal::ZERO)`/空串。违反 CLAUDE.md「禁止静默丢弃错误」红线，且 D6 正好在改这个函数（§5.4/§8），不顺手修就是遗留。
- **P2（新发现）**：D6 统一 `total_amount` 到冻结 `wage_amount` 后，`WageDetail.unit_price` 仍取当前 `work_order_routings.unit_price`（`:177-179`），同一行展示 `unit_price=10`（当前价）但 `wage_amount=qty×8`（报工时冻结旧价），用户无法对账。
- **P2（新发现）**：§5.3 把 `has_report` 守卫放在「执行 (b) 前」即 (a) `bom_step_prices` upsert 之后。功能上单事务回滚等价，但语义误导（先写真相源再判该不该写）。
- **P2（新发现）**：Q3/D11 confirm re-check 措辞「非空」有歧义。`:224 unit_price.unwrap_or(Decimal::ZERO)` 把 None 和 Some(ZERO) 都变成 0 工资静默通过；release drawer（`:2319`）已校验 `is_none() || == Some(ZERO)`，confirm re-check 若只拒 None 不拒 Zero 则等于没堵住。
- **P2（新发现）**：Q7 月度审计要求无 `bom_step_price_history` 表支撑。`bom_step_prices` 原地 UPDATE 覆盖（upsert by product_code+step_order），`operator_id+updated_at` 只记最后一次改的人，无法回答「这个价从 WO#123 第一次填、后在 WO#456 被改」的溯源链。

### 🗺️ 业务专家 / 实施顾问（制造业 Domain Expert）

**summary**：设计在数据结构上自洽（`bom_operations` 自洽行 + `bom_step_prices` 真相源分离合理），D6/D7/D8 三条升级决策方向正确。但从「能不能在车间跑起来」的角度仍有 **4 个 P1 运营缺口 + 4 个 P2**：

- **P1（新发现）**：D8 per-order lock 在「工单已报工 step1 后发现 BOM 漏配一道工序」场景下无任何补救路径。`confirm_routing_step` 防跳序 guard（`:204 current_step != step_no - 1`）拒绝跳过缺失中间工序，最后工序完成判定（`:354-355 max_step`）又要求走到 `max_step` 才能完工——工单被卡死。设计 §5.2 只讨论了「改 step2/step3」，没覆盖「加 step」。车间真实场景：产线开工后发现 IE 漏了道清洗/检验工序，工单不能 reload、不能跳过、不能手工补——只能停线等 DBA 直改 SQL。
- **P1（新发现）**：`bom_step_prices` 是跨工单共享的人工成本真相源（§5.3「未来工单加载源」），首个填价者定的单价被所有后续同 BOM 工单自动复用——这是敏感人工成本科目。但 `release_order`（`:2291`）权限是 `WORK_ORDER update`，任何有工单更新权限的人（计划员、班长、轮班组长）都能设定影响全员工资的单价。Q7 只拍板「月度审计报告 + 不做 draft→active 状态机」，未定义定价权限粒度。grep 全仓无 `LABOR_PRICE`/`STEP_PRICE`/`PIECE_RATE` 权限枚举——中国制造业计件工资涉及劳动法合规，单点定价无审批是实质性内控风险。
- **P1（新发现）**：§7.4（:592-594）规定「价只在 legacy」的 BOM 按 `(product_code, sort_order↔step_order 映射)` 补价到 `bom_step_prices`，**但映射规则未定义**。`bom_labor_processes.sort_order`（013:89）是 legacy 表独立排序字段（Excel 导入填入），`bom_operations.step_order` 是 BOM 内联工序序号（routing_steps 拷贝而来）。两套编号无任何保证一致——若按 `sort_order==step_order` 直接 JOIN，会把「焊接的计件价」挂到「测试工序」，沉默的数据错配（M3 COUNT 门禁检测不出来）。
- **P1（新发现）**：Q7 月度审计报告 schema 无历史表支撑（同后端 P2）。单价从 5 元改成 50 元（10 倍）和改成 5.5 元（10%）风险等级完全不同，当前 schema 无法区分——审计报告无 diff 幅度等于没有审计。
- **P2（新发现）**：D7 把填价收敛到 release drawer，`release_order:2319-2323` 对 `unit_price` 空或零硬阻断下达。夜班/周末 IE 不在场时，急单工单卡在「无法下达」状态——产线等料等单却被价卡住，会引发「绕过系统口头定价→事后补录」的影子流程，反而加剧 Q7 担心的内控风险。
- **P2（否-确认）**：Q3 报工 re-check 错误消息「先定价」不可操作。车间工人不知道：(a) 找谁定价（IE？班长？）；(b) 在哪里填；(c) 该工序 product_code/step_order 是什么。
- **P2（新发现）**：`bom_operations.process_name` 物化落库（§3.1），copy-on-write 后字典改名不同步，`resync_process_names`（§4.4）只刷 `bom_operations` 不刷已加载的 `work_order_routings.process_name`。`list_all_wage_summaries`（`:177`）从快照读 `process_name` → 字典「焊接」标准化为「点焊」后，工资汇总 Excel 按工序名 group 会把同一工序拆成两行。
- **P2（新发现）**：工单 cancel（`work_order/implt.rs:320`）在已有报工但无完工入库时可执行，cancel 后 `work_reports`（含冻结 `wage_amount`）保留。设计 §5.2 未显式说明「工单取消/作废后，已报工的 wage 是否仍计入发薪周期、`bom_step_prices` 是否回滚」——退单场景下已做工量是否计薪是劳动法问题。

### 📐 产品经理（业务闭环与边界视角）

**summary**：设计在功能闭环和决策记录上相当完备（D1-D11 + Q1-Q7 覆盖关键取舍），但从「分步部署中间态」和「业务边界场景」压测，暴露 **2 个 P0 部署窗口断链 + 4 个 P1 边界缺口**。核心矛盾：§9 把本应原子发布的「routing 退役 / bom_operations 编辑器 / load 切源」拆成了可独立回滚的 5 步，但步骤之间存在硬依赖。

- **P0（新发现）**：§9 步骤 1（routing 退役）与步骤 2（bom_operations 编辑器 + 建表）拆成两步独立部署，但步骤 1 有两个硬依赖步骤 2 的矛盾：(1) 步骤 1 要加「拷贝工序到 BOM」按钮 + `apply_routing_to_bom` handler，而该 handler 实现是 `INSERT INTO bom_operations`——但 `bom_operations` 表在步骤 2（migration 098 M1）才建。步骤 1 部署后该 handler 一调用即运行时报错 `relation bom_operations does not exist`。(2) 步骤 1 删除 `routing_detail` overlay 整条 drawer 链（`:588-594`「维护产出」按钮是唯一入口），而 `bom_edit.rs` 当前完全无工序/产出编辑能力，步骤 2 才建 BOM 工序编辑器。步骤 1→2 部署窗口期间，运营对任何 BOM 都无法维护产出品/工序——已迁移的 RT000001 家族（388×8 量级）若要改产出品，UI 入口为零。
- **P0（新发现）**：§9 步骤 2（建 bom_operations + 回填 + BOM 编辑器 upsert）与步骤 4（切源 load）之间存在两个真相源 split-brain：步骤 2 后用户在 BOM 编辑器改工序/产出 → 写 `bom_operations`；但步骤 4 前工单 create/release 仍走 `try_load_routings_from_bom`（`:51-75`）→ `get_bom_routing` → `load_routings_from_template`（读 routing_steps × bom_routing_outputs 双源）。结果是「用户改了工序，工单看不到」——BOM 编辑器的写入对工单完全不生效，且无校验阻断、无告警。§9 步骤 2 验证项「BOM 编辑页工序 CRUD + 拷贝回归」完全不覆盖「工单 load 是否读到改动」，会被当成通过而隐藏断链。
- **P1（新发现）**：D6（§5.4 `list_all_wage_summaries` 统一到冻结口径）被排在 §9 步骤 4（收尾），但 `priceWriteBack`（§5.3 `set_work_order_step_price`）在步骤 3（release drawer 填价）就上线。步骤 3 上线后运营即开始填价/改价，`work_order_routings.unit_price` 被频繁变更——而此时 `list_all_wage_summaries:185` 仍用重算、`get_wage_summary:112` 仍用冻结，两套口径在步骤 3→4 窗口被放大可见。设计 §5.4 自己定性「这是计件工资系统的红线」「发薪页打架」，却把红线修复排在触发源之后。
- **P1（新发现）**：BOM 复制是制造业 ERP 标配。当前 BOM 模块无复制功能（grep 确认 `bom/implt.rs` 无 `duplicate_bom`/`copy_bom`），看似不是缺口，但正因如此风险更大：未来产品经理提「BOM 复制」需求时，实现者很可能只复制 `boms` + `bom_nodes`，遗漏 `bom_operations` + `bom_step_prices`（按 `product_code` 关联而非 `bom_id`，新产品 `product_code` 不同，工序行不会自动随拷）→ 复制出的 BOM 有物料无工序无价，工单下达后 `bom_operations` 为空，报工环节才炸。设计文档未预留这个约束声明。
- **P1（否-确认，深化）**：§10-Q1 建议「在 products 改名路径加守卫」，但读代码确认 `UpdateProductReq`（`:230-237`）根本不含 `product_code` 字段——应用层完全没有改 `product_code` 的入口。Q1 守卫的落点是空的，设计文档「在 products 改名路径加守卫」会误导实现者去找不存在的入口。
- **P2（新发现）**：§5.2 per-order lock 定义「任一 step 报工即整单工序结构冻结」，但未定义工单取消（Cancelled）/完工（Completed）状态下的改价解锁。工单取消后已报工 step 想修正单价 → 被 `has_any_report` 拒。设计未声明「取消单是否解锁改价」。
- **P2（新发现）**：`bom_operations`/`bom_step_prices` 按 `product_code` 关联（非 `bom_id`），而 `find_published_by_product_code`（`bom/repo.rs:320-342`）`ORDER BY bom_id DESC LIMIT 1` 取最新已发布 BOM。同成品多 BOM 版本共享同一组工序行——v2 发布后，`bom_operations` 的工序「真相源」对 v1 在产工单也变了。运营的心智模型是「BOM 版本隔离」，工序的共享行为会违反预期。物料走 `bom_snapshots`（release 时整版 JSONB 冻结），工序走 `product_code`（跨版本共享）——版本隔离粒度不对称。

### 🎨 前端 / UIUX 工程师（交互与前端性能视角）

**summary**：设计总体交互方向正确（drawer 化、HTMX 三原则、事件解耦），但 §9 步骤 2/3 的若干前端实现细节未钉死，会导致实现期返工或上线后体验问题。发现 **8 个 P1 + 1 个 P2**：

- **P1（新发现）**：设计 §10 M7 / §9 步骤 2 写「抽 `cat-select` 到 `app.js` 全局函数（`toggleOpCombo`/`selectCat`/`filterCatOptions`）」，**但代码现状是**：`selectCat`（`app.js:554`）和 `filterCatOptions`（`app.js:543`）早已是 `window.*` 全局函数，`category_select.rs:64/72` 已通过 `call` 复用；只有 `toggleOpCombo`/`closeOpCombo` 是 `routing_create.rs:706/728` 内联。设计把三个函数并列为「待抽取」误述现状，会让实现者重复抽取已有的两个。真正要处理的是 `toggleOpCombo` 的定位策略（position:fixed + getBoundingClientRect）在 drawer 内可滚动工序列表的适配。
- **P1（新发现）**：设计 §9 步骤 2 / M7 写「上/下移按钮服务端 swap 相邻 step_order 后返回刷新片段」，但**未定义「刷新片段」的边界**。若 handler 只返回被点击行（`hx-target="closest tr"`），相邻那一行的 `step_order` 文本不会被刷新，显示成旧序号——「第 3 行上移后还是 3，第 2 行变成 2」的错乱。违反 htmx-patterns 核心原则「handler 返回完整片段、前端 hx-select 选取」——move 操作影响面是两行，必须返回两行所在的 tbody。
- **P1（新发现）**：设计 §5.3 / §9 步骤 3 把价编辑落 release drawer 行内（`mes_work_center.rs:2125-2130` 外层 `<form hx-post=WcReleasePath>`，`render_release_routing_row:2227` 在 `<td>` 内放 `<input hx-post=WoStepPricePath hx-trigger=blur hx-target="closest tr">`）。两个问题：(a) **竞态**：用户填完价直接点「确认下达」——input blur 触发改价 POST，按钮 click 同时触发外层 form release POST。两个请求并发到达顺序不保证，`release_order:2313-2324` 读 routings 校验 `price_missing` 时，若改价请求未先到，仍看到 NULL→误判失败（重渲染整 form）。(b) **失败重渲染清空未 blur 输入**：`:2329-2333` 失败时 `render_release_drawer_body(&data, Some(&errors))` 重渲染整个 form body，会把所有行里用户已输入但尚未 blur 的 `<input>` 值全部清空（outerHTML 重建节点）。
- **P1（新发现）**：设计 §6.4 / §8 直接删除 `routing_create.rs:548-557` 的警告条（「删除或重排已有工序将被拒绝」），理由是「护栏已移除」。但 routing 降级为 copy-on-write 模板后，编辑者面对的就是「我改了模板，到底影不影响已绑的 BOM」的核心心智问题——比 clean break 期更需要提示，而不是更少。直接删 → 用户以为改模板仍会同步到 BOM（旧活绑定心智残留）。
- **P1（新发现）**：设计 §6.4 让 `routing_detail.rs:588-594` 每行关联 BOM 的「维护产出」按钮改为「拷贝工序到此 BOM」，调用 `apply_routing_to_bom(force=false)`。但 §4.1 trait 契约写明 `force=false` 时「已有工序行则 Err」，§6.4 又规定失败「按 §5.6 form 校验失败规范返回重渲染列表片段 + 行内 alert，禁 toast」。结果：BOM 已有 `bom_operations` 的行上，用户点「拷贝」→ 看到一行小红字 → 没别的视觉变化。按钮既不 disabled、也无「已拷贝/已编辑」状态，是典型 dead-click UX。
- **P1（新发现）**：设计 §9 步骤 2 写「新增 `#bom-op-drawer` 复用 retired `output_edit_drawer` 骨架」。但 `components::overlay::drawer_shell`（`:45-55`）**硬编码 `z-[90]`**，且与 `modal_shell`（`:28` 接受 `z_class` 形参）不同——`drawer_shell` 没有 `z_class` 参数。`bom_edit.rs` 既有 3 个 `modal_shell` 全是 `z-[1000]`（`:448/:728/:842`）。新增 drawer 落 `z-90` → 一旦用户在 drawer 内要触发产出品候选选择，picker 必须在 `z-1100` 才能盖住 drawer，而当前 `product_picker` 通常用 `z-1000`/`modal_shell`——会被 drawer 遮挡或与其它 bom modal 同层乱堆。同时设计指向「retired output_edit_drawer 骨架」（`routing_detail.rs:658-680` 手写 hyperscript）而非已收敛的 `drawer_shell` 单一来源，方向倒退。
- **P1（新发现）**：`overlay.rs:17-21` `overlay_hs()` 同时给 `modal_shell` 和 `drawer_shell` 生成 `on keydown[event.key is 'Escape'] from body remove .<open_class>`，两者都监听 body keydown。`bom_edit.rs:796` 的 bom-edit-slot 又有自己的守卫 `on keydown[event.key is 'Escape' and #bom-edit-modal] from body remove .is-open from #bom-edit-modal`——守卫条件是「`#bom-edit-modal` 存在于 DOM」，一旦用户曾打开过节点编辑（slot 被填充），守卫永远为真。结果：在 BOM 编辑页若 `bom-op-drawer`（.open）与任一 `bom-*` modal（.is-open）同时存在，按一次 Esc → body 上两个监听都触发 → drawer 和 modal 同时关闭。用户预期通常只关最内层。
- **P1（新发现）**：设计 §8 / M6 让 `bom_detail.rs:1006-1007` 的「去工艺路径配置计件单价 →」链接改指向 `BomEditPath#bom-ops-card`。但 `bom-ops-card` 在 §9 步骤 2 的工序列表实现里只有「工序 / 产出品 / 工作中心 / 委外 / 上下移」，**没有计件单价输入框**——价独立在 `bom_step_prices` 表，D7 把填价时机推迟到 release drawer。结果链接把用户带到 `bom-ops-card`，用户在那里找不到任何价输入——比原来指向 `RoutingListPath` 还误导。
- **P2（新发现）**：设计 §9 步骤 3 / M9 要求 release drawer 内联价编辑「命名事件 `woPriceChanged`」。但全文未指定任何监听方——inline 价保存只需行自替换（`hx-target="closest tr"` + outerHTML），不需要事件广播。htmx-patterns §4.2 明确「事件名 = `<资源>Changed`，全局唯一，实际有监听方」——没有监听方的事件是噪声。

### 👷 最终使用者（操作员/计划员/车班长视角）

**summary**：设计把「价的真相源」从 BOM 定义期推迟到工单下达期（release drawer 填价）这一核心决策，对操作面冲击最大却没有被 §9 实现计划完整承接。发现 **1 个 P0 + 4 个 P1 + 4 个 P2**：

- **P0（新发现）**：release drawer 的校验失败文案与 D7/§5.1 直接冲突，会驱使操作员做错误且破坏性的动作。`release_order` 校验 `price_missing` 时（`:2319-2323`）走 `ReleaseErrors::summary()`，其文案 `:1785` 写死「请在工艺路线模板补全产出品/单价后重新创建工单」、`empty_routings` 分支 `:1775` 写「请到「工艺路线管理」关联后重新创建工单」。但定稿 D7 已把填价收敛到 release drawer 本身（§5.3 input cell），§5.1 release 兜底还会在下达时 `try_load` 补工序——也就是说既不用去工艺模板、也不用重建工单。§9 步骤 3 完全漏掉 `summary()` 这两段文案和「`price_missing` 仍作为 release 阻断」的语义。结果：车班长看到红框，按文案去删工单、重建、再跑去 routing 模板改价——全是无效且危险的操作。
- **P1（新发现）**：在 release drawer 给一道工序改价，会静默改写「该 BOM 所有后续工单」的工资单价，操作员无从感知这是共享写入。`set_work_order_step_price`（§4.4/§5.3）单事务两步：(a) `bom_step_prices.upsert` 写真相源；(b) `UPDATE work_order_routings` 刷本工单快照。`render_release_routing_row`（`:2252-2254`）的价 cell 既不区分「本工单临时改」还是「改的是跨工单主数据」，也没有任何「此单价将应用于该产品未来所有工单」的提示。车班长为救一个急单把某工序从 0.5 改到 0.8，全车间后续同 BOM 工单的计件工资全部跟着涨——工资系统的红线盲操。
- **P1（新发现）**：逐 step `blur`→`hx-post`→整行 outerHTML 自替换的填价方式，破坏连续键盘录入，新 BOM 首单 N 道工序要断 N 次焦。§5.3 ②「blur 触发 hx-post WoStepPricePath → 行自替换」。对一个 8 道工序的新 BOM 首次下达，操作员要逐个点进 8 个 input、每次 blur 都触发一次 POST 并把整 tr 重建——outerHTML 自替换会丢焦点，Tab 顺序被打断。8 次 POST + 8 次重定位。
- **P1（新发现，同前端 UIUX）**：§8 退役清单给 `bom_detail` 计件单价链接拟的替换文案「去 BOM 配置工序单价 →」本身是错的——BOM 编辑页按 §9 步骤 2 只有「工序 CRUD + 产出品 + 工作中心 + 上下移」，根本没有单价字段（单价按 D7 只在 release drawer 填）。操作员点过去会扑空。
- **P1（新发现）**：BOM 成本报告人工费展示在新设计下对未投产 BOM 永久退化。`bom_detail.rs:1003`「（所有工序单价为0）」+ `:1005 has_any_zero_price` 触发条件读的是 `labor_costs.unit_price`。切源后（§7.5）该数据改从 `bom_step_prices` 取，而 `bom_step_prices` 在「该 BOM 的第一张工单被下达并现场填价」之前永远是空的。任何新建 BOM、任何还没下过工单的 BOM，在 BOM 详情页成本报告里都会显示「所有工序单价为0」+ 一条误导链接。IE/成本会计在投产前评估 BOM 人工成本的能力被结构性拿掉了。
- **P1（否-确认，深化）**：Q3 报工 re-check 的拒绝消息内容未规定，工人被打回后不知道该找谁、去哪定价（同业务专家 P2）。生产现场确认点在 `confirm_routing_step:224`，工人是手机/扫码端报工，看到「先定价」三个字完全不知道下一步。
- **P2（新发现）**：同一张 BOM 编辑页上，物料树用拖拽、工序用上下移按钮，两种排序交互并存，不符合车间直觉且低效。`bom_edit.rs:700` `bom-sortable-tbody` + `:929 draggable=true` 物料节点是 SortableJS 拖拽；§9 步骤 2 给新工序列表定的却是「上/下移按钮」。一个 10 道工序的工艺，把最后一道移到第一道要点 9 次「上移」。禁 fetch-on-drop 的技术理由成立，但 SortableJS drop 事件完全可以用 hyperscript `call` 桥接到 HTMX `hx-post`，不必退化到按钮。
- **P2（否-确认，深化）**：`apply_routing_to_bom` 的 copy-on-write 独立性文案漏实现术语、且两个拷贝入口的提示不一致。§6.2 给 routing_detail 拷贝按钮拟的文案「force 重拷」是 §4.1 trait 参数的实现术语，操作员看不懂。同时 §9 步骤 2 说 BOM 编辑页也有「拷贝」按钮，但那里要不要弹同样的独立性警告、文案是什么，design 完全没说。
- **P2（新发现）**：release drawer 没有区分「价是这次自动从 BOM 加载的」还是「本会话刚手工填的」，操作员无法 3 秒判断这单的单价是不是自己想要的。§5.1 load 流程把 `bom_step_prices` 的值写进 `work_order_routings.unit_price`；§5.3 填价后行自替换显示已填价。两种来源在 `render_release_routing_row:2252-2254` 里都渲染成同一个 `fmt_qty` 数字。对工资敏感数据，车班长下达前想核对「这个 0.5 是上次 IE 定的、还是刚才小张临时填错的」——无从分辨。

---

## 三、最终整合方案（修改项清单）

合并 6 角色 findings，去重后按 P0→P1→P2 排序。每条指向具体 file:符号。

| 序号 | 修改项 | 涉及文件/符号 | 修改类型 | 优先级 | 新发现? |
|------|--------|-------------|---------|--------|--------|
| R-1 | **迁移前审计 `bom_labor_processes.quantity<>1` 分布**：跑 `SELECT product_code, COUNT(*) FILTER (WHERE quantity<>1) FROM bom_labor_processes WHERE deleted_at IS NULL GROUP BY 1`。三选一：(a) 全=1 则迁移 SQL 显式 `WHERE quantity=1` 断言；(b) 有真实语义则 `bom_step_prices` 加 `quantity` 列或成本报告公式补 quantity 维度；(c) 至少把迁移改为「等价折算」`unit_price*quantity` 后写入单件单价。§7.4 M3 门禁必须加「迁移前后 `BomCostReport` 总人工成本全量比对」 | `abt-core/src/master_data/bom/implt.rs:821`（`try_build_labor_from_routing` quantity=ONE）+ `:847`（`build_labor_from_legacy` quantity=r.quantity）+ `abt-core/src/master_data/bom_labor_process/model.rs:13`（BomLaborProcess.quantity）+ `docs/uml-design/bom-operation-inline.md:118-127`（§3.2 bom_step_prices schema）+ `:580-596`（§7.4） | 修改（设计 + 迁移 SQL） | **P0** | 是 |
| R-2 | **§9 步骤 1 与步骤 2 合并为一次原子部署**。若坚持分步，步骤 1 必须：(a) 把 `apply_routing_to_bom` handler 也挪到步骤 2（步骤 1 只删 overlay + 护栏，不加拷贝按钮，或按钮置灰 + 提示「即将上线」）；(b) 在步骤 1 发布说明里显式公告「产出品/工序维护暂冻结，X 日内步骤 2 上线后恢复」，并保留 `bom_routing_outputs` 表只读视图供运营查阅现状 | `abt-web/src/pages/routing_detail.rs:588-594`（维护产出按钮）+ `docs/uml-design/bom-operation-inline.md` §9 步骤 1 / §4.1 `apply_routing_to_bom` | 修改（设计 §9） | **P0** | 是 |
| R-3 | **§9 步骤 2 与步骤 4 合并为一次原子部署**。若必须分步，二选一：(a) 步骤 2 的 BOM 编辑器只开放「只读展示 + 从 routing 拷贝」，不开放 upsert/delete（编辑能力随步骤 4 切源同步开放）；(b) 步骤 2 阶段 BOM 编辑器保存时双写（`bom_operations` + `bom_routing_outputs` 同步）。推荐 (a)，并在 §9 步骤 2 验证项补一条「工单 load 读取的数据源断言」回归用例 | `abt-core/src/mes/work_order/implt.rs:51-75`（`try_load_routings_from_bom` 仍读老路径）+ `docs/uml-design/bom-operation-inline.md` §9 步骤 2/步骤 4 | 修改（设计 §9） | **P0** | 是 |
| R-4 | **release drawer 校验失败文案改写**。`ReleaseErrors::summary()` 两段文案改为：`empty_routings`→「该产品 BOM 尚未配置工序，请到 BOM 编辑页配置（或从工艺路线拷贝）后直接重新下达，无需重建工单」；`price_missing`→「以下工序尚未定价，请在下方表格直接填写单价后点下达」。§9 步骤 3 显式追加此项，`release_order:2319` 的 `price_missing` 校验保留作兜底但文案指向 drawer 内联填写 | `abt-web/src/pages/mes_work_center.rs:1775`（`empty_routings` 文案）+ `:1785`（`price_missing` 文案）+ `:2319-2323`（release_order 校验） | 修改 | **P0** | 是 |
| R-5 | **`bom_step_prices` 分表后的级联清理**。`replace_operations`（BOM 保存）和 `apply_routing_to_bom(force=true)` 内部，`DELETE bom_operations` 后同步 `DELETE FROM bom_step_prices WHERE product_code = $1`（整组清，随 BOM 保存事务原子）。补一条 e2e：BOM 有价后删 step2、改 step3→step2 保存，新工单不应加载到旧 step2 的价 | `docs/uml-design/bom-operation-inline.md` §4.1 `replace_operations` / §6.2 `apply_routing_to_bom` force=true（对照 `abt-core/src/master_data/bom_routing_output/repo.rs:9-37` upsert 同行级联） | 新增（设计 + 实现） | **P0**（原 P1 升级） | 是 |
| R-6 | **产出品校验措辞校准**。§3.5 / §4.1 / Q2 的措辞从「∈ 该 BOM 非叶子节点」校准为「∈ 该 `product_code` 下 Draft+Published 所有 BOM 版本的非叶子节点并集（与既有 overlay 行为一致）」。补 Q2 已提到的周期性一致性检查脚本：扫描 `bom_operations.output_product_id` 不 ∈ 其 `product_code` 对应【已发布】BOM（status=2）非叶子节点集的行输出告警 | `abt-core/src/master_data/bom/repo.rs:361-378`（`list_non_leaf_product_ids_by_codes` WHERE status IN (1,2)）+ `docs/uml-design/bom-operation-inline.md:207`（§3.5）/ `:250`（§4.1）/ `:736`（Q2） | 修改（设计文档） | P1 | 是 |
| R-7 | **双溯源单源真相**。`apply_routing_to_bom`（含 force=true 分支）内部同步更新 `bom_routings.routing_id`（调 `set_bom_routing` 或直接 UPDATE），保持 `bom_routings` 与 `source_routing_id` 首行一致。若不想动 `bom_routings`，则 §5.1 step7 的 routing_id 取值改为「优先 `source_routing_id` 首行，回退 `bom_routings`」 | `docs/uml-design/bom-operation-inline.md:455-461`（§6.3）+ `:360`（§5.1 step7）+ `abt-core/src/mes/work_order/implt.rs:70`（`update_routing_id`）+ `abt-core/src/master_data/routing/repo.rs:266`（`list_boms_by_routing`） | 修改（设计 + 实现） | P1 | 否-确认 |
| R-8 | **Released 未报工工单的主动 reload 入口**。(a) release drawer 打开时对「全无报工」的工单自动触发一次 `try_load_operations_from_bom`；或 (b) `bom_step_prices.wage` 改价后广播事件让 Released 未报工工单的 release drawer 行自刷新。至少在 §5.2 显式记录该缺口：「Released 未报工工单的工序快照不会随 BOM 自动同步，需用户在 release drawer 主动触发 reload」 | `abt-core/src/mes/work_order/implt.rs:196`（release 兜底仅 is_none 触发）+ `abt-core/src/mes/production_batch/implt.rs:591`（reload 状态门）+ `docs/uml-design/bom-operation-inline.md:374`（§5.2 per-order lock） | 新增（设计 + 实现） | P1 | 是 |
| R-9 | **per-order lock 的 TOCTOU 加锁**。`load_operations_from_bom` 在事务起始 `SELECT id FROM work_orders WHERE id=$1 FOR UPDATE`（或 SELECT FOR UPDATE 全部 work_order_routings 行），再做 `has_any_report` 检查 + reload。至少在 §5.1 step5 显式声明锁策略与 FK 撞击时的错误映射（把 FK violation 映射为 business_rule 友好消息） | `abt-core/src/mes/production_batch/repo.rs:378-395`（`has_any_report`）+ `docs/uml-design/bom-operation-inline.md` §5.1 step5 | 修改（设计 + 实现） | P1 | 是 |
| R-10 | **`list_all_wage_summaries` 静默吞 DB 错误清理**。把 `.ok().unwrap_or_default()`（`:154-157`）改为 `.map_err(\|e\| DomainError::Internal(e.into()))?` 正常传播，与 `calculate_wage`（`:96-98` 已是 map_err 传播）对齐。D6 改动 `:177-185` 时一并处理 `:154-157` | `abt-core/src/mes/work_report/implt.rs:154-157` | 修改 | P1 | 是 |
| R-11 | **`has_routing` 改读 `work_order_routings` 实际存在性**。改为 `SELECT EXISTS(SELECT 1 FROM work_order_routings WHERE work_order_id=$1)`，而非 `routing_id.is_some()`。或至少在事件 payload 里并行加 `operations_count` 字段 | `abt-core/src/mes/work_order/implt.rs:274`（`has_routing`）+ `:571`（`routing_doc` 读 `order.routing_id`） | 修改 | P1 | 否-确认 |
| R-12 | **per-order lock 后补「工单级补工序」运维接口**。如 `ProductionBatchService::append_work_order_routing(wo_id, after_step_no, process_name, ...)`，绕过 per-order lock 的 reload 路径，单事务 INSERT `work_order_routings` + 初始化 `batch_routing_progress`，记审计日志标「补工序（突破整单冻结）」，要求更高权限（SUPERUSER 或独立 WORK_ORDER_AMEND）。或在 §5.2 显式记录该缺口 + 文档化「DBA 直改」标准操作流程作为短期兜底 | `abt-core/src/mes/production_batch/implt.rs:204`（防跳序 guard）+ `:354-355`（`max_step` 判定）+ `docs/uml-design/bom-operation-inline.md:365-376`（§5.2 D8） | 新增 | P1 | 是 |
| R-13 | **计件单价定价权限粒度**。新增独立资源权限（如 `BOM_STEP_PRICE`/`update`），`set_work_order_step_price` handler 用该权限而非 `WORK_ORDER update`。短期若不加权限枚举，至少在 §5.3 / Q7 记录「定价权限=WORK_ORDER update」是接受的妥协，并要求月度审计报告按 `operator_id` 聚合「谁定了哪些价」 | `abt-web/src/pages/mes_work_center.rs:2291`（`release_order` 权限）+ `docs/uml-design/bom-operation-inline.md:378-406`（§5.3）+ `:741`（Q7） | 新增（设计 + 实现） | P1 | 是 |
| R-14 | **legacy 补价映射规则用 process_code 对齐**。§7.4 明确：legacy 补价必须经 `process_code`（稳定身份键）做二次对齐——`JOIN bom_labor_processes blp ON blp.product_code=bo.product_code AND blp.process_code=bo.process_code`（而非 `sort_order==step_order`）。M3 门禁补一条「补价后每个 product_code 的有价工序数 = legacy 有价工序数」校验 | `docs/uml-design/bom-operation-inline.md:592-594`（§7.4 二次回填映射）+ `abt-core/migrations/013_create_labor_routing.sql:89`（`sort_order`）+ `abt-core/src/shared/excel/labor_process_import.rs:38` | 修改（设计 + 迁移 SQL） | P1 | 是 |
| R-15 | **单价变更历史审计表**。二选一：(a) 新增 `bom_step_price_history`（product_code, step_order, old_price, new_price, source_type, source_wo_id, operator_id, created_at），`set_work_order_step_price` 与 BOM 定价入口都追加一行；(b) 至少在 `upsert_price` 实现里强制调 `audit_log_service.record` 记 `changes=format!("{}→{}", old, new)`。Q7 升级为「至少记文档 + 落实最小溯源字段」 | `docs/uml-design/bom-operation-inline.md:118-127`（§3.2 schema）+ `:302-305`（§4.2 `upsert_price`）+ `:741`（Q7） | 新增（设计 + 实现） | P1 | 是 |
| R-16 | **D6 wage 口径修复前移到步骤 3 同次部署**。不能拖到步骤 4。更稳妥：D6 作为独立 hotfix 先于整个重构上线（它是与本次重构解耦的独立现存 latent bug，先修可降低本次重构的回归面）。§9 步骤 4 的 D6 条目改为「验证步骤 3 前置修复后两个接口返回一致」 | `abt-core/src/mes/work_report/implt.rs:185`（重算）vs `:112`（冻结）+ `docs/uml-design/bom-operation-inline.md` §9 步骤 4 / §5.4 | 修改（设计 §9） | P1 | 是 |
| R-17 | **`bom_detail` 计件单价链接落点钉死**。三选一：(a) 链接改为指向该产品的待下达工单列表（release drawer 是真正填价处）；(b) 在 BOM ops card 补一个只读「当前计件单价」列 + 「去工单下达填价」入口；(c) 若坚持 §4.2 注释承诺的「BOM 页直接定价」，则在 §9 步骤 2 的 ops card 显式补价 input。设计 §8 / §9 步骤 2 必须二选一钉死。同时接受未投产 BOM 成本报告人工费恒为 0 并在 `:1003` 提示文案写清「单价在首张工单下达时确定，投产前此处为 0 属正常」 | `abt-web/src/pages/bom_detail.rs:1003-1007`（链接 + 提示文案）+ `docs/uml-design/bom-operation-inline.md:652`（M6）/ §9 步骤 2 | 修改（设计 + 实现） | P1 | 是 |
| R-18 | **M7 措辞校正 + toggleOpCombo 抽取策略**。改写 M7：「`toggleOpCombo`/`closeOpCombo` 抽到 `app.js`（`selectCat`/`filterCatOptions` 已存在，直接复用）；产出品/工序选择器收敛到 `components/process_picker.rs`」。显式选定定位策略：`routing_create` 的 `toggleOpCombo` 用 `position:fixed` + `getBoundingClientRect`（适配可滚动表格），`category_select` 用 `position:absolute top-full`（简单）；在 drawer 内的可滚动工序列表里 `fixed` 方案更稳，`process_picker` 统一用 `fixed` | `abt-web/src/pages/routing_create.rs:706,728`（`toggleOpCombo`/`closeOpCombo` 内联）+ `static/app.js:543,554`（`selectCat`/`filterCatOptions` 已全局）+ `abt-web/src/components/category_select.rs:64,72`（已 call 全局）+ `docs/uml-design/bom-operation-inline.md` §10 M7 / §9 步骤 2 | 修改（设计文档） | P1 | 是 |
| R-19 | **move up/down 返回完整 tbody 片段**。钉死交互契约：上/下移按钮 `hx-post=BomOperationMovePath` + `hx-target="closest tbody"` + `hx-select="#bom-ops-tbody"` + `hx-swap="outerHTML"`，handler 返回完整工序列表片段（重新查 `list_operations` 排序后渲染整个 tbody）。设计 §9 步骤 2 显式写明这两条 hx 属性 | `docs/uml-design/bom-operation-inline.md:681`（§9 步骤 2 上/下移按钮段） | 修改（设计文档） | P1 | 是 |
| R-20 | **release drawer 内联价编辑的竞态 + 失败重渲染防御**。(a) 给价 input 加 `hx-sync="closest form:replace"`，让外层 submit 与内层改价排队（改价优先）；或 `release_order` 入口对「同 form 内刚改过价的 step」宽容（读 form 提交值而非 DB）。(b) 在 `render_release_routing_row` 的价 input 上加 `hx-preserve`（htmx-patterns §2.2 / `fms_ap_ledger.rs:293-348` 范本），outerHTML 自替换时保留用户输入；或把 errors 重渲染改为只刷出错行/顶部 alert，不动其它行 | `abt-web/src/pages/mes_work_center.rs:2125-2130`（外层 form + afterRequest 守卫）+ `:2227-2264`（`render_release_routing_row`）+ `:2329-2333`（`release_order` 失败重渲染）+ `docs/uml-design/bom-operation-inline.md:378-406`（§5.3）/ §9 步骤 3 | 修改（设计 + 实现） | P1 | 是 |
| R-21 | **release drawer 填价改 blur 不自替换整行**。两选一：(a) blur 保存但不自替换整行——改用 `hx-swap-oob` 只刷新一个小的「✓已保存」徽标，input 节点保留不断焦；(b) 干脆不在 blur 逐 step 保存，改为「下达」按钮一次提交所有价（单事务批量 upsert `bom_step_prices` + `work_order_routings`），既不断焦又原子。任一方案都应在 cell 上给出「已保存·下次自动加载」的微提示 | `abt-web/src/pages/mes_work_center.rs:2227-2264`（`render_release_routing_row`）+ `docs/uml-design/bom-operation-inline.md` §5.3 ② | 修改（设计 + 实现） | P1 | 是 |
| R-22 | **release drawer 改价共享写入提示**。价 cell 旁加状态徽标：「主数据·本产品通用」并用 tooltip 写「此单价保存后对该产品后续所有工单生效」；对「只想改本单」的场景，至少在 design 里显式记录为已知缺口（当前无 per-order-only 价位），或给一个「仅本工单」checkbox 走只更 `work_order_routings`、不 upsert `bom_step_prices` 的分支。同时区分「自动加载自 BOM 主数据」vs「本会话手工填入已落库」两种来源的视觉标记 | `abt-web/src/pages/mes_work_center.rs:2252-2254`（`render_release_routing_row` 价 cell）+ `docs/uml-design/bom-operation-inline.md` §4.4 `set_work_order_step_price` step(a) / §5.1 / §5.3 | 新增（设计 + 实现） | P1 | 是 |
| R-23 | **`routing_create` 警告条改写为正向信息条**（不删除）。文案改为：「此模板已绑定 N 个 BOM；编辑只影响未来新拷贝，已拷贝的 BOM 工序独立（需到 BOM 页 force 重拷才能同步）」。N 来自 `routing_detail` 已有的 `paginate_boms_by_routing` count，注入到 `routing_create` 模板（编辑模式下） | `abt-web/src/pages/routing_create.rs:548-557`（警告条 `@if is_edit && bound_count > 0`） | 修改 | P1 | 是 |
| R-24 | **`routing_detail` 拷贝按钮三态防呆**。渲染关联 BOM 列表时，一并查每个 `product_code` 是否已有 `bom_operations`（以及 `source_routing_id` 是否 = 当前 routing 且步骤数匹配），分三态渲染：(1) 未拷贝→「拷贝工序到此 BOM」可点；(2) `source_routing_id` 匹配且步骤数一致→「已同步」badge（禁用按钮）；(3) 有 `bom_operations` 但 source 偏离→「已自行编辑，force 重拷」次级按钮（带 confirm）。把 §4.1 的 force 出口落到 UI | `abt-web/src/pages/routing_detail.rs:588-594`（现「维护产出」按钮）+ `docs/uml-design/bom-operation-inline.md:451-453`（按钮文案） | 修改 | P1 | 是 |
| R-25 | **`drawer_shell` 加 `z_class` 形参**。改 `drawer_shell(id, z_class, width_class, inner)`，`z_class` 默认 `"z-[90]"`，BOM 页场景传 `"z-[1080]"` 让 drawer 盖住 bom modals。设计 §9 步骤 2 把「复用 retired output_edit_drawer 骨架」改为「复用 `components::overlay::drawer_shell`（单一来源）」。明确产出品选择用嵌套 picker（`z-1100`）还是 drawer 内 native `<select>`——若 BOM 非叶子节点数量通常 ≤ 20，native `<select>` 足够 | `abt-web/src/components/overlay.rs:45-55`（`drawer_shell` 硬编码 z-[90]）+ `abt-web/src/pages/bom_edit.rs:448,728,842`（z-[1000] modals） | 修改（组件 + 设计） | P1 | 是 |
| R-26 | **Esc 多浮层冲突：只关最顶层 overlay**。`overlay_hs` 的 Esc 守卫改为「仅当自己是最顶层 overlay 时才关」：可用 `me is event.target`（事件 target === 当前监听者）或在 keydown handler 里 `querySelector` 找到最后一个 `.is-open`/`.open` 的 overlay 只关它。或在 `drawer_shell`/`modal_shell` 显式记 z-index，Esc 只关 z 最高的那一个。设计 §9 步骤 2 显式声明本浮层 Esc 不与 bom modals 互踩 | `abt-web/src/components/overlay.rs:17-21`（`overlay_hs`）+ `abt-web/src/pages/bom_edit.rs:796`（bom-edit-slot Esc 守卫恒真） | 修改（组件 + 设计） | P1 | 是 |
| R-27 | **Q3 报工 re-check 错误消息可操作化**。`design §5.5` 把 re-check 的 `DomainError::business_rule` 文案写死为带动作路径的完整句：「工序 {step_no}「{process_name}」未定价，无法报工。请联系车间主任/IE 在工单下达页（工作中心→下达）填写计件单价后重试（工单 {doc_number}，产品 {product_code}）」。并确认该错误经 `abt-web/src/errors.rs` 映射后在工人端以可读 toast/横幅呈现。confirm re-check 统一为 `if routing.unit_price.is_none() \|\| routing.unit_price == Some(Decimal::ZERO)`（与 release_order `:2319` 对齐，拒 None + Zero） | `abt-core/src/mes/production_batch/implt.rs:224`（`unit_price.unwrap_or(ZERO)`）+ `docs/uml-design/bom-operation-inline.md:427`（§5.5）/ §10-Q3 | 修改（设计 + 实现） | P1 | 否-确认深化 |
| R-28 | **BOM 复制前瞻约束声明**。在 §3.1 或 §9 补一条前瞻约束：「BOM 复制（未来功能）须同步复制 `bom_operations` + `bom_step_prices`，`product_code` 字段映射到新产品的 code；`source_routing_id` 置 NULL（复制产物非拷贝自 routing）」。并建议把该约束落进 `BomCommandService` 的文档注释 | `abt-core/src/master_data/bom/implt.rs`（无 duplicate/copy 路径）+ `docs/uml-design/bom-operation-inline.md` §3.1 / §9 | 新增（设计文档） | P1 | 是 |
| R-29 | **`unit_price` 精度迁移显式 CAST**。迁移 SQL（§7.1 M2(b) 若扩充到从 `bom_labor_processes` 补价）显式 `CAST(unit_price AS NUMERIC(18,6))`，迁移前 `SELECT MAX(unit_price) FROM bom_labor_processes` 确认无 `> 10^12` 异常值。设计 §3.2 注释一句「`unit_price` 精度 NUMERIC(18,6)，与 `bom_routing_outputs` 对齐；`bom_labor_processes(20,4)` 迁入时精度收窄，已确认现网无溢出」 | `abt-core/migrations/013_create_labor_routing.sql:87`（`NUMERIC(20,4)`）+ `abt-core/migrations/096_routing_decouple_bom_routing_outputs.sql:16`（`NUMERIC(18,6)`）+ `docs/uml-design/bom-operation-inline.md:122` | 修改（设计 + 迁移 SQL） | P2 | 是 |
| R-30 | **`WageDetail.unit_price` 显示口径统一**。二选一：(a) 反算展示价 `unit_price = wage_amount / (completed_qty + non_operator_defect_qty)`（注意除零，两者都=0 时不显示）；(b) 长期方案给 `work_reports` 加 `unit_price_snapshot` 列，报工落库时与 `wage_amount` 同冻结。至少在 §5.4 文档说明该显示语义 | `abt-core/src/mes/work_report/implt.rs:105-108`（`calculate_wage`）+ `:177-179`（`list_all_wage_summaries`）+ 消费方 `abt-web/src/pages/mes_wage_list.rs:51` | 修改 | P2 | 是 |
| R-31 | **§5.3 has_report 守卫前置**。明确守卫在最前：① `get_by_work_order_and_step` 取 `wor.id` ② `has_report(wor.id)` 拒 ③ `upsert bom_step_prices` ④ `UPDATE work_order_routings`。§5.3 编号顺序重排为守卫前置 | `docs/uml-design/bom-operation-inline.md` §5.3 | 修改（设计文档） | P2 | 是 |
| R-32 | **`woPriceChanged` 事件删掉或指定监听方**。二选一：(a) 删掉 `woPriceChanged` 事件广播，价保存只做行自替换（YAGNI）；(b) 给它指定监听方——例如在 drawer 顶部 summary 区放一个「已填 N/M 道工序单价」计数器，监听 `woPriceChanged from:body` 刷新计数 | `docs/uml-design/bom-operation-inline.md:690`（§9 步骤 3 `woPriceChanged`） | 修改（设计文档） | P2 | 是 |
| R-33 | **per-order lock 状态矩阵补充**。§5.2 显式补充：「per-order lock 覆盖所有非 Draft/Released 状态（含 Cancelled/Completed），已报工 step 的改价与工序 reload 一律拒绝，即使工单取消——已做工的 wage 按原冻结价结算，不追溯调整」。若业务确认取消单允许改价，则在 §5.3 守卫里加状态白名单。退单/作废 wage 处置：`work_reports.wage_amount` 不因工单 cancel 回滚，`CostEntry` 已归集人工成本在工单作废时是否冲销交成本会计确认（记为开放问题） | `abt-core/src/mes/production_batch/repo.rs:378`（`has_any_report`）+ `abt-core/src/mes/work_order/implt.rs:320-393`（cancel 不清 work_reports）+ `docs/uml-design/bom-operation-inline.md` §5.2 / §5.4 | 修改（设计文档） | P2 | 是 |
| R-34 | **BOM 多版本运营公告 + 编辑器提示**。§3.1 或 §10 补一条：「同 `product_code` 多 `bom_id` 时，`bom_operations`/`bom_step_prices` 按 `product_code` 跨版本共享；运营若需版本级隔离工序，须为不同版本分配不同 `product_code`。物料（`bom_snapshots`）与工序（`bom_operations`）的版本隔离粒度不对称是已知设计取舍」。建议在 BOM 编辑器的工序列表顶部加一行提示文案 | `abt-core/src/master_data/bom/repo.rs:320-342`（`find_published_by_product_code` ORDER BY bom_id DESC LIMIT 1）+ `docs/uml-design/bom-operation-inline.md` §3.1 | 修改（设计文档 + UI 提示） | P2 | 是 |
| R-35 | **工资汇总按 `process_code` group**。两条路：(a) 工资汇总/Excel 导出改按 `process_code`（稳定身份键）group，`process_name` 仅作展示列——需 `WageDetail` 增加 `process_code` 字段（`work_order_routings` 快照里没有，load 时需补下沉）；(b) 不改 schema，文档化「字典改名后历史工资明细工序名不一致，按 `process_code` 对齐」作为已知行为 | `abt-core/src/mes/work_report/implt.rs:177`（`list_all_wage_summaries` 读 `process_name`）+ `docs/uml-design/bom-operation-inline.md:83`（§3.1 process_name 物化）/ `:277-279`（`resync_process_names` 不刷快照） | 修改 | P2 | 是 |
| R-36 | **release drawer「临时估价」逃生通道**。在 release drawer 增「临时估价」标记（`work_order_routings` 加 `is_provisional_price` 或 `bom_step_prices.unit_price` 旁加 `provisional` 标志列）：班长可标「临时」下达，`bom_step_prices` 写入但标 `provisional=true` 不被后续工单自动复用（或复用但标黄提示），待 IE 上班后 confirm 转正。或允许工单以「未定价」状态下达但禁止报工（Q3 的 re-check 已拦），把「卡下达」降级为「卡报工」 | `abt-web/src/pages/mes_work_center.rs:2319-2323`（`price_missing` 硬阻断）+ `docs/uml-design/bom-operation-inline.md:378-406`（§5.3 release drawer 唯一填价入口） | 新增（设计 + 实现） | P2 | 是 |
| R-37 | **工序排序用拖拽（与物料树一致）**。工序排序也用拖拽，drop 时由 hyperscript `_="on end call htmx.ajax('POST', movePath, {target:..., swap:'outerHTML', values:{...}})"` 触发服务端 swap（仍走 HTMX、不 fetch）。或至少在设计里记录「物料拖拽/工序按钮」的不一致是已知取舍 | `abt-web/src/pages/bom_edit.rs:330,700,879`（物料树 SortableJS）+ `docs/uml-design/bom-operation-inline.md` §9 步骤 2 工序上下移按钮 | 修改（设计 + 实现） | P2 | 是 |
| R-38 | **`apply_routing_to_bom` 文案操作员化 + 两入口统一**。文案改操作员语言：「拷贝后这些工序归本 BOM 所有，修改原模板不会影响这里；若日后模板更新想重新拉取，需先删除本 BOM 全部工序再拷贝」。两个入口（routing_detail 行内 + BOM 编辑页）用同一段 confirm 文案，且都走 `apply_routing_to_bom` 的 force=false/true 同一守卫 | `docs/uml-design/bom-operation-inline.md` §6.2（routing_detail 拷贝按钮文案）/ §9 步骤 2（BOM 编辑页拷贝按钮）+ `abt-web/src/pages/routing_detail.rs:586-604`（现「维护产出」入口） | 修改（设计文档 + 实现） | P2 | 否-确认深化 |

---

## 四、评审结论

### 4.1 P0 清单（必须实现前先处理）

| # | 修改项 | 核心风险 |
|---|--------|---------|
| **R-1** | 迁移前审计 `bom_labor_processes.quantity<>1` 分布，`bom_step_prices` 加 quantity 列或等价折算 | **计件工资/成本核算数据正确性红线**——legacy `quantity` 语义丢失致成本腰斩 |
| **R-2** | §9 步骤 1 与步骤 2 合并原子部署（或步骤 1 不加 handler/不删入口） | `apply_routing_to_bom` 运行时报错 `relation bom_operations does not exist` + 运营无 UI 维护产出品 |
| **R-3** | §9 步骤 2 与步骤 4 合并原子部署（或步骤 2 编辑器只读 + 拷贝） | `bom_operations` ↔ `routing_steps` 双真相源 split-brain——用户改工序工单看不到，验证项不覆盖 |
| **R-4** | release drawer 校验失败文案改写（`summary()` 两段） | 文案反向指引 D7 核心决策，驱使操作员删工单/改路由模板的危险操作 |
| **R-5** | `bom_step_prices` 分表后的级联清理（`replace_operations` / `apply_routing_to_bom force=true`） | step_order 复用把「焊接的价」挂到「测试工序」——计件工资资产错配（数据正确性红线，由 P1 升级） |

### 4.2 设计是否可进入实现

**结论：不可直接进入实现。** 5 条 P0 必须先回填到设计文档（`docs/uml-design/bom-operation-inline.md`）的对应章节（§3.2 schema / §5.3 交互闭环 / §7.4 迁移审计 / §9 分步计划 / §8 退役清单），其中：

- **R-1/R-5 是数据正确性红线**（计件工资资产），必须在迁移 SQL 与 `replace_operations`/`apply_routing_to_bom` 实现里落实，不能只记文档；
- **R-2/R-3 是部署窗口断链**，必须改 §9 分步计划（合并步骤或调整步骤边界 + 验证项）；
- **R-4 是核心决策反向指引**，必须改 `ReleaseErrors::summary()` 文案（`mes_work_center.rs:1775,1785`）。

P1 清单（R-6 ~ R-28，共 23 条）建议在实现启动前的设计评审会上一并确认落点，其中**计件工资内控相关（R-13 权限粒度、R-15 审计历史、R-22 共享写入提示）建议本次一并实现**——它们是劳动法合规与内控审计的硬要求，拖延即技术债。

P2 清单（R-29 ~ R-38，共 10 条）多为文档措辞校正、显示口径统一、体验优化，可在实现过程中顺手处理或在下一迭代补齐。

### 4.3 仍需用户决策的新开放问题

| # | 问题 | 背景 |
|---|------|------|
| **N-1** | `bom_labor_processes.quantity` 现网是否有 `<>1` 的真实业务语义？（R-1 三选一的前提） | 决定 `bom_step_prices` 是否需要加 `quantity` 列、或迁移走「等价折算」、或直接 `WHERE quantity=1` 断言。需跑审计 SQL 交业务确认 |
| **N-2** | §9 是否接受合并步骤 1+2、步骤 2+4 为两次原子部署？还是坚持 5 步分步（接受窗口期冻结公告）？（R-2/R-3） | 合并部署更安全但回滚面变大；分步部署需明确中间态冻结运营公告 + 补强验证项 |
| **N-3** | 计件单价是否本次就加独立权限枚举（`BOM_STEP_PRICE`/`update`）+ 月度审计历史表？（R-13/R-15） | 中国制造业计件工资涉及劳动法合规、工会 scrutiny。建议本次加；若接受妥协（沿用 `WORK_ORDER update`），至少审计报告按 `operator_id` 聚合供成本会计复核 |
| **N-4** | 未投产 BOM 是否需要「预定价」入口（让成本报告投产前有数）？（R-17） | 决定 `bom_detail` 链接落点 + 是否在 BOM 编辑器补价 input。若否，接受未投产 BOM 成本报告人工费恒为 0 并在提示文案写清 |
| **N-5** | release drawer「临时估价」逃生通道是否本次做？（R-36） | 夜班/周末 IE 不在场时急单卡下达会引发影子流程。建议加 `is_provisional_price` 标记；若否，至少允许「未定价下达但禁止报工」降级 |

### 4.4 与已吸收 D6-D11 的对比

| 维度 | 前轮 phase 3 已覆盖（D6-D11 + Q1-Q7） | 本次新发现 |
|------|--------------------------------------|-----------|
| **数据正确性** | D6 wage 口径统一（冻结值）；M3 migration 门禁；M4 孤儿 bom_routing_outputs 预检 | **R-1 quantity 迁移红线**（legacy quantity≠1 致成本腰斩，§7.4 审计盲区）；**R-5 bom_step_prices 分表后级联清理缺失**（step_order 复用错配价） |
| **锁定策略** | D8 per-order lock（任一报工即整单冻结） | **R-8 Released 未报工工单无 reload 入口**（计件价不随 BOM 同步）；**R-9 TOCTOU 加锁**（并发报工撞 FK）；**R-12 漏工序无运维接口**（工单死锁） |
| **填价落点** | D7 release drawer（工单详情页已下线） | **R-4 文案反向指引**（summary 文案与 D7 冲突）；**R-20 竞态 + 失败重渲染清空输入**；**R-21 blur 断焦**；**R-22 共享写入无提示** |
| **双源/溯源** | D9 routing_id 停写保留作溯源；Q6 bom_routings 保留 | **R-7 双溯源单源真相**（force 重拷后 bom_routings 与 source_routing_id 分叉）；**R-11 has_routing 改读 work_order_routings 实际存在性** |
| **迁移链** | M2(a)/(b) 回填 + M3 门禁 + §7.4 bom_labor_processes 双轨审计 | **R-1 quantity 审计盲区**；**R-14 sort_order↔step_order 映射规则未定义**（须用 process_code 对齐）；**R-29 精度 CAST** |
| **部署计划** | §9 五步分步（每步可独立验证 + 回滚） | **R-2 步骤 1↔2 硬依赖**（handler 调用不存在的表）；**R-3 步骤 2↔4 split-brain**（编辑器写入对工单不生效）；**R-16 D6 修复排序晚于触发源** |
| **内控/合规** | Q7 月度审计报告（记文档） | **R-13 定价权限粒度**（无 dedicated 闸门）；**R-15 审计无历史表支撑**（无 diff 幅度）；**R-22 工资盲操** |
| **前端实现** | M7 cat-select 抽取；M9 两路径渲染同一组件 | **R-18 M7 措辞误述**（selectCat/filterCatOptions 已全局）；**R-19 move 返回片段 scope 未钉死**；**R-25 drawer_shell 无 z_class**；**R-26 Esc 多浮层冲突**；**R-17 bom_detail 链接扑空** |
| **业务边界** | Q1 product_code 改名断链（记技术债）；Q2 产出品校验 | **R-6 措辞与实现不符**（跨版本并集非单 BOM）；**R-28 BOM 复制前瞻约束未声明**；**R-33 取消单改价边界未定义**；**R-34 多版本共享行为未公告** |

**总结**：前轮 phase 3 评审聚焦架构层与 latent bug，D6-D11 + Q1-Q7 已妥善覆盖架构方向、wage 口径、锁定策略、填价落点、迁移门禁等核心议题。本次 6 角色评审**新发现集中在三个维度**：(1) **数据正确性盲区**（R-1 quantity / R-5 级联清理 / R-14 映射规则 / R-29 精度——迁移链的细节）；(2) **部署计划的原子性**（R-2/R-3/R-16——§9 五步拆分的硬依赖被低估）；(3) **车间落地与内控**（R-4 文案反向 / R-12 漏工序死锁 / R-13/R-15 权限与审计 / R-22 工资盲操——操作面与合规维度）。这三维前轮基本未触及，是本次评审的主要增量价值。
