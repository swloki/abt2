---
description: "ABT HTMX 范式正文：单端点原则、组件三原则、列表页三大控件、HX-Trigger 多组件联动、Modal/Drawer 三模式、AI 易错点纠错"
globs: ["abt-web/**/*.rs"]
---

# HTMX 开发范式说明

> 本文档是 abt-web 前端交互的**系统性范式正文**。[`abt-web/CLAUDE.md`](../../abt-web/CLAUDE.md) 是强约束入口（必读速查），本文是其展开：写页面 / 组件遇到交互决策时，来这里查「用哪种模式、为什么、踩过哪些坑」。
>
> 技术栈：Axum Handler + Maud SSR + HTMX 2.x + Hyperscript + UnoCSS。所有示例为 Maud `html!` 宏写法。每个模式标注真实代码出处（`file:line`），可直接跳转核对。

---

## 核心原则：一个端点 = 一个完整片段

这是整个 HTMX 范式的第一性原则，**下面所有章节都是它的展开**：

> **一个页面或一个组件，只对应一个 URL（端点）。Handler 每次都返回该 URL 的完整 HTML（逻辑自洽、不感知请求来源）；前端用 `hx-select` / `hx-select-oob` 从完整响应里选取要更新的位置。**

四个要点：

| 要点 | 含义 | 违反的写法（禁止） |
|---|---|---|
| **单一 URL** | 一个页面 / 一个交互组件 = 一个 handler / 路由 | 为 tab、搜索、分页各开一个 handler |
| **逻辑自洽** | handler 只看入参（query / form）决定渲染什么，不关心请求来自首次加载还是局部刷新 | handler 里按「谁调的」走不同分支、返回不同结构 |
| **完整返回** | 每次返回该 URL 对应的**全部** HTML（整页或整个组件），不是「只返回变化的那一小块」 | 为省流量只返回一个 `<tr>`、让后端配合前端「恰好返回那一块」 |
| **前端选取** | 用 `hx-select`（从响应选一块替换 target）/ `hx-select-oob`（额外替换别处）决定更新哪些位置 | 后端为每种局部需求定制返回内容 |

### 两个粒度

这条原则同时管**页面**和**组件**两层：

- **页面粒度** —— 一个 list / detail 页面 = 一个 URL。首次访问返回完整页（layout + content）；局部刷新（tab / 搜索 / 分页）走**同一个 URL**，handler 用 `is_htmx` 分流：非 HTMX 返回整页，HTMX 返回该页完整的 content 片段。前端用 `hx-select="#data-card"` 选取数据区、`hx-select-oob="#status-tabs"` 同时刷新 tab 栏。→ 详见 §2 列表页单端点模式。
- **组件粒度** —— 一个交互组件（计数按钮、行操作、modal 表单）= 一个 URL。Handler 永远返回**完整组件**，组件自身就是替换边界（`hx-target="this"` + `hx-swap="outerHTML"`），不依赖任何外部 ID。→ 详见 §1 组件化三原则。

### ID 引用的粒度规则（何为「外部 ID」）

「禁止引用外部 ID」（§1 三原则、§7 反模式）容易被误读成「任何 `#id` 都不能用」。其实禁的是**跨组件 / 跨单元的硬编码**，判据是**被引用的 ID 属于谁**：

| 引用对象 | 合法性 | 说明 |
|---|---|---|
| **引用自己** `#自己` | ✅ | data div `hx-select="#wc-release-routings"`（§4）—— 节点引用自身 ID，自包含 |
| **同一页面单元内的兄弟区域** | ✅ | list 页里 tab `hx-select-oob="#status-tabs"`、`hx-target="#data-card"` —— 这些 ID 同属一个 list 页这个自包含单元，是页面内协作，不是跨组件耦合 |
| **别的组件 / 跨单元** | ❌ 违规 | 一个通用 picker 组件去 `hx-target="#调用方页面的某id"` —— 组件不该知道调用方的 DOM |

一句话：**页面是一个自包含单元，单元内的 ID 互相引用合法；跨单元（尤其通用组件引用调用方 DOM）才是要禁的「外部 ID」**。这条规则统一了 §1 / §7（组件粒度禁外部 ID）与 §2 / §4（页面粒度引用兄弟区域）—— 它们是同一条规则在两个粒度的投影，不矛盾。

### 为什么这样设计

- **Handler 无状态化** —— 不感知请求来源，同一个 URL 对首次加载、tab 切换、搜索、事件触发都返回一致视图，不会因「谁调的」而变形，天然可组合、可复用。
- **职责清晰** —— 后端只负责「返回完整正确的 HTML」，前端用 `hx-select` 声明「我要更新哪里」。后端不用为每种局部需求定制端点，前端不用猜后端这次返回了什么结构。
- **同一响应，多选取** —— 一份完整响应，不同触发点用不同 `hx-select` 选取不同区域：点 tab 选 `#data-card` + oob `#status-tabs`；点搜索只选 `#data-card`；点行操作 oob 多区。一个端点支撑所有交互。

### 推论：一个页面需要多个 URL ⟹ 该拆成组件

核心原则说「一个页面 / 组件 = 一个 URL」。反过来也成立：**如果你发现一个页面需要多个 URL（多个端点），那不是核心原则失灵，而是这个页面该被拆成多个组件的信号** —— 把每个独立区域拆成组件，各自一个 URL、各自返回完整片段、各自用 `hx-select` 选取。页面退化为这些组件的「外壳」，只负责组装，不持有组件的端点。

典型范例：工作台类页面（`pages/mes_work_center.rs:4-7` 模块注释）首页是外壳，内联 3 个 card 占位 div，每个 card 用 `hx-trigger="load"` 拉自己的 GET 端点、`hx-select="#wc-xxx-card"` 局部刷新；写操作广播事件，各 card 监听自刷新（§4）。**3 个 card = 3 个组件 = 3 个 URL**，页面本身不持有这 3 个端点。

判断标准：

- 页面只有「整页 + 同一区域的局部刷新」→ **单 URL**（§2 列表页）。
- 页面有多个**独立刷新的区域**（各自 tab / 筛选 / 数据源）→ **拆成组件**，每组件一个 URL，页面做外壳。
- 拆分边界 = 替换边界：每个组件 `hx-target="this"` + `hx-select="#自己"` 自包含（§1 三原则）。

