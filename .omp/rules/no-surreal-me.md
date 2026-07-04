---
description: "禁止 Surreal.js me() / me(this) / hs* 函数，已废弃，改用 Hyperscript _="
condition: "me\\s*\\(\\s*(this\\s*\\)|['\"]#|null\\s*\\))"
scope: "tool:edit(*.rs), tool:write(*.rs)"
---

你刚写了 Surreal.js 的 `me(...)`。Surreal.js 在 ABT 项目里已**完全废弃**，所有 `me()` / `hsAdd` / `hsRemove` / `hsToggle` 等兼容函数均已删除或即将删除。

**迁移对照**（`.omp/rules/hyperscript-patterns.md` §7 完整表）：

| 旧 (Surreal / hs*) | 新 (Hyperscript `_*=`) |
|---|---|
| `onclick="me('#m').classAdd('is-open')"` / `hsAdd(null,'#m','is-open')` | `_="on click add .is-open to #m"` |
| `onclick="hsRemove(null,'#m','is-open')"` | `_="on click remove .is-open from #m"` |
| `onclick="hsRemoveClosest(this,'.overlay','is-open')"` | `_="on click remove .is-open from closest .overlay"` |
| `onclick="hsRemoveClosestEl(this,'tr')"` | `_="on click remove closest tr"` |
| `onclick="hsTake(this,'.tab','active')"` | `_="on click take .active from .tab"` |
| `onclick="hsToggle(null,'#m','is-open')"` | `_="on click toggle .is-open on #m"` |
| `onclick="hsBackdropClose(this,event,'is-open')"` | `_="on click[me is event.target] remove .is-open"` |
| `hx-on::after-request="hsAdd(...)"` | `_="on htmx:after-request[detail.xhr.status < 400] add .open to #drawer"`（放触发元素上） |
| `onkeydown="if(event.key==='Escape')..."` | `_="on keydown[event.key is 'Escape'] remove .open"` |

依据：`AGENTS.md` "JS 与交互" 显式禁止 `<script>me().on(...)`。
