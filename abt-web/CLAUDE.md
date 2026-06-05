# abt-web

Rust 全栈前端，Axum + Maud + HTMX + Surreal.js + UnoCSS，直接调用 `abt-core` Service trait。

## Commands

```bash
cargo run                     # 启动服务器 (port 3000)
cargo check                   # 快速编译检查
cargo clippy                  # Lint 检查
```

## Architecture

### CSS 管理规则

**所有样式通过 UnoCSS 统一管理，禁止新增独立 CSS 文件。**

- `static/app.css` 是 UnoCSS 生成的唯一样式输出文件，由 `npm run build:css` 或 `npm run watch` 生成
- `uno.config.ts` 通过 `preflights` 注入基础 CSS（CSS 变量、重置、布局、组件样式）
- 新增组件样式优先使用 UnoCSS `shortcuts`（工具类组合）；复杂选择器、伪元素、媒体查询等无法用 shortcuts 表达的样式放入 `preflights`
- HTML 模板只引用 `/app.css`，不引用其他 CSS 文件
- 禁止在 `static/` 下新建 CSS 文件

### 组件样式类名速查 (uno.config.ts)

| 类名 | 用途 | 说明 |
|------|------|------|
| `data-card` | 数据卡片容器 | 白色圆角卡片，带阴影 |
| `data-table` | 数据表格 | 全宽表格，标准间距 |
| `data-card-scroll` | 表格溢出滚动 | 包裹 data-table 的 div |
| `form-section` | 表单分区 | 带 margin-bottom 的容器 |
| `form-section-title` | 表单分区内标题 | 灰色下边框小标题 |
| `form-grid` | 表单双列网格 | `grid-template-columns: 1fr 1fr` |
| `form-field` | 表单字段组 | 包裹 label + input/select/textarea |
| `form-field.span-2` | 跨两列字段 | `.span-2 { grid-column: 1 / -1 }` |
| `form-field.field-full` | 跨两列字段（别名） | 同 span-2 |
| `form-input` | 输入框工具类 | UnoCSS shortcut，标准边框/圆角/聚焦 |
| `form-select` | 下拉选择工具类 | UnoCSS shortcut，`appearance-none` |
| `filter-bar` | 筛选栏 | flex 布局，带 gap |
| `filter-select` | 筛选下拉框 | 自定义箭头，聚焦高亮 |
| `search-wrap` + `search-input` | 搜索输入框 | 左侧放大镜图标 |
| `supplier-info-bar` | 供应商信息条 | 联系人/电话/地址/合作年限展示 |
| `status-tabs` | 状态 Tab 栏 | 采购列表页 Tab 筛选 |
| `status-pill` | 状态标签 | 详情页状态标记 |
| `page-header` | 页面头部 | 标题 + 操作按钮区域 |
| `page-title` | 页面标题 | 大号加粗 |
| `back-link` | 返回链接 | 左箭头 + 文字 |
| `create-action-bar` | 创建页底部操作栏 | sticky 底部，取消/提交按钮 |
| `add-row-bar` | 添加行按钮栏 | 表格底部添加行按钮 |
| `btn-add-row` | 添加行按钮 | 虚线边框样式 |
| `btn-remove-row` | 删除行按钮 | 红色叉号 |
| `modal-overlay` | 模态框遮罩 | 半透明黑色背景 |
| `modal` / `modal-lg` | 模态框 | 白色居中弹窗 |
| `modal-head` / `modal-body` / `modal-foot` | 模态框区域 | 头部/内容/底部 |
| `product-search-*` | 产品搜索组件 | 搜索栏 + 结果列表 |
| `product-select-*` | 产品选择列表 | 产品名称/编码/规格 |
| `line-num` | 行号 | 序号列 |
| `line-subtotal` | 行小计 | 自动计算的金额 |
| `num-right` | 数字右对齐 | 表格数字列 |
| `mono` | 等宽字体 | 编号/金额 |
| `info-card` | 详情信息卡片 | 白色卡片，网格布局 |
| `info-grid` | 详情信息网格 | 多列 grid |
| `info-item` | 详情信息项 | label + value |
| `amount-summary` | 金额汇总区 | 底部金额统计 |
| `amount-row` | 金额汇总行 | label + value |
| `workflow-steps` | 工作流步骤条 | 流程进度展示 |
| `wf-step` | 工作流单步 | 步骤圆点 + 文字 |
| `stat-card` | 统计卡片 | Dashboard 数字统计 |
| `pagination` | 分页组件 | 页码导航 |
- **禁止在 Maud 模板中使用 `style` 属性内联样式。** 所有样式必须提取到 `uno.config.ts` 中作为 CSS 类，Maud 模板只使用 `class` 引用。这样做的好处：（1）样式可复用，避免重复定义；（2）统一管理，修改样式只需改一处；（3）减少 HTML 体积。唯一例外是 `<col>` 等 HTML 元素上必须用 style 的极少数场景。

