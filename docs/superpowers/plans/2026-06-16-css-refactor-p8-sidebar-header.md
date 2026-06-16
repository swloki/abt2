# P8: Sidebar + Header + User Menu 原子化迁移实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 base.css 中 `#sidebar`、sidebar-body/rail/item/nav/user 系列、top-header/breadcrumb/header-icon-btn 系列、mobile-nav 系列、avatar/avatar-c0~c7/avatar-sm、user-menu 系列和 dropdown-backdrop 的 class 定义翻译为 UnoCSS 原子 class，在 Maud 模板中替换后删除 base.css 对应定义。

**Architecture:** P8 涉及 ~48 个 class，全部集中在 `layout/sidebar.rs`（侧边栏渲染）和 `layout/header.rs`（顶栏渲染）两个文件。sidebar 使用深色渐变背景（`#0a1628→#0f1d32`），rail 侧使用更深的 `#070f1e`。active 状态使用伪元素竖线指示器（`::before`）。头像有 8 种渐变色变体。分为 5 个 Task：Icon Rail → Sidebar Body/Nav/User → Top Header + Mobile Nav → Avatar + User Menu → Dropdown Backdrop。注意 sidebar collapsed 状态由 `_` hyperscript 控制（`.sidebar-collapsed`），需要保留这一交互逻辑。

**Tech Stack:** UnoCSS v66.7.0 + presetWind4, Rust/Maud, base.css

**设计文档:** `docs/superpowers/specs/2026-06-16-css-architecture-atomic-refactor-design.md`

---

## 原子 class 映射总表

### Icon Rail 族映射（base.css 行 139-185）

| 原 class | CSS 属性 | 原子 class 替换 |
|---|---|---|
| `.sidebar-rail` | `width:56px; min-width:56px; background:#070f1e; display:flex; flex-direction:column; align-items:center; border-right:1px solid rgba(255,255,255,.04)` | `w-14 min-w-[56px] bg-[#070f1e] flex flex-col items-center border-r border-white/[0.04]` |
| `.rail-brand` | `width:36px; height:36px; border-radius:md; background:linear-gradient(135deg,accent,accent-hover); display:grid; place-items:center; margin-bottom:space-3; flex-shrink:0` | `w-9 h-9 rounded-md bg-[linear-gradient(135deg,var(--accent),var(--accent-hover))] grid place-items-center mb-3 shrink-0` |
| `.rail-brand svg` | `width:18px; height:18px; stroke:#fff` | 子选择器 → Maud 中给 svg 加 `w-[18px] h-[18px] stroke-white` |
| `.rail-modules` | `flex:1; display:flex; flex-direction:column; align-items:center; gap:2px; width:100%; overflow-y:auto; padding:0 space-1` | `flex-1 flex flex-col items-center gap-0.5 w-full overflow-y-auto px-1` |
| `.rail-item` | `width:44px; display:flex; flex-direction:column; align-items:center; gap:3px; padding:8px 0 6px; border:none; background:transparent; border-radius:sm; color:rgba(255,255,255,.4); cursor:pointer; position:relative; transition:all fast` | `w-11 flex flex-col items-center gap-[3px] py-2 pb-1.5 border-none bg-transparent rounded-sm text-white/40 cursor-pointer relative transition-all duration-150` |
| `.rail-item:hover` | `color:rgba(255,255,255,.85); background:rgba(255,255,255,.06)` | `hover:text-white/85 hover:bg-white/[0.06]` |
| `.rail-item.active` | `color:#fff; background:rgba(37,99,235,.15)` | `text-white bg-[rgba(37,99,235,0.15)]` |
| `.rail-item.active::before` | `content:''; position:absolute; left:-4px; top:50%; transform:translateY(-50%); width:3px; height:20px; background:accent; border-radius:0 3px 3px 0` | `before:content-[''] before:absolute before:-left-1 before:top-1/2 before:-translate-y-1/2 before:w-[3px] before:h-5 before:bg-accent before:rounded-r-[3px]` |
| `.rail-icon` | `width:20px; height:20px; display:grid; place-items:center` | `w-5 h-5 grid place-items-center` |
| `.rail-icon svg` | `width:18px; height:18px` | 子选择器 → svg 加 `w-[18px] h-[18px]` |
| `.rail-label` | `font-size:10px; line-height:1; white-space:nowrap; letter-spacing:.01em` | `text-[10px] leading-none whitespace-nowrap tracking-[0.01em]` |
| `.rail-item.active .rail-icon svg` | `stroke:accent` | 给 svg 加 `stroke-accent` |
| `.rail-bottom` | `display:flex; flex-direction:column; align-items:center; width:100%; padding-top:space-3; border-top:1px solid rgba(255,255,255,.06); margin-top:space-2` | `flex flex-col items-center w-full pt-3 border-t border-white/[0.06] mt-2` |
| `.rail-bottom .rail-item` | `color:rgba(255,255,255,.25)` | rail-item 在 rail-bottom 内：追加 `text-white/25`（覆盖上面的 `text-white/40`） |
| `.rail-bottom .rail-item:hover` | `color:rgba(255,255,255,.6)` | 追加 `hover:text-white/60` |
| `.rail-collapse svg` | `width:16px!important; height:16px!important; opacity:.7` | svg 加 `w-4! h-4! opacity-70` |
| `.rail-collapse:hover svg` | `opacity:1` | hover 逻辑 → svg 加 `hover:opacity-100` 在 rail-collapse 上 |

### Sidebar Body/Nav/User 族映射（base.css 行 115-117, 188-237）