### 违反本原则 = 反模式

- 为局部刷新创建独立 handler / 路由（违反**单一 URL**，见 §7 反模式清单）。
- Handler 按请求来源返回不同结构（违反**逻辑自洽 / 完整返回**）。
- 后端「优化」成只返回变化的小片段配合前端（违反**完整返回**——应让前端用 `hx-select` 选取）。

### 速查：前端五条红线

写任何页面前先记住这五条，背景见各章及 §7：

1. **一个页面 / 组件 = 一个 URL** —— 不为 tab / 搜索 / 分页开独立 handler（§2）。
2. **Handler 返回完整片段** —— 不为省流量只返回一个 `<tr>`，前端用 `hx-select` 选取（核心原则）。
3. **纯前端 UI 用 Hyperscript，不发 HTMX 请求** —— modal 显隐 / class 切换走 `_=`，禁 `onclick`（§0）。
4. **`hx-vals` 用序列化，不手拼 JSON** —— 用 `vals_json()` / `serde_json::json!`，禁 `format!` 拼字符串值（§1.2）。
5. **列表页禁 `hx-push-url`** —— 地址栏 push 导致刷新 / 分享 / 回退异常（§2.3）。

---

## 0. 范式总览：三层技术分工

ABT 前端是「混合群岛」架构，按交互是否涉及服务端状态选择技术，**职责不重叠**：

| 层 | 职责 | 技术 | 红线 |
|---|---|---|---|
| 服务端状态 | 表单提交、分页、搜索、状态流转、写操作 | **HTMX** `hx-post` / `hx-get` | 禁 `fetch()` 提交表单 |
| 纯前端 UI | Modal / Drawer 显隐、Dropdown、Tab 切换、class 切换 | **Hyperscript** `_="on click ..."` | 禁 `onclick` / `me().on()` |
| 复杂前端状态 | 拖拽排序、行项目计算、持久化状态 | **独立 JS**（`static/app.js`、`static/bom-edit.js`） | Hyperscript 用 `call fn()` 调用 |

**一条红线**：纯前端 UI 状态由 Hyperscript 闭环，**禁止通过 HTMX 向后端发请求**；涉及服务端状态的交互才用 HTMX。

**HTMX 零扩展**：`layout/page.rs:20` 仅加载 `/htmx.min.js`（HTMX 2.x 核心），**无任何扩展脚本**。本文用到的 `intersect`（懒加载）、`closest`（`hx-target` 相对定位）、`hx-disabled-elt`、`hx-sync`、`hx-preserve`、`hx-swap-oob` 等均为 **2.x 核心能力**，无需额外注册扩展。新功能若要用到非核心扩展（如 `morph`、`multi-swap`），须先在 `page.rs` 加载对应扩展脚本并在此声明。

---

## 1. 组件化三原则（自包含）

所有交互组件遵循三原则，本质是「组件自身就是替换边界，不依赖任何外部 ID」——这是上方**核心原则**在组件粒度的落地（一个组件 = 一个 URL，每次返回完整组件）。三原则：

> **替换后行为不丢**：Hyperscript 引擎内置 DOM 监听，会自动初始化 HTMX `outerHTML` 换入的新元素上的 `_` 属性——替换后的新组件会重新获得完整事件行为，**无需手动重绑定**。这是三原则能成立的前提：组件可以放心地「自我替换」。

### 1.1 绝对内聚 — `hx-target="this"` + `hx-swap="outerHTML"`

组件自身是替换边界：

```rust
div class="counter" {
    span { (count) }
    button hx-post=(path) hx-target="this" hx-swap="outerHTML" { "+1" }
}
```

Handler 永远返回**完整组件**，无需感知请求来源 → 一个 URL 一个 Handler。

> **何时要加 `hx-select="#自己"`？** 组件 = 端点（端点只返回该组件）时，`hx-target="this"` + `hx-swap="outerHTML"` 就够，**不需要 `hx-select`**（如本节计数按钮）。只有当组件是**更大响应单元的子区域**——端点返回完整 drawer / 整页，组件只是其中一块——才需要 `hx-select="#自己"` 从完整响应里切出自己（如 §4 data div 与打开它的按钮共用 drawer 端点）。判别规则：**端点返回的东西 ＞ 组件本身 → 加 `select`；等于组件本身 → 不加**。

### 1.2 状态随身 — `hx-vals` 把 Rust 上下文绑在节点上

避免依赖全局状态或 DOM 查询。`hx-vals` 的值是 JSON，**必须用序列化生成，禁止 `format!` 手拼字符串值**——一旦值含引号 / 反斜杠（用户输入、自由文本），手拼的 JSON 即非法，HTMX 静默丢弃整个参数。

```rust
use serde_json::json;

// ✓ 用 serde_json::json! —— 值自动转义
tr hx-vals=(json!({ "item_id": id, "status": status }).to_string())
   hx-post=(path) hx-target="this" hx-swap="outerHTML" { ... }
```

项目通用做法是封装 `vals_json(...)` 辅助函数（`components/customer_search.rs:315`、`components/supplier_search.rs:314`）把组件实例参数编成 `hx-vals` JSON。纯数字 id 用 `format!` 不受影响（数字无引号问题），但**凡字符串 / 枚举值，一律走 `json!` 或 `vals_json`**。

> ⚠️ 现存脆弱写法：`pages/mes_work_center.rs:868`、`pages/purchase_work_center.rs:1131` 等仍用 `format!` 拼字符串 status 值。当前塞的是程序内枚举常量（安全），但写法脆弱，新增字段勿照抄。

### 1.3 视觉闭环 — `hx-indicator` 把 Loading 写在组件内部

```rust
div class="search" {
    input hx-get=(path) hx-target="this" hx-swap="outerHTML"
          hx-indicator=".search .loading" {};
    div class="loading htmx-indicator" { "搜索中..." }
}
```

> 当 `this` 不满足需求时，才退而用 `closest <selector>` 等相对定位（见 §5.5 行内编辑）。

---

## 2. 列表页单端点模式

### 2.1 页面粒度：一个 URL 服务整页 + 局部刷新

列表页是上方**核心原则**在页面粒度的典型落地：一个 `list` handler，通过 `admin_page(is_htmx)` 同时服务完整页面（首次访问 / 刷新）和 HTMX 局部片段（tab 切换、搜索、分页），每次都返回该页完整的 content。**禁止为 tab / 搜索 / 分页创建额外 handler 或路由**。