### SSR + HTMX

- **Axum Handler** 直接调用 `abt-core` Service trait，无 gRPC 中间层
- **Maud** 编译期 HTML 宏渲染完整页面或局部片段
- **HTMX 2.0.10** 处理表单提交、分页、搜索等需要服务器状态的交互
- **Surreal.js** 处理纯前端 UI 状态，通过内联 `<script>me().on(...)</script>` 在 HTML 元素内部闭环
- **HTMX 2.x 事件模型**（关键）：
  - `htmx:afterRequest` → 触发在 **trigger 元素**（发起请求的元素）上
  - `htmx:afterSettle` → 触发在 **target 元素**（swap 目标）上
  - 用 surreal.js 绑定时注意区别：`me().on('htmx:afterRequest', ...)` 绑在 trigger 上能收到；`me().on('htmx:afterSettle', ...)` 需绑在 target 上
- **`hx-select` 继承问题**：HTMX 2.x 会将 `hx-select` 继承给所有子元素。如果父元素有 `hx-select="#app-id"`（用于自身刷新），子元素的 HTMX 请求也会被过滤。解法：在父元素加 `hx-disinherit="hx-select"`，阻止继承

### 数据访问层（强制）

**`abt-web` 禁止直接访问数据库。所有数据操作必须通过 `abt-core` 的 Service trait 完成。**

- **禁止** 在 `abt-web` 中使用 `sqlx::query`、`sqlx::query_as`、`sqlx::query_scalar` 等直接执行 SQL
- **禁止** 在 `abt-web` 中直接操作 `PgPool` / `PgConnection` 执行查询（仅用于传递给 Service trait）
- **必须** 通过 `AppState` 持有的 Service 实例（如 `state.customer_service()`、`state.shipping_service()`）调用业务方法
- **必须** 遵循 `abt-core` 的 Service trait 接口签名，包括 `ServiceContext` 参数
- 如果 `abt-core` 缺少所需接口，应先在 `abt-core` 中补充 Service 方法，再在 `abt-web` 中调用
- 修改或新增 `abt-core` Service 接口时，**必须同步更新 `docs/uml-design/` 下的相应设计文档**

**原因**：直接写 SQL 会导致列名/表结构与 `abt-core` 模型定义不一致（如 `id` vs `customer_id`、`name` vs `customer_name`），绕过业务逻辑校验，造成数据不一致。

### 项目结构

```
src/
├── main.rs              # 入口：加载配置、初始化 PgPool、启动 Axum
├── config.rs            # 环境变量（WEB_PORT, DATABASE_URL, JWT_SECRET）
├── state.rs             # AppState：PgPool + 共享服务实例
├── errors.rs            # DomainError → HTTP 响应
├── auth/
│   ├── middleware.rs     # JWT 验证中间件
│   └── session.rs       # Session 类型
├── layout/
│   ├── base.rs          # HTML 壳（HTMX + Surreal.js + UnoCSS）
│   ├── admin.rs         # Admin 布局（sidebar + header + content）
│   ├── sidebar.rs       # 侧边栏
│   └── header.rs        # 顶部栏
├── components/          # 共享 UI 组件
└── pages/               # 页面模块（高内聚：TypedPath + Maud + Handler 同文件）
```

### 环境变量

- `DATABASE_URL`（必须，PostgreSQL 连接串）
- `JWT_SECRET`（必须）
- `WEB_PORT`（默认 3000）
- `WEB_HOST`（默认 0.0.0.0）
- `MAX_CONNECTION`（默认 20）

---

## 组件化实现范式 (Component Patterns in Maud)

### 函数式组件 (Functional Components)

适用于无状态、仅依赖入参的轻量级 UI 单元。

```rust
use maud::{html, Markup};

pub fn user_button(name: &str, is_admin: bool) -> Markup {
    html! {
        button class="btn" {
            "用户: " (name)
            @if is_admin {
                span class="badge-admin" { " [管理员]" }
            }
        }
    }
}
```

### 结构体组件与 Render Trait

适用于带有复杂内部数据、需要统一接口的标准组件。实现 `Render` 后可直接在 `html!` 宏中以 `(struct_instance)` 形式挂载。

