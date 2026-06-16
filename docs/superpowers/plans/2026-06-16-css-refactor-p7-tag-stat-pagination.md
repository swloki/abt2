# P7: Tag/Badge/Stat/Pagination 原子化迁移实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 base.css 中所有 tag-chip 变体、role-tag、stat-card/stat-grid/stat-mini、pagination、各类 badge（change/fqc/sys/status-banner）的 class 定义翻译为 UnoCSS 原子 class，在 Maud 模板中替换后删除 base.css 对应定义。

**Architecture:** P7 涉及 ~60 个 class，分布在所有 173 个 Maud 页面中。按族分 5 个 Task 执行：Tag Chip 族 → Role/Dept Tag 族 → Stat Card/Icon 族 → Pagination 族 → Badge 族。每批完成后从 base.css 删除已迁移的 class 定义，运行 `npm run build:css` 验证。Tag 变体共享同一基础原子串，仅颜色不同，通过追加颜色 class 实现差异。

**Tech Stack:** UnoCSS v66.7.0 + presetWind4, Rust/Maud, base.css

**设计文档:** `docs/superpowers/specs/2026-06-16-css-architecture-atomic-refactor-design.md`

---

## 原子 class 映射总表

### Tag Chip 族映射（base.css 行 1147-1153, 2012-2015, 2057-2061）

| 原 class | CSS 属性 | 原子 class 替换 |
|---|---|---|
| `tag-chip` | `display:inline-flex; align-items:center; padding:2px 10px; border-radius:pill; font-size:11px; font-weight:500; letter-spacing:.01em` | `inline-flex items-center px-2.5 py-0.5 rounded-full text-[11px] font-medium tracking-[0.01em]` |
| `tag-key` | `background:#e6f4ff; color:accent` | + `bg-[#e6f4ff] text-accent` |
| `tag-normal` | `background:surface; color:#666` | + `bg-surface text-[#666]` |
| `tag-potential` | `background:#f0fff0; color:success` | + `bg-[#f0fff0] text-success` |
| `tag-primary` | `background:#e8f4ff; color:accent` | + `bg-[#e8f4ff] text-accent` |
| `tag-inactive` (base.css 1152) | `background:#fff2f0; color:danger` | + `bg-[#fff2f0] text-danger` |
| `tag-inactive` (base.css 2060) | `background:surface; color:#8c8c8c; border:1px solid border` | + `bg-surface text-[#8c8c8c] border border-border` |
| `tag-danger` | `background:rgba(245,63,63,.08); color:danger` | + `bg-[rgba(245,63,63,0.08)] text-danger` |
| `tag-warn` | `background:rgba(250,173,20,.1); color:#d48806` | + `bg-[rgba(250,173,20,0.1)] text-[#d48806]` |
| `tag-info` | `background:rgba(22,119,255,.08); color:accent` | + `bg-[rgba(22,119,255,0.08)] text-accent` |
| `tag-muted` | `background:rgba(0,0,0,.04); color:muted` | + `bg-[rgba(0,0,0,0.04)] text-muted` |
| `tag-pill` | `font-size:10px; padding:2px 8px; border-radius:3px; font-weight:600; letter-spacing:.02em` | `inline-flex items-center text-[10px] px-2 py-0.5 rounded-[3px] font-semibold tracking-[0.02em]` |
| `tag-active` | `background:#f0fff0; color:#389e0d; border:1px solid #d1f5e0` | + `bg-[#f0fff0] text-[#389e0d] border border-[#d1f5e0]` |
| `tag-super` (base.css 1854) | `font-size:10px; padding:2px 6px; border-radius:3px; background:#f3e8ff; color:#7c3aed; border:1px solid #e8d5ff; margin-left:6px; font-weight:600; letter-spacing:.02em` | `inline-flex items-center text-[10px] px-1.5 py-0.5 rounded-[3px] bg-[#f3e8ff] text-[#7c3aed] border border-[#e8d5ff] ml-1.5 font-semibold tracking-[0.02em]` |
| `tag-super` (base.css 2059) | same as 1854 but no margin-left | 同上去掉 `ml-1.5` |
| `tag-dept` | `background:#e8f4ff; color:accent; border:1px solid #d6e4ff` | + `bg-[#e8f4ff] text-accent border border-[#d6e4ff]` |
| `tag-list` | `display:flex; flex-wrap:wrap; gap:space-2; padding:space-3 0` | `flex flex-wrap gap-2 py-3` |

### Role/Dept/Acquire/Type Tag 族映射（base.css 行 329-330, 1859-1868, 2103-2104, 2323-2334, 2879-2884, 4107-4114）

