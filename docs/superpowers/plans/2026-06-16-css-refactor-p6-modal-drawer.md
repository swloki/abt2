# P6: Modal + Drawer + Dialog 原子化迁移实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 modal-overlay/modal/modal-head/modal-body/modal-foot、drawer-overlay/drawer-panel/drawer-head/drawer-body/drawer-foot、dialog-overlay/dialog/dialog-body/dialog-foot 等 34 个弹窗类 class 从 base.css 迁移到 100% 原子 UnoCSS class。

**Architecture:** 三类弹窗组件（Modal、Drawer、Dialog）共享相似的覆盖层结构但有显著差异：Modal 居中弹出并使用 `.is-open` 切换状态，Drawer 右侧滑出使用 `.open` 切换状态，Dialog 是确认对话框也使用 `.open`。组件文件（`modal.rs`、`drawer.rs`、`confirm_dialog.rs`、`input_dialog.rs`）是所有弹窗的入口——优先迁移这 4 个组件文件即可覆盖大部分页面。剩余页面中直接内联编写弹窗结构的（如 `bom_edit.rs`、`om_outsourcing_detail.rs`、`mes_order_detail.rs`）需逐一迁移。base.css 中 dialog 有两处重复定义（行 1075-1106 和 3598-3636）。

**Tech Stack:** UnoCSS v66.7.0 + presetWind4, Rust + Maud HTML 宏

**设计文档:** `docs/superpowers/specs/2026-06-16-css-architecture-atomic-refactor-design.md`

---

## 原子 class 映射总表

### Modal 族

| 原 class | 原子 class 替换 |
|---|---|
| `modal-overlay` | `fixed inset-0 z-[1000] grid place-items-center opacity-0 invisible transition-all duration-240 ease-decelerate` |
| `modal-overlay.is-open` | 追加 `opacity-100 visible bg-[rgba(15,23,42,0.5)] backdrop-blur-sm` |
| `modal` | `bg-bg rounded-xl w-[680px] max-w-[92vw] max-h-[85vh] flex flex-col overflow-hidden shadow-[0_25px_60px_rgba(15,23,42,0.18),0_8px_20px_rgba(15,23,42,0.08),0_0_0_1px_rgba(255,255,255,0.1)]` |
| `modal-lg` | `w-[900px] max-w-[94vw]` |
| `modal-sm` | `max-w-[420px]` |
| `modal-import` | `max-w-[560px]` |
| `modal-head` | `px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0` |
| `modal-body` | `p-6 overflow-y-auto flex-1 min-h-0` |
| `modal-foot` | `px-6 py-4 border-t border-border-soft flex justify-end gap-3 bg-surface-raised shrink-0` |
| `modal-title` | `text-lg font-semibold text-fg mb-3` |
| `modal-desc` | `text-sm text-muted leading-relaxed mb-5` |
| `modal-actions` | `flex justify-end gap-3` |
| `modal-close-btn` | `bg-transparent border-none cursor-pointer text-xl text-muted p-1 hover:text-fg` |
| `modal-close-plain` | `bg-transparent border-none cursor-pointer text-xl text-muted p-1` |
| `modal-checkbox-list` | `flex flex-col gap-2 max-h-[400px] overflow-y-auto` |
| `modal-checkbox-label` | `flex items-center gap-2 cursor-pointer py-2` |

### Drawer 族

| 原 class | 原子 class 替换 |
|---|---|
| `drawer-overlay` | `fixed inset-0 bg-[rgba(0,0,0,0.35)] z-[1000] flex justify-end opacity-0 invisible transition-opacity duration-200` |
| `drawer-overlay.open` | 追加 `opacity-100 visible` |
| `drawer-panel` | `bg-white h-full w-[420px] max-w-[90vw] flex flex-col translate-x-full transition-transform duration-240 ease-[cubic-bezier(0.2,0,0,1)] shadow-[-8px_0_30px_rgba(0,0,0,0.1)]` |
| `.drawer-overlay.open .drawer-panel` | 需 JS 控制：`translate-x-0`（通过 hyperscript 添加 class 或内联 style） |
| `drawer-head` | `flex items-center justify-between px-6 py-5 border-b border-border-soft` |
| `drawer-head-left` | `flex items-center gap-3` |
| `drawer-head-icon` | `w-9 h-9 rounded-md bg-accent-bg flex items-center justify-center shrink-0` |
| `drawer-body` | `flex-1 overflow-y-auto p-6` |
| `drawer-section` | `mb-6 last:mb-0` |
| `drawer-label` | `text-[11px] font-semibold text-muted uppercase tracking-wide mb-3 pb-2 border-b border-border-soft` |
| `drawer-foot` | `px-6 py-4 border-t border-border-soft flex justify-end gap-3 bg-white shrink-0` |
| `drawer-tip` | `bg-accent-bg border border-accent/10 rounded-md px-4 py-3 text-xs text-fg-2 leading-relaxed flex gap-2` |
| `drawer-close` | `w-8 h-8 border-none bg-transparent cursor-pointer text-muted rounded-sm grid place-items-center transition-all hover:bg-surface hover:text-fg` |

### Dialog 族

