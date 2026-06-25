# 页面对齐修复计划 — 工单工作台 detail-header

**日期**：2026-06-25
**范围**：MES 工单工作台 `/admin/mes/orders/{id}` 的 detail-header 区
**原型**：`04-order-hub.html`（Open Design）
**实现**：`abt-web/src/pages/mes_order_detail.rs` + `abt-web/src/components/{material_badge,status_step_bar,disclosure}.rs`

---

## 总览

| 区块 | 原型 | 实现 | 浏览器差异 | 代码定位 | 🔴 | 🟡 |
|------|------|------|-----------|---------|-----|-----|
| 标题行（doc_no + status-pill + 状态按钮） | ✅ | ✅ | 一致 | — | 0 | 0 |
| sub-row（产品·自制·数量·车间·排程） | ✅ | ✅ | 一致（车间缺失=数据，非 UI） | — | 0 | 0 |
| 状态步骤条 wo-steps（4 步三态） | ✅ | ✅ | 一致 | — | 0 | 0 |
| 物料徽章 mat-badge（4 级） | ✅ | ✅ 结构 | **点击无反应** | `material_badge.rs:42` | 1 | 0 |
| 来源链 source-trace | ✅ | ✅ | 一致 | — | 0 | 0 |
| 进度条 wo-progress | ✅ | ✅ | 一致 | — | 0 | 0 |
| 摘要带 stat-strip（4 格 drill-down） | ✅ | ✅ 结构 | **3 格点击无反应** | `mes_order_detail.rs:827/843/852` | 1 | 0 |
| 容器圆角 | 12px | 6px | 视觉偏差 | `mes_order_detail.rs:648` | 0 | 1 |

**整体匹配度**：结构 100%、交互 0%（drill-down 全失效）、视觉 ~95%
**待修复**：🔴 1 类（4 处）+ 🟡 1 处（可选）

---

## 已对齐项（无需改动）

浏览器 snapshot + eval 逐项确认，以下原型与实现完全一致：

- **标题行**：`WO-2026-06-001183` + status-pill（已下达）+ 状态驱动按钮（拆批/反下达/关闭/取消）
- **sub-row**：产品名 + `自制` tag + 数量 + 排程（车间项 `@if let Some(wc)` 条件渲染，当前工单未绑工作中心故不显示，属数据非缺陷）
- **状态步骤条**：草稿 → 已下达(●active) → 生产中 → 已关闭，done/active/pending 三态正确
- **物料徽章**：4 级（齐套/待齐套/迟料/缺料）颜色 token 正确，当前显示「齐套」(success)
- **来源链**：SO → PP → WO → 批次/入库
- **进度条**：完工入库进度 `0 / 111 · 0%`，fill width 正确
- **摘要带 4 格**：完工入库 / 在制 / 批次 / FQC，文案与结构对齐
- **padding 24px / margin-bottom 16px / border**：一致

---

## 逐项修复清单

### 🔴 1. Hyperscript 语法错误 — drill-down 交互完全失效（4 处）

**现象**：控制台报 8 个 `hyperscript parse error`：
```
on click add .open to #d-info then call #d-info.scrollIntoView() with {behavior:'smooth',block:'center'}
                                                                     ^^^^          ^
                                                                     Unexpected Token : with
                                                                     Expected dotOrColonPath
```
点击 detail-header 的「物料徽章」和摘要带的「完工入库 / 批次 / FQC」3 格，**无法展开对应 disclosure + 滚动定位**，与原型行为不符（原型点击 → `toggleAndScroll()` → 展开 + smooth 滚动）。

**根因**：hyperscript 的 `with` 关键字不能跟在 `call <method>()` 之后传递 JS 对象参数。`call #x.scrollIntoView() with {...}` 是非法语法，整条 `_=` 解析失败 → 事件处理器未注册 → 点击无反应。

**出错位置**（均为同一非法模式 `call #x.scrollIntoView() with {...}`）：

