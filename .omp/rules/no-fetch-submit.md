---
description: "禁止 fetch() 调服务端，ABT 服务端交互一律走 HTMX hx-post / hx-get"
condition: "fetch\\s*\\(\\s*['\"]"
scope: "tool:edit(*.rs), tool:write(*.rs)"
---

你刚写了 `fetch('...')`。ABT 前端禁止用 `fetch()` 调服务端：

- **服务端状态交互**（表单提交、分页、搜索、状态流转）→ **HTMX** `hx-post` / `hx-get`（Maud SSR 渲染响应）
- **纯前端 UI**（modal 显隐、tab 切换、class 切换）→ **Hyperscript** `_="on click ..."`，不发请求

正确写法：

```rust
// 表单提交
form hx-post=(path) hx-target="#xxx" hx-swap="outerHTML" { ... }

// 局部刷新
a hx-get=(path) hx-target="#data-card" hx-select="#data-card" { "..." }
```

**唯一例外**：纯静态资源读取（`fetch('/static/xxx.json')`）允许，但 ABT 项目中极少出现。如果是这种场景，明确加上 `// allow: static resource` 注释绕过本规则。

依据：`AGENTS.md` "JS 与交互" 红线、`.omp/rules/htmx-patterns.md` §0 三层技术分工。