| 原 class | 原子 class 替换 |
|---|---|
| `dialog-overlay` | `fixed inset-0 bg-[rgba(15,23,42,0.45)] backdrop-blur-md z-[1100] place-items-center` |
| `dialog` | `bg-bg rounded-lg w-[480px] max-w-[92vw] shadow-[0_25px_60px_rgba(15,23,42,0.18),0_8px_20px_rgba(15,23,42,0.08),0_0_0_1px_rgba(255,255,255,0.1)] animate-dialog-slide-in` |
| `dialog-icon-wrap` | `w-14 h-14 rounded-full bg-danger/10 flex items-center justify-center mx-auto mb-5` |
| `dialog-title` | `text-lg font-semibold text-fg text-center mb-2` |
| `dialog-desc` | `text-sm text-muted text-center leading-relaxed` |
| `dialog-body` | `p-8 flex flex-col items-center` |
| `dialog-foot` | `px-6 py-4 border-t border-border-soft flex justify-center gap-3 bg-surface` |

---

## 关于状态切换的重要说明

Modal/Drawer/Dialog 的显隐通过 CSS class 切换实现：
- **Modal**: `.modal-overlay.is-open` — 切换 opacity/visibility/background/backdrop
- **Drawer**: `.drawer-overlay.open` — 切换 opacity/visibility；`.drawer-panel` 的 `translateX` 由 `.open` 父级选择器控制
- **Dialog**: `.dialog-overlay.open` — 切换 display:none → display:grid

**原子化后的处理方案**：

由于 Hyperscript (`_=` 属性) 通过 `add .is-open` / `remove .is-open` / `add .open` / `remove .open` 来切换状态，原子化后这些 class 名仍需保留作为**纯状态标记 class**（无 CSS 定义），配合内联 style 或 UnoCSS 的 group/peer 机制控制显隐。

**推荐方案**：保留 `is-open` / `open` 作为标记 class，在组件中用 `[&.is-open]:` / `[&.open]:` UnoCSS arbitrary variant 组合实现状态样式。这样 Hyperscript 的 `add/remove .is-open` 逻辑完全不需要修改。

```rust
// modal-overlay 的原子 class（包含 is-open 状态样式）：
div class="modal-overlay fixed inset-0 z-[1000] grid place-items-center
           opacity-0 invisible transition-all duration-240 ease-decelerate
           [&.is-open]:opacity-100 [&.is-open]:visible
           [&.is-open]:bg-[rgba(15,23,42,0.5)] [&.is-open]:backdrop-blur-sm"
```

---

### Task 1: 迁移 modal.rs 组件文件

**Files:**
- Modify: `abt-web/src/components/modal.rs`

- [ ] **Step 1: 替换 modal.rs 中的 class 定义**

文件 `abt-web/src/components/modal.rs`。

将整个 `modal` 函数替换为：

```rust
pub fn modal(modal_id: &str, title: &str, submit_label: &str, form_id: &str, hx_post: &str, body: Markup) -> Markup {
    html! {
        div id=(modal_id) class="modal-overlay fixed inset-0 z-[1000] grid place-items-center opacity-0 invisible transition-all duration-240 ease-decelerate [&.is-open]:opacity-100 [&.is-open]:visible [&.is-open]:bg-[rgba(15,23,42,0.5)] [&.is-open]:backdrop-blur-sm"
            _="on click[me is event.target] remove .is-open" {
            form id=(form_id)
                class="bg-bg rounded-xl w-[680px] max-w-[92vw] max-h-[85vh] flex flex-col overflow-hidden shadow-[0_25px_60px_rgba(15,23,42,0.18),0_8px_20px_rgba(15,23,42,0.08),0_0_0_1px_rgba(255,255,255,0.1)]"
                hx-post=(hx_post) hx-swap="none"
                _="on 'htmx:afterRequest'[detail.xhr.status < 400] remove .is-open from closest .modal-overlay then reset me" {
                div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0" {
                    h2 class="text-lg font-semibold flex items-center gap-2 m-0" { (title) }
                    button type="button" class="bg-transparent border-none cursor-pointer text-xl text-muted p-1 hover:text-fg"
                        _=(format!("on click remove .is-open from closest .modal-overlay then reset #{}", form_id)) { "×" }
                }
                div class="p-6 overflow-y-auto flex-1 min-h-0" {
                    (body)
                }
                div class="px-6 py-4 border-t border-border-soft flex justify-end gap-3 bg-surface-raised shrink-0" {
                    button type="button" class="btn btn-default"
                        _=(format!("on click remove .is-open from closest .modal-overlay then reset #{}", form_id)) { "取消" }
                    button type="submit" class="btn btn-primary" { (submit_label) }
                }
            }
        }
    }
}
```

- [ ] **Step 2: 验证编译**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

Expected: 无 error

---

### Task 2: 迁移 drawer.rs 组件文件

**Files:**
- Modify: `abt-web/src/components/drawer.rs`

- [ ] **Step 1: 替换 drawer.rs 中的 class 定义**

文件 `abt-web/src/components/drawer.rs`。

将 `drawer` 和 `drawer_with_footer` 两个函数都替换。

`drawer` 函数：