```
用户请求 ──→ 单一 list handler
                ├── is_htmx=false → admin_page(false, ...) → 完整 HTML 页面
                └── is_htmx=true  → admin_page(true, ...)  → 只有 content 片段
                                    HTMX 从响应中选取 #data-card + #status-tabs 替换
```

### 2.2 三大控件

**Status Tabs**（`components/tabs.rs`）：

```rust
// 默认 oob 刷新 #status-tabs
status_tabs_with_param(ListPath::PATH, "#data-card", "#filter-form", &tabs, &active, "status")
// 需要切换 tab 时同时刷新其它区域（如带 hidden status 的 filter-form）：
status_tabs_with_oob(ListPath::PATH, "#data-card", "#filter-form", "#status-tabs,#filter-form", &tabs, &active, "status")
```

每个 `<a>` 自带（`components/tabs.rs:79-89`）：

| 属性 | 值 | 作用 |
|---|---|---|
| `hx-get` | `ListPath::PATH` | 请求同一 list 端点 |
| `hx-target` / `hx-select` | `#data-card` | 替换 + 选取数据区（**标准 CSS `#id`**） |
| `hx-select-oob` | `#status-tabs`（默认） | 同时替换 tab 栏自身 |
| `hx-swap` | `outerHTML` | 外层替换 |
| `hx-vals` | `{"status": "2"}` | 携带状态参数（空值 tab → `{"status":""}` 即「全部」） |
| `hx-include` | `#filter-form` | 携带搜索表单参数 |

> **为什么 tab 切换要 oob 重渲染 filter-form？** 源码注释（`components/tabs.rs:30-37`）解释了根因：filter-form 里若有 hidden status input，切换 tab 后若不重渲染它，hidden status 会**变 stale**，后续一次筛选（搜索或行操作触发的 event 刷新）会发送**旧 status**，视图跳回第一个 tab。所以只要 filter-form 带 status，tab 切换必须 oob 把它一起刷新。

**Filter Form**：form 包裹所有筛选控件，统一 `hx-get`，子元素无需独立 `hx-*`：

```rust
form class="filter-bar filter-form" id="xxx-filter-form"
    hx-get=(ListPath::PATH)
    hx-trigger="change, keyup changed delay:300ms from:.search-input"
    hx-target="#data-card"
    hx-select="#data-card"
    hx-swap="outerHTML"
    hx-include="#xxx-filter-form" {   // 指向自身 id，GET 自动携带所有字段
    // input / select ...
}
```

- **`hx-include="#xxx-filter-form"` 是「显式自举」**：`<form>` 自身发起的 GET 本就会序列化其内全部命名字段，这里写自身 id 是冗余但无害的强调。**真正必需**的 `hx-include` 是跨区域包含——Status Tabs 的 `<a>` 标签本身没有表单字段，必须 `hx-include="#filter-form"`（`components/tabs.rs`）才能把搜索条件带上；也可指向别的 form（`hx-include="#another-form"`）。
- `delay:300ms` 防抖、`changed` 只在值变化时触发——修饰符速查见 §2.2.1。
- **保留筛选输入值**：filter-form 在 `#data-card` 上被 `outerHTML` 刷新时输入框会清空。给需保留的控件加 `hx-preserve`，HTMX 把原节点搬到新 DOM、避免重建丢值。范本：`pages/fms_ap_ledger.rs:293-348`、`pages/fms_ar_ledger.rs:295-348`（日期 / 单号 / 名称等筛选项全部 `hx-preserve`）。
- 进阶：混合触发源，见 `pages/user_list.rs:401`（搜索输入 + 自定义事件 `userToggled from:body` 都刷新同一 form）

**Pagination**（`components/pagination.rs`）—— 单一组件，`status_tabs` 同款 HTMX 局部刷新（3 个旧组件 `pagination` / `htmx_pagination` / `htmx_pagination_inherited` 已合并为这一个）：

```rust
// page 经 hx-vals 传，筛选经 hx-include 携带 —— 无 hidden input、无 hyperscript
pagination(ListPath::PATH, "#order-data-card", "#filter-form", total, page, total_pages)
```

每个链接自带 `hx-get` + `hx-target=target_sel` + `hx-select=target_sel` + `hx-vals={"page":N}` + `hx-include=form_sel`，与 `status_tabs` 完全对称。点击 → HTMX 局部替换 `target_sel`（URL 不变，列表页禁 `hx-push-url`）。

- `target_sel`：替换目标（如 `"#order-data-card"`、`".transaction-panel"`）
- `form_sel`：携带筛选的 form 选择器（如 `"#filter-form"`）；同页多 form 各传各的

**约定**：

- filter-form `id="filter-form"`（被 `hx-include` 引用），自身已有 `hx-get` + `hx-target="#data-card"` + `hx-select="#data-card"` + `hx-trigger="change, keyup..."`（搜索用，标准列表页本就有）
- **切 tab 回第 1 页**：`status_tabs` 的 `hx-vals` 强制 `page=1`（`components/tabs.rs`）
- **搜索回第 1 页**：filter-form 无 page 字段 → 搜索 submit 不带 page → handler 默认 page=1（自动，无需额外代码）

> **为什么用 a 链接 + hx-vals 而非事件 / hyperscript**：分页链接直接当 htmx 触发器（status_tabs 同款），page 经 `hx-vals` 传是 htmx 原生传参，无需 hidden page input、无 hyperscript、无 app.js 函数。实测备选路径（自定义 `paginate` 事件 + `hx-vals=js:event.detail`、`trigger submit`、`form.requestSubmit()`）在 htmx form 上要么不触发请求、要么改变地址栏 URL（列表页不可接受）。
>
> detail 子分页（`customer_detail` / `routing_detail` / `category_list`）：panel 容器即 `form#filter-form`（**无 hx-***，仅作 `hx-include` 取值容器，因 detail 无搜索字段），分页链接 `target_sel` 指向 panel class（如 `.transaction-panel`）。`category_list` 改用专用 `CategoryDetailPanelPath` 端点。

### 2.2.1 常用 `hx-trigger` 修饰符