| 原 class | CSS 属性 | 原子 class 替换 |
|---|---|---|
| `.app-shell.sidebar-collapsed` | `grid-template-columns:56px 1fr` | 由 hyperscript `_` 控制 class 开关，CSS 不迁移——collapsed 样式通过 Maud 条件 class 实现 |
| `.sidebar-body` | `flex:1; min-width:0; display:flex; flex-direction:column; overflow-y:auto; transition:width 240ms, opacity 150ms` | `flex-1 min-w-0 flex flex-col overflow-y-auto transition-[width,opacity] duration-200` |
| `.sidebar-collapsed .sidebar-body` | `display:none` | 在 Maud 中根据 collapsed 状态条件渲染（Hyperscript 控制可见性） |
| `.sidebar-module-header` | `padding:space-4 space-5; font-size:sm; font-weight:700; color:rgba(255,255,255,.9); letter-spacing:-.01em; border-bottom:1px solid rgba(255,255,255,.06)` | `px-5 py-4 text-sm font-bold text-white/90 tracking-[-0.01em] border-b border-white/[0.06]` |
| `.sidebar-nav` | `flex:1; overflow-y:auto; padding:space-2 0` | `flex-1 overflow-y-auto py-2` |
| `.sidebar-item` | `display:flex; align-items:center; gap:space-3; padding:9px space-5; font-size:sm; color:rgba(255,255,255,.6); transition:all fast; text-decoration:none; position:relative` | `flex items-center gap-3 py-[9px] px-5 text-sm text-white/60 transition-all duration-150 no-underline relative` |
| `.sidebar-item:hover` | `background:rgba(255,255,255,.06); color:rgba(255,255,255,.95)` | `hover:bg-white/[0.06] hover:text-white/95` |
| `.sidebar-item.active` | `background:rgba(37,99,235,.15); color:#fff; font-weight:600` | `bg-[rgba(37,99,235,0.15)] text-white font-semibold` |
| `.sidebar-item.active::before` | `content:''; position:absolute; left:0; top:50%; transform:translateY(-50%); width:3px; height:20px; background:accent; border-radius:0 3px 3px 0` | `before:content-[''] before:absolute before:left-0 before:top-1/2 before:-translate-y-1/2 before:w-[3px] before:h-5 before:bg-accent before:rounded-r-[3px]` |
| `.sidebar-item svg` | `width:18px; height:18px; flex-shrink:0; opacity:.55; transition:opacity fast` | svg 加 `w-[18px] h-[18px] shrink-0 opacity-55 transition-opacity duration-150` |
| `.sidebar-item:hover svg` | `opacity:.8` | svg 追加 `group-hover:opacity-80`（需要 sidebar-item 加 `group`） |
| `.sidebar-item.active svg` | `opacity:1; color:accent; stroke:accent` | svg 追加 `group-[.active]:opacity-100 group-[.active]:text-accent group-[.active]:stroke-accent`（或 Maud 条件 class） |
| `.sidebar-item-text` | `overflow:hidden; text-overflow:ellipsis` | `overflow-hidden text-ellipsis` |
| `.sidebar-user` | `margin-top:auto; padding:space-4 space-5; border-top:1px solid rgba(255,255,255,.06); display:flex; align-items:center; gap:space-3` | `mt-auto px-5 py-4 border-t border-white/[0.06] flex items-center gap-3` |
| `.sidebar-user-avatar` | `width:34px; height:34px; border-radius:50%; background:linear-gradient(135deg,accent,accent-hover); display:grid; place-items:center; font-size:13px; font-weight:700; color:#fff; flex-shrink:0` | `w-[34px] h-[34px] rounded-full bg-[linear-gradient(135deg,var(--accent),var(--accent-hover))] grid place-items-center text-[13px] font-bold text-white shrink-0` |
| `.sidebar-user-info` | `flex:1; min-width:0` | `flex-1 min-w-0` |
| `.sidebar-user-name` | `font-size:sm; font-weight:600; color:#fff` | `text-sm font-semibold text-white` |
| `.sidebar-user-role` | `font-size:11px; color:rgba(255,255,255,.4)` | `text-[11px] text-white/40` |

### #sidebar 容器映射（base.css 行 120-137, 378-383）

| 原 class | CSS 属性 | 原子 class 替换 |
|---|---|---|
| `#sidebar` | `background:linear-gradient(180deg,#0a1628,#0f1d32); color:rgba(255,255,255,.85); display:flex; min-width:0; position:sticky; top:0; height:100vh; z-index:20` | Maud 中保留 `id="sidebar"`，class 替换为 `bg-[linear-gradient(180deg,#0a1628,#0f1d32)] text-white/85 flex min-w-0 sticky top-0 h-screen z-20` |
| `#sidebar.sidebar-collapsed` | `width:56px; min-width:56px; overflow:visible` | collapsed 时条件追加 `w-14 min-w-[56px] overflow-visible` |
| `#sidebar` (mobile @media) | `position:fixed; left:0; top:0; bottom:0; width:280px; transform:translateX(-100%); z-index:55; transition:transform 240ms` | `md:static md:top-0 md:h-screen fixed left-0 top-0 bottom-0 w-[280px] -translate-x-full z-55 transition-transform duration-200` |
| `#sidebar.mobile-open` | `transform:translateX(0)` | 条件追加 `translate-x-0` |

### Top Header 族映射（base.css 行 242-264, 344-388）