| 原 class | CSS 属性 | 原子 class 替换 |
|---|---|---|
| `tag-sys` | `font-size:11px; padding:2px 6px; border-radius:pill; background:#fff7e6; color:#fa8c16; border:1px solid #ffe7ba; font-weight:500` | `inline-flex items-center text-[11px] px-1.5 py-0.5 rounded-full bg-[#fff7e6] text-[#fa8c16] border border-[#ffe7ba] font-medium` |
| `tag-custom` | `font-size:11px; padding:2px 6px; border-radius:pill; background:#f0f5ff; color:accent; border:1px solid #d6e4ff; font-weight:500` | `inline-flex items-center text-[11px] px-1.5 py-0.5 rounded-full bg-[#f0f5ff] text-accent border border-[#d6e4ff] font-medium` |
| `role-tags` | `display:flex; flex-wrap:wrap; gap:4px` | `flex flex-wrap gap-1` |
| `role-tag` (base.css 1860) | `font-size:10px; padding:2px 7px; border-radius:3px; background:#e8f4ff; color:accent; border:1px solid #d6e4ff; font-weight:500` | `inline-flex items-center text-[10px] px-[7px] py-0.5 rounded-[3px] bg-[#e8f4ff] text-accent border border-[#d6e4ff] font-medium` |
| `role-tag` (base.css 2103) | `font-size:10px; padding:1px 6px; border-radius:3px; font-weight:500` | `inline-flex items-center text-[10px] px-1.5 py-px rounded-[3px] font-medium` + 颜色变体 |
| `role-tag-built-in` | `background:#fff7e6; color:#d46b08; border:1px solid #ffe7ba` | + `bg-[#fff7e6] text-[#d46b08] border border-[#ffe7ba]` |
| `dept-tag` | `font-size:10px; padding:2px 7px; border-radius:3px; background:#f0fff0; color:#389e0d; border:1px solid #d1f5e0; font-weight:500` | `inline-flex items-center text-[10px] px-[7px] py-0.5 rounded-[3px] bg-[#f0fff0] text-[#389e0d] border border-[#d1f5e0] font-medium` |
| `source-badge` | `display:inline-flex; align-items:center; gap:4px; padding:2px 10px; border-radius:pill; font-size:11px; font-weight:500; background:rgba(124,58,237,.08); color:#7c3aed` | `inline-flex items-center gap-1 px-2.5 py-0.5 rounded-full text-[11px] font-medium bg-[rgba(124,58,237,0.08)] text-[#7c3aed]` |
| `bom-level-badge` | `display:inline-flex; align-items:center; justify-content:center; min-width:22px; height:22px; border-radius:sm; font-size:11px; font-weight:700; line-height:1` | `inline-flex items-center justify-center min-w-[22px] h-[22px] rounded-sm text-[11px] font-bold leading-none` |
| `bom-level-badge.level-1` | `background:#f3e8ff; color:#7c3aed` | + `bg-[#f3e8ff] text-[#7c3aed]` |
| `bom-level-badge.level-2` | `background:#fef3c7; color:#b45309` | + `bg-[#fef3c7] text-[#b45309]` |
| `bom-level-badge.level-default` | `background:#f1f5f9; color:#64748b` | + `bg-[#f1f5f9] text-[#64748b]` |
| `priority-badge` | `display:inline-flex; align-items:center; justify-content:center; width:28px; height:28px; border-radius:sm; font-size:sm; font-weight:700; font-family:mono` | `inline-flex items-center justify-center w-7 h-7 rounded-sm text-sm font-bold font-mono` |
| `count-badge` | `display:inline-flex; align-items:center; justify-content:center; min-width:18px; height:18px; padding:0 5px; border-radius:9px; background:accent-bg; color:accent; font-size:11px; font-weight:600` | `inline-flex items-center justify-center min-w-[18px] h-[18px] px-[5px] rounded-[9px] bg-accent-bg text-accent text-[11px] font-semibold` |
| `acquire-tag` | `display:inline-flex; align-items:center; gap:3px; padding:2px 6px; border-radius:3px; font-size:10px; font-weight:600; letter-spacing:.02em` | `inline-flex items-center gap-[3px] px-1.5 py-0.5 rounded-[3px] text-[10px] font-semibold tracking-[0.02em]` |
| `acquire-tag.self` | `background:#fef3c7; color:#92400e` | + `bg-[#fef3c7] text-[#92400e]` |
| `acquire-tag.purchase` | `background:#dbeafe; color:#1d4ed8` | + `bg-[#dbeafe] text-[#1d4ed8]` |
| `acquire-tag.outsource` | `background:#ede9fe; color:#6d28d9` | + `bg-[#ede9fe] text-[#6d28d9]` |
| `acquire-tag.non-inventory` | `background:#f1f5f9; color:#64748b` | + `bg-[#f1f5f9] text-[#64748b]` |
| `type-tag` (base.css 2323) | `display:inline-flex; align-items:center; padding:2px 10px; border-radius:pill; font-size:11px; font-weight:500; font-family:mono; letter-spacing:.02em` | `inline-flex items-center px-2.5 py-0.5 rounded-full text-[11px] font-medium font-mono tracking-[0.02em]` |
| `type-tag-putaway` | `background:#e8f4ff; color:accent-active` | + `bg-[#e8f4ff] text-accent-active` |
| `type-tag-pick` | `background:#f0fff0; color:#389e0d` | + `bg-[#f0fff0] text-[#389e0d]` |
| `type-tag` (base.css 2879) | `display:inline-flex; align-items:center; padding:2px 10px; border-radius:pill; font-size:12px; font-weight:500` | `inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium` |
| `type-tag.full` | `background:rgba(37,99,235,.08); color:accent` | + `bg-[rgba(37,99,235,0.08)] text-accent` |
| `type-tag.process` | `background:rgba(250,140,22,.08); color:#fa8c16` | + `bg-[rgba(250,140,22,0.08)] text-[#fa8c16]` |
| `type-tag.material` | `background:rgba(114,46,209,.08); color:#722ed1` | + `bg-[rgba(114,46,209,0.08)] text-[#722ed1]` |
| `type-tag.rework` | `background:rgba(245,63,63,.06); color:#f53f3f` | + `bg-[rgba(245,63,63,0.06)] text-[#f53f3f]` |
| `type-tag.mto` | `background:rgba(37,99,235,.08); color:accent` | + `bg-[rgba(37,99,235,0.08)] text-accent` |