```rust
pub fn drawer(drawer_id: &str, title: &str, submit_label: &str, form_id: &str, body: Markup) -> Markup {
    html! {
        div id=(drawer_id) class="drawer-overlay fixed inset-0 bg-[rgba(0,0,0,0.35)] z-[1000] flex justify-end opacity-0 invisible transition-opacity duration-200 [&.open]:opacity-100 [&.open]:visible"
            _="on click[me is event.target] remove .open" {
            div class="drawer-panel bg-white h-full w-[420px] max-w-[90vw] flex flex-col translate-x-full transition-transform duration-240 ease-[cubic-bezier(0.2,0,0,1)] shadow-[-8px_0_30px_rgba(0,0,0,0.1)] group-[.open]/drawer:translate-x-0"
                onclick="event.stopPropagation()" {
                div class="flex items-center justify-between px-6 py-5 border-b border-border-soft" {
                    h2 class="text-base font-semibold m-0" { (title) }
                    button class="bg-transparent border-none cursor-pointer text-[22px] text-muted p-1 leading-none hover:text-fg"
                        _="on click remove .open from closest .drawer-overlay" { "×" }
                }
                div class="flex-1 overflow-y-auto p-6" {
                    (body)
                }
                div class="px-6 py-4 border-t border-border-soft flex justify-end gap-3 bg-white shrink-0" {
                    button type="button" class="btn btn-default"
                        _="on click remove .open from closest .drawer-overlay" { "取消" }
                    button type="submit" class="btn btn-primary" form=(form_id) { (submit_label) }
                }
            }
        }
    }
}
```

> **注意 Drawer 的 transform 问题**：`.drawer-panel` 在 `.drawer-overlay.open` 下需要 `translateX(0)`。原子化后 `translate-x-full` 是初始态，`.open` 时需变为 `translate-x-0`。由于 UnoCSS 的 `[&.open]` 作用于自身而非父级，这里需要用 `group` 模式：在 overlay 上加 `group/drawer`，panel 上用 `group-[.open]/drawer:translate-x-0`。

更新 overlay class 添加 `group/drawer`：
```rust
div id=(drawer_id) class="drawer-overlay group/drawer fixed inset-0 bg-[rgba(0,0,0,0.35)] z-[1000] flex justify-end opacity-0 invisible transition-opacity duration-200 [.open]:opacity-100 [.open]:visible"
```

`drawer_with_footer` 函数：同样的替换模式，只有 footer 部分不同（使用传入的 `footer` Markup）。

- [ ] **Step 2: 验证编译**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

Expected: 无 error

---

### Task 3: 迁移 confirm_dialog.rs 组件文件

**Files:**
- Modify: `abt-web/src/components/confirm_dialog.rs`

- [ ] **Step 1: 替换 confirm_dialog.rs 中的 class 定义**

文件 `abt-web/src/components/confirm_dialog.rs`。

将整个 `confirm_dialog` 函数替换为：

```rust
pub fn confirm_dialog(
    dialog_id: &str,
    title: &str,
    desc: &str,
    confirm_label: &str,
    form_id: &str,
    form: Markup,
) -> Markup {
    html! {
        div id=(dialog_id) class="dialog-overlay fixed inset-0 bg-[rgba(15,23,42,0.45)] backdrop-blur-md z-[1100] place-items-center hidden [.open]:grid"
            _=(format!("on click[me is event.target] remove .open")) {
            div class="bg-bg rounded-lg w-[480px] max-w-[92vw] shadow-[0_25px_60px_rgba(15,23,42,0.18),0_8px_20px_rgba(15,23,42,0.08),0_0_0_1px_rgba(255,255,255,0.1)] animate-dialog-slide-in"
                onclick="event.stopPropagation()" {
                div class="p-8 pb-6 flex flex-col items-center" {
                    div class="w-14 h-14 rounded-full bg-danger/10 flex items-center justify-center mx-auto mb-5" {
                        (icon::circle_alert_icon("w-7 h-7"))
                    }
                    div class="text-lg font-semibold text-fg text-center mb-2" { (title) }
                    p class="text-sm text-muted text-center leading-relaxed m-0" { (maud::PreEscaped(desc)) }
                }
                div class="px-6 py-4 border-t border-border-soft flex justify-center gap-3 bg-surface" {
                    button type="button" class="btn btn-default min-w-[100px] justify-center"
                        _="on click remove .open from closest .dialog-overlay" { "取消" }
                    button type="button" class="btn btn-danger min-w-[100px] justify-center"
                        _=(format!("on click remove .open from closest .dialog-overlay then trigger submit on #{}", form_id))
                        { (confirm_label) }
                }
            }
            (form)
        }
    }
}
```

> **注意**：dialog-overlay 默认 `hidden`，`.open` 时变为 `grid`。用 `hidden [.open]:grid` 实现。Hyperscript 仍使用 `add .open` / `remove .open`。

- [ ] **Step 2: 验证编译**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

Expected: 无 error

---

### Task 4: 迁移 input_dialog.rs 组件文件

**Files:**
- Modify: `abt-web/src/components/input_dialog.rs`

- [ ] **Step 1: 替换 input_dialog.rs 中的 class 定义**

文件 `abt-web/src/components/input_dialog.rs`。

将 `input_dialog` 函数中的 class 替换为原子 class。结构使用 `modal-overlay` + `modal modal-sm`，与 Task 1 相同的模式：