| 原 class | CSS 属性 | 原子 class 替换 |
|---|---|---|
| `.top-header` | `height:header-h; background:bg; border-bottom:1px solid border-soft; display:flex; align-items:center; justify-content:space-between; padding:0 space-8; position:sticky; top:0; z-index:10; box-shadow:xs` | `h-[var(--header-h)] bg-bg border-b border-border-soft flex items-center justify-between px-8 sticky top-0 z-10 shadow-xs` |
| `.top-header-left` | `display:flex; align-items:center; gap:space-4` | `flex items-center gap-4` |
| `.top-header-right` | `display:flex; align-items:center; gap:space-4` | `flex items-center gap-4` |
| `.breadcrumb` | `display:flex; align-items:center; gap:space-2; font-size:sm; color:muted` | `flex items-center gap-2 text-sm text-muted` |
| `.breadcrumb-sep` | `color:border; font-size:12px` | `text-border text-xs` |
| `.header-icon-btn` | `width:36px; height:36px; border-radius:sm; border:1px solid border-soft; background:bg; display:grid; place-items:center; position:relative; cursor:pointer; transition:background fast` | `w-9 h-9 rounded-sm border border-border-soft bg-bg grid place-items-center relative cursor-pointer transition-colors duration-150` |
| `.header-icon-btn:hover` | `background:surface; border-color:border` | `hover:bg-surface hover:border-border` |
| `.header-icon-btn svg` | `width:18px; height:18px; color:muted` | svg 加 `w-[18px] h-[18px] text-muted` |
| `.header-dot` | `position:absolute; top:7px; right:7px; width:7px; height:7px; border-radius:50%; background:danger; border:2px solid bg` | `absolute top-[7px] right-[7px] w-[7px] h-[7px] rounded-full bg-danger border-2 border-bg` |

### Mobile Nav 族映射（base.css 行 344-388）

| 原 class | CSS 属性 | 原子 class 替换 |
|---|---|---|
| `.mobile-menu-btn` | `display:none; width:38px; height:38px; border:none; background:transparent; border-radius:sm; place-items:center; cursor:pointer; flex-shrink:0; transition:background fast` | `hidden w-[38px] h-[38px] border-none bg-transparent rounded-sm grid place-items-center cursor-pointer shrink-0 transition-colors duration-150` |
| `.mobile-menu-btn:hover` | `background:surface` | `hover:bg-surface` |
| `.mobile-menu-btn svg` | `width:22px; height:22px; color:fg` | svg 加 `w-[22px] h-[22px] text-fg` |
| `.mobile-menu-btn` (@media 768) | `display:grid` | 追加 `md:hidden` 策略：改为 `md:hidden grid` — 始终用 `grid`，在小屏显示。实际写法 `grid md:hidden` |
| `.mobile-nav` | `display:none; position:fixed; bottom:0; left:0; right:0; height:60px; background:bg; border-top:1px solid border-soft; z-index:30; box-shadow:0 -2px 10px rgba(0,0,0,.06)` | `hidden fixed bottom-0 left-0 right-0 h-[60px] bg-bg border-t border-border-soft z-30 shadow-[0_-2px_10px_rgba(0,0,0,0.06)]` |
| `.mobile-nav` (@media 768) | `display:block` | 追加 `md:block` → 完整写法 `hidden md:block fixed bottom-0 ...` |
| `.mobile-nav-scroll` | `height:100%; overflow-x:auto; -webkit-overflow-scrolling:touch; scrollbar-width:none` | `h-full overflow-x-auto [-webkit-overflow-scrolling:touch] [scrollbar-width:none]` |
| `.mobile-nav-scroll::-webkit-scrollbar` | `display:none` | 追加 `[&::-webkit-scrollbar]:hidden` |
| `.mobile-nav-inner` | `display:flex; height:100%; min-width:max-content; padding:0 space-1` | `flex h-full min-w-max px-1` |
| `.mobile-nav-item` | `display:flex; flex-direction:column; align-items:center; justify-content:center; gap:3px; padding:0 14px; font-size:10px; color:muted; text-decoration:none; white-space:nowrap; min-width:60px; transition:color fast` | `flex flex-col items-center justify-center gap-[3px] px-[14px] text-[10px] text-muted no-underline whitespace-nowrap min-w-[60px] transition-colors duration-150` |
| `.mobile-nav-item svg` | `width:20px; height:20px` | svg 加 `w-5 h-5` |
| `.mobile-nav-item.active` | `color:accent; font-weight:600` | `text-accent font-semibold` |
| `.mobile-nav-item.active svg` | `stroke:accent` | svg 追加 active 时的 `stroke-accent`（条件 class） |
| `.mobile-sidebar-overlay` | `display:none; position:fixed; inset:0; background:rgba(0,0,0,.45); z-index:50; backdrop-filter:blur(2px)` | `hidden fixed inset-0 bg-[rgba(0,0,0,0.45)] z-50 backdrop-blur-[2px]` |
| `.mobile-sidebar-overlay.open` | `display:block` | 条件追加 `block` |

### Avatar 族映射（base.css 行 265-271, 1836-1848）

| 原 class | CSS 属性 | 原子 class 替换 |
|---|---|---|
| `.avatar` | `width:34px; height:34px; border-radius:50%; background:linear-gradient(135deg,accent,accent-hover); display:grid; place-items:center; font-size:12px; font-weight:700; color:#fff; flex-shrink:0` | `w-[34px] h-[34px] rounded-full bg-[linear-gradient(135deg,var(--accent),var(--accent-hover))] grid place-items-center text-xs font-bold text-white shrink-0` |
| `.avatar-sm` | `width:32px; height:32px; border-radius:10px; display:flex; align-items:center; justify-content:center; font-size:12px; font-weight:600; flex-shrink:0; color:#fff` | `w-8 h-8 rounded-[10px] flex items-center justify-center text-xs font-semibold shrink-0 text-white` |
| `.avatar-c0` | `background:linear-gradient(135deg,#7c3aed,#a78bfa)` | `bg-[linear-gradient(135deg,#7c3aed,#a78bfa)]` |
| `.avatar-c1` | `background:linear-gradient(135deg,accent,accent-hover)` | `bg-[linear-gradient(135deg,var(--accent),var(--accent-hover))]` |
| `.avatar-c2` | `background:linear-gradient(135deg,#13c2c2,#36cfc9)` | `bg-[linear-gradient(135deg,#13c2c2,#36cfc9)]` |
| `.avatar-c3` | `background:linear-gradient(135deg,#fa8c16,#ffc53d)` | `bg-[linear-gradient(135deg,#fa8c16,#ffc53d)]` |
| `.avatar-c4` | `background:linear-gradient(135deg,#d46b08,#fa8c16)` | `bg-[linear-gradient(135deg,#d46b08,#fa8c16)]` |
| `.avatar-c5` | `background:linear-gradient(135deg,success,#95de64)` | `bg-[linear-gradient(135deg,var(--success),#95de64)]` |
| `.avatar-c6` | `background:linear-gradient(135deg,#eb2f96,#ff85c0)` | `bg-[linear-gradient(135deg,#eb2f96,#ff85c0)]` |
| `.avatar-c7` | `background:linear-gradient(135deg,#8c8c8c,#bfbfbf)` | `bg-[linear-gradient(135deg,#8c8c8c,#bfbfbf)]` |