| 修饰符 | 作用 | 出处 |
|---|---|---|
| `delay:300ms` | 防抖，停顿后才触发（搜索框） | §2.2 filter form |
| `changed` | 仅当值改变才触发，避免重复请求 | §2.2 `keyup changed` |
| `once` | 只触发一次（懒加载 / 一次性初始化） | `components/product_picker.rs:195` 等 `intersect once`；`pages/mes_work_center.rs:385` `click once` |
| `from:.xxx` / `from:body` | 监听别处元素 / body 广播的事件 | §2.2 `from:.search-input`；§4 `from:body` |
| `load` | 元素加载即触发（首次拉取） | §4 `load, xxxChanged from:body` |

> **请求去重用 `hx-sync`，不用 `queue`**：高频请求（实时搜索）的去重，ABT 统一用 `hx-sync="this:replace"`（§5.3）——它**真正取消进行中的旧请求**，比 `hx-trigger` 的 `queue:` 修饰符（仅控制排队、不取消）更彻底，两者不要混用。

### 2.3 列表页踩坑（已收编）

- **`hx-select` 不支持 `closest`** —— `closest` 是 HTMX 内置的相对选择器语法，**仅在 `hx-target` 中有效**（零扩展，见 §0）。`hx-select` 从响应 HTML 选取片段，必须用标准 CSS（`#id` / `.class`）。
- **`hx-select-oob` 支持逗号分隔** —— 可同时替换多个区域：`hx-select-oob="#status-tabs, #filter-form, #stats-bar"`（见 `pages/user_list.rs:398`、`pages/permission_config.rs:572`）。
- **`TypedPath::PATH` 需要 trait 在 scope** —— 页面文件用 `XxxPath::PATH` 必须显式 `use axum_extra::routing::TypedPath;`，否则报 `no associated item named PATH`。
- **`Serialize` 与 `TypedPath` derive 冲突** —— `#[derive(TypedPath, Serialize, Deserialize, Clone)]` 会阻止 `PATH` 常量生成，去掉 `Serialize`。
- **列表页禁用 `hx-push-url`** —— tab / 搜索 / 分页时 push 地址栏会导致刷新 / 分享 / 回退行为异常。组件层（`tabs.rs` / `pagination.rs`）已不含该属性，**写列表页不要再加**。刷新回默认状态（首个 tab、无搜索）是预期行为。（已于 2026-06-26 全部清理，见附录。）
- **`outerHTML` 自替换丢失滚动位置 / 元素状态** —— 列表页 `hx-target="#data-card" hx-swap="outerHTML"` 用「新 #data-card 替换旧 #data-card」，容器节点本身被重建，**滚动位置、第三方库持有的 DOM 引用会丢失**（长列表点 tab 后可能跳顶）。ABT 里这属可接受行为。若必须保留：给内部需保状态的元素加 `hx-preserve`（见 §2.2），或用 `hx-swap="outerHTML show:top"` 修饰符显式控制滚动。注意不要为此改用 `innerHTML`——那要求 handler 只返回容器内部、破坏「完整片段」结构。

### 2.4 OOB 多区域更新：两种声明方式

一个端点要同时刷新多个区域时（点 tab 同时刷 `#data-card` + `#status-tabs`；保存后固定刷 summary + list），HTMX 有两种 OOB 声明方式。**ABT 以 `hx-select-oob`（前端选取）为主** —— 它契合核心原则「前端用 `hx-select` 声明更新位置」，且是页面粒度协作（同端点、不同触发点选不同区域）的主场；`hx-swap-oob`（后端标记）作为特例，仅用在「更新区域固定、发起方不该感知 DOM」时。**主响应始终是完整片段，两种都不违反核心原则** —— 区别只在「谁来声明额外的 OOB 区域」：

| 策略 | 写在哪 | 适用场景 |
|---|---|---|
| **前端选取** `hx-select-oob="#a,#b"` | 发起请求的元素上 | **同一端点、不同触发点要选不同区域**（点 tab 选 `#data-card`+`#status-tabs`；点搜索只选 `#data-card`）—— 灵活，ABT 默认。范本：§2.2 status tabs、`pages/permission_config.rs:572`。 |
| **后端标记** `hx-swap-oob="true"` | 响应体的 OOB 节点上 | **更新区域固定、且不想让发起元素感知 DOM 结构**（无论谁触发都固定连带刷新某区）—— 发起方更干净。范本：`pages/om_outsourcing_create.rs:372`。 |

后端标记的真实写法（主响应完整片段 + 尾部追加 OOB 节点）：

```rust
let mut html = routing_select_fragment(&s).into_string();   // 主响应：完整片段
if outsourced_count == 1 {
    html.push_str(&format!(r#"<tbody id="material-tbody" hx-swap-oob="true">{rows}</tbody>"#));
}
Ok(Html(html))
```

> 选型原则：**前端按触发点灵活选取 → `hx-select-oob`；后端固定连带刷某区且发起方不该关心 → `hx-swap-oob`**。两者可混用：发起方用 `hx-select-oob` 选主区域，响应体里再带若干 `hx-swap-oob` 节点。不要误以为 `hx-swap-oob` 是「只返回小块」——那会违反完整返回原则。

---

## 3. Modal / Drawer 编辑流（三模式）

按内容来源选模式，**优先复用通用组件**。

### 3.1 模式 A — 通用组件 `components::modal`（静态表单，首选）

表单内容在服务端已知时，直接用 `components::modal`：

```rust
modal(
    "edit-modal",      // modal_id：overlay div 的 id，调用方用 Hyperscript 切换 .is-open
    "编辑用户",         // title
    "保存",            // submit_label
    "edit-form",       // form_id：footer 提交按钮通过它关联 <form>
    &EditPath::PATH,   // hx_post
    body_markup,       // body：表单内容插槽
)
```

组件内置（`components/modal.rs:10-56`）：

- 表单级 `hx-post` + `hx-swap="none"`
- **成功后自动关闭 + 重置**：`_="on 'htmx:afterRequest'[detail.xhr.status < 400] remove .is-open from closest .modal-overlay then reset me"`
- 取消按钮 / × 按钮：`on click remove .is-open from closest .modal-overlay then reset #form-id`
- 背景点击关闭：overlay `on click[me is event.target] remove .is-open`

### 3.2 模式 B — 动态加载（HTMX 填充内容后打开）

表单内容依赖上下文（如「编辑某行」需先取该行数据）时，用空 modal + `hx-get` 加载，并在**目标容器（modal / drawer 本身）**上监听 `htmx:afterSettle` 来唤醒打开。

