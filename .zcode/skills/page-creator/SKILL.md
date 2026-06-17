---
name: page-creator
description: >
  ABT 项目页面创建技能 — 将原型设计（Open Design / HTML 原型）逐步转化为 Maud + HTMX + UnoCSS + Surreal.js 的 SSR 页面。
  当用户提到"实现页面"、"做页面"、"转化原型"、"创建页面"、"page creator"、"实现这个原型"、"把这个原型做出来"、
  "新建页面"、"加一个页面"、"写页面"、"原型转代码" 时触发。也在以下场景触发：用户提供了原型设计文件或 URL，
  要求按原型实现功能页面；用户说要按设计稿实现前端；用户提到某个模块需要新增页面；用户要求实现一组页面的 UI。
  即使用户只是说"帮我把这个做出来"并提供了原型参考，也应触发。
allowed-tools: Bash(agent-browser:*), Bash(npx agent-browser:*), Bash(psql:*), Bash(bun:*), Bash(cargo:*), Bash(npm:*)
---

# ABT 页面创建 — 原型到代码的渐进式转化

将原型设计逐步转化为符合项目规范的 SSR 页面。核心原则：**一次一个 section，每完成一个就测试验证，确保与原型对齐**。

## 你必须先读的文件

在开始任何页面创建之前，必须先读取以下文件获取项目规范：

1. **`AGENTS.md`** — 项目架构、模块结构、代码规范（必读）
2. **`abt-web/CLAUDE.md`** — 组件三原则、HTMX/Surreal.js 边界、CSS 类名速查表（必读）
3. **`abt-web/src/pages/PROTOTYPE_CONTEXT.md`** — 原型对齐的共享上下文（必读）
4. **`docs/uml-design/README.md`** — 设计文档索引，了解 Service trait 接口

## 工作流程

整个页面创建分为 **四个阶段**，严格按顺序执行：

```
收集页面 → 逐页规划 → 逐 section 转化+测试 → 收尾注册
```

### 阶段一：收集页面清单

1. **确认原型来源** — 询问用户原型在哪里：
   - Open Design 本地项目路径
   - 在线 URL
   - 本地 HTML 文件
   - 截图或文字描述

2. **获取全部待实现页面** — 用 `read` 读取原型目录或文件，列出所有需要实现的页面。输出一个清单让用户确认：

```
## 待实现页面清单

| # | 页面名称 | 原型路径 | 目标路由 | 页面类型 |
|---|---------|---------|---------|---------|
| 1 | 生产计划列表 | /mes/plans | /admin/mes/plans | 列表页 |
| 2 | 生产计划详情 | /mes/plans/detail | /admin/mes/plans/{id} | 详情页 |
| 3 | 新建生产计划 | /mes/plans/create | /admin/mes/plans/create | 创建页 |
```

   页面类型分为：列表页(list)、详情页(detail)、创建页(create)、编辑页(edit)、仪表盘(dashboard)。

3. **确认实现顺序** — 一般按 列表页 → 详情页 → 创建/编辑页 的顺序。与用户确认后进入下一阶段。

### 阶段二：逐页规划

对每个页面，在写代码之前先做规划：

#### 1. 分析原型结构

读取原型 HTML/截图，拆解页面的各个 section。一个典型的列表页拆解如下：

```
## 页面规划：生产计划列表 (/admin/mes/plans)

### Section 拆解
1. page-header — 标题 + 新建按钮
2. filter-bar — 状态筛选 + 搜索框
3. data-table — 数据表格（表头 + 数据行 + 空状态）
4. pagination — 分页组件

### 数据依赖
- Service: `state.production_plan_service()`
- 列表方法: `list(ctx, db, filter, page, page_size)`
- 需要 Filter 结构体和 ListItem 模型

### 路由规划
- `PlanListPath` — GET 列表页
- `PlanTablePath` — GET 表格局部刷新
- `PlanDetailPath` — GET 详情页
- `PlanCreatePath` — GET/POST 创建页
```

#### 2. 确认 Service 接口

查看 `abt-core/src/<domain>/<module>/service.rs` 确认 Service trait 有哪些方法可用。如果需要的方法不存在，先在 `abt-core` 中添加接口，再实现页面。

**如果接口不存在**，告知用户需要先在 abt-core 中补充接口，并列出需要的方法签名。

### 阶段三：逐 Section 转化 + 测试

这是最核心的阶段。**每次只做一个 section，做完立即测试**。

#### 转化节奏