### User Menu 族映射（base.css 行 272-312）

| 原 class | CSS 属性 | 原子 class 替换 |
|---|---|---|
| `.user-menu` | `position:relative` | `relative` |
| `.user-menu-trigger` | `display:flex; align-items:center; gap:space-2; border:none; background:transparent; cursor:pointer; padding:4px; border-radius:sm; transition:background fast` | `flex items-center gap-2 border-none bg-transparent cursor-pointer p-1 rounded-sm transition-colors duration-150` |
| `.user-menu-trigger:hover` | `background:surface` | `hover:bg-surface` |
| `.user-menu-trigger .avatar` | `width:32px; height:32px; font-size:11px` | trigger 内的 avatar 追加 `w-8 h-8 text-[11px]` |
| `.user-menu-dropdown` | `position:absolute; top:calc(100% + 8px); right:0; min-width:240px; background:bg; border:1px solid border-soft; border-radius:md; box-shadow:lg; opacity:0; visibility:hidden; transform:translateY(-8px); transition:opacity fast, transform fast, visibility fast` | `absolute top-[calc(100%+8px)] right-0 min-w-[240px] bg-bg border border-border-soft rounded-md shadow-lg opacity-0 invisible -translate-y-2 transition-[opacity,transform,visibility] duration-150` |
| `.user-menu.is-open .user-menu-dropdown` | `opacity:1; visibility:visible; transform:translateY(0)` | is-open 时条件追加 `opacity-100 visible translate-y-0` |
| `.user-menu-header` | `display:flex; align-items:center; gap:space-3; padding:space-3 space-2; margin-bottom:space-2` | `flex items-center gap-3 py-3 px-2 mb-2` |
| `.user-menu-header .avatar` | `width:40px; height:40px; font-size:14px` | header 内 avatar 追加 `w-10 h-10 text-sm` |
| `.user-menu-info` | `display:flex; flex-direction:column; min-width:0` | `flex flex-col min-w-0` |
| `.user-menu-name` | `font-size:sm; font-weight:600; color:fg` | `text-sm font-semibold text-fg` |
| `.user-menu-email` | `font-size:12px; color:muted; overflow:hidden; text-overflow:ellipsis` | `text-xs text-muted overflow-hidden text-ellipsis` |
| `.user-menu-item` | `display:flex; align-items:center; gap:space-3; padding:space-2 space-3; border-radius:sm; font-size:sm; color:fg; text-decoration:none; cursor:pointer; transition:background fast` | `flex items-center gap-3 py-2 px-3 rounded-sm text-sm text-fg no-underline cursor-pointer transition-colors duration-150` |
| `.user-menu-item:hover` | `background:surface` | `hover:bg-surface` |
| `.user-menu-item svg` | `width:16px; height:16px; color:muted` | svg 加 `w-4 h-4 text-muted` |
| `.user-menu-item:hover svg` | `color:accent` | svg 追加 `group-hover:text-accent`（item 加 `group`） |
| `.user-menu-divider` | `height:1px; background:border-soft; margin:space-2 0` | `h-px bg-border-soft my-2` |
| `.user-menu-logout` | `color:danger` | `text-danger` |
| `.user-menu-logout svg` | `color:danger` | svg 追加 `text-danger` |
| `.user-menu-form` | `margin:0` | `m-0` |
| `.user-menu-form .user-menu-item` | `width:100%; border:none; background:none; text-align:left` | form 内 item 追加 `w-full border-none bg-none text-left` |

### Dropdown Backdrop 映射（base.css 行 572-573）

| 原 class | CSS 属性 | 原子 class 替换 |
|---|---|---|
| `.dropdown-backdrop` | `display:none; position:fixed; inset:0; z-index:49` | `hidden fixed inset-0 z-49` |
| `.row-actions:has(.row-actions-menu.is-open) .dropdown-backdrop` | `display:block` | 父级 `has` 选择器：在 Maud 中根据菜单 open 状态条件追加 `block` |

---

### Task 1: Icon Rail 族迁移

**Files:**
- Modify: `static/base.css:139-185`（删除 sidebar-rail/rail-* 定义）
- Modify: `abt-web/src/layout/sidebar.rs`（rail 渲染函数）

- [ ] **Step 1: 在 sidebar.rs 中定位 rail 渲染代码**

Run（使用 search 工具）:
- 搜索 `abt-web/src/layout/sidebar.rs` 中 `sidebar-rail`, `rail-brand`, `rail-modules`, `rail-item`, `rail-icon`, `rail-label`, `rail-bottom`, `rail-collapse`

- [ ] **Step 2: 替换 sidebar-rail 容器**

将 `class="sidebar-rail"` 替换为:
```rust
class="w-14 min-w-[56px] bg-[#070f1e] flex flex-col items-center border-r border-white/[0.04]"
```

- [ ] **Step 3: 替换 rail-brand**

