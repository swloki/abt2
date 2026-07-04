---
description: "ABT Hyperscript 语法用法正文：心智模型、命令体系、项目高频模式、AI 易错点纠错（halt/set-vs-put/closest query/HTMX 事件名）、Surreal 迁移"
globs: ["abt-web/**/*.rs"]
---

# Hyperscript 语法与用法

> 本文档是 abt-web 前端 **Hyperscript 的系统性语法与用法正文**，解决「写 hyperscript 时生涩、不确定语法」的问题。[`abt-web/CLAUDE.md`](../../abt-web/CLAUDE.md) 保留命令速查入口，本文是其展开：心智模型 → 语法体系 → 项目高频模式 → **AI 易错点纠错**。
>
> 官方权威：[reference](https://hyperscript.org/reference/)（语法全集）、[patterns](https://hyperscript.org/patterns/)（模式目录）。Hyperscript 版本 `0.9.91`。所有示例为 Maud `html!` 宏写法，标注真实代码出处（`file:line`）。

---

## 0. 心智模型：把 hyperscript 当英语句子读

Hyperscript 不是「另一种 JS」，而是**声明式的、英语句子式的事件处理语言**，写在元素的 `_` 属性里。核心心法：

> **一句话 = 一个事件处理器：`on <事件> <做点什么>`。从左到右读成一句英语。**

```rust
// 读法：「on click, add the class is-open to #modal」→ 点击时给 #modal 加 .is-open
button _="on click add .is-open to #modal" { "打开" }
```

三条根本认知（生涩往往来自没建立这三条）：

1. **主语默认是 `me`（当前元素）**，命令的隐式目标也是 `me`。`add .x` = `add .x to me`；`remove me` = 删自己。
2. **句子用 `then` 串起来，用换行（Maud 里 `\n`）分隔多个 `on`**。一段 `_=` 可以有多个独立的事件监听。
3. **它能直接操作 DOM（class、显隐、属性、增删），但不是通用编程语言**。逻辑一复杂就退化为「难读的伪英语」——此时应 `call jsFn()` 委托给 `static/app.js`（见 §6.7）。

---

## 1. 语法骨架

一个完整 `_=` 由一到多条「事件处理器」组成，每条形如：

```
on <event>[<filter>]  <command> [then <command> ...]
```

| 组成 | 说明 | 示例 |
|---|---|---|
| `on <event>` | 监听什么事件 | `on click` / `on 'htmx:afterRequest'` |
| `[<filter>]` | 事件过滤器（可选），不满足则不执行 | `[me is event.target]` / `[event.key is 'Escape']` |
| `<command>` | 做什么 | `add .is-open to #modal` |
| `then <command>` | 顺序串联下一条命令 | `then reset me` |
| 多个 `on` | 用换行分隔（Maud 里字面 `\n`） | `on load ...\non click ...` |

### Maud 里 `_=` 的三种写法

```rust
// ① 字面字符串（最常见，内容固定）
_="on click[me is event.target] remove .is-open"

// ② format! 拼接（内容含 Rust 变量）
_=(format!("on click remove .is-open from #{}", modal_id))

// ③ 多语句：用字面 \n 分隔多个 on（hyperscript 用换行分句）
_="on htmx:afterSettle add .is-open\non click[me is event.target] remove .is-open" {}
```

> ⚠ `then` 是**同一句里**串联命令；`\n` 是**分隔多个独立 on 句**。两者不同：`on click A then B`（一句，A 后做 B）；`on click A\non click B`（两句，都响应 click，都会跑）。

范本：`components/modal.rs:14`、`components/entity_picker.rs:112`、`pages/bom_edit.rs:798`。

---

## 2. 事件监听（`on`）

### 2.1 基础事件

| 事件 | 触发时机 | 项目实例 |
|---|---|---|
| `click` | 点击 | `layout/header.rs:16` |
| `change` | 表单值变更 | `pages/fms_adjustment_create.rs:244` |
| `input` | 输入（每次按键） | `pages/fms_journal_create.rs:294` |
| `submit` | 表单提交 | （常配合 `trigger submit on #form`） |
| `keydown` | 按键 | `pages/department_list.rs:314` |
| `animationend` | CSS 动画结束 | `abt-web/src/toast.rs:164` |
| `load` | 元素载入后（常用于初始化） | `layout/page.rs:43` |

### 2.2 事件过滤器 `[条件]`（生涩高发区）

过滤器写在事件名后的 `[...]` 里，**不满足则整句不执行**。这是 hyperscript 最强大的特性之一，也是最容易写错的。

| 过滤器 | 含义 | 实例 |
|---|---|---|
| `[me is event.target]` | 只有点中元素**本身**（非子元素）才执行 → 背景关闭 | `components/modal.rs:14` |
| `[event.key is 'Escape']` | 按 ESC 才执行 | `pages/department_list.rs:314` |
| `[event.animationName is 'toast-in']` | 特定动画结束 | `abt-web/src/toast.rs:164` |
| `[not (event.target matches <button/>)]` | 点到的不是按钮（点卡片空白处才折叠） | `pages/wms_stock_in_create.rs:1197` |
| `[detail.xhr.status < 400]` | HTMX 请求成功（状态码 < 400） | `components/modal.rs:21` |

### 2.3 HTMX 事件（易错！必须单引号 + 驼峰）

监听 HTMX 发出的事件时，**事件名必须用单引号包起来，且用驼峰**：

```rust
// ✅ 正确：单引号 + 驼峰
_="on 'htmx:afterRequest'[detail.xhr.status < 400] remove .is-open from closest .modal-overlay then reset me"
_="on 'htmx:afterSettle' add .is-open"

// ❌ 错误：on htmx:after-request（没单引号、用了 kebab-case）→ 语法错误或不触发
```

- `'htmx:afterRequest'` → 触发在 **trigger 元素**（发起请求的元素），请求完成时
- `'htmx:afterSettle'` → 触发在 **target 元素**（swap 目标），swap + settle 后（见 [htmx-patterns.md §3.2](htmx-patterns.md#32-模式-b--动态加载htmx-填充内容后打开)）

范本：`components/modal.rs:21`、`pages/department_list.rs:342`、`pages/bom_edit.rs:798`。

### 2.4 `from`：监听别处的事件

| 写法 | 含义 |
|---|---|
| `on xxx from:body` | 监听 body 广播的 `xxx` 事件（配合 HTMX 的 `HX-Trigger`，见 [htmx-patterns.md §4](htmx-patterns.md#41-模式写操作返回事件名data-div-监听自刷新)） |
| `on click from elsewhere` | 点击元素**外部**时触发 → click-away 关闭 dropdown |

---

## 3. 命令体系（按用途分组）

### 3.1 class 操作

| 命令 | 用途 | 项目实例 |
|---|---|---|
| `add .cls to <target>` | 加 class | `add .is-open to #modal`（`pages/bom_edit.rs:628`） |
| `remove .cls from <target>` | 删 class | `remove .is-open from closest .modal-overlay`（`components/modal.rs:21`） |
| `toggle .cls on <target>` | 切换 class | `toggle .open on closest <tr/>`（`pages/mes_order_list.rs:327`） |
| `take .cls from <set>` | **抢占**：从同组所有元素移除该 class，加给自己 → Tab 高亮 | `take .active from .rail-item`（`layout/sidebar.rs:568`） |

> `take` 是 Tab 切换的灵魂：一句 `take .active from .rail-item` 完成「清掉其他 tab 的 active、给自己加 active」。

### 3.2 DOM 寻址（query 语法是关键）

寻址元素有三类引用：

| 引用 | 写法 | 示例 |
|---|---|---|
| id | `#id` | `#modal` / `#password` |
| class | `.cls` | `.active` / `.modal-overlay` |
| **query**（标签/属性/伪类） | `<.../>` | `<div/>` / `<form/>` / `<input[name='x']/>` / `<:focused/>` |

相对定位（基于 `me`）：

| 表达式 | 含义 | 项目实例 |
|---|---|---|
| `closest <selector/>` | 最近祖先（**必须 query 语法**） | `closest <form/>`（`pages/bom_detail.rs:835`）、`closest <table/>`（`pages/mes_demand_pool.rs:594`） |
| `next <selector/>` | 下一兄弟 | `next <div/>`（`pages/bom_detail.rs:600`）、`next .cat-dropdown`（`components/category_select.rs:39`） |
| `previous <selector/>` | 上一兄弟 | `previous <input/>`（`pages/user_create.rs:245`） |
| `first <selector/> in <scope>` | 范围内第一个 | `first <input/> in next .cat-dropdown`（`components/category_select.rs:39`） |

> ⚠ `closest` / `next` / `previous` **必须用 query 语法** `<tag/>` 或 `.cls`，**不能裸写标签名** `closest form`（会报错）。详见 §6.3。

### 3.3 显示 / 隐藏

| 命令 | 用途 | 实例 |
|---|---|---|
| `show <target>` | 显示 | `show next .cat-dropdown`（`components/category_select.rs:39`） |
| `hide <target>` | 隐藏 | `hide me`（`components/confirm_dialog.rs:25`） |
| `show <target> when <cond>` | 条件显示（官方 Filter by Search 模式） | `show me when my @data-name contains theSearch` |
| `hide <target> when <cond>` | 条件隐藏 | 同上 |

> 项目里显隐**多数用 class + CSS**（`toggle .is-open` + UnoCSS `[&.is-open]:` variant），`show/hide` 用得少（`display` 直接操作）。两种都可，class 方式更契合项目的 UnoCSS 状态 variant 体系。

### 3.4 表单与属性

| 命令 | 用途 | 项目实例 |
|---|---|---|
| `set <ref>'s value to X` | **设 input/select 的值**（推荐） | `set #source_sales_order_id's value to ''`（`pages/mes_order_create.rs:126`） |
| `put X into <ref>` | 写入（input 设 value 不可靠，见 §6.2；设 `<span>` 的 innerHTML 用这个） | `put '' into #cp-display's innerHTML`（`pages/fms_journal_create.rs:406`） |
| `set <ref>'s <attr> to X` | 设属性 | `set #password's type to 'text'`（`pages/login.rs:238`） |
| `reset <form>` | 重置表单 | `reset me` / `reset #role-assign-form`（`pages/user_detail.rs:751`） |
| `trigger submit on <form>` | 触发提交 | `trigger submit on closest <form/>`（`pages/bom_detail.rs:835`） |
| `trigger <event> on <target>` | 触发自定义事件 | `trigger poCardsUpdated on body`（`pages/wms_stock_in_create.rs:1207`） |

### 3.5 流程控制

| 命令 | 用途 | 实例 |
|---|---|---|
| `if <cond> then ... else ... end` | 条件 | `if next .cat-dropdown's style's display is 'none' then show ... else hide ...`（`components/category_select.rs:39`） |
| `wait <time>` | 等待 | `wait 3.5s then add .toast-dismiss`（`abt-web/src/toast.rs:164`） |
| `settle` | 等 CSS transition 结束 | `add .fade-out then settle`（官方 Fade & Remove 模式） |
| `exit` | 提前退出本句 | `if x is null exit` |
| `repeat for x in ...` | 循环 | （复杂循环建议 `call jsFn()`） |

### 3.6 删除与导航

| 命令 | 用途 | 项目实例 |
|---|---|---|
| `remove <target>` | 删除元素 | `remove closest .po-card`（`pages/wms_stock_in_create.rs:1207`）、`remove me` |
| `call jsFn()` / `get jsExpr` | 执行 JS | `call toggleAllDemands(me, closest <table/>)`（`pages/mes_demand_pool.rs:594`） |
| `go to <url>` | 跳转（少用，跳转一般走 HTMX `HX-Redirect`） | — |

---

## 4. Magic Values 与表达式

### 4.1 Magic Values（句中自带变量）

| 值 | 含义 |
|---|---|
| `me` / `my` | 当前元素（带 `_=` 的那个） |
| `it` / `result` | 上一条命令的结果（如 `fetch ... then put it into ...`） |
| `you` / `yourself` | `tell` 切换的目标元素 |
| `event` / `target` / `detail` / `sender` | 事件对象及其字段 |
| `body` | document body |

### 4.2 表达式语法

| 类别 | 写法 | 示例 |
|---|---|---|
| 链式取属性（possessive） | `a's b's c` | `next .cat-dropdown's style's display`、`my @data-name`（`@` = 属性） |
| 比较 | `is` / `is not` / `matches` / `<` `>` `<=` `>=` | `event.target matches <button/>`、`detail.xhr.status < 400` |
| 逻辑 | `and` / `or` / `no` | `no element.children` |
| 转换 | `<val> as <Type>` | `"10" as Int`、表单 `as Values` |
| 字面量 | JS 风格 | `1` / `3.14` / `true` / `null` / `"str"` / `'str'` / `[1,2,3]` / `{a:1}` |

---

## 5. 项目高频模式（ground 真实用法）

### 5.1 Modal / Drawer 开关

```rust
// 打开
_="on click add .is-open to #bom-add-modal then call bomLoadProducts()"          // pages/bom_edit.rs:628
// 关闭 + 清空
_="on click remove .is-open from #bom-edit-modal then empty #bom-edit-modal"     // pages/bom_edit.rs:460
// 背景点击关闭（只有点 overlay 本身才关）
_="on click[me is event.target] remove .is-open"                                  // components/modal.rs:14
// 动态内容加载后打开（内容 settle 后）
_="on htmx:afterSettle add .is-open\non click[me is event.target] remove .is-open" // pages/bom_edit.rs:798
// 提交成功后关闭 + 重置表单
_="on 'htmx:afterRequest'[detail.xhr.status < 400] remove .is-open from closest .modal-overlay then reset me" // components/modal.rs:21
```

### 5.2 Tab / 导航高亮（`take`）

```rust
_="on click take .active from .rail-item"      // layout/sidebar.rs:568
_="on click take .cat-active from .cat-row"    // pages/category_list.rs:424
```

### 5.3 折叠 / 展开行

```rust
_="on click toggle .open on closest <tr/>"                                          // pages/mes_order_list.rs:327
_="on click[not (event.target matches <button/>)] toggle .is-collapsed on closest .po-card" // pages/wms_stock_in_create.rs:1197
```

### 5.4 键盘 ESC 关闭

```rust
_="on keydown[event.key is 'Escape'] remove .open from #deptDrawer"   // pages/department_list.rs:314
```

### 5.5 Toast 动画（`animationend` 过滤 + `wait`）

```rust
_="on animationend[event.animationName is 'toast-in'] wait 3.5s then add .toast-dismiss
   on animationend[event.animationName is 'toast-out'] remove me"     // abt-web/src/toast.rs:164
```

### 5.6 状态持久化（`localStorage` + `on load`）

```rust
// 折叠侧边栏并持久化
_="on click toggle .sidebar-collapsed on .app-shell then if .app-shell matches .sidebar-collapsed call localStorage.setItem('sidebar-collapsed','true') else call localStorage.removeItem('sidebar-collapsed')" // layout/sidebar.rs:585
// 载入时恢复
_="on load if localStorage.getItem('sidebar-collapsed') is 'true' add .sidebar-collapsed"     // layout/page.rs:43
```

### 5.7 表单桥接（清空 + 触发）

```rust
// 清空显示框 + hidden input
_="on change put '' into #cp-display's innerHTML then put '' into #cp-id's value"   // pages/fms_journal_create.rs:406
// 触发最近表单提交
_="on click halt the event then trigger submit on closest <form/>"                   // pages/bom_detail.rs:835
```

### 5.8 调 JS（复杂逻辑委托 `app.js`）

```rust
_="on input call cjCalcCny()"                                          // pages/fms_journal_create.rs:294
_="on change call toggleAllDemands(me, closest <table/>)"              // pages/mes_demand_pool.rs:594
_="on click call addSplitRow(me)"                                      // pages/mes_work_center.rs:1150
```

### 5.9 密码显隐（改属性）

```rust
_="on click toggle .pw-visible on closest <div/> then if (closest <div/>) matches .pw-visible set #password's type to 'text' else set #password's type to 'password'" // pages/login.rs:238
```

---

## 6. AI 易错点纠错（核心，生涩之源）

这是本文档最重要的部分。下面每条都是高频踩坑，**写之前先看这里**。

### 6.1 `halt` 副作用强，不要当「防冒泡」通用手段

`halt the event` = `preventDefault()` + `stopPropagation()` **两者一起**。

| 场景 | 正确做法 |
|---|---|
| 搜索框隔离事件（不要冒泡触发父级） | `on keyup halt on change halt on input halt`（`components/customer_search.rs:220`）—— 搜索框内合法 |
| disclosure 按钮阻止冒泡 | `on click halt the event then ...`（`components/disclosure.rs:99`）—— 非链接元素合法 |
| `<a href>` 上阻止冒泡 | ❌ 禁用 halt（会吞掉跳转）。改用 `js(event) event.stopPropagation() end` |
| modal 内层防背景误关 | ❌ 禁用 halt（`preventDefault` 会屏蔽内部 checkbox/input）。改用背景层 `[me is event.target]` |
| submit 按钮提交 | ❌ 禁用 halt（`preventDefault` 阻止提交，见 `pages/purchase_approval_rules.rs:443` 注释） |

**原则**：halt 只在「确实要阻止默认行为 + 冒泡」时用；只想阻止冒泡 → `js(event) event.stopPropagation() end`；只想防背景误关 → `[me is event.target]`。

### 6.2 填 `<input>` 的值用 `set`，不要用 `put into`

```rust
// ✅ 填 input（hidden/text/readonly 都算）—— 一定生效
set #shift's value to '1'
set <.product-search-input/>'s value to ''                         // pages/bom_edit.rs:782

// ❌ put into <input> —— 静默失败（落到 innerHTML，input 无 children）
put '1' into #shift     // 框里没填进去，但后续 trigger change 照常触发，表现诡异
```

`put into` 只用于设 `<span>`/`<div>` 的 **innerHTML**（如清空显示框 `put '' into #cp-display's innerHTML`）。详见 `pages/fms_journal_create.rs:406`。

### 6.3 `closest` / `next` / `previous` 必须 query 语法

```rust
// ✅ query 语法
closest <form/>          // pages/bom_detail.rs:835
closest <table/>         // pages/mes_demand_pool.rs:594
closest .modal-overlay   // class 也可以
next <div/>              // pages/bom_detail.rs:600

// ❌ 裸标签名 —— 语法错误
closest form
next div
```

### 6.4 HTMX 事件名：单引号 + 驼峰

```rust
// ✅
on 'htmx:afterRequest'
on 'htmx:afterSettle'

// ❌ 没单引号 / kebab-case
on htmx:afterRequest      // 会被当成普通事件名解析，不触发
on 'htmx:after-request'   // 名字错了，不触发
```

### 6.5 `then` vs `\n`：串联命令 vs 分隔事件

- `on click A then B` → **一句**，click 后顺序执行 A、B
- `on click A\non keydown[event.key is 'Escape'] B` → **两句**，分别响应 click 和 ESC

混用典型（一个 modal 容器：settle 时打开 + 背景点击关闭）：

```rust
_="on htmx:afterSettle add .is-open\non click[me is event.target] remove .is-open"   // pages/bom_edit.rs:798
```

### 6.6 Maud 里 `_` 属性值的边界

- 内容**固定** → 字面串 `_="on click ..."`
- 内容含 **Rust 变量** → `format!`: `_=(format!("on click remove .is-open from #{}", id))`（`components/entity_picker.rs:112`）
- **多个独立 on** → 字面 `\n` 分隔（不是 `then`）
- 千万**别**在 Maud 里写 `script { "..." }`（会被 HTML 转义），复杂原生 JS 用 `maud::PreEscaped` 或直接 `call jsFn()`

### 6.7 `_` 里别写大段逻辑 → `call jsFn()`

Hyperscript 是声明式的，**逻辑越复杂越难读**。判断标准：

| 该用 hyperscript | 该用 `call jsFn()`（放 `static/app.js`） |
|---|---|
| 加/删/切 class、显隐、开关 modal | 行项目计算、金额汇总 |
| 设个值、清空、触发提交 | checkbox 全选/反选、拖拽排序 |
| 简单 `if ... then ...` | 多步 DOM 收集、循环、异步 |
| 1-2 个 `then` 串联 | 超过 3 个 `then` 或嵌套 `if` |

复杂逻辑写进 `app.js` 的全局函数，hyperscript 只负责「`on 事件 call fn(args)`」触发：

```rust
// ✅ 复杂逻辑放 JS，hyperscript 只触发
_="on submit call collectItems() then put it into #items_json"   // 收集行项目
_="on change call toggleAllDemands(me, closest <table/>)"        // 全选
```

---

## 7. 从 `onclick` / Surreal.js 迁移对照

| 旧写法 | Hyperscript `_=` |
|---|---|
| `onclick="me('#m').classAdd('is-open')"` | `on click add .is-open to #m` |
| `onclick="me('#m').classRemove('is-open')"` | `on click remove .is-open from #m` |
| `onclick="me(this).closest('.overlay').classRemove('is-open')"` | `on click remove .is-open from closest .overlay` |
| `onclick="if(event.target===this) close()"`（背景关） | `on click[me is event.target] remove .is-open` |
| `onclick="me(this).siblings().classRemove('active')"`（tab） | `on click take .active from .tab` |
| `onkeydown="if(event.key==='Escape') close()"` | `on keydown[event.key is 'Escape'] remove .open from #m` |
| `event.stopPropagation()`（在 `<a>` 上） | `on click js(event) event.stopPropagation() end` |

---

## 8. 边界：何时用 Hyperscript / HTMX / 独立 JS

见 [htmx-patterns.md §0 范式总览](htmx-patterns.md#0-范式总览三层技术分工) 的三层分工表。一句话：

- **服务端状态**（提交、搜索、分页、写操作）→ **HTMX**
- **纯前端 UI 状态**（modal 显隐、tab、dropdown、class 切换）→ **Hyperscript `_=`**
- **复杂前端逻辑**（拖拽、计算、持久化）→ **独立 JS**（`app.js`），hyperscript 用 `call` 触发

> 红线：纯前端 UI **禁止**通过 HTMX 发请求；Hyperscript **禁止**用 `fetch()` 调服务端（用 HTMX）。

---

## 9. 响应式特性（live / when / bind）— 值变化自动联动

hyperscript 的 **Features 层**（写在 `_=` 里的顶层特性，区别于 `on` 事件处理器）有三个响应式特性，适合「值变化时自动联动」，比 `on input ... then set ...` 链式更声明式。源自官方 [reference §Features](https://hyperscript.org/reference/#features)。

> ⚠ **版本**：`live`/`when`/`bind` 是 hyperscript 较新特性（0.9.x 后期加入核心）。ABT 加载 `static/hyperscript.min.js`（0.9.91），使用前**先在小元素实测**——若不生效说明当前版本未编译这些特性，回退到 `on input call fn()` 经典范式（§5.8）。本项目优先 HTMX + 经典 Hyperscript，响应式仅在前述模式别扭时用。

### 9.1 `live` — 派生值自动重算

依赖变化时自动重跑命令，适合「总价 = 单价 × 数量」这类派生值。`live` 块里读到的 `$var`（带 `$` 前缀的元素/全局变量）被追踪，任一变化触发整块重跑：

```rust
// 总价随单价/数量自动重算（$price/$qty 变化 → live 块重跑）
span _="live set my innerHTML to ($price * $qty as Number)" { "0" }
```

### 9.2 `when` — 值变化触发副作用

跟 `live` 一样追踪依赖，但 `when` 强调「变化时做副作用」（异步 / 多命令 / 触发事件），不只是重算值：

```rust
// 库存 ≤ 0 时切红框
input _="when $qty changes if $qty <= 0 then add .border-danger to me end"
```

### 9.3 `bind` — 两值双向同步

任一侧变化，另一侧自动跟上。适合「checkbox ↔ 主题 class」「input ↔ 显示值」这类镜像关系：

```rust
// 复选框勾选 ↔ body 加 .dark（双向）
input type="checkbox" _="bind my checked and .dark on body"
```

### 9.4 何时用响应式 vs 经典 `on`

| 场景 | 推荐 |
|---|---|
| 派生值（总价=单价×数量） | `live` 声明式；行项目计算复杂仍用 `app.js`（§6.7） |
| 值变化的副作用 | `when` 或经典 `on change` |
| 两值镜像（checkbox↔class） | `bind` |
| 涉及服务端状态 | 仍用 HTMX（响应式只管前端） |

> 红线不变：响应式是**纯前端**联动，涉及服务端状态必走 HTMX（§8）。

---

## 附录 A：命令速查（完整）

**项目高频**（§3 / §5 详述）：

| 命令 | 用途 |
|---|---|
| `add` / `remove` / `toggle` / `take` | class 增删切换抢占 |
| `show` / `hide` | 显示隐藏（含 `show ... when` 条件） |
| `put <val> into <target>` | 写入（input 用 `set`，见 §6.2） |
| `set <target> to <val>` / `set <ref>'s <attr> to <val>` | 设变量 / 属性 / input value |
| `reset <form>` | 重置表单 |
| `remove <target>` | 删除元素 |
| `call <js>` / `get <js>` | 执行 JS 表达式 |
| `send` / `trigger <event> to/on <target>` | 触发事件 |
| `if ... then ... else ... end` | 条件 |
| `repeat for x in ...` / `break` / `continue` | 循环 |
| `wait <time>` / `settle` / `transition <prop> to <val>` | 等待 / 等动画 / 过渡 |
| `halt [the event]` | 阻止事件（默认 + 冒泡，见 §6.1） |
| `exit` / `return <val>` / `throw <msg>` | 退出 / 返回 / 抛错 |
| `log <expr>` / `beep` / `breakpoint` | 调试（`breakpoint` = DevTools 断点） |
| `tell <target>` | 切换隐式目标（`you`） |
| `make a <Type> from ...` / `measure <el>` | 构造 / 测量 |

**补充命令**（reference 收录，项目暂未用，按需取）：

| 命令 | 用途 | 示例 |
|---|---|---|
| `append <val> to <x>` | 追加到字符串/数组/元素 | `append ",end" to myStr` |
| `increment <var>` / `decrement <var>` | 增/减变量（默认步长 1） | `increment counter` |
| `default <var> to <val>` | 仅未定义时设默认 | `default x to 0` |
| `empty <el>` / `clear <input>` | 清空内容/输入值/集合 | `empty #results`、`clear #search` |
| `focus <el>` / `blur <el>` | 聚焦 / 失焦 | `focus #search-input` |
| `scroll to/by ...` | 滚动 | `scroll to #section smoothly` |
| `morph <el> to <content>` | morph DOM 保留身份 | `morph #target to newHtml` |
| `pick first N of / match of` | 选取 | `pick first 3 of arr` |
| `render <tpl> with <data>` | 渲染模板 | `render #tpl with items: data` |
| `swap <a> with <b>` | 交换两值 | `swap x with y` |
| `open` / `close` | 打开/关闭 dialog/popover/fullscreen | `open #dialog`、`close fullscreen` |
| `select <input>` | 选中文本 | `select #search-input` |
| `ask` / `answer` | prompt / alert·confirm 对话框 | `ask "名字?"`（结果在 `it`） |
| `start a view transition` | View Transition API（很新，实测） | `start a view transition using "fade" ... end` |
| `fetch <url>` | ⚠ **ABT 禁用**（提交走 HTMX，§8） | — |

## 附录 B：Magic Values 速查

`me`（当前元素）· `you`（tell 目标）· `it`/`result`（上次结果）· `event`/`target`/`detail`/`sender`（事件）· `body` · `cookies[...]` · `clipboard`（系统剪贴板，读写）· `selection`（当前选中文本）

## 附录 C：表达式与操作符速查（含集合表达式）

### 集合表达式（数组/字符串）

| 表达式 | 用途 | 示例 |
|---|---|---|
| `where` | 过滤 | `items where its active` |
| `sorted by` | 排序 | `items sorted by its name descending` |
| `mapped to` | 投影 | `items mapped to its id` |
| `split by` | 字符串拆数组 | `"a,b" split by ","` |
| `joined by` | 数组接字符串 | `items joined by ", "` |
| `pick first N of` | 取前 N | `pick first 3 of arr` |

### 操作符

| 操作符 | 用途 | 示例 |
|---|---|---|
| `no` | 空检查 | `no element.children` |
| `some` | 存在检查（`no` 的反） | `some <.results/>` |
| `in` | 包含 | `"foo" in myArray` |
| `starts with` / `ends with` | 前缀 / 后缀 | `url starts with "https"` |
| `is between X and Y` | 范围（闭区间） | `qty is between 1 and 10` |
| `ignoring case` | 大小写无关（修饰符） | `x contains "hi" ignoring case` |
| `precedes` / `follows` | DOM 文档顺序 | `#a precedes #b` |
| `<val> as <Type>` | 类型转换 | `"10" as Int`、表单 `as Values` |
| `<val> \| <conv>` | pipe 链式转换 | `x as Values \| JSONString` |
| `x:String` / `x:Int!` | 类型断言（`!` 非空，不匹配抛错） | `event.detail:String` |

### 字面量补充（reference 新增）

| 字面量 | 示例 |
|---|---|
| 模板字符串（插值） | `"Hello ${name}"` |
| 时间 | `200ms` / `2s` |
| CSS 单位 | `10px` / `2em` / `50%` |
| block literal（匿名函数） | `\ x -> x * x` |

## 附录 D：扩展特性（需扩展脚本，ABT 暂不可用）

以下 Features 需额外的 hyperscript 扩展脚本（核心 `hyperscript.min.js` 之外）。ABT 仅加载核心，**未加载扩展**，故不可用；需要时先在 `layout/page.rs` 加载对应扩展脚本：

| 特性 | 用途 | ABT 是否需要 |
|---|---|---|
| `components` | 自定义元素 + 响应式模板 | ❌ 走 Maud SSR + HTMX，不需要 |
| `socket` | WebSocket 实时 | ⚠ 需服务端推送时考虑（目前 HTMX 轮询/事件够用） |
| `eventsource` | SSE 服务端推送 | ⚠ 同上 |
| `worker` | Web Worker 后台计算 | ⚠ 重计算时（目前无需求） |
| `intercept` | Service Worker 缓存/离线 | ❌ PWA 离线（目前无需求） |

> 即便加载扩展，`fetch` 命令仍**禁用于提交表单**（ABT 红线：用 HTMX，见 §8）。

## 关联文档

- [`htmx-patterns.md`](htmx-patterns.md) — HTMX 交互范式（服务端状态层、HX-Trigger、Modal 动态加载）
- [`abt-web/CLAUDE.md`](../../abt-web/CLAUDE.md) — 前端强约束入口、Hyperscript 命令速查
- [`AGENTS.md`](../../AGENTS.md) — Surreal→Hyperscript 迁移对照表（英文）
- 官方：[reference](https://hyperscript.org/reference/) · [patterns](https://hyperscript.org/patterns/)