```
对于每个页面：
  对于每个 section：
    1. 写 section 代码
    2. 写对应的路由和 handler（如果还没写）
    3. cargo clippy 验证编译
    4. 用 page-test 技能测试该 section
    5. 修复问题，直到测试通过
    6. 标记 ✅，继续下一个 section
```

#### 代码文件组织

每个页面的代码分布在这几个位置：

**路由定义** — `abt-web/src/routes/<module>.rs`
```rust
// 1. 定义 TypedPath
#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/<domain>/<resource>")]
pub struct XxxListPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/<domain>/<resource>/table")]
pub struct XxxTablePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/<domain>/<resource>/create")]
pub struct XxxCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/<domain>/<resource>/{id}")]
pub struct XxxDetailPath {
    pub id: i64,
}

// 2. 注册路由
pub fn router() -> Router<AppState> {
    Router::new()
        .route(XxxListPath::PATH, get(xxx_list::get_list))
        .route(XxxTablePath::PATH, get(xxx_list::get_table))
        .route(XxxDetailPath::PATH, get(xxx_detail::get_detail))
        .route(XxxCreatePath::PATH, get(xxx_create::get_create).post(xxx_create::create))
}
```

**页面渲染** — `abt-web/src/pages/<module>_<page>.rs`

#### 各页面类型的标准结构

##### 列表页 (xxx_list.rs)

```rust
use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::<domain>::<module>::{XxxService, XxxListFilter, XxxListItem};
use abt_core::shared::types::PaginatedResult;

use crate::components::icon;
use crate::components::pagination::pagination;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::<route_module>::{XxxListPath, XxxTablePath, XxxCreatePath};
use crate::utils::{empty_as_none, RequestContext};
use abt_macros::require_permission;

// 状态标签辅助函数 — 返回 (文字, 背景色, 文字色)
fn status_label(s: &XxxStatus) -> (&'static str, &'static str, &'static str) {
    match s {
        XxxStatus::Draft => ("草稿", "rgba(0,0,0,0.04)", "var(--muted)"),
        XxxStatus::Active => ("进行中", "rgba(22,93,255,0.08)", "var(--primary)"),
    }
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct QueryParams {
    pub keyword: Option<String>,
    pub status: Option<i16>,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub page: Option<u32>,
}

#[require_permission("RESOURCE", "read")]
pub async fn get_list(
    _path: XxxListPath, ctx: RequestContext, Query(params): Query<QueryParams>,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.xxx_service();
    let page = params.page.unwrap_or(1);
    let filter = XxxListFilter { keyword: params.keyword.clone(), status: params.status };
    let result = svc.list(&service_ctx, &mut conn, filter, page, 20).await?;
    let content = list_page(&result, &params);
    Ok(Html(admin_page(is_htmx, "页面标题", &claims, "module_id", XxxListPath::PATH, "模块名", None, content).into_string()))
}

// HTMX 局部刷新 handler — 只返回表格区域
#[require_permission("RESOURCE", "read")]
pub async fn get_table(
    _path: XxxTablePath, ctx: RequestContext, Query(params): Query<QueryParams>,
) -> Result<Html<String>> {
    let RequestContext { mut conn, state, service_ctx, .. } = ctx;
    let svc = state.xxx_service();
    let page = params.page.unwrap_or(1);
    let filter = XxxListFilter { keyword: params.keyword.clone(), status: params.status };
    let result = svc.list(&service_ctx, &mut conn, filter, page, 20).await?;
    Ok(Html(table_fragment(&result, &params).into_string()))
}
```

**Maud 渲染函数的层次**：
- `list_page()` — 完整页面壳（page-header + table_fragment）
- `table_fragment()` — 可被 HTMX 替换的表格区域（filter-bar + data_card）
- `data_card()` — 数据卡片（表格 + 分页）

这种分层让 HTMX 可以只刷新数据区域而不重载整个页面。

##### 详情页 (xxx_detail.rs)

详情页用 `info-card` + `info-grid` 展示信息：

```rust
let content = html! { div {
    div class="page-header" {
        div class="page-header-left" {
            a class="back-link" href=(XxxListPath::PATH) { "← 返回列表" }
            h1 class="page-title" { "单号 " (item.doc_number) }
        }
        div class="page-actions" {
            // 操作按钮
        }
    }
    div class="info-card" {
        div class="info-grid" {
            div class="info-item" { label { "字段名" } span { (value) } }
            div class="info-item" { label { "字段名" } span class="mono" { (value) } }
            div class="info-item span-2" { label { "备注" } span { (remark) } }
        }
    }
}};
```

