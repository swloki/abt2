# abt-web

Rust 全栈前端，Axum + Maud + HTMX + Hyperscript + UnoCSS，直接调用 `abt-core` Service trait。

## Constraints（必须遵守）
 **Rust 2024 Maud 陷阱**：字符串 `"xxx-yyy"` 后直接跟属性名（如 `style`）会被 Rust 2024 lexer 解析为 prefix literal，编译报错 `prefix 'yyy' is unknown`。解法：字符串末尾加空格 `"xxx-yyy "`。

**数据访问**
- **禁止直接访问数据库** — 所有数据操作必须通过 `abt-core` Service trait（`state.xxx_service()`）完成
- **禁止** 在 `abt-web` 中使用 `sqlx::query*` 或直接操作 `PgPool`/`PgConnection` 执行查询
- 如果 `abt-core` 缺少所需接口，应先在 `abt-core` 中补充，**同步更新 `docs/uml-design/` 设计文档**

**路由与组件**
- **必须使用 `TypedPath`**，禁止硬编码字符串 URL
- **使用 `hx-target="this"`** 让组件自包含，禁止硬编码 `#id` 作为 target。当 `this` 不满足需求时才用 `closest <selector>` 等相对定位
- **禁止为局部刷新单独创建 Handler** — 列表页统一用单端点模式（一个 list handler 服务完整页面和 HTMX 局部刷新），禁止创建独立的 table handler

**样式（100% 原子化 UnoCSS）**
- **禁止在 Maud 模板中使用 `style` 属性内联样式** — 所有样式用 UnoCSS 原子类写在 `class=""` 中（`<col>` 元素例外）
- **禁止手动修改 `static/app.css`**（UnoCSS 生成文件），仅通过 `npm run build:css` 生成
- **禁止新建 CSS 文件** — `static/base.css` 已删除，不再有手写 CSS 文件
 - **禁止新建 CSS 文件或手写 CSS** — 所有样式通过 UnoCSS 原子类或 shortcuts（高频复用语义类）实现
 - **`uno.config.ts` shortcuts 仅用于高频复用模式** — 当前有 `data-table`、`data-card`、`form-field`、`form-section`、`field-full` 五个 shortcut，新增需满足"10+ 文件复用且 class 字符串 >100 字符"标准
- **修改 CSS 变量 / 新增动画** — 在 `uno.config.ts` 的 `preflights`（`:root` 块）或 `theme.animation.keyframes` 中操作
- 详见 `AGENTS.md` 的 "CSS Management" 部分获取完整的原子化语法速查表和自定义 variants 说明

**JS 与交互**
- **禁止 `fetch()` 提交表单**，用 HTMX `hx-post` 原生处理
- **禁止用 `onclick`/`<script>me().on(...)` 做 UI 操作**，用 Hyperscript `_="on click ..."` 属性（详见下方参考手册）
- **禁止在 Maud 模板里用 `script { "..." }`**（会被 HTML 转义），复杂逻辑用 `maud::PreEscaped("<script>...</script>")` 包裹原生 JS
- **纯前端 UI 状态**（Dropdown 菜单、Modal 显隐、Tab 切换）由 Hyperscript `_=` 属性闭环，**禁止通过 htmx 向后端发请求**
- **涉及服务端状态**的交互（表单提交、分页、搜索）才用 htmx

## Architecture

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
│   ├── base.rs          # HTML 壳（HTMX + Hyperscript + UnoCSS）
│   ├── admin.rs         # Admin 布局（sidebar + header + content）
│   ├── sidebar.rs       # 侧边栏
│   └── header.rs        # 顶部栏
├── components/          # 共享 UI 组件
### CSS 管理（100% 原子化）

样式文件位于项目根级 `static/` 目录（非 `abt-web/static/`）：

- **`static/base.css`** — **已删除**，不再有手写 CSS 文件
 - **`uno.config.ts`**（项目根级）— UnoCSS 配置文件，包含 `preflights`（:root 变量 + reset + 少量不可原子化的组件状态 CSS）+ `theme`（颜色/字号/间距/圆角/阴影/动画）+ `variants`（自定义状态前缀）+ `shortcuts`（`data-table`、`data-card`、`form-field`、`form-section`、`field-full` 五个高频复用模式）

**核心原则：所有样式直接内联在 Maud 的 `class=""` 中，使用 UnoCSS 原子类组合。**

常用原子类模式（取代旧的语义化 class 名）：