### Stat Card/Grid 族映射（base.css 行 317-325, 2229-2244, 2724, 2889-2890, 1951-1959, 4387-4401）

| 原 class | CSS 属性 | 原子 class 替换 |
|---|---|---|
| `stat-card` | `display:flex; align-items:center; gap:space-4; padding:space-5; background:bg; border:1px solid border-soft; border-radius:md; box-shadow:xs; transition:box-shadow 240ms` | `flex items-center gap-4 p-5 bg-bg border border-border-soft rounded-md shadow-xs transition-shadow duration-200` |
| `stat-icon` (base.css 318) | `width:48px; height:48px; border-radius:md; display:grid; place-items:center; flex-shrink:0` | `w-12 h-12 rounded-md grid place-items-center shrink-0` |
| `stat-icon` (base.css 2235) | `width:44px; height:44px; border-radius:md; display:grid; place-items:center; flex-shrink:0` | `w-11 h-11 rounded-md grid place-items-center shrink-0` |
| `stat-icon.blue` | `background:linear-gradient(135deg,#e6f4ff,#d6e8ff); color:accent` | + `bg-[linear-gradient(135deg,#e6f4ff,#d6e8ff)] text-accent` |
| `stat-icon.green` | `background:linear-gradient(135deg,#f0fff0,#e0ffe0); color:success` | + `bg-[linear-gradient(135deg,#f0fff0,#e0ffe0)] text-success` |
| `stat-icon.orange` | `background:linear-gradient(135deg,#fff8eb,#fff0d6); color:warn` | + `bg-[linear-gradient(135deg,#fff8eb,#fff0d6)] text-warn` |
| `stat-icon.red` | `background:linear-gradient(135deg,#fff2f0,#ffe8e6); color:danger` | + `bg-[linear-gradient(135deg,#fff2f0,#ffe8e6)] text-danger` |
| `stat-icon.purple` | `background:linear-gradient(135deg,#f3e8ff,#e8d5ff); color:#7c3aed` | + `bg-[linear-gradient(135deg,#f3e8ff,#e8d5ff)] text-[#7c3aed]` |
| `stat-icon-purple` | `background:linear-gradient(135deg,#f3e8ff,#e8d5ff) !important; color:#7c3aed !important` | `bg-[linear-gradient(135deg,#f3e8ff,#e8d5ff)]! text-[#7c3aed]!` |
| `stat-value` (base.css 324) | `font-size:2xl; font-weight:700; line-height:1.1; letter-spacing:-.02em` | `text-2xl font-bold leading-[1.1] tracking-[-0.02em]` |
| `stat-value` (base.css 2243) | `font-size:24px; font-weight:700; color:fg; line-height:1.2` | `text-2xl font-bold text-fg leading-tight` |
| `stat-label` (base.css 325) | `font-size:12px; color:muted; margin-top:3px; font-weight:500` | `text-xs text-muted mt-[3px] font-medium` |
| `stat-label` (base.css 2244) | `font-size:13px; color:muted; margin-top:2px` | `text-[13px] text-muted mt-0.5` |
| `stat-grid-4` | `display:grid; grid-template-columns:repeat(4,1fr); gap:space-5; margin-bottom:space-6` | `grid grid-cols-4 gap-5 mb-6` |
| `stat-grid-5` | `display:grid; grid-template-columns:repeat(5,1fr); gap:space-5; margin-bottom:space-6` | `grid grid-cols-5 gap-5 mb-6` |
| `stat-chip` | `display:inline-flex; align-items:center; gap:4px; padding:4px 12px; background:surface; border-radius:pill; font-size:12px; color:muted; font-weight:500` | `inline-flex items-center gap-1 px-3 py-1 bg-surface rounded-full text-xs text-muted font-medium` |
| `dash-stat` | `background:#fff; border:1px solid border-soft; border-radius:md; padding:space-5; box-shadow:xs; transition:box-shadow 240ms` | `bg-white border border-border-soft rounded-md p-5 shadow-xs transition-shadow duration-200` |
| `board-stats` | `display:flex; gap:12px; margin-bottom:20px` | `flex gap-3 mb-5` |
| `board-stat-card` | `flex:1; display:flex; flex-direction:column; align-items:center; padding:16px 12px; background:#fff; border-radius:12px; box-shadow:0 1px 3px rgba(0,0,0,.06); border:1px solid #f0f0f0; position:relative; overflow:hidden` | `flex-1 flex flex-col items-center p-4 px-3 bg-white rounded-xl shadow-[0_1px_3px_rgba(0,0,0,0.06)] border border-[#f0f0f0] relative overflow-hidden` |
| `board-stat-value` | `font-size:28px; font-weight:700; line-height:1.2` | `text-[28px] font-bold leading-tight` |
| `board-stat-label` | `font-size:13px; color:#8c8c8c; margin-top:4px` | `text-[13px] text-[#8c8c8c] mt-1` |
| `board-stat-card.bs-primary::before` | `content:''; position:absolute; left:0; top:0; bottom:0; width:4px; background:#4f7df9` | + `before:content-[''] before:absolute before:left-0 before:top-0 before:bottom-0 before:w-1 before:bg-[#4f7df9]` |
| `board-stat-card.bs-pending::before` | `... background:#fa8c16` | + `before:bg-[#fa8c16]` |
| `board-stat-card.bs-progress::before` | `... background:#52c41a` | + `before:bg-[#52c41a]` |
| `board-stat-card.bs-receipt::before` | `... background:#722ed1` | + `before:bg-[#722ed1]` |
| `board-stat-card.bs-done::before` | `... background:#8c8c8c` | + `before:bg-[#8c8c8c]` |