##### 创建页 (xxx_create.rs)

创建页用 `form-section` + `form-grid` 布局，底部 `create-action-bar`：

```rust
let content = html! { div {
    div class="page-header" {
        div class="page-header-left" {
            a class="back-link" href=(XxxListPath::PATH) { "← 返回列表" }
            h1 class="page-title" { "新建XXX" }
        }
    }
    form hx-post=(XxxCreatePath::PATH) hx-swap="none" {
        div class="form-section" {
            div class="form-section-title" { "基本信息" }
            div class="form-grid" {
                div class="form-field" {
                    label class="form-label" { "名称" }
                    input class="form-input" type="text" name="name" required;
                }
                div class="form-field" {
                    label class="form-label" { "类型" }
                    select class="form-select" name="type" {
                        option value="1" { "类型A" }
                    }
                }
            }
        }
        div class="create-action-bar" {
            a class="btn btn-default" href=(XxxListPath::PATH) { "取消" }
            button type="submit" class="btn btn-primary" { "提交" }
        }
    }
}};
```

#### admin_page 参数对照

```rust
admin_page(
    is_htmx,                        // ctx.is_htmx()
    "页面标题",                      // 浏览器 tab 标题
    &claims,                         // 用户信息
    "module_id",                     // sidebar 高亮模块，对应 sidebar.rs 中 NavModule.id
    "/admin/xxx/current-path",       // 当前页面路径，sidebar 高亮菜单项
    "模块名称",                      // header 面包屑模块名
    Some("/admin/xxx/parent-path"),  // 面包屑父级路径（列表页为 None）
    content,                         // Maud html! 内容
)
```

sidebar 模块 id 对照表：
| module_id | 模块 |
|-----------|------|
| `sales` | 销售管理 |
| `purchase` | 采购管理 |
| `inventory` | 仓储管理 |
| `production` | 生产管理 |
| `master` | 主数据 |
| `system` | 系统管理 |

### 阶段四：注册与收尾

所有页面代码完成后，需要注册到系统中：

#### 1. 注册页面模块

在 `abt-web/src/pages/mod.rs` 添加：
```rust
pub mod xxx_list;
pub mod xxx_detail;
pub mod xxx_create;
```

#### 2. 注册路由模块

在 `abt-web/src/routes/mod.rs` 添加模块声明和路由合并：
```rust
pub mod xxx;

// 在 router() 函数中对应区块添加：
.merge(xxx::router())
```

#### 3. 确认 Service 工厂方法

检查 `abt-web/src/state.rs` 是否已有对应的 service 工厂方法。如果没有，需要添加：
```rust
pub fn xxx_service(&self) -> impl XxxService {
    abt_core::<domain>::<module>::new_xxx_service(self.pool.clone())
}
```

#### 4. 编译验证

```bash
cargo clippy
```

修复所有编译错误和 clippy 警告。

## 测试验证策略

每完成一个 section 的代码后，使用 `page-test` 技能进行验证：

### 快速验证流程

```bash
# 1. 登录（如果还没登录）
agent-browser --session-name abt open http://localhost:3000/login
agent-browser snapshot -i
agent-browser fill @e<username_input> "admin"
agent-browser fill @e<password_input> "123456"
agent-browser click @e<login_button>
agent-browser wait 2000

# 2. 打开目标页面
agent-browser open http://localhost:3000/admin/xxx
agent-browser snapshot -i

# 3. 检查无 JS 错误
agent-browser errors --clear
# 操作后
agent-browser errors
```

### 对标原型验证

对于每个 section，对比验证要点：

| Section | 验证项 |
|---------|--------|
| page-header | 标题文字、按钮位置、返回链接 |
| filter-bar | 筛选项完整、搜索框、HTMX 触发正确 |
| data-table | 表头列名、数据行展示、空状态文案、状态标签颜色 |
| pagination | 分页显示、翻页功能 |
| info-card | 字段完整、关联名称显示（非ID）、排版对齐 |
| form | 字段完整、表单验证、提交后跳转 |
| modal | 弹出/关闭正常、表单提交正常 |

### 编译检查

每写完一个 section 的代码后立即运行：
```bash
cargo clippy
```

不通过则立刻修复，不要攒到最后。

## 代码规范要点

### 禁止事项