```rust
use maud::{html, Markup, Render};

pub struct UserCard {
    pub name: String,
    pub avatar_url: String,
}

impl Render for UserCard {
    fn render(&self) -> Markup {
        html! {
            div class="user-card" {
                img src=(self.avatar_url) alt=(self.name);
                h3 { (self.name) }
            }
        }
    }
}
```

### 插槽/容器组件 (Children Slots)

通过传递已渲染的 `Markup` 对象实现父子组件嵌套。

```rust
use maud::{html, Markup};

pub fn layout_container(title: &str, children: Markup) -> Markup {
    html! {
        html {
            head {
                title { (title) }
                script src="https://unpkg.com/htmx.org@1.9.10" {}
            }
            body {
                main class="content" { (children) }
            }
        }
    }
}
```

---

## 组件化三原则

所有交互组件必须遵循以下三条原则，实现真正独立、可移植的组件化开发：

### 1. 绝对内聚原则

使用 `hx-target="this"` + `hx-swap="outerHTML"`，让组件不依赖任何外部 ID。组件自己就是替换边界。

```rust
pub fn counter(count: i32) -> Markup {
    let increment_path = CounterPath {};
    html! {
        div class="counter" {
            span { (count) }
            button
                hx-post=(increment_path)
                hx-target="this"
                hx-swap="outerHTML" {
                "+1"
            }
        }
    }
}
```

### 2. 状态随身原则

使用 `hx-vals` 将 Rust 上下文数据序列化绑定在组件的 HTML 节点上，避免依赖全局状态或 DOM 查询。

```rust
pub fn item_row(item_id: i64, name: &str, status: &str) -> Markup {
    let toggle_path = ItemTogglePath { id: item_id };
    html! {
        tr
            hx-vals=(format!("{{\"item_id\": {}, \"current_status\": \"{}\"}}", item_id, status))
            hx-post=(toggle_path)
            hx-target="this"
            hx-swap="outerHTML" {
            td { (name) }
            td { (status) }
        }
    }
}
```

### 3. 视觉闭环原则

使用 `hx-indicator` 将 Loading（骨架屏）HTML 直接写在组件内部，交由 htmx 自动控制显隐。

```rust
pub fn search_panel(query: &str) -> Markup {
    let search_path = SearchPath {};
    html! {
        div class="search" {
            input
                type="text"
                name="q"
                value=(query)
                hx-get=(search_path)
                hx-target="this"
                hx-swap="outerHTML"
                hx-trigger="keyup changed delay:300ms"
                hx-indicator=".search .loading" {};
            div class="loading htmx-indicator" {
                "搜索中..."
            }
            // 结果区域
        }
    }
}
```

---

## 抗碎片化工程实践 (Anti-Fragmentation Strategy)

### 强类型路由 + hx-target="this" 自包含更新

**必须使用 `TypedPath`**，禁止硬编码字符串 URL。结合组件化三原则（绝对内聚），用 `hx-target="this"` 替代硬编码 ID：

- Handler **始终返回完整组件**，无需感知请求来源
- 组件自身就是替换边界，不依赖任何外部 ID
- 一个 URL、一个 Handler，**禁止为局部刷新拆出额外的 Handler**

```rust
use axum::response::IntoResponse;
use axum_extra::routing::{RouterExt, TypedPath};
use maud::{html, Markup};
use serde::{Deserialize, Serialize};

// 1. 定义强类型 URL 路径
#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/profile")]
pub struct ProfilePath {
    pub mode: String,
}

// 2. Maud 组件：hx-target="this" 替代硬编码 ID，组件完全自包含
pub fn render_component(username: &str, current_mode: &str) -> Markup {
    let edit_path = ProfilePath { mode: "edit".to_string() };
    let display_path = ProfilePath { mode: "display".to_string() };

    html! {
        div class="profile-card" {
            @if current_mode == "display" {
                p { "当前用户: " (username) }
                // hx-target="this" — 按钮自身被完整组件替换
                button
                    hx-get=(edit_path)
                    hx-target="this"
                    hx-swap="outerHTML" {
                    "编辑"
                }
            } @else {
                // hx-target="this" — 表单自身被完整组件替换
                form
                    hx-post=(display_path)
                    hx-target="this"
                    hx-swap="outerHTML" {
                    input type="text" name="username" value=(username);
                    button { "保存" }
                }
            }
        }
    }
}

// 3. Handler：无论是初始加载还是 htmx 局部刷新，统一调用完整组件
pub async fn handler(path: ProfilePath) -> impl IntoResponse {
    render_component("Lyra", &path.mode)
}

// 4. 暴露子路由组
pub fn router() -> axum::Router {
    axum::Router::new().route_with_ts(handler)
}
```