### Pagination 族映射（base.css 行 593-603, 2141, 3491-3492）

| 原 class | CSS 属性 | 原子 class 替换 |
|---|---|---|
| `pagination` | `display:flex; align-items:center; justify-content:space-between; padding:space-4 space-5; font-size:xs; color:muted` | `flex items-center justify-between px-5 py-4 text-xs text-muted` |
| `pagination-pages` | `display:flex; gap:space-1` | `flex gap-1` |
| `page-btn` | `width:34px; height:34px; display:grid; place-items:center; border:1px solid border-soft; border-radius:sm; background:bg; color:fg; font-size:sm; cursor:pointer; transition:all fast` | `w-[34px] h-[34px] grid place-items-center border border-border-soft rounded-sm bg-bg text-fg text-sm cursor-pointer transition-all duration-150` |
| `page-btn:hover` | `border-color:accent; color:accent; background:accent-bg` | `hover:border-accent hover:text-accent hover:bg-accent-bg` |
| `page-btn.active` | `background:accent; border-color:accent; color:#fff; box-shadow:0 1px 4px rgba(37,99,235,.25)` | `bg-accent border-accent text-white shadow-[0_1px_4px_rgba(37,99,235,0.25)]` |
| `pagination-info` | `font-size:13px; color:muted` | `text-[13px] text-muted` |

### Badge 族映射（base.css 行 774-779, 1689-1691, 1902, 3188-3192, 4363-4367）

| 原 class | CSS 属性 | 原子 class 替换 |
|---|---|---|
| `change-badge` | `font-size:11px; padding:2px 8px; border-radius:pill; font-weight:600; margin-left:auto` | `text-[11px] px-2 py-0.5 rounded-full font-semibold ml-auto inline-flex` |
| `change-badge.up` | `background:#fff1f0; color:#cf1322` | + `bg-[#fff1f0] text-[#cf1322]` |
| `change-badge.down` | `background:#f6ffed; color:#389e0d` | + `bg-[#f6ffed] text-[#389e0d]` |
| `fqc-badge` | `display:inline-flex; align-items:center; gap:4px; padding:4px 10px; border-radius:12px; font-size:13px; font-weight:500` | `inline-flex items-center gap-1 px-2.5 py-1 rounded-xl text-[13px] font-medium` |
| `fqc-badge--na` | `background:#f5f5f5; color:#999` | + `bg-[#f5f5f5] text-[#999]` |
| `fqc-badge--pending` | `background:rgba(255,159,67,.08); color:#ff9f43` | + `bg-[rgba(255,159,67,0.08)] text-[#ff9f43]` |
| `fqc-badge--passed` | `background:rgba(82,196,26,.08); color:#52c41a` | + `bg-[rgba(82,196,26,0.08)] text-[#52c41a]` |
| `fqc-badge--failed` | `background:rgba(245,63,63,.06); color:#f53f3f` | + `bg-[rgba(245,63,63,0.06)] text-[#f53f3f]` |
| `sys-badge` | `font-size:10px; padding:1px 4px; border-radius:3px; background:#fff7e6; color:#fa8c16; border:1px solid #ffe7ba; margin-left:auto` | `inline-flex items-center text-[10px] px-1 py-px rounded-[3px] bg-[#fff7e6] text-[#fa8c16] border border-[#ffe7ba] ml-auto` |
| `status-banner` | `padding:8px 24px; display:flex; align-items:center; gap:8px; font-size:13px` | `px-6 py-2 flex items-center gap-2 text-[13px]` |
| `status-banner.success` | `background:#ecfdf5; color:#059669; border-bottom:1px solid #d1fae5` | + `bg-[#ecfdf5] text-[#059669] border-b border-[#d1fae5]` |
| `quick-card-badge` | `display:inline-block; padding:2px 8px; border-radius:pill; font-size:10px; font-weight:600; letter-spacing:.03em; margin-top:space-2` | `inline-block px-2 py-0.5 rounded-full text-[10px] font-semibold tracking-[0.03em] mt-2` |
| `quick-card-badge.blue` | `background:#dbeafe; color:#2563eb` | + `bg-[#dbeafe] text-[#2563eb]` |
| `quick-card-badge.purple` | `background:#ede9fe; color:#7c3aed` | + `bg-[#ede9fe] text-[#7c3aed]` |
| `quick-card-badge.green` | `background:#dcfce7; color:#16a34a` | + `bg-[#dcfce7] text-[#16a34a]` |
| `quick-card-badge.orange` | `background:#fef3c7; color:#d97706` | + `bg-[#fef3c7] text-[#d97706]` |