将 `class="rail-brand"` 替换为:
```rust
class="w-9 h-9 rounded-md bg-[linear-gradient(135deg,var(--accent),var(--accent-hover))] grid place-items-center mb-3 shrink-0"
```

确保其中的 svg 元素追加 `w-[18px] h-[18px] stroke-white`。

- [ ] **Step 4: 替换 rail-modules**

将 `class="rail-modules"` 替换为:
```rust
class="flex-1 flex flex-col items-center gap-0.5 w-full overflow-y-auto px-1"
```

- [ ] **Step 5: 替换 rail-item（含 active/hover/::before）**

将 `class="rail-item"` 替换为（非 active）:
```rust
class="w-11 flex flex-col items-center gap-[3px] py-2 pb-1.5 border-none bg-transparent rounded-sm text-white/40 cursor-pointer relative transition-all duration-150 hover:text-white/85 hover:bg-white/[0.06]"
```

将 `class="rail-item active"` 替换为:
```rust
class="w-11 flex flex-col items-center gap-[3px] py-2 pb-1.5 border-none bg-transparent rounded-sm text-white cursor-pointer relative transition-all duration-150 bg-[rgba(37,99,235,0.15)] before:content-[''] before:absolute before:-left-1 before:top-1/2 before:-translate-y-1/2 before:w-[3px] before:h-5 before:bg-accent before:rounded-r-[3px]"
```

- [ ] **Step 6: 替换 rail-icon / rail-label**

将 `class="rail-icon"` 替换为:
```rust
class="w-5 h-5 grid place-items-center"
```

将 `class="rail-label"` 替换为:
```rust
class="text-[10px] leading-none whitespace-nowrap tracking-[0.01em]"
```

给 rail-icon 内的 svg 元素：active 时追加 `stroke-accent`，非 active 时保持默认。

- [ ] **Step 7: 替换 rail-bottom / rail-collapse**

将 `class="rail-bottom"` 替换为:
```rust
class="flex flex-col items-center w-full pt-3 border-t border-white/[0.06] mt-2"
```

rail-bottom 内的 rail-item 使用覆盖色：追加 `text-white/25 hover:text-white/60`（替换原 `text-white/40 hover:text-white/85`）。

将 `class="rail-collapse"` 替换为:
```rust
class="w-11 flex flex-col items-center gap-[3px] py-2 pb-1.5 border-none bg-transparent rounded-sm text-white/25 cursor-pointer relative transition-all duration-150 hover:text-white/60"
```

确保 svg 追加 `w-4! h-4! opacity-70 hover:opacity-100`。

- [ ] **Step 8: 从 base.css 删除 rail 定义**

删除行 138-185（`/* ── Icon Rail ── */` 注释到 `.rail-collapse:hover svg` 行）。

- [ ] **Step 9: 构建并验证**

Run: `cd E:/work/abt && npm run build:css && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

Expected: 无 error。

---

### Task 2: Sidebar Body/Nav/User 族迁移

**Files:**
- Modify: `static/base.css:115-117, 188-237, 377-388`（删除）
- Modify: `abt-web/src/layout/sidebar.rs`

- [ ] **Step 1: 替换 sidebar-body**

将 `class="sidebar-body"` 替换为:
```rust
class="flex-1 min-w-0 flex flex-col overflow-y-auto transition-[width,opacity] duration-200"
```

注意：`.sidebar-collapsed .sidebar-body { display:none }` 的行为由 hyperscript `_` 控制 collapsed 状态时隐藏。在 Maud 中保留 `sidebar-collapsed` 的逻辑开关——collapsed 状态通过 hyperscript 切换，不需要 CSS。但需要确保 sidebar-body 在 collapsed 时隐藏。方案：在 sidebar-body 上追加 hyperscript `_|` 或在 `#sidebar` 上用 `has-[.sidebar-collapsed]:hidden`。

最终方案：在 sidebar-body 上保持不变，在 `#sidebar` 上追加 `[&.sidebar-collapsed_.sidebar-body]:hidden`。但因为 sidebar-body 已经原子化没有 class 名了，改用条件渲染：如果 sidebar 使用 collapsed 时直接不渲染 body。检查 sidebar.rs 的实际 collapsed 逻辑——collapsed 是通过 `app-shell` 上的 class + localStorage 控制。

替代方案：给 sidebar-body 元素追加一个 data 属性 `data-sidebar-body`，然后在 `#sidebar` 元素追加 `[&.sidebar-collapsed_[data-sidebar-body]]:hidden`。但 UnoCSS 的任意值变体需要确切的 class 名。

最简方案：保留一个残留 class `sidebar-body` 仅用于 collapsed 隐藏，其他样式全部原子化：
```rust
class="sidebar-body flex-1 min-w-0 flex flex-col overflow-y-auto transition-[width,opacity] duration-200"
```
然后在 base.css 中保留一条 `.sidebar-collapsed .sidebar-body { display: none; }`（直到 P9 删除 base.css）。

- [ ] **Step 2: 替换 sidebar-module-header**

将 `class="sidebar-module-header"` 替换为:
```rust
class="px-5 py-4 text-sm font-bold text-white/90 tracking-[-0.01em] border-b border-white/[0.06]"
```

- [ ] **Step 3: 替换 sidebar-nav**

将 `class="sidebar-nav"` 替换为:
```rust
class="flex-1 overflow-y-auto py-2"
```

- [ ] **Step 4: 替换 sidebar-item（含 active/hover/::before/svg）**

将 `class="sidebar-item"` 替换为（非 active）:
```rust
class="group flex items-center gap-3 py-[9px] px-5 text-sm text-white/60 transition-all duration-150 no-underline relative hover:bg-white/[0.06] hover:text-white/95"
```