### 高内聚设计

路由定义、Handler、Maud 组件收拢在同一个领域模块内，禁止跨文件拆分。

### 约束

- 使用 `hx-target="this"` 让组件自包含，**禁止硬编码 `#id` 作为 target**
- 当 `this` 无法满足需求时（如需要替换父级元素），才使用 `closest <selector>` 等相对定位
- 禁止为"局部刷新"单独创建返回片段的 Handler

---

## 事件驱动解耦 (HX-Trigger)

当一个前端交互需要同时引发页面上多个互不隶属的局部组件刷新时，避免编写"聚合刷新路由"，采用 htmx 事件响应机制：

1. 主动组件发出 POST 请求（如 `/cart/add`）
2. 后端在响应头附带 `HX-Trigger: "cartUpdated"`，不返回大块 HTML
3. 被动组件声明 `hx-trigger="cartUpdated from:body"` 并指向各自的强类型路径

---

## 混合群岛架构边界 (Hybrid Islands Architecture)

严格界定 htmx 与纯前端交互的边界：

**使用 htmx 的场景**：交互涉及服务器状态流转、数据库读写、权限校验（表单提交、动态分页、条件搜索）。

**禁止使用 htmx 的场景**：纯前端 UI 状态切换（Dropdown 菜单展开、Modal 弹窗显隐、选项卡纯样式切换）。此类交互由 **Surreal.js** 内联 `<script>` 在前端本地闭环，严禁通过 htmx 向后端发送请求。

---

## 高级混合交互模式 (Advanced Hybrid Interactions)

### Surreal.js 内联模式（推荐）

Surreal.js 的核心用法是在 HTML 元素内部放 `<script>` 标签，`me()` 自动返回**父元素**。这是所有简单 UI 交互的标准写法：

```html
<!-- 打开 modal -->
button type="button" {
    "<script>me().on('click',function(){me('#my-modal').classAdd('is-open')})</script>"
    "打开"
}

<!-- 关闭 modal（modal overlay 自身上） -->
div id="my-modal" class="modal-overlay" {
    "<script>me().on('click',function(e){if(e.target===me())me().classRemove('is-open')})</script>"
}

<!-- 关闭按钮 -->
button {
    "<script>me().on('click',function(){me('.modal-overlay',me().parentElement).classRemove('is-open')})</script>"
    "×"
}

<!-- Tab 切换 -->
button class="tab-btn" {
    "<script>me().on('click',function(){me('.tab-btn').classRemove('active');me().classAdd('active')})</script>"
    "Tab 1"
}
```

**关键要点**：
- `me()` 在 `<script>` 内指向其**父元素**
- `me(selector)` 等同于 `document.querySelector(selector)`
- `me(selector, start)` 从 start 元素开始搜索
- `any(selector)` 返回匹配元素数组，可直接 `.forEach()`
- `me().on(event, handler)` 等同于 `addEventListener`
- `me(el).classAdd(cls)` / `classRemove(cls)` / `classToggle(cls)` 操作 class
- `me(el).attribute(name, value?)` 读写属性
- Maud 中必须用 `maud::PreEscaped("<script>...</script>")` 包裹，否则引号被 HTML 转义

### HTMX + Surreal.js 联合模式

场景：按钮用 `hx-get` 加载内容到 modal → 成功后自动打开 modal。

**关键**：`htmx:afterSettle` 只在成功 swap 后触发，且触发在 **target 元素**上（即 modal 容器）。出错不会触发。

```rust
// 按钮：只需 hx-get + hx-target + hx-swap，不需要任何 <script>
button type="button" title="编辑"
    hx-get=(format!("/admin/md/boms/{}/nodes/{}", bom_id, node_id))
    hx-target="#bom-edit-modal" hx-swap="innerHTML" {
    (icon::edit_icon("w-3.5 h-3.5"))
}

// modal 容器 + 外部 <script> 链式绑定 afterSettle 和 click
div id="bom-edit-modal" class="modal-overlay" { }
(maud::PreEscaped(r#"<script>
    me('#bom-edit-modal')
        .on('htmx:afterSettle',function(){me(this).classAdd('is-open')})
        .on('click',function(ev){if(ev.target===me('#bom-edit-modal'))me('#bom-edit-modal').classRemove('is-open')});
</script>"#))
```