---

### Task 1: Tag Chip 族迁移

**Files:**
- Modify: `static/base.css:1146-1153, 2011-2015, 2057-2061`（删除 tag-chip/tag-key/tag-normal/tag-potential/tag-primary/tag-inactive/tag-danger/tag-warn/tag-info/tag-muted/tag-pill/tag-active/tag-super/tag-dept/tag-list 定义）
- Modify: 所有引用以上 class 的 `.rs` 文件（使用 `search` 定位）

- [ ] **Step 1: 在 Maud 模板中搜索所有 tag-chip 族引用**

Run（使用 search 工具）:
- 搜索 `abt-web/src/**/*.rs` 中 `tag-chip`, `tag-key`, `tag-normal`, `tag-potential`, `tag-primary`, `tag-inactive`, `tag-danger`, `tag-warn`, `tag-info`, `tag-muted`, `tag-pill`, `tag-active`, `tag-super`, `tag-dept`, `tag-list`

记录所有引用文件和行号。

- [ ] **Step 2: 逐文件替换 tag-chip 基础 class**

将每个 `class="tag-chip ..."` 替换为:
```rust
class="inline-flex items-center px-2.5 py-0.5 rounded-full text-[11px] font-medium tracking-[0.01em] ..."
```
保留追加的颜色变体（如 `tag-key` → 追加 `bg-[#e6f4ff] text-accent`，合并到同一 class 串中）。

- [ ] **Step 3: 替换 tag-pill 系列**

将 `class="tag-pill tag-active"` 替换为:
```rust
class="inline-flex items-center text-[10px] px-2 py-0.5 rounded-[3px] font-semibold tracking-[0.02em] bg-[#f0fff0] text-[#389e0d] border border-[#d1f5e0]"
```

将 `class="tag-pill tag-inactive"` 替换为:
```rust
class="inline-flex items-center text-[10px] px-2 py-0.5 rounded-[3px] font-semibold tracking-[0.02em] bg-surface text-[#8c8c8c] border border-border"
```

- [ ] **Step 4: 替换 tag-list**

将 `class="tag-list"` 替换为:
```rust
class="flex flex-wrap gap-2 py-3"
```

- [ ] **Step 5: 从 base.css 删除已迁移的 tag chip 定义**

删除 base.css 行 1146-1161（`/* ─── Tag Chips ─── */` 注释到 `.tag-list` 行）和行 2011-2061（`.tag-danger` 到 `.tag-dept` 区块）。

- [ ] **Step 6: 构建并验证**

Run: `cd E:/work/abt && npm run build:css`

Expected: 成功，无错误。

---

### Task 2: Role/Dept/Acquire/Type Tag 族迁移

**Files:**
- Modify: `static/base.css:329-330, 1854-1868, 2103-2104, 2323-2334, 2879-2884, 4107-4114, 4137-4147`（删除）
- Modify: 所有引用以上 class 的 `.rs` 文件

- [ ] **Step 1: 搜索所有引用**

搜索 `abt-web/src/**/*.rs` 中 `tag-sys`, `tag-custom`, `role-tag`, `role-tags`, `role-tag-built-in`, `dept-tag`, `source-badge`, `bom-level-badge`, `priority-badge`, `count-badge`, `acquire-tag`, `type-tag`, `type-tag-pick`, `type-tag-putaway`。

- [ ] **Step 2: 替换 role-tag 系列**

将 `class="role-tags"` 替换为:
```rust
class="flex flex-wrap gap-1"
```

将 `class="role-tag"` (with built-in variant `class="role-tag role-tag-built-in"`) 替换为:
```rust
class="inline-flex items-center text-[10px] px-1.5 py-px rounded-[3px] font-medium bg-[#fff7e6] text-[#d46b08] border border-[#ffe7ba]"
```

将 `class="role-tag"` (default accent variant) 替换为:
```rust
class="inline-flex items-center text-[10px] px-[7px] py-0.5 rounded-[3px] bg-[#e8f4ff] text-accent border border-[#d6e4ff] font-medium"
```

- [ ] **Step 3: 替换 dept-tag**

将 `class="dept-tag"` 替换为:
```rust
class="inline-flex items-center text-[10px] px-[7px] py-0.5 rounded-[3px] bg-[#f0fff0] text-[#389e0d] border border-[#d1f5e0] font-medium"
```

- [ ] **Step 4: 替换 acquire-tag 系列**