将 `class="sidebar-item active"` 替换为:
```rust
class="group flex items-center gap-3 py-[9px] px-5 text-sm text-white font-semibold transition-all duration-150 no-underline relative bg-[rgba(37,99,235,0.15)] before:content-[''] before:absolute before:left-0 before:top-1/2 before:-translate-y-1/2 before:w-[3px] before:h-5 before:bg-accent before:rounded-r-[3px]"
```

给 sidebar-item 内的 svg 元素追加:
```rust
class="w-[18px] h-[18px] shrink-0 opacity-55 transition-opacity duration-150 group-hover:opacity-80"
```

active 的 svg 追加: `opacity-100 text-accent stroke-accent`（替换 `opacity-55`）。

- [ ] **Step 5: 替换 sidebar-item-text**

将 `class="sidebar-item-text"` 替换为:
```rust
class="overflow-hidden text-ellipsis"
```

- [ ] **Step 6: 替换 sidebar-user 族**

将 `class="sidebar-user"` 替换为:
```rust
class="mt-auto px-5 py-4 border-t border-white/[0.06] flex items-center gap-3"
```

将 `class="sidebar-user-avatar"` 替换为:
```rust
class="w-[34px] h-[34px] rounded-full bg-[linear-gradient(135deg,var(--accent),var(--accent-hover))] grid place-items-center text-[13px] font-bold text-white shrink-0"
```

将 `class="sidebar-user-info"` 替换为:
```rust
class="flex-1 min-w-0"
```

将 `class="sidebar-user-name"` 替换为:
```rust
class="text-sm font-semibold text-white"
```

将 `class="sidebar-user-role"` 替换为:
```rust
class="text-[11px] text-white/40"
```

- [ ] **Step 7: 从 base.css 删除已迁移定义**

删除行 187-237（`/* ── Sidebar Body ── */` 到 `.sidebar-user-role`）。

注意：保留行 192 `.sidebar-collapsed .sidebar-body { display: none; }` 直到 sidebar collapsed 逻辑完全重构——在 Step 1 中已决定保留残留 class。

- [ ] **Step 8: 构建并验证**

