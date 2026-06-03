# abt-web

Rust 全栈前端，Axum + Maud + HTMX + hyperscript + UnoCSS，直接调用 `abt-core` Service trait。

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
- **禁止在 Maud 模板中使用 `style` 属性内联样式。** 所有样式必须提取到 `uno.config.ts` 中作为 CSS 类，Maud 模板只使用 `class` 引用。这样做的好处：（1）样式可复用，避免重复定义；（2）统一管理，修改样式只需改一处；（3）减少 HTML 体积。唯一例外是 `<col>` 等 HTML 元素上必须用 style 的极少数场景。

### SSR + HTMX

- **Axum Handler** 直接调用 `abt-core` Service trait，无 gRPC 中间层
- **Maud** 编译期 HTML 宏渲染完整页面或局部片段
- **HTMX** 处理表单提交、分页、搜索等需要服务器状态的交互
- **hyperscript** 处理纯前端 UI 状态（dropdown、modal、tab 切换），通过 Maud 的 `_=` 属性绑定

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
│   ├── base.rs          # HTML 壳（HTMX + hyperscript + UnoCSS）
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

**禁止使用 htmx 的场景**：纯前端 UI 状态切换（Dropdown 菜单展开、Modal 弹窗显隐、选项卡纯样式切换）。此类交互由 **hyperscript**（通过 Maud 的 `_=` 属性，如 `_="on click toggle .is-open on #modal"`）在前端本地闭环，严禁通过 htmx 向后端发送请求，防止路由碎片化。

---

## 高级混合交互模式 (Advanced Hybrid Interactions)

### hyperscript 与 htmx 的数据桥接模式 (Hidden Input Binding)

场景：当组件存在复杂的纯前端高频交互（如拖拽、动态行项目表单），直接用 htmx 请求后端会导致严重的路由碎片化与网络延迟。

解法：复杂前端交互逻辑用独立 JS 文件（vanilla JS）管理状态，简单 UI 交互用 hyperscript 处理。状态通过 hidden input 桥接，htmx 提交时自动携带前端状态。

```rust
pub fn star_rating_component(current_rating: u32) -> Markup {
    html! {
        div class="rating-component" {
            input type="hidden" name="rating" value=(current_rating);

            div class="stars" _="on click set @aria-pressed to 'true'" {
                @for i in 1..=5 {
                    span class="star" _=(format!("on click set #rating-value to {}", i)) { "★" }
                }
            }

            button hx-post="/save-rating" hx-target="this" hx-swap="outerHTML" {
                "保存评分到数据库"
            }
        }
    }
}
```

---

## 表单开发模式 (Form Development Pattern)

所有涉及复杂前端交互的表单（动态行项目、计算、条件显示等）必须遵循以下架构：

### 架构分工

| 层 | 职责 | 技术 |
|---|---|---|
| 简单 UI 交互 | modal 开关、class 切换 | hyperscript (`_=` 属性) |
| 复杂前端状态 | 动态行项目、拖拽排序、计算 | vanilla JS（独立 JS 文件） |
| 复杂数据桥接 | `input type="hidden" name="items_json"` 序列化嵌套数据 | JS + HTML |
| 表单提交 | `hx-post` + `hx-swap="none"` | htmx |
| 成功导航 | 服务端返回 `HX-Redirect` 响应头 | htmx |
| 错误提示 | `htmx:responseError` 事件 → Notyf toast | htmx + JS |
| 服务端接收 | `Form<Struct>` + `serde_json::from_str(&form.items_json)` 解析嵌套数据 | Axum |

### 标准实现步骤

1. **JS 文件**：IIFE 封装的 vanilla JS 模块，管理状态和 DOM 交互
2. **Maud 模板**：`data-*` 属性传递初始数据，hyperscript `_=` 处理简单交互
3. **隐藏 input 桥接**：`input type="hidden" name="items_json"` 将嵌套数据暴露给 htmx
4. **外部数据集成**：htmx 搜索结果通过 `data-product` JSON 推入 JS 状态
5. **服务端**：`Form<QuotationCreateForm>` 接收，`items_json: String` 字段用 `serde_json::from_str()` 解析