| 场景 | 原子 class 示例 |
|---|---|
 | 数据卡片 | 使用 `data-card` shortcut（107+ 页面共用），或内联 `bg-bg border border-border-soft rounded-md p-5 mb-5 shadow-[var(--shadow-card)]` |
 | 表单字段 | 使用 `form-field` shortcut（59+ 页面）：自动处理 label（block/xs/medium/fg-2/mb-1/nowrap）+ input/select/textarea（w-full/px-3/py-2/border-border/rounded-sm/sm/bg-white/fg/focus→accent+shadow）+ textarea（resize-y/min-h-72px） |
 | 表单分区 | 使用 `form-section` shortcut：`bg-bg border border-border rounded-md p-6 mb-6`（22+ 页面） |
 | 表单跨列 | 使用 `field-full` shortcut：`col-span-full`（18+ 页面） |
| 页面标题 | `text-xl font-bold text-fg tracking-tight` |
| 状态标签 | `inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-xs before:content-[''] before:w-1.5 before:h-1.5 before:rounded-full before:bg-success` |
| 侧边栏导航项 | `flex items-center gap-3 px-5 py-[9px] text-sm text-white/60 rounded-sm cursor-pointer hover:bg-white/[0.06] [&_svg]:w-4.5 [&_svg]:h-4.5 [&_svg]:opacity-55` |
| 深色容器边框 | `[border-right:1px_solid_rgba(255,255,255,0.04)]`（用 arbitrary shorthand 解决 currentColor 继承） |

详见 `AGENTS.md` 的 "CSS Management" 部分获取完整的 UnoCSS 高级语法速查表。
### SSR + HTMX

- **Axum Handler** 直接调用 `abt-core` Service trait，无 gRPC 中间层
- **Maud** 编译期 HTML 宏渲染完整页面或局部片段
- **HTMX 2.0.10** 处理表单提交、分页、搜索等需要服务器状态的交互
- **Hyperscript 0.9.91** 处理纯前端 UI 状态，通过元素 `_="on click ..."` 属性声明式闭环（`static/hyperscript.min.js`）

**HTMX 2.x 事件模型**：
- `htmx:afterRequest` → 触发在 **trigger 元素**（发起请求的元素）上
- `htmx:afterSettle` → 触发在 **target 元素**（swap 目标）上
- `hx-select` 会被继承给子元素 → 解法：父元素加 `hx-disinherit="hx-select"`

---

## 组件化实现范式 (Component Patterns in Maud)

### 函数式组件 — 无状态，仅依赖入参

```rust
pub fn user_button(name: &str, is_admin: bool) -> Markup {
    html! { button class="btn" { "用户: " (name) } }
}
```

### 结构体组件 — 实现 Render trait 后可直接在 `html!` 中 `(struct_instance)` 挂载

```rust
pub struct UserCard { pub name: String, pub avatar_url: String }
impl Render for UserCard {
    fn render(&self) -> Markup {
        html! { div class="user-card" { img src=(self.avatar_url); h3 { (self.name) } } }
    }
}
```

### 插槽/容器组件 — 传递已渲染的 `Markup` 实现父子嵌套

```rust
pub fn layout_container(title: &str, children: Markup) -> Markup {
    html! { main class="content" { (children) } }
}
```

---

## 组件化三原则

### 1. 绝对内聚 — `hx-target="this"` + `hx-swap="outerHTML"`

组件自身就是替换边界，不依赖任何外部 ID：

```rust
html! {
    button hx-post=(path) hx-target="this" hx-swap="outerHTML" { "+1" }
}
```

### 2. 状态随身 — `hx-vals` 将 Rust 上下文绑定在 HTML 节点

避免依赖全局状态或 DOM 查询：

```rust
html! {
    tr hx-vals=(format!("{{\"item_id\": {id}, \"status\": \"{status}\"}}"))
       hx-post=(path) hx-target="this" hx-swap="outerHTML" { ... }
}
```

### 3. 视觉闭环 — `hx-indicator` 将 Loading HTML 写在组件内部

```rust
html! {
    div class="search" {
        input hx-get=(path) hx-target="this" hx-swap="outerHTML"
              hx-indicator=".search .loading" {};
        div class="loading htmx-indicator" { "搜索中..." }
    }
}
```

---

## 抗碎片化工程实践

**高内聚**：TypedPath、Handler、Maud 组件收拢在同一领域模块，禁止跨文件拆分。

**自包含更新**：用 `hx-target="this"` 替代硬编码 ID → Handler 始终返回完整组件 → 一个 URL 一个 Handler，无需感知请求来源。

```rust
#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/profile")]
pub struct ProfilePath { pub mode: String }

pub fn render_component(username: &str, mode: &str) -> Markup {
    html! {
        div class="profile-card" {
            @if mode == "display" {
                p { (username) }
                button hx-get=(ProfilePath { mode: "edit".into() })
                       hx-target="this" hx-swap="outerHTML" { "编辑" }
            } @else {
                form hx-post=(ProfilePath { mode: "display".into() })
                     hx-target="this" hx-swap="outerHTML" {
                    input type="text" name="username" value=(username);
                    button { "保存" }
                }
            }
        }
    }
}
```