```rust
pub fn input_dialog(
    dialog_id: &str, title: &str, desc: Markup,
    input_id: &str, input_label: &str, input_type: &str,
    input_placeholder: &str, input_step: &str,
    confirm_label: &str, confirm_action: &str,
) -> Markup {
    html! {
        div id=(dialog_id)
            class="modal-overlay fixed inset-0 z-[1000] grid place-items-center opacity-0 invisible transition-all duration-240 ease-decelerate [&.is-open]:opacity-100 [&.is-open]:visible [&.is-open]:bg-[rgba(15,23,42,0.5)] [&.is-open]:backdrop-blur-sm"
            _="on click[me is event.target] remove .is-open" {
            div class="bg-bg rounded-xl w-[680px] max-w-[92vw] max-h-[85vh] flex flex-col overflow-hidden shadow-[0_25px_60px_rgba(15,23,42,0.18),0_8px_20px_rgba(15,23,42,0.08),0_0_0_1px_rgba(255,255,255,0.1)] max-w-[420px]"
                onclick="event.stopPropagation()" {
                div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0" {
                    h2 class="text-lg font-semibold m-0" { (title) }
                }
                div class="p-6 overflow-y-auto flex-1 min-h-0" {
                    p class="text-sm text-muted leading-relaxed mb-5 m-0" { (desc) }
                    div class="form-field" {
                        label { (input_label) }
                        input id=(input_id) class="form-input" type=(input_type)
                            step=(input_step) min="1" placeholder=(input_placeholder);
                    }
                }
                div class="px-6 py-4 border-t border-border-soft flex justify-end gap-3 bg-surface-raised shrink-0" {
                    button type="button" class="btn btn-default"
                        _=(format!("on click remove .is-open from closest .modal-overlay")) {
                        "取消"
                    }
                    button type="button" class="btn btn-primary"
                        _=(format!("on click {confirm_action}")) {
                        (confirm_label) }
                }
            }
        }
    }
}
```

- [ ] **Step 2: 验证编译**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

Expected: 无 error

---

### Task 5: 迁移 entity_picker.rs / product_picker.rs / import_modal.rs 组件文件

**Files:**
- Modify: `abt-web/src/components/entity_picker.rs`
- Modify: `abt-web/src/components/product_picker.rs`
- Modify: `abt-web/src/components/import_modal.rs`

- [ ] **Step 1: 迁移 entity_picker.rs**

文件 `abt-web/src/components/entity_picker.rs`。

行 97-104 的 class 替换：

```rust
// 原：
// div class="modal-overlay" id=(cfg.modal_id) _=(open_hs) {
//     div class="modal modal-lg" _="on click halt" {
//         div class="modal-head" { ... }
//         div class="modal-body" style="padding:0" { ... }

// 替换为：
div class="modal-overlay fixed inset-0 z-[1000] grid place-items-center opacity-0 invisible transition-all duration-240 ease-decelerate [&.is-open]:opacity-100 [&.is-open]:visible [&.is-open]:bg-[rgba(15,23,42,0.5)] [&.is-open]:backdrop-blur-sm"
    id=(cfg.modal_id) _=(open_hs) {
    div class="bg-bg rounded-xl w-[900px] max-w-[94vw] max-h-[85vh] flex flex-col overflow-hidden shadow-[0_25px_60px_rgba(15,23,42,0.18),0_8px_20px_rgba(15,23,42,0.08),0_0_0_1px_rgba(255,255,255,0.1)]"
        _="on click halt" {
        div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0" { ... }
        div class="overflow-y-auto flex-1 min-h-0" style="padding:0" { ... }
```

注意 `modal-body` 原有 `style="padding:0"` 覆盖——原子化后直接不写 `p-6` 即可。

- [ ] **Step 2: 迁移 product_picker.rs**

文件 `abt-web/src/components/product_picker.rs`。

行 70-79 的 class 替换（与 entity_picker 相同模式）：

```rust
div class="modal-overlay fixed inset-0 z-[1000] grid place-items-center opacity-0 invisible transition-all duration-240 ease-decelerate [&.is-open]:opacity-100 [&.is-open]:visible [&.is-open]:bg-[rgba(15,23,42,0.5)] [&.is-open]:backdrop-blur-sm"
    id=(modal_id) _=(format!("on click remove .is-open from #{}", modal_id)) {
    div class="bg-bg rounded-xl w-[900px] max-w-[94vw] max-h-[85vh] flex flex-col overflow-hidden shadow-[0_25px_60px_rgba(15,23,42,0.18),0_8px_20px_rgba(15,23,42,0.08),0_0_0_1px_rgba(255,255,255,0.1)]"
        _="on click halt" {
        div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0" { ... }
        div class="overflow-y-auto flex-1 min-h-0" style="padding:0" { ... }
```

- [ ] **Step 3: 迁移 import_modal.rs**

文件 `abt-web/src/components/import_modal.rs`。

行 15-22 的 class 替换：

```rust
div id=(modal_id) class="modal-overlay fixed inset-0 z-[1000] grid place-items-center opacity-0 invisible transition-all duration-240 ease-decelerate [&.is-open]:opacity-100 [&.is-open]:visible [&.is-open]:bg-[rgba(15,23,42,0.5)] [&.is-open]:backdrop-blur-sm"
    _="on click[me is event.target] remove .is-open" {
    div class="bg-bg rounded-xl w-[680px] max-w-[92vw] max-h-[85vh] flex flex-col overflow-hidden shadow-[0_25px_60px_rgba(15,23,42,0.18),0_8px_20px_rgba(15,23,42,0.08),0_0_0_1px_rgba(255,255,255,0.1)] max-w-[560px]" {
        div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0" {
            h2 class="text-lg font-semibold m-0" { (config.title) }
            button type="button" class="bg-transparent border-none cursor-pointer text-xl text-muted p-1 hover:text-fg"
                _="on click remove .is-open from closest .modal-overlay" { "×" }
        }
        div class="p-6 overflow-y-auto flex-1 min-h-0" { ... }
```