#### 为什么是 `afterSettle` 而不是 `afterRequest`

HTMX 一次请求的事件顺序：`afterRequest`（请求完成，响应**尚未 swap**）→ `beforeSwap` → swap → `afterSwap` → `afterSettle`（swap 完成且 DOM **已稳定**）。

| 事件 | 触发位置 | 内容状态 | 能否用来打开 modal |
|---|---|---|---|
| `htmx:afterRequest` | **trigger**（发起请求的按钮） | 响应还没 swap 进 modal，**内容未就位** | ❌ 打开会看到空壳 |
| `htmx:afterSettle` | **target**（modal 容器） | 内容已 swap 进来并稳定，**完整表单就位** | ✅ 此时打开才正确 |

所以动态 modal / drawer 的「打开」动作必须挂在 **target（容器）** 的 `afterSettle` 上 —— 等内容就位再唤醒。这正是「在目标 after settle 来唤醒打开」。

#### 完整事件流

```
① 触发按钮  hx-get=edit_path  hx-target="#edit-modal"  hx-swap="innerHTML"
            → 请求编辑表单 HTML，swap 进 #edit-modal
② #edit-modal 上 afterSettle 触发（内容已就位）
            → Hyperscript `on htmx:afterSettle add .is-open` → modal 显示
③ 用户填表 → form hx-post 提交
④ 提交成功 → 关闭 modal（见 §3.1 的 afterRequest 关闭）+ 广播事件刷新数据（见 §4）
```

#### 范本（`pages/bom_edit.rs:796`）

```rust
// ① 触发：加载编辑表单到 modal 容器（target=#bom-edit-modal，swap=innerHTML）
button hx-get=(edit_node_path) hx-target="#bom-edit-modal" hx-swap="innerHTML" { "编辑" }

// ② 空 modal 容器：初始隐藏，内容 settle 后 add .is-open 显示，背景点击关闭
div id="bom-edit-modal"
    class="fixed inset-0 z-[1000] grid place-items-center bg-[rgba(15,23,42,0.45)]
           backdrop-blur-sm opacity-0 pointer-events-none transition-opacity duration-200
           [&.is-open]:opacity-100 [&.is-open]:pointer-events-auto"
    _="on htmx:afterSettle add .is-open\non click[me is event.target] remove .is-open" {}
```

要点：

- **`afterSettle` 挂在 modal / drawer 容器（target）上，不是触发按钮**。Hyperscript 在 `_=` 里用字面 `\n` 分隔多条语句（`add .is-open` 与背景关闭）。
- **显隐靠 UnoCSS 状态 variant**：`opacity-0 pointer-events-none` 初始隐藏 + `[&.is-open]:opacity-100 [&.is-open]:pointer-events-auto` 加 `.is-open` 即显示，**无需 JS 操作 display**。
- **打开时内容已是完整表单**（核心原则：端点返回完整片段），可直接交互、提交。
- **Drawer 完全同理** —— 把容器换成 drawer 外壳，`afterSettle` 唤醒逻辑不变。
- **与 §3.1 的区别**：§3.1 静态 modal 表单提交后用 trigger 上的 `afterRequest` **关闭**（内容本就在，无需等 settle）；§3.2 动态 modal 用 target 上的 `afterSettle` **打开**（要等内容 settle）。一开一关，事件不同。

### 3.3 模式 C — 自定义结构（特殊布局）

复杂布局（搜索 + 结果列表、多分区）手写 `modal-overlay`：

```rust
div id="bom-add-modal"
    class="modal-overlay fixed inset-0 z-[1000] grid place-items-center
           bg-[rgba(15,23,42,0.45)] backdrop-blur-sm opacity-0 pointer-events-none
           transition-opacity duration-200
           [&.is-open]:opacity-100 [&.open]:pointer-events-auto"
    _="on click[me is event.target] remove .is-open" {
    div class="modal bg-bg rounded-xl w-[680px] max-h-[85vh] flex flex-col overflow-hidden shadow-xl" {
        // header / body / footer
    }
}
```

范本：`pages/bom_edit.rs:727`。

### 3.4 Modal Hyperscript 踩坑（已收编）

Modal 内的 Hyperscript 有几个高频陷阱：

- **`halt the event` 会屏蔽 checkbox / input** —— modal 内层用 `halt the event` 阻止冒泡时，其 `preventDefault` 会**连带阻止 checkbox toggle / input 输入**。防背景误关改用事件过滤器 `on click[me is event.target]`，不要用 `halt`。
- **`halt the event` 在 `<a href>` 上会阻止跳转** —— 行内链接要阻止冒泡，用内联 JS `js(event) event.stopPropagation() end`，不要用 `halt`。
- **`put <val> into <input>` 静默失败** —— `put` 设 input 的 `innerHTML` 无效（input 没有 children）。填 input 值用 `set #id's value to <val>`。
- **关闭最近 overlay** —— `remove .is-open from closest .modal-overlay`，注意 `closest` 必须用 query 语法（`.modal-overlay` 或 `<div/>`），不能裸写标签名。
- **form/容器的 `htmx:afterRequest` 监听被子元素 hx-\* 冒泡误触发** —— form 上挂 `on 'htmx:afterRequest'[...] remove .open`（提交成功后关 drawer），form 内子元素（如 `<a hx-get>` 拉详情弹窗、`<button hx-get>`）发起的请求也会冒泡到 form，**误触发关闭**。`htmx:afterRequest` 是 bubbles 的 CustomEvent，从 trigger 元素冒泡到祖先，form 监听它就会被波及。**守卫加 `detail.elt is me`**：
  ```rust
  // ✗ 子元素的 hx-get 也会触发，误关 drawer
  _="on 'htmx:afterRequest'[detail.xhr.status < 400] remove .open from #create-plan-overlay"
  // ✓ 只有 form 自身（hx-post）发起的请求才关；子元素 detail.elt ≠ form 被过滤
  _="on 'htmx:afterRequest'[detail.xhr.status < 400 and detail.elt is me] remove .open from #create-plan-overlay"
  ```
  真实案例：`mes_work_center.rs` 创建计划 drawer 内订单号 `<a hx-get>` 拉订单详情 modal，结果点击订单号把 drawer 也关了（afterRequest 冒泡到 form）。同理 report/release drawer 内若有 hx-get 子元素也要防。