### 列表页单端点模式

**核心原则：一个 URL = 一个 Handler**。列表页只保留一个 `list` handler，通过 `admin_page(is_htmx)` 同时服务完整页面（首次访问）和 HTMX 局部刷新（tab 切换、搜索、分页）。**禁止为 tab 切换、搜索、分页创建额外的 handler 或路由**。

```
用户请求 ──→ 单一 list handler
                ├── is_htmx=false → admin_page(false, ...) → 完整 HTML 页面
                └── is_htmx=true  → admin_page(true, ...)  → 只有 content 片段
                                    HTMX 从响应中选取 #data-card + #status-tabs 替换
```

**为什么**：tab 切换、搜索、分页本质上是同一个数据视图的不同参数组合，应该是**一个端点 + 不同 query params**，而不是拆成多个端点。HTMX 的 `hx-get` + `hx-vals` / `hx-include` 天然支持参数组合，不需要后端参与。

#### 三大控件

**Status Tabs**（`status_tabs_with_param`）：

```rust
// 组件内部生成的每个 <a> 自带：
// hx-get=ListPath::PATH          → 同一个 list 端点
// hx-target="#data-card"         → 替换数据区（标准 CSS #id，禁止 closest）
// hx-select="#data-card"         → 从响应中选取数据区
// hx-select-oob="#status-tabs"   → 同时替换 tab 栏自身
// hx-push-url="true"             → 浏览器地址栏反映当前状态
// hx-vals='{"status": "2"}'      → 携带状态参数
// hx-include="#filter-form"      → 携带搜索表单参数
status_tabs_with_param(ListPath::PATH, "#data-card", "#filter-form", tabs, &active_value, "status")
```

**Filter Form**：

```rust
// form 包裹所有筛选控件，统一 hx-get
// 子元素（input/select）无需独立 hx-* 属性，change 事件由 form 的 hx-trigger 捕获
form class="filter-bar filter-form" id="xxx-filter-form"
    hx-get=(ListPath::PATH)
    hx-trigger="change, keyup changed delay:300ms from:.search-input"
    hx-target="#data-card"
    hx-select="#data-card"
    hx-swap="outerHTML"
    hx-push-url="true"
    hx-include="#xxx-filter-form" {   // 指向自身 id，GET 自动携带所有字段
    // input, select ...
}
```

**Pagination**：

```rust
// 服务端拼接完整 query_string（包含 status、keyword 等），确保分页保持筛选状态
pagination(ListPath::PATH, &query_string, total, page, total_pages)
```

#### 关键约束

- **`hx-select` 只支持标准 CSS 选择器**：`closest` 是 HTMX 扩展伪选择器，仅在 `hx-target` 中有效。`hx-select` 从服务器响应 HTML 中选取片段，必须用 `#id`
- **`hx-select-oob` 支持逗号分隔**：可同时替换多个区域 `hx-select-oob="#status-tabs, #filter-form"`
- **`TypedPath::PATH` 需要 trait 在 scope 中**：页面文件必须 `use axum_extra::routing::TypedPath;`，否则报 `no associated item named PATH`
- **`Serialize` 与 `TypedPath` derive 冲突**：`#[derive(TypedPath, Serialize, ...)]` 会阻止 `PATH` 常量生成，去掉 `Serialize`

---

## 事件驱动解耦 (HX-Trigger)

多组件联动场景，避免编写"聚合刷新路由"：

1. 主动组件 POST → 后端响应头 `HX-Trigger: "cartUpdated"`
2. 被动组件声明 `hx-trigger="cartUpdated from:body"` 指向各自的强类型路径

---

## 混合群岛架构边界

| 交互类型 | 技术 | 示例 |
|----------|------|------|
| 涉及服务器状态 | **HTMX** | 表单提交、动态分页、条件搜索 |
| 纯前端 UI | **Hyperscript `_=`** | Dropdown、Modal 显隐、Tab 切换 |
| 复杂前端逻辑 | **独立 JS 文件** | 拖拽排序（SortableJS）、持久化状态 |

当前独立 JS 文件：

| 文件 | 用途 |
|------|------|
| `static/bom-edit.js` | BOM 编辑页 — SortableJS 拖拽 + collapse/expand 状态持久化 |
| `static/app.js` | 全局工具 — `lineItemCalc` 行项目计算、`hs*` 兼容函数、分类树 |

---

## Hyperscript 参考手册