行 131 的 modal-foot 也需替换：
```rust
div class="px-6 py-4 border-t border-border-soft flex justify-end gap-3 bg-surface-raised shrink-0" { ... }
```

- [ ] **Step 4: 验证编译**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

Expected: 无 error

---

### Task 6: 迁移页面内联 Modal（bom_edit, mes_order_create, mes_plan_create, purchase_*_create 等）

**Files:**
- Modify: `abt-web/src/pages/bom_edit.rs`
- Modify: `abt-web/src/pages/mes_order_create.rs`
- Modify: `abt-web/src/pages/mes_plan_create.rs`
- Modify: `abt-web/src/pages/purchase_order_create.rs`
- Modify: `abt-web/src/pages/purchase_order_edit.rs`
- Modify: `abt-web/src/pages/purchase_quotation_create.rs`
- Modify: `abt-web/src/pages/purchase_recon_create.rs`
- Modify: `abt-web/src/pages/quotation_create.rs`
- Modify: `abt-web/src/pages/product_create.rs`
- Modify: `abt-web/src/pages/om_outsourcing_create.rs`
- Modify: `abt-web/src/pages/purchase_approval_rules.rs`

这些页面不使用组件函数，而是直接在 Maud 模板中编写 modal HTML 结构。

- [ ] **Step 1: 迁移 bom_edit.rs 中的 modal**

文件 `abt-web/src/pages/bom_edit.rs`。

该文件有 4 处 modal（行 432、639、689、735）。逐一替换 class：

```rust
// modal-overlay（行 639、735 等）→
"modal-overlay fixed inset-0 z-[1000] grid place-items-center opacity-0 invisible transition-all duration-240 ease-decelerate [&.is-open]:opacity-100 [&.is-open]:visible [&.is-open]:bg-[rgba(15,23,42,0.5)] [&.is-open]:backdrop-blur-sm"

// modal modal-lg（行 641）→
"bg-bg rounded-xl w-[900px] max-w-[94vw] max-h-[85vh] flex flex-col overflow-hidden shadow-[0_25px_60px_rgba(15,23,42,0.18),0_8px_20px_rgba(15,23,42,0.08),0_0_0_1px_rgba(255,255,255,0.1)]"

// modal（行 737，普通宽度）→
"bg-bg rounded-xl w-[680px] max-w-[92vw] max-h-[85vh] flex flex-col overflow-hidden shadow-[0_25px_60px_rgba(15,23,42,0.18),0_8px_20px_rgba(15,23,42,0.08),0_0_0_1px_rgba(255,255,255,0.1)]"

// modal-head（行 642、738）→
"px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0"

// modal-body（行 647、743）→
"p-6 overflow-y-auto flex-1 min-h-0"
// 注意行 647 有 style="padding:0"，替换后省略 p-6

// modal-foot（行 467、751）→
"px-6 py-4 border-t border-border-soft flex justify-end gap-3 bg-surface-raised shrink-0"
```

- [ ] **Step 2: 迁移 mes_order_create.rs 中的 modal**

文件 `abt-web/src/pages/mes_order_create.rs`。

行 274（so-modal）和行 310（pp-modal）的 class 替换。使用 modal-lg 宽度变体。模式与 Task 6 Step 1 相同。

- [ ] **Step 3: 迁移 mes_plan_create.rs 中的 modal**

文件 `abt-web/src/pages/mes_plan_create.rs`。

行 212 的 product-picker modal 替换。使用 modal-lg 宽度变体。

- [ ] **Step 4: 迁移 purchase_*_create.rs 和 quotation_create.rs 中的 modal**

以下文件都有相同的 product-modal 结构（modal-overlay + modal modal-lg + modal-head + modal-body）：
- `purchase_order_create.rs`（行 544）
- `purchase_order_edit.rs`（行 394）
- `purchase_quotation_create.rs`（行 437）
- `quotation_create.rs`（行 436）

逐一替换。模式相同。

- [ ] **Step 5: 迁移 purchase_recon_create.rs 中的 modal**

文件 `abt-web/src/pages/purchase_recon_create.rs`。

行 229 的 order-modal 替换。modal-foot 有自定义 style 覆盖（行 251），保留 style 属性。

- [ ] **Step 6: 迁移 product_create.rs 中的 modal**

文件 `abt-web/src/pages/product_create.rs`。

行 275 的 category-modal 替换。普通宽度 modal。

- [ ] **Step 7: 迁移 om_outsourcing_create.rs 中的 modal**

文件 `abt-web/src/pages/om_outsourcing_create.rs`。

行 391 的 material-modal 替换。普通宽度 modal。

- [ ] **Step 8: 迁移 purchase_approval_rules.rs 中的 modal**

文件 `abt-web/src/pages/purchase_approval_rules.rs`。

行 396 的 rule-modal（overlay）和行 418 的 modal 替换。

- [ ] **Step 9: 验证编译**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

Expected: 无 error

---

### Task 7: 迁移页面内联 Modal（mes_order_detail, om_outsourcing_detail, product_list）

**Files:**
- Modify: `abt-web/src/pages/mes_order_detail.rs`
- Modify: `abt-web/src/pages/om_outsourcing_detail.rs`
- Modify: `abt-web/src/pages/product_list.rs`

- [ ] **Step 1: 迁移 mes_order_detail.rs 中的 modal**

文件 `abt-web/src/pages/mes_order_detail.rs`。

