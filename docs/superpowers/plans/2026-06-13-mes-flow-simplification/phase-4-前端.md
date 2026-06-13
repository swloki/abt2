# 阶段 4：前端 + 排程 + 文档

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 完成前端页面改造和排程 V1，同步 UML 设计文档。

**Architecture:** 前端基于 Maud + HTMX + UnoCSS + Surreal.js 的 SSR 模式。页面按 abt-web/CLAUDE.md 的组件化三原则实现。排程 V1 按交期倒推 + 优先级 + 工作中心分组。

**Tech Stack:** Rust (abt-web) + Maud + HTMX + UnoCSS + Surreal.js

**前置:** 阶段 3 已完成

**验收:** 完整的计划员操作流程：需求池选需求 → 设排程参数 → 生成计划并下达 → 工单 Released → 报工 → 完工入库

---

## 前提阅读

实现前必须先读：
- `abt-web/CLAUDE.md` — 组件化三原则、抗碎片化实践
- UI 原型设计 — `C:\Users\weichen\AppData\Roaming\Open Design\namespaces\release-stable-win\data\projects\63ce2980-2f4e-45a7-9b34-8050e32135c2`
- 相关 memory — `Alpine.js 表单开发模式`、`HTMX 列表页单端点模式`

---

## 文件结构

| 操作 | 文件/页面 | 职责 |
|------|----------|------|
| 新增/修改 | `abt-web/src/pages/mes/demand.rs` | 需求池：排程参数 + "生成计划并下达"按钮 |
| 新增/修改 | `abt-web/src/pages/mes/plan_detail.rs` | 计划详情："确认并下达"按钮 + warnings 展示 |
| 修改 | `abt-web/src/pages/master_data/product_detail.rs` | 产品详情：Routing 关联状态 + material_consumption_mode 切换 |
| 修改 | `abt-core/src/mes/production_plan/implt.rs` | 排程 V1（交期倒推 + 优先级 + 工作中心分组） |
| 修改 | `docs/uml-design/04-mes.html` | UML 设计文档同步 |

---

### Task 1: 需求池页面 — 排程参数 + 生成计划按钮

**Files:**
- Create/Modify: `abt-web/src/pages/mes/demand.rs`
- Create/Modify: `abt-web/src/handlers/mes/demand_handler.rs` (如有 Web handler 需要修改)

> **注意**: 此 Task 的实现细节取决于 Open Design 原型文件的具体设计。
> 以下为骨架实现，实际 UI 元素需要对齐原型。

- [ ] **Step 1: 读取原型设计文件**

在 Open Design 项目目录中找到需求池相关原型，理解：
- 排程参数输入（scheduled_start、优先级、工作中心分配）
- "生成计划并下达"按钮的位置和行为
- 选中需求的交互方式

- [ ] **Step 2: 实现需求池页面改动**

核心功能点：
1. 需求列表中每行增加 checkbox 支持多选
2. 底部增加操作栏：
   - 排程参数输入：`scheduled_start` 日期选择器
   - "生成计划"按钮 → POST `/mes/plans` 创建计划
   - "生成并下达"按钮 → POST `/mes/plans` + POST `/mes/plans/{id}/release`
3. 下达成功后展示 BatchReleaseResult（成功数/失败数/warnings）

实现遵循 `abt-web/CLAUDE.md` 的模式：
- Alpine.js 状态驱动 + Hidden Input 桥接 HTMX
- HTMX `hx-post` + `hx-target` 局部更新
- 状态 tabs + filter-form + pagination 控件化

- [ ] **Step 3: 确认 Web handler 路由**

检查并确保以下路由存在：
- `POST /mes/plans` — 创建计划（从需求池选中的需求）
- `POST /mes/plans/:id/release` — 下达计划
- `GET /mes/demands` — 需求池列表页面

如需新增路由，在 `abt-web/src/handlers/` 中添加。

- [ ] **Step 4: 验证页面渲染**

Run: 浏览器访问需求池页面，确认 UI 渲染正确

- [ ] **Step 5: Commit**

```bash
git add abt-web/
git commit -m "feat(mes-ui): demand pool — scheduling params + create-and-release button"
```

---

### Task 2: 计划详情页 — 确认并下达 + warnings

**Files:**
- Create/Modify: `abt-web/src/pages/mes/plan_detail.rs`

- [ ] **Step 1: 读取原型设计文件**

在 Open Design 项目目录中找到计划详情相关原型。

- [ ] **Step 2: 实现计划详情页改动**

核心功能点：
1. "确认并下达"按钮 → POST `/mes/plans/{id}/release`
2. 下达结果展示：
   - 成功工单列表（doc_number、product、status）
   - 失败列表（error message）
   - Warnings 列表（无 Routing、无 BOM、物料短缺等）
3. 预校验结果（`pre_validate` 返回的 `ReleaseValidation`）展示

实现模式：
- 下达前可先调用 `GET /mes/plans/{id}/validate` 获取预校验结果
- 展示 warnings 作为"继续下达？"的确认条件
- 确认后 `hx-post` 下达

- [ ] **Step 3: 添加 Web handler**

确保以下路由和 handler 存在：
- `GET /mes/plans/:id` — 计划详情页面
- `GET /mes/plans/:id/validate` — 预校验 API
- `POST /mes/plans/:id/release` — 下达