Hyperscript 写在元素的 `_` 属性里（Maud：`_="on click ..."` 或 `_=(format!(...))`）。完整文档：https://hyperscript.org/reference/

### 核心模式（项目最常用）

```rust
// 打开 modal
button _="on click add .is-open to #my-modal" { "打开" }
// 关闭最近的 overlay
button _="on click remove .is-open from closest .modal-overlay" { "×" }
// 背景点击关闭（事件过滤器：只有点 overlay 本身）
div.modal-overlay _="on click[me is event.target] remove .is-open" { }
// 下拉菜单点击外部关闭（elsewhere = 本元素之外）
div.dropdown _="on click from elsewhere remove .is-open" { }
// Tab 选中（take：移除同组其他元素的 class，加给自己）
button.tab _="on click take .active from .tab" { "Tab 1" }
// HTMX 请求成功后打开 drawer（事件名带冒号必须用单引号 + 驼峰）
button hx-get=(path) hx-target="#d-body" _="on 'htmx:afterRequest'[detail.xhr.status < 400] add .open to #drawer" { }
```

### 命令速查

| 命令 | 说明 | 示例 |
|------|------|------|
| `add .cls to #id` | 加 class | `add .is-open to #modal` |
| `remove .cls from #id` | 删 class | `remove .active from .tab` |
| `toggle .cls [on #id]` | 切换 | `toggle .expanded` |
| `take .cls from <set>` | 抢占（移除同组，加给自己） | `take .active from .tree-node-row` |
| `closest <sel/>` | 最近祖先（**必须用 query 语法**：`.cls` 或 `<tag/>`，不能裸写 `tr`/`form`） | `remove .is-open from closest .modal-overlay` · `remove closest <tr/>`（删整行） |
| `next <sel/>` | 下一兄弟 | `toggle .is-open on next <div/>` |
| `reset #form` | 重置表单 | `then reset #my-form` |
| `call jsFn()` | 执行 JS | `call quotationSubmit()` |
| `put val into #id's value` | 写入属性 | `put '1' into #shift's value` |
| `halt` | 阻止事件 | `on click halt the event then ...` |
| `trigger evt on #id` | 触发事件 | `trigger submit on #form` |
| `from elsewhere` | 点击外部（click-away） | `on click from elsewhere remove .open` |

**Magic values**: `me`(当前元素) · `it`(上次结果) · `event`/`target`/`detail`(事件)。
**HTMX 事件**：事件名含冒号，必须用**单引号字符串**且**驼峰**：`on 'htmx:afterRequest'`、`on 'htmx:afterSettle'`（不是 `htmx:after-request`）。
**复杂逻辑**（表单行收集/全选）：保留 `<script>` 原生 JS（`document.querySelector`），用 `_="on submit call fn()"` 调用。

### HTMX + Hyperscript 联合模式

按钮用 `hx-get` 加载内容到 modal → 成功后自动打开（`_=` 放在发起请求的元素上）：

```rust
button type="button" hx-get=(path) hx-target="#edit-modal" hx-swap="innerHTML"
    _="on 'htmx:afterRequest' add .is-open to #edit-modal" { "编辑" }
div id="edit-modal" class="modal-overlay" _="on click[me is event.target] remove .is-open" { }
```


### HTMX 表单替代 JS

用 `<form hx-post>` 替代 `onclick="htmx.ajax(...)"`，完全不需要 JS：

```rust
form hx-post=(path) hx-swap="none" hx-include="[name='parent_id']" {
    input type="hidden" name="product_id" value=(id) {}
    button type="submit" { "选择" }
}
```

### 行项目计算器（lineItemCalc）

报价单、销售订单、采购单的行项目计算逻辑统一用 `lineItemCalc(tbodyId)`：

```rust
tr oninput="lineItemCalc('#quotation-item-tbody').calcRow(this)" { ... }
form onsubmit="lineItemCalc('#order-item-tbody').collectItems()" { ... }
```

---

## 表单开发模式

| 层 | 职责 | 技术 |
|---|---|---|
| 纯前端 UI | modal 开关、class 切换 | Hyperscript `_="on click ..."` |
| 服务端交互 | 表单提交、搜索、分页 | HTMX `hx-post`/`hx-get` |
| 复杂前端状态 | 拖拽排序、持久化 | 独立 JS 文件 |
| 数据桥接 | hidden input 传 JSON | JS `lineItemCalc().collectItems()` |
| 成功导航 | 页面跳转 | 服务端 `HX-Redirect` |
| 错误提示 | toast 提示 | `htmx:responseError` → Notyf |
| 服务端接收 | 表单解析 | `Form<Struct>` + `serde_json::from_str()` |