完整 Hyperscript 命令速查见 [`abt-web/CLAUDE.md`](../../abt-web/CLAUDE.md) 的 Hyperscript 参考手册。

---

## 4. HX-Trigger 多组件联动（核心范式）

一个写操作需要刷新多个独立组件时，**避免写「聚合刷新路由」**，改用事件解耦。这是 ABT 工作台类页面的核心模式。

### 4.1 模式：写操作返回事件名，data div 监听自刷新

**关键决策：更新 / 修改 / 删除等写操作 handler 成功后，不返回 HTML 片段，而是返回一个事件名称（`HX-Trigger` 响应头）+ 空 HTML 体。** 需要刷新的数据容器（data div）各自声明监听该事件，事件触发时自动重新拉取自己 —— 主动方（写操作）与被动方（data div）解耦，互不感知。

```
① 主动：写操作 POST（如 /wo/{id}/release）→ 业务成功
② 服务端：响应头 HX-Trigger: "woChanged"，响应体为空（不返回片段）
③ 被动：data div 声明 hx-trigger="woChanged from:body"
        + hx-get（自己的端点）+ hx-select（选自己）
        → 事件来时自动重新加载自己
```

#### 写 handler ↔ data div 标准配对

```rust
// ── 写操作 handler：成功后只广播事件名，不返回片段 ──
pub async fn release_wo(/* ... */) -> Result<impl IntoResponse> {
    // ... 业务逻辑（事务包裹）...
    Ok(([("HX-Trigger", "woChanged")], Html(String::new())))  // 空 HTML + 事件名
}

// ── data div：监听事件，事件来时自我重新加载 ──
div id="wc-release-routings"
    hx-get=(WcReleaseDrawerPath { order_id }.to_string())  // 自己的端点（核心原则：返回完整片段）
    hx-target="this"                                        // 替换自己
    hx-select="#wc-release-routings"                        // 从响应里选自己
    hx-swap="outerHTML"
    hx-trigger="routingChanged from:body" {                 // 监听事件
    (render_release_routings(/* ... */))                    // 初始内容
}
```

范本：`pages/mes_work_center.rs`（模块注释 `:4-7` 完整描述了该范式；data div `:1187-1191`；写 handler 广播 `:1493 / 1548 / 1610`）。

要点：

- **写操作返回事件名，不是 HTML 片段** —— 这样一个写操作能同时唤醒任意多个 data div（工单下达后，工序区、摘要带、card 各自监听 `woChanged` 分别刷新），**无需写「聚合刷新路由」**。
- **data div 自我重新加载** —— `hx-get` 指向自己的端点、`hx-target="this"` + `hx-select="#自己"` 替换自己，正是核心原则的落地（一个端点返回完整片段，前端 `hx-select` 选取）。
- **`from:body`** —— 事件由 body 广播，任何 data div 都能收到，不依赖事件源是某个特定元素。
- **初始加载 + 事件刷新共存** —— card 类容器常用 `hx-trigger="load, woChanged from:body"`：`load` 首次拉取、`woChanged from:body` 后续写操作后刷新，逗号合并在一条 `hx-trigger` 里。
- **事件名约定** —— 用 `<对象>Changed` / `<对象>Updated`（如 `woChanged`、`poChanged`、`permUpdated`、`nodeUpdated`），见 §4.2 真实案例。

### 4.2 真实案例

| 广播事件 | 出处 | 监听方 |
|---|---|---|
| `nodeUpdated` | `pages/bom_edit.rs:302,326` | BOM 节点树 |
| `batchChanged` / `reportChanged` / `requisitionChanged` / `receiptChanged` | `pages/mes_order_detail.rs:320,418,502,570` | 批次 disclosure + 摘要带（`mes_order_detail.rs:930` 同时监听三个事件） |
| `woChanged` | `pages/mes_work_center.rs:1493,1548,1610` | 工单 card |
| `poChanged` / `reconChanged` | `pages/purchase_work_center.rs:442,478` | PO card / 对账 card |
| `routingSelected` / `routingChanged` | `pages/mes_work_center.rs:1192` / `mes_order_detail.rs:2091` | 工序编辑区 |
| `permUpdated` | `pages/permission_config.rs:295` | 权限面板 + 统计条（oob `#stats-bar,#role-list`） |

> **事件命名强约束**：事件名 = `<资源>Changed`（如 `woChanged`、`poChanged`、`permUpdated`），**全局唯一、避免重名**。事件由 body 广播、任意 data div 可监听——一旦两个无关写操作撞名（都叫 `itemChanged`），会互相误触发对方数据区的刷新。新增事件前先 grep 全仓确认无重名。

### 4.3 JSON 多事件组合（刷新 + 关 Modal）

一个写操作常常要「刷新数据 + 关闭编辑 Modal」，用 JSON 响应头组合多个事件：

```rust
Ok(([("HX-Trigger", r#"{"rulesUpdated":"", "closeRuleModal":""}"#)], Html(String::new())))
```

- `rulesUpdated` → 数据区监听刷新
- `closeRuleModal` → Modal overlay 监听 `remove .is-open`

范本：`pages/purchase_approval_rules.rs:127,149`、`pages/product_list.rs:140`。

### 4.4 关键时序决策：`HX-Trigger` vs `HX-Trigger-After-Settle`

两者触发时机不同，选错会导致监听方拿到**空的 / 旧的 DOM**：

| 响应头 | 触发时机 | 适用场景 |
|---|---|---|
| `HX-Trigger` | swap **之前** | 目标元素已存在、监听方自取数据 |
| `HX-Trigger-After-Settle` | swap + settle **之后** | 监听方依赖**刚 swap 进来的新内容**（如对新行重编号、汇总、填库位） |

> **真实教训**（`pages/wms_stock_in_create.rs:298`）：PO 选择器填充明细后需要重编号 / 汇总 / 填库位，监听器要操作的是 swap 后的 `#po-cards` 明细行。若用标准 `HX-Trigger`，事件在 swap 前触发，`#po-cards` 尚空，监听器无对象可操作。必须用 `HX-Trigger-After-Settle`：
>
> ```rust
> Ok(([("HX-Trigger-After-Settle", r#"{"closePoPicker":"","poCardsUpdated":""}"#)], Html(html)))
> ```