行 439（unrelease-dialog）和行 664（split-dialog）的 `modal-overlay` + `modal modal-sm` 替换。

```rust
// modal-overlay →
"modal-overlay fixed inset-0 z-[1000] grid place-items-center opacity-0 invisible transition-all duration-240 ease-decelerate [&.is-open]:opacity-100 [&.is-open]:visible [&.is-open]:bg-[rgba(15,23,42,0.5)] [&.is-open]:backdrop-blur-sm"

// modal modal-sm →
"bg-bg rounded-xl max-w-[420px] max-w-[92vw] max-h-[85vh] flex flex-col overflow-hidden shadow-[0_25px_60px_rgba(15,23,42,0.18),0_8px_20px_rgba(15,23,42,0.08),0_0_0_1px_rgba(255,255,255,0.1)]"
```

modal-desc（行 445、671）替换：
```rust
// p class="modal-desc" →
p class="text-sm text-muted leading-relaxed mb-5 m-0"
```

- [ ] **Step 2: 迁移 om_outsourcing_detail.rs 中的 4 个 modal**

文件 `abt-web/src/pages/om_outsourcing_detail.rs`。

行 556（record-node-modal）、行 623（receive-modal）、行 685（convert-modal）、行 722（cancel-modal）。

每个 modal 使用不同宽度（有的 `style="width:520px"`、有的 `style="width:480px"`、有的无宽度覆盖）。替换 modal-overlay/modal/modal-head/modal-body/modal-foot 的 class，保留各自的 style 属性。

```rust
// 例：record-node-modal（行 556-557）→
div id="record-node-modal" class="modal-overlay fixed inset-0 z-[1000] grid place-items-center opacity-0 invisible transition-all duration-240 ease-decelerate [&.is-open]:opacity-100 [&.is-open]:visible [&.is-open]:bg-[rgba(15,23,42,0.5)] [&.is-open]:backdrop-blur-sm"
    _="on click[me is event.target] remove .is-open" {
    div class="bg-bg rounded-xl w-[680px] max-w-[92vw] max-h-[85vh] flex flex-col overflow-hidden shadow-[0_25px_60px_rgba(15,23,42,0.18),0_8px_20px_rgba(15,23,42,0.08),0_0_0_1px_rgba(255,255,255,0.1)]"
        style="width:520px" {
```

- [ ] **Step 3: 迁移 product_list.rs 中的 modal 和 dialog**

文件 `abt-web/src/pages/product_list.rs`。

行 710 的 `dialog-overlay open` 和行 735 的 `modal-overlay is-open`：

```rust
// dialog-overlay open（行 710）→
div class="dialog-overlay fixed inset-0 bg-[rgba(15,23,42,0.45)] backdrop-blur-md z-[1100] place-items-center grid"
    _="on click remove .open" {
    div class="bg-bg rounded-lg w-[480px] max-w-[92vw] shadow-[0_25px_60px_rgba(15,23,42,0.18),0_8px_20px_rgba(15,23,42,0.08),0_0_0_1px_rgba(255,255,255,0.1)] animate-dialog-slide-in"
        onclick="event.stopPropagation()" {
        // dialog-body / dialog-foot 也需替换
    }
}

// modal-overlay is-open（行 735）→
div class="modal-overlay fixed inset-0 z-[1000] grid place-items-center opacity-100 visible bg-[rgba(15,23,42,0.5)] backdrop-blur-sm"
    _="on click remove .is-open" {
```

注意：`is-open` 在初始渲染时已写死在 class 中（非 JS 切换），原子化后直接写展开后的样式。

- [ ] **Step 4: 验证编译**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

Expected: 无 error

---

### Task 8: 迁移页面内联 Drawer（bom_detail, bom_list, department_list）

**Files:**
- Modify: `abt-web/src/pages/bom_detail.rs`
- Modify: `abt-web/src/pages/bom_list.rs`
- Modify: `abt-web/src/pages/department_list.rs`

- [ ] **Step 1: 迁移 bom_detail.rs 中的 drawer**

文件 `abt-web/src/pages/bom_detail.rs`。

行 327 的 cost-drawer 和行 348 的 labor-drawer。两者都有 `style="max-width:1000px;width:100%"` / `style="max-width:800px;width:100%"` 覆盖。

```rust
// drawer-overlay（行 327）→
div id="cost-drawer" class="drawer-overlay group/drawer fixed inset-0 bg-[rgba(0,0,0,0.35)] z-[1000] flex justify-end opacity-0 invisible transition-opacity duration-200 [.open]:opacity-100 [.open]:visible"
    _="on click remove .open from #cost-drawer" {
    div class="drawer-panel bg-white h-full w-[420px] max-w-[90vw] flex flex-col translate-x-full transition-transform duration-240 ease-[cubic-bezier(0.2,0,0,1)] shadow-[-8px_0_30px_rgba(0,0,0,0.1)] group-[.open]/drawer:translate-x-0"
        style="max-width:1000px;width:100%" onclick="event.stopPropagation()" {
        div class="flex items-center justify-between px-6 py-5 border-b border-border-soft" {
            // drawer-head 内容
        }
        // drawer-body / drawer-foot
    }
}
```

- [ ] **Step 2: 迁移 bom_list.rs 中的 drawer**

文件 `abt-web/src/pages/bom_list.rs`。

行 210（cost-drawer）和行 231（labor-drawer）。模式与 Step 1 相同。

- [ ] **Step 3: 迁移 department_list.rs 中的 drawer**

