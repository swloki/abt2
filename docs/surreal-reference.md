# Surreal.js Reference Guide

Source: https://github.com/gnat/surreal

---

## Overview

Surreal.js 是一个极小的 jQuery 替代库（~5KB），提供 Locality of Behavior 的 DOM 操作 API。无依赖，vanilla JS 友好。

---

## 核心函数

### `me(selector?)` — 选择元素

```javascript
me()                // 当前 script 标签的父元素
me('#my-id')        // CSS 选择器
me(el)              // 包装已有元素
me(ev)              // 从 event 中提取 target
```

### `.on(event, handler)` — 事件监听

```javascript
me().on('click', ev => { ... })
me('#btn').on('click', ev => { ... })
```

### `.classAdd(cls)` / `.classRemove(cls)` / `.classToggle(cls)` — 类操作

```javascript
me(el).classAdd('active')
me(el).classAdd('.active')     // 前导 . 可选
me(el).classRemove('active')
me(el).classToggle('active')
```

### `.styles(styleObj)` — 设置样式

```javascript
me(el).styles('color: red')
me(el).styles({ color: 'red', background: 'blue' })
me(el).styles({ background: null })  // 移除样式
```

### `.attribute(...)` — 属性操作

```javascript
me(el).attribute('data-x')                      // 获取
me(el).attribute('data-x', 'value')             // 设置
me(el).attribute({ 'data-x': 'yes', 'data-y': 'no' })  // 批量设置
me(el).attribute('data-x', null)                // 移除
```

### `.remove()` — 移除元素

```javascript
me(el).remove()
```

### `.fadeOut()` / `.fadeIn()` — 动画

```javascript
me(el).fadeOut()    // 淡出并移除
```

### `sleep(ms)` — 等待

```javascript
await sleep(1000)
```

### `halt(ev)` — 阻止事件传播

```javascript
halt(ev)  // stopPropagation + preventDefault
```

---

## 项目专用 Helpers（app.js）

项目在 `app.js` 中封装了一组 `window.hs*()` 辅助函数，底层使用 Surreal.js API，供 Maud 模板通过 `onclick` 调用：

| 函数 | 说明 | 用法 |
|------|------|------|
| `hsAdd(null, selector, cls)` | 给目标添加 class | `onclick="hsAdd(null,'#modal','is-open')"` |
| `hsRemove(null, selector, cls)` | 给目标移除 class | `onclick="hsRemove(null,'#modal','is-open')"` |
| `hsRemove(this, null, cls)` | 给自身移除 class | `onclick="hsRemove(this,null,'open')"` |
| `hsRemoveClosest(this, sel, cls)` | 给最近祖先移除 class | `onclick="hsRemoveClosest(this,'.overlay','open')"` |
| `hsToggle(null, selector, cls)` | 切换目标 class | `onclick="hsToggle(null,'.sidebar','collapsed')"` |
| `hsToggleSelf(el, cls)` | 切换自身 class | `onclick="hsToggleSelf(this,'active')"` |
| `hsTake(this, siblingSel, cls)` | 从兄弟移除 class，加到自身 | `onclick="hsTake(this,'.tab','active')"` |
| `hsBackdropClose(this, event, cls)` | 仅当 target===self 时移除 class | `onclick="hsBackdropClose(this,event,'open')"` |
| `hsToggleSidebar()` | 切换侧栏折叠 + localStorage | `onclick="hsToggleSidebar()"` |
| `hsSetAndTrigger(sel, val, event)` | 设置 input 值并触发事件 | `onclick="hsSetAndTrigger('.search','','keyup')"` |
| `hsRemoveClosestEl(this, sel)` | 移除最近的祖先元素 | `onclick="hsRemoveClosestEl(this,'tr')"` |

---

## 常用模式对照（Hyperscript → Surreal.js）

| Hyperscript `_=` | Surreal.js `onclick` |
|---|---|
| `_="on click add .is-open to #modal"` | `onclick="hsAdd(null,'#modal','is-open')"` |
| `_="on click remove .is-open from #modal"` | `onclick="hsRemove(null,'#modal','is-open')"` |
| `_="on click remove .open"` | `onclick="hsRemove(this,null,'open')"` |
| `_="on click remove .open from closest .drawer-overlay"` | `onclick="hsRemoveClosest(this,'.drawer-overlay','open')"` |
| `_="on click if event.target is me remove .is-open"` | `onclick="hsBackdropClose(this,event,'is-open')"` |
| `_="on click toggle .active on .target"` | `onclick="hsToggle(null,'.target','active')"` |
| `_="on click call event.stopPropagation()"` | `onclick="event.stopPropagation()"` |
| `_="on click call myFunc()"` | `onclick="myFunc()"` |
| `_="on click remove .active from .items then add .active to me"` | `onclick="hsTake(this,'.items','active')"` |
| `_="on click remove the closest <tr/>"` | `onclick="hsRemoveClosestEl(this,'tr')"` |
| `_="on click set value of .search to '' then trigger keyup"` | `onclick="hsSetAndTrigger('.search','','keyup')"` |
| `_="on htmx:afterRequest remove .is-open from #modal"` | `hx-on::after-request="hsRemove(null,'#modal','is-open')"` |