将 `class="acquire-tag self"` 替换为:
```rust
class="inline-flex items-center gap-[3px] px-1.5 py-0.5 rounded-[3px] text-[10px] font-semibold tracking-[0.02em] bg-[#fef3c7] text-[#92400e]"
```

同理处理 `purchase`（`bg-[#dbeafe] text-[#1d4ed8]`）、`outsource`（`bg-[#ede9fe] text-[#6d28d9]`）、`non-inventory`（`bg-[#f1f5f9] text-[#64748b]`）。

- [ ] **Step 5: 替换 type-tag 系列**

将 `class="type-tag type-tag-pick"` 替换为:
```rust
class="inline-flex items-center px-2.5 py-0.5 rounded-full text-[11px] font-medium font-mono tracking-[0.02em] bg-[#f0fff0] text-[#389e0d]"
```

将 `class="type-tag type-tag-putaway"` 替换为:
```rust
class="inline-flex items-center px-2.5 py-0.5 rounded-full text-[11px] font-medium font-mono tracking-[0.02em] bg-[#e8f4ff] text-accent-active"
```

将 MES 工单 `type-tag` 变体（`.full`/`.process`/`.material`/`.rework`/`.mto`）按映射表替换。

- [ ] **Step 6: 替换 source-badge**

将 `class="source-badge"` 替换为:
```rust
class="inline-flex items-center gap-1 px-2.5 py-0.5 rounded-full text-[11px] font-medium bg-[rgba(124,58,237,0.08)] text-[#7c3aed]"
```

- [ ] **Step 7: 替换 bom-level-badge 系列**

将 `class="bom-level-badge level-1"` 替换为:
```rust
class="inline-flex items-center justify-center min-w-[22px] h-[22px] rounded-sm text-[11px] font-bold leading-none bg-[#f3e8ff] text-[#7c3aed]"
```

同理处理 `level-2`（`bg-[#fef3c7] text-[#b45309]`）和 `level-default`（`bg-[#f1f5f9] text-[#64748b]`）。

- [ ] **Step 8: 替换 priority-badge 和 count-badge**

将 `class="priority-badge"` 替换为:
```rust
class="inline-flex items-center justify-center w-7 h-7 rounded-sm text-sm font-bold font-mono"
```

将 `class="count-badge"` 替换为:
```rust
class="inline-flex items-center justify-center min-w-[18px] h-[18px] px-[5px] rounded-[9px] bg-accent-bg text-accent text-[11px] font-semibold"
```

- [ ] **Step 9: 从 base.css 删除已迁移定义**

删除行 329-330（`.tag-sys`/`.tag-custom`）、1854-1868（`.tag-super`/`.role-tags`/`.role-tag`/`.dept-tag`）、2103-2104（`.role-tag`/`.role-tag-built-in`）、2323-2334（`.type-tag` 系列）、2879-2884（`.type-tag` MES 变体）、4107-4114（`.acquire-tag` 系列）、4137-4147（`.source-badge`）、4247（`.count-badge`）、1280-1287（`.bom-level-badge` 系列）、2330-2334（`.priority-badge`）。

- [ ] **Step 10: 构建并验证**

