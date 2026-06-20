# 委外单详情页重新设计（还原原型）

**日期**: 2026-06-20
**范围**: `abt-web/src/pages/om_outsourcing_detail.rs` + `abt-core/src/om/outsourcing_order/`（service 补接口）
**权威参考**: Open Design 原型 `05-outsourcing-detail.html`（项目 `63ce2980`）

## 1. 背景与目标

委外单详情页 `/admin/om/outsourcing/{id}` 当前实现只还原了原型的一部分（Hero 卡片 + 追踪时间线），**缺失两个核心业务区块**：发料明细表、收发记录表，且视觉精致度不足（无动画/渐变）。

用户反馈"设计太简单"。本设计目标：**完整还原原型 `05-outsourcing-detail.html` 的 5 个区块**，用 UnoCSS 原子类实现，业务数据通过 abt-core Service trait 查询填充。

## 2. 现状 vs 原型

| 原型区块 | 当前实现 | 差距 |
|---------|---------|------|
| ① Hero 卡片（单号/状态/按钮/6字段/进度环） | 有 | 视觉简化；缺发料源仓库字段；缺 shimmer/渐变环 |
| ② 追踪时间线（7节点快递式） | 有 | 当前节点高亮样式与原型有差距 |
| ③ 发料明细表 + 金额汇总栏 | **无** | 完全缺失（代码注释 "materials not loaded"） |
| ④ 收发记录表 | **无** | 完全缺失 |
| ⑤ 5 个 Modal | 有 | 视觉基本对齐 |

## 3. 设计：页面区块结构（自上而下）

### ① Hero 卡片（升级）
- **shimmer 流动彩条**：顶部 4px，`linear-gradient(90deg, accent, #60a5fa, accent)` + `background-size:200%` + shimmer 动画（6s 循环）
- **大单号**：44×44 圆角图标 + 24px 单号
- **meta 行**：状态 pill + 类型 tag + 版本号
- **操作按钮**：按 `OutsourcingStatus` 门控（已实现：Draft 发料/记录节点/转自制/取消；Sent 收货登记/转自制等）
- **key grid（3列）**：供应商 / 产品 / 关联工单（工单号）/ 关联工序 / 虚拟仓库 / **发料源仓库（新增显示）** / 预计交期
- **detail row**：计划数量 / 完成数量 / 单价 / 总金额 / 创建人 / 创建时间 / 更新时间
- **渐变进度环**：SVG `<linearGradient>`（accent→#60a5fa），完成进度 = completed_qty / planned_qty

### ② 追踪时间线（视觉对齐）
- 7 节点（SendMaterial→Warehoused），左侧轨迹线渐变（success→accent→border）
- 每节点：圆点(completed/active/pending 三态) + 标签 + 时间 + 备注 + 状态 pill
- **当前节点**：accent 渐变背景 + "当前" 小标签
- 顶部彩条 `linear-gradient(90deg, success, accent, #60a5fa)`

### ③ 发料明细表（新增）
表头：物料 | 应发数量 | 已发数量 | 已收回数量 | 在途数量 | 单位成本 | 小计
- 物料列：双行（名称 + 编码）
- 在途数量 = sent_qty − returned_qty，用 warn 色高亮（>0 时）
- 底部金额汇总栏：
  - 在途物料金额 = Σ(sent_qty − returned_qty) × unit_cost
  - 加工费 = planned_qty × unit_price

### ④ 收发记录表（新增）
表头：时间 | 类型 | 物料 | 数量 | 来源→去向 | 操作人
- 类型：发料 / 部分收货 / 收货（status pill 区分颜色）
- 来源→去向：如 "原材料仓 → 委外虚拟仓"
- 数据来源：WMS `inventory_transactions` 按 `source_doc_number` 过滤

### ⑤ Modal（5 个，视觉对齐原型，已有）

## 4. 数据来源

| 区块 | 数据源 | 接口 |
|------|--------|------|
| Hero 字段 | `OutsourcingOrder` + 关联名（供应商/产品/工单号/仓库） | 已有（`get_detail` 已加载） |
| 发料源仓名/虚拟仓名 | warehouses | 已有 |
| 发料明细 | `outsourcing_materials` | **新增** `OutsourcingOrderService::list_materials(id)` |
| 收发记录 | WMS `inventory_transactions` | WMS `InventoryTransactionService` 按 `source_doc_number` 查（确认 send/receive 写入流水时 `source_doc_number` = 委外单号 `OO-...`；若实际是调拨单号，则通过调拨单关联反查） |
| 金额 | 由 materials + order 计算 | handler 内计算 |