### 范例参考

- JS：`static/quotation-create.js` — 报价单创建交互
- Rust：`src/pages/quotation_create.rs` — `quotation_create_page()` + `product_list_fragment()`

### 强制规则

- **禁止** `fetch()` 提交表单，用 htmx `hx-post` 原生处理
- 简单 UI 交互（modal 开关、class 切换）用 hyperscript `_=` 属性处理
- 复杂交互逻辑抽取到独立 JS 文件，禁止在 Maud 模板里内联 `<script>` 写大段 JS
- htmx 仅处理需要服务端状态的交互，纯前端交互由 hyperscript / vanilla JS 闭环

---

## 前端交互组件组织模式 (Frontend Component Organization)

当页面的交互逻辑较复杂（含拖拽、动态行项目、localStorage、状态管理等），必须将逻辑抽取到独立的 JS 文件中，禁止在 Maud 模板里内联 `<script>` 写大段 JS。简单交互（modal 开关、class 切换）直接用 hyperscript `_=` 属性。

### hyperscript 用法（简单交互）

hyperscript 通过 Maud 的 `_=` 属性绑定，用于纯前端 UI 操作：

```rust
// Modal 开关
button _="on click add .is-open to #my-modal" { "打开" }
button _="on click remove .is-open from #my-modal" { "关闭" }

// 事件响应
div _="on contactChanged from the body remove .is-open from #contact-modal" { ... }
```

### 独立 JS 文件（复杂交互）

- 放在 `static/` 目录下，文件名与功能对应，如 `cost-drawer.js`、`bom-edit.js`、`return-create.js`
- 使用 IIFE 封装，监听 `htmx:afterSettle` 和 `DOMContentLoaded` 自动初始化
- 在使用该组件的页面底部用 `<script src="/xxx.js?v=日期" />` 引入（放在 `html! {}` 块内）

### 标准结构

```javascript
// static/cost-drawer.js
(function () {
    'use strict';

    function storageKey(bomId) {
        return 'bom-cost-temp-prices:' + bomId;
    }

    function loadTempPrices(bomId) { ... }
    function saveTempPrices(bomId, map) { ... }

    function initCostDrawer(container) {
        // 初始化交互逻辑
    }

    function tryInit(target) {
        var el = target.querySelector('.cost-drawer');
        if (el) initCostDrawer(el);
    }

    // 监听 HTMX 交换和 DOM 加载
    document.addEventListener('htmx:afterSettle', function (e) {
        tryInit(e.target);
    });
    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', function () { tryInit(document); });
    } else {
        tryInit(document);
    }
})();
```

### Rust 端集成

```rust
// 在页面 HTML 输出末尾引入 JS 文件
html! {
    div {
        // 页面内容，简单交互用 hyperscript
        button _="on click add .is-open to #modal" { "打开" }
    }
    script src="/cost-drawer.js?v=20260602" {}
}
```

### Maud 与 hyperscript 注意事项

- hyperscript 通过 `_=` 属性绑定：`button _="on click toggle .active on me" {}`
- 空元素（`input`、`br`、`hr`）必须以 `{}` 结尾
- 需要动态 hyperscript 表达式时用 Maud 的 `()` 插值：`_=(format!("on click call window.myFunc({})", id))`
- 服务器数据通过 `data-*` 属性传入，JS 端用 `dataset` 读取

### 已有参考

| JS 文件 | 用途 |
|---------|------|
| `static/cost-drawer.js` | BOM 成本报告，临时价格覆盖 + localStorage |
| `static/bom-edit.js` | BOM 编辑页，拖拽排序 + 节点管理 |
| `static/return-create.js` | 销售退货单，动态行项目 |
| `static/app.js` | 分类树选择器组件 |