| # | 文件 | 行 | 元素 | 目标 disclosure |
|---|------|----|------|----------------|
| a | `abt-web/src/components/material_badge.rs` | 42 | 物料徽章 mat-badge | `#d-mat` |
| b | `abt-web/src/pages/mes_order_detail.rs` | 827 | 摘要带「完工入库」格 | `#d-info` |
| c | `abt-web/src/pages/mes_order_detail.rs` | 843 | 摘要带「批次」格 | `#d-matrix` |
| d | `abt-web/src/pages/mes_order_detail.rs` | 852 | 摘要带「FQC」格 | `#d-rcpt` |

**修复方式**：采用项目已验证的惯用模式 ——「`app.js` 全局函数 + hyperscript `call`」（参考 `app.js` 的 `entityPickerSelect` / `positionDropdown` / `lineItemCalc`，且 `app.js:75` 已有同款 `scrollIntoView({behavior:'smooth',block:'center'})` 用法）。

1. **`static/app.js`** 新增全局函数（放文件末尾）：
   ```js
   // ── Disclosure drill-down：展开目标区块并 smooth 滚动定位 ──
   // Hyperscript 调用：_="on click call openAndScroll('d-info')"
   // （工单工作台 detail-header 物料徽章 / 摘要带点击 drill-down 用）
   window.openAndScroll = function (id) {
       var el = document.getElementById(id);
       if (!el) return;
       el.classList.add('open');
       el.scrollIntoView({behavior: 'smooth', block: 'center'});
   };
   ```

2. **4 处 `_=` 改写**：
   - `material_badge.rs:41-44` → `_=(format!("on click call openAndScroll('{}')", target_id))`
   - `mes_order_detail.rs:827` → `_="on click call openAndScroll('d-info')"`
   - `mes_order_detail.rs:843` → `_="on click call openAndScroll('d-matrix')"`
   - `mes_order_detail.rs:852` → `_="on click call openAndScroll('d-rcpt')"`

**验证**：`cargo clippy -p abt-web` 编译通过 → 刷新页面 → 点击 4 个元素 → 对应 disclosure 展开 + smooth 滚动 + 控制台无 parse error。

---

### 🟡 2. detail-header 容器圆角（可选，需决策）

**现象**：原型 `.detail-header` `border-radius: 12px`（`--radius-lg`），实现用 UnoCSS `rounded-md` = 6px。

**Trade-off（重要）**：原型整套卡片（detail-header / stat-strip / disclosure）都是 12px；实现整套都是 6px（`mes_order_detail.rs:648/818`、`disclosure.rs:47`）。
- 若**只改 detail-header** → 与紧邻的 stat-strip(6px)、disclosure(6px) 圆角割裂，更难看。
- 若**全改 12px** → `disclosure` 是全局共享组件（全站工作台/详情页在用），改动影响面大。
- **保持 6px** → 与 ABT 全站 UnoCSS 卡片体系一致，仅与原型有 6px 细微差异。

**建议**：**保持现状（6px）**。6px vs 12px 是极细微差异，且实现整套统一、与全站一致；为对齐原型而改全局 disclosure 组件风险/收益不划算。

**若用户坚持对齐**：方案是把 detail-header(`:648`) + stat-strip(`:818`) + disclosure 组件(`disclosure.rs:47`) 三处的 `rounded-md` 统一改为 `rounded-xl`（12px），需另评估 disclosure 全局影响。

---

## 涉及文件

| 文件 | 改动 |
|------|------|
| `static/app.js` | 新增 `window.openAndScroll` 全局函数 |
| `abt-web/src/components/material_badge.rs` | 第 42 行 `_=` 改写 |
| `abt-web/src/pages/mes_order_detail.rs` | 第 827 / 843 / 852 行 `_=` 改写 |
| `abt-web/src/pages/mes_order_detail.rs`（可选） | 第 648 行圆角，需用户决策 |

---

## 执行顺序

1. 🔴 修复 4 处 hyperscript（app.js + 3 个 .rs）— **核心，必做**
2. `cargo clippy -p abt-web` 验证
3. 浏览器刷新 + 点击验证 drill-down 恢复
4. 🟡 圆角 — 等用户决策（默认不动）