**决策**：监听逻辑需要读取「这次响应刚渲染出来的 DOM」→ `HX-Trigger-After-Settle`；否则用 `HX-Trigger`。

### 4.5 Toast 提示

成功 / 失败 toast 统一走 `HX-Trigger: "showToast"`（`abt-web/src/toast.rs:77` 提供便捷函数），客户端监听后用 Notyf 渲染。

---

## 5. 表单提交后行为决策树

提交成功后做什么，按下表选：

| 目标 | 手段 | 出处 |
|---|---|---|
| **刷新局部（首选）** | 广播事件 `HX-Trigger` + 空 body | §4；新代码默认此方式 |
| 跳转整页（避免，仅必要场景） | 响应头 `HX-Redirect` | `pages/bom_create.rs:96`（既有 150+ 处，逐步迁移） |
| 刷新多个区域 | `hx-select-oob="#a,#b"` | `pages/permission_config.rs:572` |
| 刷新当前组件 | `hx-target="this"` + `hx-swap="outerHTML"` | 三原则 §1.1 |
| 仅关闭 Modal（不跳转） | `hx-swap="none"` + `on 'htmx:afterRequest' remove .is-open` | `components/modal.rs:21` |
| 跨模块刷新外部区域 | `hx-select-oob="#ext-id:outerHTML"` | `pages/wms_work_center.rs:757` |

### 5.1 防重复提交 — `hx-disabled-elt`

```rust
button hx-post=(path) hx-disabled-elt="this" { "下达" }       // 禁用自身
button hx-post=(path) hx-disabled-elt="#submit-btn" { ... }   // 禁用指定按钮
```

范本：`pages/mes_order_detail.rs:825`、`pages/wms_stock_in_create.rs:839`。

### 5.2 确认对话框 — `hx-confirm`

```rust
button hx-post=(path) hx-confirm="确定要发布此 BOM 吗？发布后将无法修改。" { "发布" }
// 动态文案
button hx-confirm=(format!("确认删除 BOM {}？", name)) { "删除" }
```

> 测试注意：agent-browser 测 `hx-confirm` 按钮时原生 confirm 框会阻塞，需临时移除该属性（见 §8）。

### 5.3 请求去重 — `hx-sync`

搜索框 / 自动补全等高频请求，用 `hx-sync` 取消未完成的前序请求：

```rust
input hx-get=(search_path) hx-trigger="keyup changed delay:300ms"
       hx-sync="this:replace" { ... }   // 新请求替换旧请求（默认）
       // 或 this:drop —— 旧请求进行中时丢弃新请求
```

范本：`pages/bom_edit.rs:755`（`this:replace`）、`pages/mes_demand_pool_create.rs:290`（`this:drop`）。

### 5.4 文件上传 — `hx-encoding`

```rust
form hx-post=(import_path) hx-encoding="multipart/form-data" { input type="file" ... }
```

范本：`components/import_modal.rs:61`。

### 5.5 行内编辑 — `hx-target="closest tr"`

删除 / 行内操作替换整行，用 `closest` 相对定位：

```rust
td button hx-post=(delete_path) hx-target="closest tr" hx-swap="outerHTML"
         hx-confirm="确认删除？" { "删除" }
```

范本：`pages/quotation_list.rs:340`。

### 5.6 校验失败与错误反馈

**兜底机制（非 form 写操作 / 未捕获错误）**：写操作失败时，`abt-web/src/errors.rs` 把 `DomainError` 统一映射成纯文本消息 + 4xx/5xx（`Validation` / `BusinessRule` / `Duplicate` → 400、`InsufficientStock` → 422、`ConcurrentConflict` → 409，见 `errors.rs:33-61`）。前端全局监听 `htmx:responseError` 读 `responseText` 弹 Notyf toast。HTMX 对 4xx/5xx 响应默认不 swap。**注意：form 校验失败不走此机制（toast 无法定位字段、用户感知差），必须用下文字段级回显。**

**强制规范（form 校验失败 → 字段级回显）**：**form 提交校验失败一律用此方式，禁止 toast** —— 让用户直接看到哪个字段错、错在哪。用**符合三原则**的范式——表单自替换 + 回填值 + 内联标注，**零外部 ID**：

```rust
// 表单：自身是替换边界（§1.1），不引用任何外部 ID
form hx-post=(save_path) hx-target="this" hx-swap="outerHTML" {
    // 字段：value 回填用户已输入的值；错误时加红框 + 错误文案
    input name="qty" value=(submitted.qty)
          class=(if qty_err { "border-danger" } else { "" });
    @if let Some(e) = qty_err {
        p class="text-danger text-xs" { (e) }
    }
    // ... 其它字段
    button type="submit" { "保存" }
}

// handler：成功 → 空 body + 事件（关 drawer / 刷新列表）；失败 → 返回【同一个 form】带错误标注。
//          两分支必须返回同类型 (headers, Html) —— impl IntoResponse 单一类型约束，否则编译不过。
//          失败也返回 200（不是 4xx）：让 htmx 正常 swap 重渲染 form；走 Err→4xx 纯文本会被全局 toast 吞掉。
match svc.save(ctx, input).await {
    Ok(_) => Ok(([("HX-Trigger", "xxxChanged")], Html(String::new()))),
    Err(e) => Ok(([("HX-Trigger", "")], Html(render_form(Some(&input), Some(&e))))),  // 回填 + 标错
}
```

要点：

- **表单 `hx-target="this"` 自替换** —— 失败时返回的表单替换旧表单，用户看到的就是带错误标注、值已回填的表单，**不需要 `HX-Retarget`**。
- **后端回填是关键** —— `outerHTML` 替换会重建节点，用户输入必须由 handler 从 `input` 重新回填进返回的表单（`value=(submitted.xxx)`），否则输入丢失。
- **handler 逻辑自洽** —— 成功 / 失败都返回完整片段（成功返回空体 + 事件，失败返回带错表单），不感知请求来源，符合核心原则。

> ⚠️ **不用 `HX-Retarget` / `HX-Reswap`**：它们要求后端在响应头里硬编码前端错误容器 ID（`HX-Retarget: #form-errors`），是**后端引用前端外部 ID**，违反「逻辑自洽 + 禁外部 ID」（见核心原则「ID 引用的粒度规则」）。表单自替换范式更干净，优先采用。
>
> **现状**：全仓 `HX-Retarget` / `HX-Reswap` 为 0 处；字段级回填表单已落地（见下方案例），其余写操作失败仍统一走 toast。新增「需要指明错误字段」的表单时，按本节「表单自替换 + 回填」范式实现。

