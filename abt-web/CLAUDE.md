# abt-web

Rust 全栈前端，Axum + Maud + HTMX + Surreal.js + UnoCSS，直接调用 `abt-core` Service trait。

## Constraints（必须遵守）

**数据访问**
- **禁止直接访问数据库** — 所有数据操作必须通过 `abt-core` Service trait（`state.xxx_service()`）完成
- **禁止** 在 `abt-web` 中使用 `sqlx::query*` 或直接操作 `PgPool`/`PgConnection` 执行查询
- 如果 `abt-core` 缺少所需接口，应先在 `abt-core` 中补充，**同步更新 `docs/uml-design/` 设计文档**

**路由与组件**
- **必须使用 `TypedPath`**，禁止硬编码字符串 URL
- **使用 `hx-target="this"`** 让组件自包含，禁止硬编码 `#id` 作为 target。当 `this` 不满足需求时才用 `closest <selector>` 等相对定位
- **禁止为局部刷新单独创建 Handler** — Handler 始终返回完整组件

**样式**
- **禁止在 Maud 模板中使用 `style` 属性内联样式** — 所有样式必须提取到 `uno.config.ts` 中作为 CSS 类。唯一例外是 `<col>` 等必须用 style 的极少数场景
- **禁止手动修改 `static/app.css`**（UnoCSS 生成文件），仅通过 `npm run build:css` 生成
- **禁止在 `static/` 下新建独立 CSS 文件**