Run: `cd E:/work/abt && npm run build:css && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

Expected: 无 error。

---

### Task 3: Stat Card/Grid 族迁移

**Files:**
- Modify: `static/base.css:317-325, 1159, 1601-1608, 1877-1880, 1951-1959, 2229-2244, 2724, 2889-2890, 4387-4401, 4291-4292`（删除）
- Modify: 所有引用以上 class 的 `.rs` 文件

- [ ] **Step 1: 搜索所有引用**

搜索 `abt-web/src/**/*.rs` 中 `stat-card`, `stat-value`, `stat-label`, `stat-icon`, `stat-grid-4`, `stat-grid-5`, `stat-chip`, `stat-item`, `stat-progress`, `stat-mini`, `stat-mini-grid`, `dash-stat`, `board-stat-card`, `board-stat-value`, `board-stat-label`。

- [ ] **Step 2: 替换 stat-card 族**

将 `class="stat-card"` 替换为:
```rust
class="flex items-center gap-4 p-5 bg-bg border border-border-soft rounded-md shadow-xs transition-shadow duration-200"
```

将 `class="stat-icon blue"` 替换为:
```rust
class="w-12 h-12 rounded-md grid place-items-center shrink-0 bg-[linear-gradient(135deg,#e6f4ff,#d6e8ff)] text-accent"
```

同理处理 `green`/`orange`/`red`/`purple` 变体（按映射表替换渐变色和文字色）。

- [ ] **Step 3: 替换 stat-grid 系列**

将 `class="stat-grid-4"` 替换为:
```rust
class="grid grid-cols-4 gap-5 mb-6"
```

将 `class="stat-grid-5"` 替换为:
```rust
class="grid grid-cols-5 gap-5 mb-6"
```

- [ ] **Step 4: 替换 stat-value / stat-label**

将 `class="stat-value"` 替换为:
```rust
class="text-2xl font-bold leading-[1.1] tracking-[-0.02em]"
```

将 `class="stat-label"` 替换为:
```rust
class="text-xs text-muted mt-[3px] font-medium"
```

注意：stat-value 和 stat-label 在 base.css 中有两处定义（行 324-325 和行 2243-2244），属性略有不同（font-size 和 margin-top）。在 Maud 模板中根据上下文选择对应映射。行 324（48px icon 版）用于大卡片，行 2243（44px icon 版）用于 dashboard 小卡片。

- [ ] **Step 5: 替换 stat-chip**

将 `class="stat-chip"` 替换为:
```rust
class="inline-flex items-center gap-1 px-3 py-1 bg-surface rounded-full text-xs text-muted font-medium"
```

- [ ] **Step 6: 替换 dash-stat**

将 `class="dash-stat"` 替换为:
```rust
class="bg-white border border-border-soft rounded-md p-5 shadow-xs transition-shadow duration-200"
```

- [ ] **Step 7: 替换 board-stat-card 族**

将 `class="board-stat-card bs-primary"` 替换为:
```rust
class="flex-1 flex flex-col items-center p-4 px-3 bg-white rounded-xl shadow-[0_1px_3px_rgba(0,0,0,0.06)] border border-[#f0f0f0] relative overflow-hidden before:content-[''] before:absolute before:left-0 before:top-0 before:bottom-0 before:w-1 before:bg-[#4f7df9]"
```

同理处理 `bs-pending`（`before:bg-[#fa8c16]`）、`bs-progress`（`before:bg-[#52c41a]`）、`bs-receipt`（`before:bg-[#722ed1]`）、`bs-done`（`before:bg-[#8c8c8c]`）。

board-stat-value 中的颜色变体（`.bs-primary .board-stat-value` 等）直接在 Maud 中追加文字颜色 class（如 `text-[#4f7df9]`）。

将 `class="board-stat-value"` 替换为:
```rust
class="text-[28px] font-bold leading-tight"
```

将 `class="board-stat-label"` 替换为:
```rust
class="text-[13px] text-[#8c8c8c] mt-1"
```

- [ ] **Step 8: 从 base.css 删除已迁移定义**

删除行 317-325（`/* ─── Stat Icons ─── */` 区块）、1159（`.stat-chip`）、1601-1608（`.stat-item`/`.stat-label`/`.stat-value`/`.stat-progress`）、1877-1880（`.stat-icon-purple`）、1951-1959（`.stat-mini-grid`/`.stat-mini` 系列）、2229-2244（`/* Stat card for dashboard */` 区块）、2724（`.dash-stat`）、2889-2890（`.stat-grid-5`/`.stat-grid-4`）、4387-4401（`.board-stats`/`.board-stat-card` 系列）、4291-4292（`.flow-stat .stat-label`/`.flow-stat .stat-val` — flow-stat 中的 stat-label 引用）。

注意：行 4291 的 `.flow-stat .stat-label` 是在 P9 的 flow-stat 迁移中处理，此处跳过。

- [ ] **Step 9: 构建并验证**

Run: `cd E:/work/abt && npm run build:css && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

Expected: 无 error。

---

### Task 4: Pagination 族迁移

**Files:**
- Modify: `static/base.css:593-603, 2141, 3491-3492`（删除）
- Modify: 所有引用以上 class 的 `.rs` 文件

- [ ] **Step 1: 搜索所有引用**

搜索 `abt-web/src/**/*.rs` 中 `pagination`, `pagination-pages`, `page-btn`, `pagination-info`。

- [ ] **Step 2: 替换 pagination 容器**

将 `class="pagination"` 替换为:
```rust
class="flex items-center justify-between px-5 py-4 text-xs text-muted"
```

将 `class="pagination-pages"` 替换为:
```rust
class="flex gap-1"
```

将 `class="pagination-info"` 替换为:
```rust
class="text-[13px] text-muted"
```

- [ ] **Step 3: 替换 page-btn**

将 `class="page-btn"` 替换为:
```rust
class="w-[34px] h-[34px] grid place-items-center border border-border-soft rounded-sm bg-bg text-fg text-sm cursor-pointer transition-all duration-150 hover:border-accent hover:text-accent hover:bg-accent-bg"
```

将 `class="page-btn active"` 替换为:
```rust
class="w-[34px] h-[34px] grid place-items-center border border-accent rounded-sm bg-accent text-white text-sm cursor-pointer shadow-[0_1px_4px_rgba(37,99,235,0.25)]"
```

- [ ] **Step 4: 从 base.css 删除已迁移定义**

删除行 593-603（`/* ─── Pagination ─── */` 区块）、2141（`.pagination-info`）、3491-3492（FMS scoped `.fms-list-page .pagination` 和 `.fms-list-page .page-btn.active` — 这些在 P9 FMS 迁移中处理，此处仅删除全局定义）。

- [ ] **Step 5: 构建并验证**

Run: `cd E:/work/abt && npm run build:css && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

Expected: 无 error。

---

### Task 5: Badge 族迁移

**Files:**
- Modify: `static/base.css:774-779, 1689-1691, 1902, 3188-3192, 4363-4367`（删除）
- Modify: 所有引用以上 class 的 `.rs` 文件

- [ ] **Step 1: 搜索所有引用**

搜索 `abt-web/src/**/*.rs` 中 `change-badge`, `fqc-badge`, `sys-badge`, `status-banner`, `quick-card-badge`。

- [ ] **Step 2: 替换 change-badge**

将 `class="change-badge up"` 替换为:
```rust
class="inline-flex text-[11px] px-2 py-0.5 rounded-full font-semibold ml-auto bg-[#fff1f0] text-[#cf1322]"
```

将 `class="change-badge down"` 替换为:
```rust
class="inline-flex text-[11px] px-2 py-0.5 rounded-full font-semibold ml-auto bg-[#f6ffed] text-[#389e0d]"
```

- [ ] **Step 3: 替换 fqc-badge 系列**

将 `class="fqc-badge fqc-badge--na"` 替换为:
```rust
class="inline-flex items-center gap-1 px-2.5 py-1 rounded-xl text-[13px] font-medium bg-[#f5f5f5] text-[#999]"
```

将 `class="fqc-badge fqc-badge--pending"` 替换为:
```rust
class="inline-flex items-center gap-1 px-2.5 py-1 rounded-xl text-[13px] font-medium bg-[rgba(255,159,67,0.08)] text-[#ff9f43]"
```

将 `class="fqc-badge fqc-badge--passed"` 替换为:
```rust
class="inline-flex items-center gap-1 px-2.5 py-1 rounded-xl text-[13px] font-medium bg-[rgba(82,196,26,0.08)] text-[#52c41a]"
```

将 `class="fqc-badge fqc-badge--failed"` 替换为:
```rust
class="inline-flex items-center gap-1 px-2.5 py-1 rounded-xl text-[13px] font-medium bg-[rgba(245,63,63,0.06)] text-[#f53f3f]"
```

- [ ] **Step 4: 替换 sys-badge**

将 `class="sys-badge"` 替换为:
```rust
class="inline-flex items-center text-[10px] px-1 py-px rounded-[3px] bg-[#fff7e6] text-[#fa8c16] border border-[#ffe7ba] ml-auto"
```

- [ ] **Step 5: 替换 status-banner**

将 `class="status-banner success"` 替换为:
```rust
class="px-6 py-2 flex items-center gap-2 text-[13px] bg-[#ecfdf5] text-[#059669] border-b border-[#d1fae5]"
```

- [ ] **Step 6: 替换 quick-card-badge 系列**

将 `class="quick-card-badge blue"` 替换为:
```rust
class="inline-block px-2 py-0.5 rounded-full text-[10px] font-semibold tracking-[0.03em] mt-2 bg-[#dbeafe] text-[#2563eb]"
```

同理处理 `purple`（`bg-[#ede9fe] text-[#7c3aed]`）、`green`（`bg-[#dcfce7] text-[#16a34a]`）、`orange`（`bg-[#fef3c7] text-[#d97706]`）。

- [ ] **Step 7: 从 base.css 删除已迁移定义**

删除行 774-779（`.change-badge` 系列）、1689-1691（`.status-banner`）、1902（`.sys-badge`）、3188-3192（`.quick-card-badge` 系列）、4362-4367（`/* ── FQC Badge ── */` 区块）。

- [ ] **Step 8: 构建并验证**

Run: `cd E:/work/abt && npm run build:css && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

Expected: 无 error。

---

### Task 6: 最终验证与提交

**Files:**
- 无文件修改（验证步骤）

- [ ] **Step 1: 用 agent-browser 验证页面渲染**

Run:
```bash
agent-browser --cdp 9222 open "http://localhost:8000/admin/dashboard"
agent-browser --cdp 9222 eval "JSON.stringify({
  statCard: getComputedStyle(document.querySelector('.stat-card, [class*=items-center][class*=rounded-md]'))?.display || 'N/A',
  tagChip: getComputedStyle(document.querySelector('[class*=rounded-full][class*=font-medium]'))?.borderRadius || 'N/A',
  pagination: getComputedStyle(document.querySelector('[class*=justify-between][class*=text-xs]'))?.display || 'N/A',
})"
```

Expected: 布局正常，无样式丢失。

- [ ] **Step 2: 检查多个代表性页面**

用 agent-browser 打开以下页面，视觉确认 tag/badge/stat/pagination 渲染正常：
- Dashboard（stat-card, stat-icon, quick-card-badge）
- 用户列表（role-tag, dept-tag, avatar, tag-super）
- QMS 列表（fqc-badge, status-pill）
- MES 工单列表（type-tag, board-stat-card）
- 任意含分页的列表页（pagination, page-btn）

- [ ] **Step 3: 验证 cargo clippy 无错误**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

Expected: 无 error。

- [ ] **Step 4: Git 提交**

```bash
cd E:/work/abt && git add -A && git commit -m "refactor(css): P7 — migrate tag/badge/stat/pagination to atomic UnoCSS

- Replace ~60 classes: tag-chip variants, role-tag, dept-tag, acquire-tag,
  type-tag, stat-card/stat-icon/stat-grid, board-stat, pagination, page-btn,
  change-badge, fqc-badge, sys-badge, status-banner, quick-card-badge
- Delete corresponding definitions from base.css
- All styles now expressed as inline atomic classes in Maud templates"
```