**注意**：
- `<script>` 必须放在 modal 容器**外面**，HTMX swap 会替换 innerHTML 导致内部监听器丢失
- surreal.js 支持链式调用：`me(el).on(...).on(...).classAdd(...)`
- `afterSettle` 回调里用 `function(){}` （不用箭头函数），这样 `this` 指向触发事件的元素

### HTMX 表单替代 JS 函数

用 `<form hx-post>` 替代 `onclick="htmx.ajax(...)"` 等函数调用，完全不需要 JS：

```rust
form hx-post=(format!("/admin/md/boms/{}/nodes", bom_id))
    hx-swap="none"
    hx-include="[name='parent_id']" {
    input type="hidden" name="product_id" value=(product.product_id) {}
    input type="hidden" name="quantity" value="1" {}
    input type="hidden" name="unit" value=(product.unit) {}
    button type="submit" class="btn btn-sm btn-primary" { "选择" }
}
```

- `hx-include="[name='parent_id']"` 自动包含页面上 `name="parent_id"` 的 hidden input
- 服务端返回 `HX-Redirect`，页面自动跳转刷新

### 通用行项目计算器（lineItemCalc）

报价单、销售订单、采购单的行项目计算（数量×单价×折扣、合计）逻辑完全一致，统一用 `lineItemCalc(tbodyId)` 工厂函数。

```js
// app.js 中定义：
window.lineItemCalc = function(tbodyId) {
    function calcRow(row) { ... }
    function recalcTotals() { ... }
    function collectItems() { ... }
    return { calcRow, recalcTotals, collectItems };
};
```

```rust
// 模板中使用：
tr oninput="lineItemCalc('#quotation-item-tbody').calcRow(this)" { ... }
form onsubmit="lineItemCalc('#order-item-tbody').collectItems()" { ... }
```

### 独立 JS 文件（仅用于无法内联的复杂交互）

只有以下场景才需要独立 JS 文件：
- SortableJS 拖拽排序（需要初始化第三方库）
- 需要持久化状态（sessionStorage/localStorage）的复杂交互
- 不能用一两行 surreal.js 表达的逻辑

当前独立 JS 文件：

| 文件 | 用途 | 说明 |
|------|------|------|
| `static/bom-edit.js` | BOM 编辑页 | SortableJS 拖拽 + collapse/expand 状态持久化 |
| `static/app.js` | 全局工具 | `lineItemCalc` 行项目计算、`hs*` 兼容函数、分类树 |

### Surreal.js API 速查

| 用法 | 说明 |
|------|------|
| `me()` | `<script>` 的父元素 |
| `me(selector)` | `document.querySelector(selector)` |
| `me(selector, start)` | 从 start 开始搜索 |
| `any(selector)` | 返回匹配元素数组 |
| `any(selector, start)` | 从 start 开始搜索，返回数组 |
| `me().on(event, fn)` | `addEventListener` |
| `me(el).classAdd(cls)` | 添加 class |
| `me(el).classRemove(cls)` | 移除 class |
| `me(el).classToggle(cls)` | 切换 class |
| `me(el).attribute(name)` | 读取属性 |
| `me(el).attribute(name, value)` | 设置属性 |
| `me(el).remove()` | 移除元素 |
| `me(el).styles({prop:val})` | 设置样式 |

---

## 表单开发模式 (Form Development Pattern)

### 架构分工

| 层 | 职责 | 技术 |
|---|---|---|
| 纯前端 UI | modal 开关、class 切换、tab | Surreal.js `<script>me().on(...)` 内联 |
| 服务端交互 | 表单提交、搜索、分页 | HTMX `hx-post`/`hx-get` |
| 复杂前端状态 | 拖拽排序、状态持久化 | 独立 JS 文件（SortableJS 等） |
| 数据桥接 | `input type="hidden" name="items_json"` | JS `lineItemCalc().collectItems()` |
| 成功导航 | 服务端返回 `HX-Redirect` | HTMX |
| 错误提示 | `htmx:responseError` 事件 → Notyf toast | htmx + JS |
| 服务端接收 | `Form<Struct>` + `serde_json::from_str()` | Axum |

### 强制规则

- **禁止** `fetch()` 提交表单，用 HTMX `hx-post` 原生处理
- **禁止** 用 `onclick` 调用自定义 JS 函数做 UI 操作，用 Surreal.js `<script>me().on(...)` 内联
- **禁止** 在 Maud 模板里用 `script { "..." }` （会被 HTML 转义），必须用 `maud::PreEscaped("<script>...</script>")`
- 复杂交互逻辑（拖拽、持久化状态）抽取到独立 JS 文件
- HTMX 仅处理需要服务端状态的交互，纯前端交互由 Surreal.js 内联闭环