- [ ] **Step 4: 验证页面渲染和交互**

- [ ] **Step 5: Commit**

```bash
git add abt-web/
git commit -m "feat(mes-ui): plan detail — confirm-and-release button + validation warnings"
```

---

### Task 3: 产品详情页 — Routing 关联 + material_consumption_mode

**Files:**
- Modify: `abt-web/src/pages/master_data/product_detail.rs`

- [ ] **Step 1: 读取原型设计文件**

- [ ] **Step 2: 实现产品详情页改动**

核心功能点：
1. **Routing 关联状态展示**：
   - 显示产品是否已关联 Routing
   - 如已关联，显示 Routing 名称和步骤数
   - 可选：提供"关联 Routing"操作（调用 `RoutingService.set_bom_routing`）

2. **material_consumption_mode 切换**：
   - 下拉选择：`backflush` / `picking`
   - 默认 `backflush`
   - 切换后保存到 `products.meta` JSONB

实现方式：
- 通过 `PATCH /master-data/products/:id` 更新 meta 字段
- Alpine.js `x-model` 绑定下拉值
- HTMX `hx-patch` 提交

- [ ] **Step 3: 验证页面渲染**

- [ ] **Step 4: Commit**

```bash
git add abt-web/
git commit -m "feat(product-ui): show routing status + material_consumption_mode toggle"
```

---

### Task 4: 排程 V1 — 交期倒推 + 优先级 + 工作中心分组

**Files:**
- Modify: `abt-core/src/mes/production_plan/implt.rs`
- Modify: `abt-core/src/mes/production_plan/model.rs` (如有新字段)

- [ ] **Step 1: 实现排程逻辑**

在 `ProductionPlanServiceImpl` 中添加排程方法：

```rust
    /// 排程 V1：按需求交期倒推排程日期
    /// - 按优先级排序
    /// - 按工作中心分组
    /// - scheduled_start < today() → 标记紧急
    pub async fn schedule_v1(
        &self,
        ctx: &ServiceContext,
        db: PgExecutor<'_>,
        plan_id: i64,
    ) -> Result<()> {
        let mut items = ProductionPlanRepo::get_items_by_plan_id(&mut *db, plan_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

        let today = chrono::Local::now().date_naive();

        // 按优先级排序（priority 越小越优先，紧急=0）
        // 交期早的排在前面
        items.sort_by(|a, b| {
            a.priority.cmp(&b.priority)
                .then_with(|| a.scheduled_end.cmp(&b.scheduled_end))
        });

        // 按工作中心分组（已有 work_center_id 的保持，无的分配默认）
        // TODO: V2 基于产能日历的分配
        for item in &items {
            // 标记紧急
            if item.scheduled_start < today {
                // 紧急项：priority 设为最高（0）
                ProductionPlanRepo::update_item_priority(
                    &mut *db, item.id, 0,
                ).await?;
            }
        }

        Ok(())
    }
```

需要在 repo 添加：

```rust
    pub async fn update_item_priority(
        executor: &mut sqlx::postgres::PgConnection,
        item_id: i64,
        priority: i32,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE production_plan_items SET priority = $2 WHERE id = $1",
        )
        .bind(item_id)
        .bind(priority)
        .execute(&mut *executor)
        .await?;
        Ok(())
    }
```

- [ ] **Step 2: 验证编译**

Run: `cargo clippy -p abt-core 2>&1 | head -30`
Expected: 无 error

- [ ] **Step 3: Commit**

```bash
git add abt-core/src/mes/production_plan/
git commit -m "feat(plan): scheduling V1 — deadline-based backward scheduling + priority + work center grouping"
```

---

### Task 5: 更新 UML 设计文档

**Files:**
- Modify: `docs/uml-design/04-mes.html`

- [ ] **Step 1: 同步设计文档**

根据阶段 1-4 的代码变更，更新 `docs/uml-design/04-mes.html`：
1. 更新 `WorkOrderService` 接口：新增 `unrelease()` 方法
2. 更新 `ProductionPlanService` 接口：新增 `pre_validate()` 方法
3. 更新 `release()` 操作序列图：增加 BOM 快照、Routing 工序步骤
4. 更新状态流转图：增加 Draft ↔ Released 双向转换
5. 更新数据模型：`ProductMeta` 新增字段、`ReleaseValidation` 新模型
6. 更新 `BackflushService.execute()` 的仓库策略说明

- [ ] **Step 2: Commit**

```bash
git add docs/uml-design/04-mes.html
git commit -m "docs: update MES UML design doc — reflect phase 1-4 changes"
```

---

### Task 6: 验证阶段 4

- [ ] **Step 1: 全量 clippy**

Run: `cargo clippy 2>&1 | tail -20`
Expected: 无 error

- [ ] **Step 2: 运行测试**

Run: `cargo test 2>&1 | tail -30`
Expected: 所有测试通过

- [ ] **Step 3: 浏览器端到端验证**

完整流程：
```
需求池选需求 → 设排程参数 → 生成计划并下达 → 工单 Released → 报工 → 完工入库
```

- [ ] **Step 4: 最终 commit**

```bash
git add -A
git commit -m "feat(mes): phase 4 complete — frontend pages + scheduling V1 + UML docs"
```