文件 `abt-web/src/pages/department_list.rs`。

行 282 的 deptDrawer 替换。drawer-panel（行 284）有 `id="drawerPanel"`。drawer-head（行 285 附近）使用 `drawer-head-left` 和 `drawer-head-icon` 子组件。

```rust
// drawer-overlay（行 282）→
div class="drawer-overlay group/drawer fixed inset-0 bg-[rgba(0,0,0,0.35)] z-[1000] flex justify-end opacity-0 invisible transition-opacity duration-200 [.open]:opacity-100 [.open]:visible"
    id="deptDrawer" tabindex="-1"
    _="on click[me is event.target] remove .open on keydown[event.key is 'Escape'] remove .open" {
    div class="drawer-panel bg-white h-full w-[420px] max-w-[90vw] flex flex-col translate-x-full transition-transform duration-240 ease-[cubic-bezier(0.2,0,0,1)] shadow-[-8px_0_30px_rgba(0,0,0,0.1)] group-[.open]/drawer:translate-x-0"
        id="drawerPanel" {
        // drawer-head / drawer-body / drawer-foot
    }
}

// drawer-head-left →
div class="flex items-center gap-3"

// drawer-head-icon →
div class="w-9 h-9 rounded-md bg-accent-bg flex items-center justify-center shrink-0"
```

- [ ] **Step 4: 迁移 layout/page.rs 中的 global confirm dialog**

文件 `abt-web/src/layout/page.rs`。

行 104 的 `dialog-overlay` 和行 105 的 `dialog`：

```rust
// dialog-overlay（行 104）→
div class="dialog-overlay fixed inset-0 bg-[rgba(15,23,42,0.45)] backdrop-blur-md z-[1100] place-items-center hidden [.open]:grid"
    _="on click remove .open" {
    div class="bg-bg rounded-lg w-[480px] max-w-[92vw] shadow-[0_25px_60px_rgba(15,23,42,0.18),0_8px_20px_rgba(15,23,42,0.08),0_0_0_1px_rgba(255,255,255,0.1)] animate-dialog-slide-in"
        onclick="event.stopPropagation()" {
        div class="p-8 pb-6 flex flex-col items-center" {
            div class="w-14 h-14 rounded-full bg-danger/10 flex items-center justify-center mx-auto mb-5" { (PreEscaped(icon)) }
            // dialog-title / dialog-desc
        }
        div class="px-6 py-4 border-t border-border-soft flex justify-center gap-3 bg-surface" {
            // buttons
        }
    }
}
```

- [ ] **Step 5: 验证编译**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

Expected: 无 error

---

### Task 9: 删除 base.css 中所有 Modal/Drawer/Dialog CSS 定义

**Files:**
- Modify: `static/base.css`

- [ ] **Step 1: 删除 base.css 行 627-662（Modal 第一组）**

删除整个 `/* ─── Modal ─── */` 块：
```css
.modal-overlay { ... }
.modal-overlay.is-open { ... }
.modal { ... }
.modal-overlay.is-open .modal { ... }
.modal-head { ... }
.modal-head h2 { ... }
.modal-body { ... }
.modal-foot { ... }
.modal-lg { ... }
```

- [ ] **Step 2: 删除 base.css 行 1075-1109（Dialog 第一组）**

删除整个 `/* ─── Confirm Dialog ─── */` 块：
```css
.dialog-overlay { ... }
.dialog-overlay.open { ... }
.dialog { ... }
.dialog-icon-wrap { ... }
.dialog-icon-wrap svg { ... }
.dialog-title { ... }
.dialog-desc { ... }
.dialog-desc strong { ... }
.dialog-body { ... }
.dialog-foot { ... }
.dialog-foot .btn { ... }
.btn-danger { ... }
.btn-danger:hover { ... }
@keyframes dialogSlideIn { ... }
```

注意：`@keyframes dialogSlideIn` 已在 P0 中迁移到 `theme.animation`，此处删除不丢失功能。`.btn-danger` 和 `.btn-danger:hover` 属于按钮样式，如果 P1 尚未迁移 btn 系列，暂时保留这两行。

- [ ] **Step 3: 删除 base.css 行 1162-1166（modal-checkbox + modal-close-btn）**

删除：
```css
.modal-checkbox-list { ... }
.modal-checkbox-label { ... }
.modal-checkbox-label input[type="checkbox"] { ... }
.modal-close-btn { ... }
.modal-close-btn:hover { ... }
```

- [ ] **Step 4: 删除 base.css 行 1527（modal 响应式 @media 中的 width 覆盖）**

在行 1519-1529 的 `@media` 块中，删除 `.modal { width: 95vw; }`。

- [ ] **Step 5: 删除 base.css 行 1798-1825（Drawer 全部定义）**

删除整个 `/* ── Drawer ── */` 块。

- [ ] **Step 6: 删除 base.css 行 1944-1945（modal-close-plain）**

删除：
```css
/* ── Modal close btn override ── */
.modal-close-plain{background:none;border:none;cursor:pointer;font-size:20px;color:var(--muted);padding:4px}
```

- [ ] **Step 7: 删除 base.css 行 2925（modal-foot btn 覆盖）**

删除：
```css
.modal-foot .btn { min-width: 100px; justify-content: center; }
```

- [ ] **Step 8: 删除 base.css 行 3598-3636（Dialog 第二组 — FMS 重复定义）**

删除整个第二组 dialog 定义。注意保留紧随其后的其他 FMS 样式。