- ❌ 硬编码 URL 字符串 — 必须用 TypedPath
- ❌ `hx-target="#hardcoded-id"` — 用 `hx-target="this"` 或 `closest <selector>`
- ❌ `fetch()` 提交表单 — 用 HTMX `hx-post`
- ❌ `onclick="customFunction()"` 做 UI — 用 Surreal.js 内联
- ❌ Maud 中 `script { "..." }` — 会被转义，用 `maud::PreEscaped()`
- ❌ 内联 `style` 属性（`<col>` 除外）— 用 CSS 类
- ❌ 直接写 SQL — 通过 Service trait
- ❌ 为局部刷新创建额外 handler — 一个 URL 一个 handler
- ❌ `sqlx::query` 在 abt-web 中 — 禁止直接访问数据库

### CSS 类名速查

完整列表见 `abt-web/CLAUDE.md`，常用：

| 类名 | 用途 |
|------|------|
| `page-header` | 页面头部（标题 + 操作按钮） |
| `page-title` | 页面标题 |
| `back-link` | 返回链接 |
| `data-card` | 数据卡片容器 |
| `data-table` | 数据表格 |
| `data-card-scroll` | 表格溢出滚动容器 |
| `filter-bar` | 筛选栏 |
| `filter-select` | 筛选下拉框 |
| `search-wrap` + `search-input` | 搜索输入框 |
| `info-card` + `info-grid` + `info-item` | 详情信息展示 |
| `form-section` + `form-grid` + `form-field` | 表单布局 |
| `form-input` / `form-select` | 输入框/下拉框 |
| `create-action-bar` | 创建页底部操作栏 |
| `btn-primary` / `btn-default` / `btn-danger` | 按钮 |
| `status-pill` | 状态标签 |
| `pagination` | 分页 |
| `modal-overlay` + `modal` | 模态框 |
| `mono` | 等宽字体（编号/金额） |
| `num-right` | 数字右对齐 |

### HTMX 交互模式

**列表页搜索/筛选**：filter-bar 中的表单用 `hx-get` 指向 table path，`hx-target` 指向 data-card 的 id：

```rust
form class="filter-bar" hx-get=(XxxTablePath::PATH)
    hx-trigger="change, keyup changed delay:300ms from:.search-input"
    hx-target="#xxx-data-card" hx-select="#xxx-data-card"
    hx-swap="outerHTML" hx-include="closest form" {
    // 筛选控件
}
```

**状态操作**（确认、取消等）：用 `form hx-post` + `hx-swap="none"`，服务端返回 `HX-Redirect`：

```rust
form hx-post=(format!("/admin/xxx/{}/confirm", item.id)) hx-swap="none" style="display:inline" {
    button class="btn btn-primary" type="submit" { "确认" }
}
```

**Modal 弹窗**：用 Surreal.js 控制显隐，HTMX 加载内容：

```rust
// 触发按钮
button type="button" hx-get=(edit_url) hx-target="#edit-modal" hx-swap="innerHTML" {
    "编辑"
}

// Modal 容器（script 必须在 modal 外部）
div id="edit-modal" class="modal-overlay" { }
(maud::PreEscaped(r#"<script>
    me('#edit-modal')
        .on('htmx:afterSettle',function(){me(this).classAdd('is-open')})
        .on('click',function(ev){if(ev.target===me('#edit-modal'))me('#edit-modal').classRemove('is-open')});
</script>"#))
```

### Surreal.js 内联模式

所有纯前端 UI（modal 开关、dropdown、tab 切换）用 Surreal.js：

```rust
(maud::PreEscaped(r#"<script>me().on('click',function(){me('#target').classAdd('is-open')})</script>"#))
```

### 数据展示规范

- 数值字段：用 `crate::utils::fmt_qty()` 去除多余小数位
- 关联 ID → 名称：Service 应提供 lookup 方法或在列表查询中 JOIN 返回名称
- 空值显示 `"—"`
- 状态标签：用辅助函数返回 `(文字, 背景色, 文字色)` 三元组
- 金额/编号用 `class="mono"`
- 数字列用 `class="num-right"`

## 进度跟踪

使用 todo 列表跟踪转化进度：

```
## 转化进度

### 页面 1: 生产计划列表
- [x] Section 1: page-header — ✅ 已测试通过
- [x] Section 2: filter-bar — ✅ 已测试通过
- [ ] Section 3: data-table — 🔄 进行中
- [ ] Section 4: pagination

### 页面 2: 生产计划详情
- [ ] Section 1: page-header
- [ ] Section 2: info-card
...
```

每完成一个 section，更新 todo 并标记状态。