**已落地案例 — 创建工单 drawer（`mes_work_center.rs::create_plan` + `render_create_plan_drawer_body`）**：

- form 自带 `hx-target="this" hx-swap="outerHTML"`，校验失败时后端返回**重渲染的同一个 form**（带 `errors`），htmx 原地替换 → 字段标红 + 内联错误，用户输入由 handler 回填。
- **成功/失败分流**：复用 `wc_routing_edit` 范式 —— 成功返回空 body + `HX-Trigger`，失败返回非空 form。form 上 `_="on 'htmx:afterRequest'[detail.xhr.responseText.length == 0 and detail.elt is me] remove .open from #create-plan-overlay"`：仅当响应体为空（成功）才关 drawer；非空（失败重渲染）保持打开。
- **handler 不返回 Err**：入参校验失败不再 `Err(DomainError::validation)`（那会走全局 toast），而是收集 `HashMap<&str, String>`（字段名→消息，`__all__` 键放非字段特定错误）后返回 `Ok(([("HX-Trigger","")], Html(form)))`。
- **字段标红**：`field_cls(field, errors)` 按字段是否在 errors 中切 `border-danger focus:border-danger` vs 常规边框；字段下方 `@if let Some(m) = errors.get(field)` 渲染 `<p class="text-danger text-xs">`；`__all__` 错误走顶部 `alert::alert_error`。

---

## 6. HTMX 属性速查（项目实际在用）

| 属性 | 用途 | 出处 |
|---|---|---|
| `hx-sync="this:replace/drop"` | 请求去重（取消前序 / 丢弃新请求） | `pages/bom_edit.rs:755` |
| `hx-disabled-elt` | 防重复提交 | `pages/mes_order_detail.rs:825` |
| `hx-confirm` | 确认对话框（含 `format!` 动态） | `pages/bom_detail.rs:274` |
| `hx-disinherit="hx-select"` | 子元素不继承父级 `hx-select` | `pages/mes_order_detail.rs:933` |
| `hx-encoding="multipart/form-data"` | 文件上传 | `components/import_modal.rs:61` |
| `hx-vals="js:{...}"` | JS 表达式计算值（如保持滚动位置） | `pages/permission_config.rs:573` |
| `hx-select-oob="#a,#b"` | 同时替换多个区域 | `pages/user_list.rs:398` |
| `hx-select-oob="#id:outerHTML"` | 跨模块外部区域替换 | `pages/wms_work_center.rs:757` |
| `hx-swap-oob="true"` | 手写 OOB 元素（响应体内嵌，见 §2.4 两种声明方式） | `pages/om_outsourcing_create.rs:372` |
| `hx-preserve` | 局部刷新时保留元素状态（输入值 / 折叠态） | `pages/fms_ap_ledger.rs:293-348` |
| 响应头 `HX-Redirect` | 成功跳转 | `pages/bom_create.rs:96`（150+ 处） |
| 响应头 `HX-Trigger` / `HX-Trigger-After-Settle` | 事件广播（见 §4 时序决策） | `pages/wms_stock_in_create.rs:298` |

---

## 7. 反模式检查清单

提交前端代码前自检：

- [ ] **`onclick` 残留** —— UI 操作改用 Hyperscript `_=`。已知例外：列表行整行跳转详情（`pages/customer_list.rs:293`，HTMX 无直接替代）、`components/detail.rs:28` 的 `switchDetailTab`（原生 JS tab 切换）。
- [ ] **硬编码 `#id` 作 `hx-target`** —— 改用 `this` / `closest <selector>`，让组件自包含（三原则 §1.1）。
- [ ] **`hx-push-url` 残留** —— 列表页禁用（§2.3）。
- [ ] **为局部刷新建独立 handler** —— 合并到单端点（§2.1）。
- [ ] **`hx-vals` 手拼 JSON** —— 字符串 / 枚举值用 `format!` 拼 `hx-vals` 会因引号破坏 JSON，改用 `vals_json()` / `serde_json::json!`（§1.2）。
- [ ] **Maud 双 `class=""` 陷阱** —— 同一元素写两个 `class="..."`，浏览器**只认第一个**，第二个静默丢失。合并为 `class="A B"`，每元素一个 class 属性。
- [ ] **绕过业务单据链直接写库存 / 状态** —— 写操作必须编排完整单据链 + 事务包裹，见 [`abt-web/CLAUDE.md`](../../abt-web/CLAUDE.md) 的数据访问约束。

---

## 8. 测试注意（agent-browser）

用 agent-browser 测 HTMX 页面时，原生 click 对 HTMX / Hyperscript 按钮不生效，需绕过：

- **触发 HTMX 按钮** —— 不要用 `click`，用 `eval "htmx.trigger('#el', 'click')"` 或 `htmx.trigger(el, 'submit')`。
- **`hx-confirm` 按钮阻塞** —— 原生 confirm 框卡住自动化，测试前临时移除 `hx-confirm` 属性。
- **native date / entity picker** —— 用 `eval` 设值，不要模拟输入。
- 详见 memory `reference-agent-browser-htmx-click`。

---

## 附录：文档债 / 待清理

- ~~`pages/purchase_work_center.rs` 的 `hx-push-url` 残留~~ —— **已于 2026-06-26 清理**（9 处全部删除，`cargo clippy` 通过）。组件层（`tabs.rs` / `pagination.rs`）与全部页面现已统一不含 `hx-push-url`，全仓 `abt-web/src` grep 0 残留。
- **`docs/solutions/` 目录** —— 根 `CLAUDE.md` 声称「记录历史问题解决方案」但目录实际不存在，属文档债。

---

## 关联文档

- [`abt-web/CLAUDE.md`](../../abt-web/CLAUDE.md) — 前端强约束入口（必读）、Constraints、Hyperscript 命令速查
- [`AGENTS.md`](../../AGENTS.md) — Web Frontend Patterns（英文通用约束、Surreal→Hyperscript 迁移对照表）
- [`docs/ui-specs/`](../ui-specs/) — 各模块 UI 规范
- [`docs/uml-design/`](../uml-design/) — 接口与模型设计文档（接口先行）