- [ ] **Step 9: 删除 base.css 行 3887-3889（Import Modal Utilities）**

删除：
```css
/* ── Import Modal Utilities ── */
.modal-import { max-width: 560px; }
.modal-close-btn { background: none; border: none; cursor: pointer; font-size: 20px; color: var(--muted); padding: 4px; }
```

注意：`.import-error-title`、`.import-error-extra`、`.import-footer-actions`、`.file-item` 不属于本批次，保留。

- [ ] **Step 10: 删除 base.css 行 4267-4271（modal variants）**

删除：
```css
/* modal variants */
.modal-sm{max-width:420px}
.modal-title{font-size:var(--text-lg);font-weight:600;color:var(--fg);margin-bottom:var(--space-3)}
.modal-desc{font-size:var(--text-sm);color:var(--muted);line-height:1.6;margin-bottom:var(--space-5)}
.modal-actions{display:flex;justify-content:flex-end;gap:var(--space-3)}
```

- [ ] **Step 11: 验证编译和构建**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

Expected: 无 error

---

### Task 10: 搜索遗漏的 modal/drawer/dialog class 引用

**Files:**
- 可能涉及任何 `abt-web/src/**/*.rs` 文件

- [ ] **Step 1: 搜索所有剩余的 class 引用**

使用 search 工具搜索 `abt-web/src` 中以下 class 字符串：
- `modal-overlay`、`modal-head`、`modal-body`、`modal-foot`、`modal-lg`、`modal-sm`、`modal-title`、`modal-desc`、`modal-actions`、`modal-close-btn`、`modal-close-plain`、`modal-checkbox`
- `drawer-overlay`、`drawer-panel`、`drawer-head`、`drawer-body`、`drawer-foot`、`drawer-section`、`drawer-label`、`drawer-tip`、`drawer-close`
- `dialog-overlay`、`dialog-body`、`dialog-foot`、`dialog-title`、`dialog-desc`、`dialog-icon-wrap`

排除注释和文档（`.md` 文件）。

Expected: 找到的引用都需要已在前面的 Task 中被迁移。如果发现遗漏，按照映射表替换。

- [ ] **Step 2: 处理遗漏的文件**

对搜索发现的任何遗漏文件，按照本计划前面的原子 class 映射表替换。

- [ ] **Step 3: 验证编译**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

Expected: 无 error

---

### Task 11: 构建 CSS 并验证

- [ ] **Step 1: 重新构建 CSS**

Run: `cd E:/work/abt && npm run build:css`

Expected: 成功生成 app.css

- [ ] **Step 2: 验证 app.css 不再包含 modal/drawer/dialog class 定义**

使用 search 工具搜索 `static/app.css` 中的 `modal-overlay`、`drawer-panel`、`dialog-overlay`。

Expected: 无匹配（这些 class 已从 base.css 删除）

- [ ] **Step 3: 验证 animate-dialog-slide-in 在 app.css 中存在**

使用 search 工具搜索 `static/app.css` 中的 `dialog-slide-in`。

Expected: 存在（P0 中已在 theme.animation 中定义）

- [ ] **Step 4: 用 agent-browser 验证弹窗渲染**

验证 Modal（打开采购订单创建页，点击选择产品）：
```bash
agent-browser --cdp 9222 open "http://localhost:8000/admin/purchase/orders/create"
agent-browser --cdp 9222 eval "document.querySelector('[data-action=\"open-product-modal\"]')?.click(); setTimeout(() => { const modal = document.querySelector('.modal-overlay'); JSON.stringify({ exists: !!modal, isVisible: modal ? getComputedStyle(modal).visibility : 'N/A' }) }, 500)"
```

验证 Dialog（打开 BOM 编辑页，触发删除确认）：
```bash
agent-browser --cdp 9222 open "http://localhost:8000/admin/bom/1/edit"
# 手动验证删除确认弹窗显示正常
```

验证 Drawer（打开 BOM 详情页，触发成本 Drawer）：
```bash
agent-browser --cdp 9222 open "http://localhost:8000/admin/bom/1"
# 手动验证 Drawer 从右侧滑出
```

- [ ] **Step 5: cargo clippy 验证编译**

Run: `cd E:/work/abt && cargo clippy -p abt-web 2>&1 | grep "^error" | head -5`

Expected: 无 error

---

### Task 12: 提交

- [ ] **Step 1: Git 提交**

```bash
cd E:/work/abt && git add abt-web/src/components/modal.rs abt-web/src/components/drawer.rs abt-web/src/components/confirm_dialog.rs abt-web/src/components/input_dialog.rs abt-web/src/components/entity_picker.rs abt-web/src/components/product_picker.rs abt-web/src/components/import_modal.rs abt-web/src/pages/*.rs abt-web/src/layout/page.rs static/base.css static/app.css && git commit -m "refactor(css): P6 — migrate modal/drawer/dialog to atomic UnoCSS

- Migrate 7 component files (modal, drawer, confirm_dialog, input_dialog, entity_picker, product_picker, import_modal)
- Migrate 15+ page files with inline modal/drawer/dialog structures
- Remove all modal-*/drawer-*/dialog-* CSS from base.css (4 scattered locations)
- Use [&.is-open]/[.open] UnoCSS arbitrary variants for state-based visibility
- Use group/drawer pattern for drawer-panel translateX animation
- Preserve all Hyperscript open/close logic (no JS changes needed)"
```