Run: `cd E:/work/abt && npm run build:css && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

Expected: 无 error。

---

### Task 3: Top Header + Mobile Nav 族迁移

**Files:**
- Modify: `static/base.css:242-264, 343-388`（删除）
- Modify: `abt-web/src/layout/header.rs`
- Modify: `abt-web/src/layout/sidebar.rs`（mobile_nav 函数）
- Modify: `abt-web/src/layout/page.rs`（mobile-sidebar-overlay）

- [ ] **Step 1: 替换 top-header 族（header.rs）**

将 `header class="top-header"` 替换为:
```rust
header class="h-[var(--header-h)] bg-bg border-b border-border-soft flex items-center justify-between px-8 sticky top-0 z-10 shadow-xs" {
```

将 `class="top-header-left"` 替换为:
```rust
class="flex items-center gap-4"
```

将 `class="top-header-right"` 替换为:
```rust
class="flex items-center gap-4"
```

- [ ] **Step 2: 替换 mobile-menu-btn**

将 `class="mobile-menu-btn"` 替换为:
```rust
class="grid md:hidden w-[38px] h-[38px] border-none bg-transparent rounded-sm place-items-center cursor-pointer shrink-0 transition-colors duration-150 hover:bg-surface"
```

给 svg 追加 `w-[22px] h-[22px] text-fg`。

- [ ] **Step 3: 替换 breadcrumb 族**

将 `class="breadcrumb"` 替换为:
```rust
class="flex items-center gap-2 text-sm text-muted"
```

将 `class="breadcrumb-sep"` 替换为:
```rust
class="text-border text-xs"
```

- [ ] **Step 4: 替换 header-icon-btn / header-dot**

将 `class="header-icon-btn"` 替换为:
```rust
class="w-9 h-9 rounded-sm border border-border-soft bg-bg grid place-items-center relative cursor-pointer transition-colors duration-150 hover:bg-surface hover:border-border"
```

给 svg 追加 `w-[18px] h-[18px] text-muted`。

将 `class="header-dot"` 替换为:
```rust
class="absolute top-[7px] right-[7px] w-[7px] h-[7px] rounded-full bg-danger border-2 border-bg"
```

- [ ] **Step 5: 替换 mobile-nav 族（sidebar.rs mobile_nav 函数）**

将 `class="mobile-nav"` 替换为:
```rust
class="hidden md:block fixed bottom-0 left-0 right-0 h-[60px] bg-bg border-t border-border-soft z-30 shadow-[0_-2px_10px_rgba(0,0,0,0.06)]"
```

将 `class="mobile-nav-scroll"` 替换为:
```rust
class="h-full overflow-x-auto [-webkit-overflow-scrolling:touch] [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
```

将 `class="mobile-nav-inner"` 替换为:
```rust
class="flex h-full min-w-max px-1"
```

将 `class="mobile-nav-item"` 替换为:
```rust
class="flex flex-col items-center justify-center gap-[3px] px-[14px] text-[10px] text-muted no-underline whitespace-nowrap min-w-[60px] transition-colors duration-150"
```

将 `class="mobile-nav-item active"` 替换为:
```rust
class="flex flex-col items-center justify-center gap-[3px] px-[14px] text-[10px] text-accent font-semibold no-underline whitespace-nowrap min-w-[60px] transition-colors duration-150"
```

给 mobile-nav-item 内的 svg 追加 `w-5 h-5`（active 时追加 `stroke-accent`）。

- [ ] **Step 6: 替换 mobile-sidebar-overlay（page.rs）**

将 `class="mobile-sidebar-overlay"` 替换为:
```rust
class="hidden fixed inset-0 bg-[rgba(0,0,0,0.45)] z-50 backdrop-blur-[2px]"
```

注意：`.open` 状态由 hyperscript `_="on click remove .open"` 控制。因为 class 已原子化，open 状态需要改为追加 `block`：
```rust
class="mobile-sidebar-overlay hidden fixed inset-0 bg-[rgba(0,0,0,0.45)] z-50 backdrop-blur-[2px]"
_="on click remove .block then remove .open"
```

修改 mobile-menu-btn 的 hyperscript：从 `add .open to .mobile-sidebar-overlay` 改为 `add .block to .mobile-sidebar-overlay`。

- [ ] **Step 7: 处理 #sidebar 移动端响应式**

在 sidebar.rs 的 sidebar 渲染中，`#sidebar` 的 class 需要处理移动端：
```rust
div id="sidebar"
    class="bg-[linear-gradient(180deg,#0a1628,#0f1d32)] text-white/85 flex min-w-0 sticky top-0 h-screen z-20
           md:static md:top-0 md:h-screen
           fixed left-0 top-0 bottom-0 w-[280px] -translate-x-full z-55 transition-transform duration-200" {
```

当 mobile-open 时追加 `translate-x-0`（由 hyperscript 控制）。

- [ ] **Step 8: 处理 app-shell sidebar-collapsed**

`.app-shell.sidebar-collapsed { grid-template-columns: 56px 1fr; }` 需要保留——在 page.rs 中，app-shell 的 class 已经有 hyperscript 控制 collapsed。在 app-shell 上追加 `[&.sidebar-collapsed]:grid-cols-[56px_1fr]`。

将 page.rs 中 `class="app-shell"` 替换为:
```rust
class="app-shell grid grid-cols-[240px_1fr] lg:grid-cols-1 [&.sidebar-collapsed]:grid-cols-[56px_1fr]"
```

注意：保留 `app-shell` class 名用于 hyperscript 选择器和 `.sidebar-collapsed` 逻辑。

- [ ] **Step 9: 从 base.css 删除已迁移定义**

删除行 242-264（`/* ─── Top Header ─── */` 到 `.header-dot`）、343-388（`/* ─── Mobile ─── */` 到 `@media` 结束）、120-137（`#sidebar` 和 `.sidebar-collapsed` 定义）、114-117（`.app-shell` 和 `.app-shell.sidebar-collapsed` 定义）。

保留：行 192 `.sidebar-collapsed .sidebar-body { display: none; }`（直到 sidebar-body 完全脱离 class 名依赖）和行 377 `.app-shell` 响应式 @media（直到 app-shell 完全原子化）。

实际清理：app-shell 在 Step 8 已用 `[&.sidebar-collapsed]:grid-cols-[56px_1fr]` 原子化，`@media (max-width:768px)` 中的 `.app-shell { grid-template-columns: 1fr !important }` 改为在 class 中用 `md:grid-cols-[240px_1fr]`（默认 1 列，md 以上 2 列）。

- [ ] **Step 10: 构建并验证**

Run: `cd E:/work/abt && npm run build:css && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

Expected: 无 error。

---

### Task 4: Avatar + User Menu 族迁移

**Files:**
- Modify: `static/base.css:265-312, 1836-1848`（删除）
- Modify: `abt-web/src/layout/header.rs`
- Modify: `abt-web/src/layout/sidebar.rs`（avatar_initials 函数调用处）
- Modify: 所有引用 `avatar`, `avatar-sm`, `avatar-c0`~`avatar-c7` 的 `.rs` 文件

- [ ] **Step 1: 搜索所有 avatar 引用**

搜索 `abt-web/src/**/*.rs` 中 `avatar`, `avatar-sm`, `avatar-c0`, `avatar-c1` 到 `avatar-c7`。

- [ ] **Step 2: 替换 avatar（header.rs）**

将 `class="avatar"` 替换为:
```rust
class="w-[34px] h-[34px] rounded-full bg-[linear-gradient(135deg,var(--accent),var(--accent-hover))] grid place-items-center text-xs font-bold text-white shrink-0"
```

user-menu-trigger 内的 avatar（`width:32px; height:32px`）替换为:
```rust
class="w-8 h-8 rounded-full bg-[linear-gradient(135deg,var(--accent),var(--accent-hover))] grid place-items-center text-[11px] font-bold text-white shrink-0"
```

user-menu-header 内的 avatar（`width:40px; height:40px`）替换为:
```rust
class="w-10 h-10 rounded-full bg-[linear-gradient(135deg,var(--accent),var(--accent-hover))] grid place-items-center text-sm font-bold text-white shrink-0"
```

- [ ] **Step 3: 替换 avatar-sm**

将 `class="avatar-sm"` 替换为:
```rust
class="w-8 h-8 rounded-[10px] flex items-center justify-center text-xs font-semibold shrink-0 text-white"
```

- [ ] **Step 4: 替换 avatar-c0~c7 色变体**

avatar-c0~c7 是背景色变体，与 avatar/avatar-sm 配合使用。在 Maud 中需要合并到同一个 class 串中。

将 `class="avatar-sm avatar-c0"` 替换为:
```rust
class="w-8 h-8 rounded-[10px] flex items-center justify-center text-xs font-semibold shrink-0 text-white bg-[linear-gradient(135deg,#7c3aed,#a78bfa)]"
```

同理处理 c1~c7（按映射表替换渐变色值）。

- [ ] **Step 5: 替换 user-menu 族（header.rs）**

将 `class="user-menu"` 替换为:
```rust
class="user-menu relative"
```

注意：保留 `user-menu` class 名用于 hyperscript `_="on click toggle .is-open on .user-menu"` 和 `is-open` 状态控制。

将 `class="user-menu-trigger"` 替换为:
```rust
class="flex items-center gap-2 border-none bg-transparent cursor-pointer p-1 rounded-sm transition-colors duration-150 hover:bg-surface"
```

将 `class="user-menu-dropdown"` 替换为（注意 is-open 逻辑）:
```rust
class="user-menu-dropdown absolute top-[calc(100%+8px)] right-0 min-w-[240px] bg-bg border border-border-soft rounded-md shadow-lg opacity-0 invisible -translate-y-2 transition-[opacity,transform,visibility] duration-150"
```

注意：`.user-menu.is-open .user-menu-dropdown` 需要 is-open 时变为 visible。保留 `user-menu-dropdown` class 名，然后在 base.css 中保留一条残留规则直到 P9：
```css
.user-menu.is-open .user-menu-dropdown { opacity: 1; visibility: visible; transform: translateY(0); }
```
或者使用 UnoCSS 的 `[.is-open_&]:opacity-100` 变体语法（如果 presetWind4 支持）。

替代方案（推荐）：给 dropdown 追加 UnoCSS group 变体——在 user-menu 上用 `group`，dropdown 上用 `group-[.is-open]:opacity-100 group-[.is-open]:visible group-[.is-open]:translate-y-0`。需要将 user-menu class 改为 `user-menu group relative`。

将 `class="user-menu-header"` 替换为:
```rust
class="flex items-center gap-3 py-3 px-2 mb-2"
```

将 `class="user-menu-info"` 替换为:
```rust
class="flex flex-col min-w-0"
```

将 `class="user-menu-name"` 替换为:
```rust
class="text-sm font-semibold text-fg"
```

将 `class="user-menu-email"` 替换为:
```rust
class="text-xs text-muted overflow-hidden text-ellipsis"
```

将 `class="user-menu-item"` 替换为:
```rust
class="group flex items-center gap-3 py-2 px-3 rounded-sm text-sm text-fg no-underline cursor-pointer transition-colors duration-150 hover:bg-surface"
```

给 user-menu-item 内 svg 追加 `w-4 h-4 text-muted group-hover:text-accent`。

将 `class="user-menu-divider"` 替换为:
```rust
class="h-px bg-border-soft my-2"
```

将 `class="user-menu-item user-menu-logout"` 替换为:
```rust
class="group flex items-center gap-3 py-2 px-3 rounded-sm text-sm text-danger no-underline cursor-pointer transition-colors duration-150 hover:bg-surface"
```

logout 内 svg 追加 `w-4 h-4 text-danger`。

将 `class="user-menu-form"` 替换为:
```rust
class="m-0"
```

- [ ] **Step 6: 从 base.css 删除已迁移定义**

删除行 265-312（`.avatar` 到 `.user-menu-form`）、1836-1848（`.avatar-sm`/`.avatar-c0`~`c7`）。

保留行 288-290（`.user-menu.is-open .user-menu-dropdown`）直到 is-open 逻辑完全用 group 变体替换。如果 Step 5 使用了 group 变体方案，则此处也可以删除。

- [ ] **Step 7: 构建并验证**

Run: `cd E:/work/abt && npm run build:css && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

Expected: 无 error。

---

### Task 5: Dropdown Backdrop 迁移 + 最终验证

**Files:**
- Modify: `static/base.css:572-573`（删除）
- Modify: 所有引用 `dropdown-backdrop` 的 `.rs` 文件

- [ ] **Step 1: 搜索 dropdown-backdrop 引用**

搜索 `abt-web/src/**/*.rs` 中 `dropdown-backdrop`。

- [ ] **Step 2: 替换 dropdown-backdrop**

将 `class="dropdown-backdrop"` 替换为:
```rust
class="dropdown-backdrop hidden fixed inset-0 z-49"
```

注意：保留 `dropdown-backdrop` class 名用于 `.row-actions:has(.row-actions-menu.is-open) .dropdown-backdrop { display: block; }` 的级联控制。

替代方案：在 row-actions 上根据 open 状态条件追加 class。检查实际 Maud 代码中 row-actions-menu 的 open 控制方式。

- [ ] **Step 3: 从 base.css 删除已迁移定义**

删除行 572-573（`.dropdown-backdrop` 和 `:has` 选择器）。

如果使用条件 class 方案，需要在 Maud 中将 `display:block` 的逻辑用 hyperscript 或条件 class 实现。

- [ ] **Step 4: 构建并验证**

Run: `cd E:/work/abt && npm run build:css && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

Expected: 无 error。

- [ ] **Step 5: 用 agent-browser 验证页面渲染**

Run:
```bash
agent-browser --cdp 9222 open "http://localhost:8000/admin/dashboard"
```

视觉验证：
- 侧边栏渲染正常（深色渐变背景、rail 图标、active 状态竖线指示器）
- 折叠/展开侧边栏正常
- 顶栏渲染正常（高度、面包屑、通知图标 + 红点）
- 用户菜单点击展开/关闭正常
- 用户菜单内头像、名称、邮箱渲染正常
- 移动端响应式（缩小窗口验证移动端导航栏和菜单按钮）

- [ ] **Step 6: 检查多个代表性页面**

用 agent-browser 打开以下页面，确认侧边栏和顶栏一致：
- Dashboard
- 用户列表（sidebar-user-avatar 验证）
- 任意 MES 页面

- [ ] **Step 7: Git 提交**

```bash
cd E:/work/abt && git add -A && git commit -m "refactor(css): P8 — migrate sidebar/header/user-menu to atomic UnoCSS

- Replace ~48 classes: #sidebar, sidebar-rail/rail-*, sidebar-body/nav/item/user,
  top-header/breadcrumb/header-icon-btn/header-dot, mobile-nav/mobile-menu-btn/
  mobile-sidebar-overlay, avatar/avatar-sm/avatar-c0~c7, user-menu-*,
  dropdown-backdrop
- Preserve hyperscript interaction hooks for sidebar collapse and user menu toggle
- Delete corresponding definitions from base.css"
```