## 5. abt-core 改动

1. **`OutsourcingOrderService` trait 加方法**：
   ```rust
   async fn list_materials(&self, ctx, db, outsourcing_id: i64) -> Result<Vec<OutsourcingMaterial>>;
   ```
   实现委托 `OutsourcingMaterialRepo::list_by_outsourcing`（repo 已存在）。
2. **收发记录**：不新增 service 方法，复用 WMS `InventoryTransactionService` 的列表/过滤接口（按 `source_doc_number`）。若 WMS service 未暴露按 source 查询，在 WMS service 补一个 `list_by_source_doc(source_doc_number)` 方法（实现期间确认）。
3. **同步设计文档** `docs/uml-design/05-outsourcing.html`：`OutsourcingOrderService` 接口需同步新增 `list_materials` 方法签名（接口变更，按 CLAUDE.md 双向同步原则）。UI 区块（发料明细表/收发记录表）属于页面布局，不在 UML 类图/状态机范围。

## 6. abt-web 改动

1. **`get_detail` handler**：除现有数据外，多查：
   - `materials = svc.list_materials(id)`
   - `transactions = wms_svc.list_by_source_doc(order.doc_number)`（或等价过滤）
   - 计算金额（在途金额 / 加工费）
   - 解析发料源仓库名（`source_warehouse_id` → warehouse name）
2. **`detail_page` 签名扩展**：新增参数 `materials`, `transactions`, `source_warehouse_name`, 金额。
3. **新增组件函数**：
   - `materials_section(materials, in_transit_amount, processing_fee)` → 发料明细表 + 金额栏
   - `transactions_section(transactions)` → 收发记录表
4. **Hero key grid** 补"发料源仓库"字段。
5. **视觉升级**（全部 UnoCSS 原子类，禁止 inline style）：
   - shimmer 动画：`uno.config.ts` 的 `theme.animation` 加 `shimmer-bar` keyframes；Hero accent 条用 `animate-shimmer-bar`
   - 渐变进度环：SVG `<defs><linearGradient>` + `stroke="url(#ringGrad)"`
   - 卡片顶部彩条：`before:` 伪元素 + `bg-[linear-gradient(...)]`
   - 卡片阴影/圆角对齐原型（`shadow-card` / `rounded-xl`）

## 7. 视觉实现约束

- **100% UnoCSS 原子类**，禁止 Maud 模板内 `style=""`（abt-web CLAUDE.md 约束）
- shimmer/新动画在 `uno.config.ts` 的 `theme.animation.keyframes` 定义，`preflights` 不动
- 颜色用项目 token（`accent`/`success`/`warn`/`danger`/`muted`/`border-soft` 等），禁止硬编码 hex（`#60a5fa` 作为 accent 渐变辅色可接受，需在原型一致处使用）

## 8. 验收标准

1. 详情页渲染 5 个区块（Hero / 时间线 / 发料明细表 / 收发记录表 / Modal），布局与原型 `05-outsourcing-detail.html` 一致
2. 发料明细表显示真实数据（物料名+码、应发/已发/已收回/在途/成本/小计），底部金额栏正确
3. 收发记录表显示该委外单的 WMS 库存流水（发料/收货记录）
4. Hero 显示发料源仓库字段
5. 视觉：shimmer 动画条流动、进度环渐变、卡片彩条均生效
6. `cargo clippy` 通过，无新增 error
7. 现有功能不回归（状态门控按钮、modal、发料/收货/转自制/取消操作正常）

## 9. 非目标（YAGNI）

- 不做成本分析图表/仪表盘（加工费/在途金额仅以数字汇总栏呈现，不做可视化图表）
- 不做供应商信息卡/关联单据链接列表（超出原型范围）
- 不改列表页/创建页/总览页
- 不新增收发记录独立表（复用 WMS 流水）
- 不改 Modal 的交互逻辑（仅视觉对齐）