**JS 与交互**
- **禁止 `fetch()` 提交表单**，用 HTMX `hx-post` 原生处理
- **禁止用 `onclick` 调用自定义 JS 函数做 UI 操作**，用 Surreal.js `<script>me().on(...)` 内联
- **禁止在 Maud 模板里用 `script { "..." }`**（会被 HTML 转义），必须用 `maud::PreEscaped("<script>...</script>")`
- **纯前端 UI 状态**（Dropdown 菜单、Modal 显隐、Tab 切换）由 Surreal.js 内联闭环，**禁止通过 htmx 向后端发请求**
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
│   ├── base.rs          # HTML 壳（HTMX + Surreal.js + UnoCSS）
│   ├── admin.rs         # Admin 布局（sidebar + header + content）
│   ├── sidebar.rs       # 侧边栏
│   └── header.rs        # 顶部栏
├── components/          # 共享 UI 组件
└── pages/               # 页面模块（高内聚：TypedPath + Maud + Handler 同文件）
```

### CSS 管理

样式文件位于项目根级 `static/` 目录（非 `abt-web/static/`）：

- **`static/base.css`** — 手写 CSS（CSS 变量、重置、布局、复杂选择器）。**可直接编辑**
- **`static/app.css`** — UnoCSS 生成文件。**禁止手动修改**
- **`uno.config.ts`**（项目根级）— UnoCSS shortcuts 配置，新增工具类组合优先在此添加
- 新增组件样式优先用 UnoCSS shortcuts；复杂选择器、伪元素、媒体查询等放入 `base.css`
- HTML 模板只引用 `/app.css`，不引用其他 CSS 文件

### 组件样式类名速查 (uno.config.ts)

| 类名 | 用途 |
|------|------|
| `data-card` | 数据卡片容器（白色圆角，带阴影） |
| `data-table` / `data-card-scroll` | 数据表格 / 表格溢出滚动容器 |
| `form-section` / `form-section-title` | 表单分区 / 分区内标题 |
| `form-grid` | 表单双列网格 |
| `form-field` / `.span-2` / `.field-full` | 表单字段组 / 跨两列字段 |
| `form-input` / `form-select` | 输入框 / 下拉选择工具类 |
| `filter-bar` / `filter-select` | 筛选栏 / 筛选下拉框 |
| `search-wrap` + `search-input` | 搜索输入框（左侧放大镜图标） |
| `supplier-info-bar` | 供应商信息条 |
| `status-tabs` / `status-pill` | 状态 Tab 栏 / 状态标签 |
| `page-header` / `page-title` / `back-link` | 页面头部 / 标题 / 返回链接 |
| `create-action-bar` | 创建页底部操作栏（sticky） |
| `add-row-bar` / `btn-add-row` / `btn-remove-row` | 添加行按钮栏 / 添加行按钮 / 删除行按钮 |
| `modal-overlay` / `modal` / `modal-lg` | 模态框遮罩 / 模态框 / 大模态框 |
| `modal-head` / `modal-body` / `modal-foot` | 模态框区域（头部/内容/底部） |
| `product-search-*` / `product-select-*` | 产品搜索组件 / 产品选择列表 |
| `line-num` / `line-subtotal` / `num-right` / `mono` | 行号 / 行小计 / 数字右对齐 / 等宽字体 |
| `info-card` / `info-grid` / `info-item` | 详情卡片 / 网格 / 信息项 |
| `amount-summary` / `amount-row` | 金额汇总区 / 汇总行 |
| `workflow-steps` / `wf-step` | 工作流步骤条 / 单步 |
| `stat-card` / `pagination` | 统计卡片 / 分页组件 |

### SSR + HTMX

- **Axum Handler** 直接调用 `abt-core` Service trait，无 gRPC 中间层
- **Maud** 编译期 HTML 宏渲染完整页面或局部片段
- **HTMX 2.0.10** 处理表单提交、分页、搜索等需要服务器状态的交互
- **Surreal.js** 处理纯前端 UI 状态，通过内联 `<script>me().on(...)</script>` 在 HTML 元素内部闭环

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
| 纯前端 UI | **Surreal.js 内联** | Dropdown、Modal 显隐、Tab 切换 |
| 复杂前端逻辑 | **独立 JS 文件** | 拖拽排序（SortableJS）、持久化状态 |

当前独立 JS 文件：

| 文件 | 用途 |
|------|------|
| `static/bom-edit.js` | BOM 编辑页 — SortableJS 拖拽 + collapse/expand 状态持久化 |
| `static/app.js` | 全局工具 — `lineItemCalc` 行项目计算、`hs*` 兼容函数、分类树 |

---

## Surreal.js 参考手册

### 核心用法

`<script>` 内联在 HTML 元素内部，`me()` 自动返回**父元素**：

```html
<!-- 打开 modal -->
button type="button" {
    "<script>me().on('click',function(){me('#my-modal').classAdd('is-open')})</script>"
    "打开"
}
<!-- 关闭 modal overlay -->
div id="my-modal" class="modal-overlay" {
    "<script>me().on('click',function(e){if(e.target===me())me().classRemove('is-open')})</script>"
}
<!-- Tab 切换 -->
button class="tab-btn" {
    "<script>me().on('click',function(){me('.tab-btn').classRemove('active');me().classAdd('active')})</script>"
    "Tab 1"
}
```

### API 速查

| 用法 | 说明 |
|------|------|
| `me()` | `<script>` 的父元素 |
| `me(selector)` | `document.querySelector(selector)` |
| `me(selector, start)` | 从 start 元素开始搜索 |
| `any(selector)` / `any(selector, start)` | 返回匹配元素数组 |
| `me().on(event, fn)` | `addEventListener` |
| `me(el).classAdd/Remove/Toggle(cls)` | 操作 class |
| `me(el).attribute(name)` / `attribute(name, value)` | 读/写属性 |
| `me(el).remove()` | 移除元素 |
| `me(el).styles({prop:val})` | 设置样式 |

### HTMX + Surreal.js 联合模式

按钮用 `hx-get` 加载内容到 modal → 成功后自动打开 modal：

```rust
// 按钮
button type="button" hx-get=(path) hx-target="#edit-modal" hx-swap="innerHTML" { "编辑" }
// modal 容器
div id="edit-modal" class="modal-overlay" { }
// script 必须在 modal 容器外面（HTMX swap 会替换 innerHTML 导致内部监听器丢失）
maud::PreEscaped(r#"<script>
    me('#edit-modal')
        .on('htmx:afterSettle',function(){me(this).classAdd('is-open')})
        .on('click',function(ev){if(ev.target===me('#edit-modal'))me('#edit-modal').classRemove('is-open')});
</script>"#)
```

**注意**：`afterSettle` 回调用 `function(){}`（不用箭头函数），`this` 才指向触发元素。支持链式调用。

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
| 纯前端 UI | modal 开关、class 切换 | Surreal.js `<script>me().on(...)` 内联 |
| 服务端交互 | 表单提交、搜索、分页 | HTMX `hx-post`/`hx-get` |
| 复杂前端状态 | 拖拽排序、持久化 | 独立 JS 文件 |
| 数据桥接 | hidden input 传 JSON | JS `lineItemCalc().collectItems()` |
| 成功导航 | 页面跳转 | 服务端 `HX-Redirect` |
| 错误提示 | toast 提示 | `htmx:responseError` → Notyf |
| 服务端接收 | 表单解析 | `Form<Struct>` + `serde_json::from_str()` |
