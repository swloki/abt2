---
description: "禁止 onclick= 做 UI 操作，用 Hyperscript _=\"on click ...\""
condition: "onclick\\s*="
scope: "tool:edit(*.rs), tool:write(*.rs)"
---

你刚写了 `onclick=`。ABT 前端禁止用 `onclick` / `onkeydown` 等原生 DOM 事件属性做 UI 操作。

**替换规则**（Surreal / 原生 JS → Hyperscript `_=`）：

| 错误（禁止） | 正确（Hyperscript） |
|---|---|
| `onclick="me('#m').classAdd('is-open')"` | `_="on click add .is-open to #m"` |
| `onclick="me('#m').classRemove('is-open')"` | `_="on click remove .is-open from #m"` |
| `onclick="me(this).closest('.overlay').classRemove('is-open')"` | `_="on click remove .is-open from closest .overlay"` |
| `onclick="if(event.target===this) close()"` | `_="on click[me is event.target] remove .is-open"` |
| `onclick="me(this).siblings().classRemove('active')"` | `_="on click take .active from .tab"` |
| `onkeydown="if(event.key==='Escape') close()"` | `_="on keydown[event.key is 'Escape'] remove .open from #m"` |

**为什么**：Hyperscript `_=` 属性是声明式的、英语句子式的事件处理，HTMX `outerHTML` 换入的新元素会被 hyperscript 引擎自动重新初始化，无需手动重绑定。原生 onclick 在 HTMX 替换后会失效。

依据：`AGENTS.md` "JS 与交互" / `.omp/rules/hyperscript-patterns.md` §0 心智模型。
