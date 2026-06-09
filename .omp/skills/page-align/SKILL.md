---
name: page-align
description: 逐元素对比设计原型与页面实现的对齐审查。当用户要求对齐页面、检查设计还原度、对比原型与实现、审查 UI 一致性时触发。适用于所有包含 "对齐"、"原型"、"还原"、"对比"、"页面审查"、"CSS 对齐"、"设计一致性" 关键词的请求，即使用户没有明确提到 "page-align"。支持单页、单模块或全部页面的批量审查。
---

# 页面对齐设计原型审查技能

分三个阶段：**浏览器对比**（`agent-browser` snapshot 对比结构与文本）→ **代码定位**（针对差异项读取源码确认修复方式）→ **写修复计划 + 执行修复**。**禁止在写完修复计划前动手改代码**。

## 核心原则

1. **只用 agent-browser**——禁止使用 `_browser` 工具（内置浏览器）。所有浏览器操作（打开页面、snapshot）一律通过 bash 调用 `agent-browser` CLI 完成。**禁止截图**——只用 `snapshot` 和 `snapshot -i`。
2. **先看再看代码**——先用 agent-browser 对比原型和实现的 snapshot，找出所有差异（结构 + 文本），再针对差异项读代码定位。
3. **原型是唯一标准**——所有偏差描述为"实现应如何调整以匹配原型"。

## 文件位置

| 资源 | 路径 |
|------|------|
| 原型 HTML | `C:\Users\weichen\AppData\Roaming\Open Design\namespaces\release-stable-win\data\projects\63ce2980-2f4e-45a7-9b34-8050e32135c2\` |
| 实现页面 | `abt-web/src/pages/`（子目录按域划分） |
| 运行中服务 | `http://localhost:8000` |

---

## 执行流程

### 第一阶段：浏览器对比（agent-browser 双 snapshot）

**这是唯一的对比手段——不使用任何内置浏览器工具，不使用截图。**

对每个待审查页面，严格执行以下流程：

#### 第一步：打开原型，获取 snapshot

```bash
# 原型 HTML 使用 file:// 协议
agent-browser open "file:///C:/Users/weichen/AppData/Roaming/Open Design/namespaces/release-stable-win/data/projects/63ce2980-2f4e-45a7-9b34-8050e32135c2/quotation-detail.html"
agent-browser snapshot          # 获取完整 DOM 结构（含 StaticText）
agent-browser snapshot -i       # 获取交互元素引用（@e1, @e2...）
```

从原型 snapshot 中提取并记录：
- **heading** 层级和文本
- **link** 文本和数量
- **button** 文本和数量
- **columnheader** 数量、文本、顺序
- **StaticText** 中的所有文本内容（标签、值、按钮文字等）
- **cell** 中的文本（表格数据行内容）
- **表格数据行** 数量

#### 第二步：打开实现页面，获取 snapshot

```bash
agent-browser open "http://localhost:8000/admin/quotations/28"
agent-browser snapshot
agent-browser snapshot -i
```

#### 第三步：逐项对比

**结构对比**（基于 snapshot）：

| 维度 | 对比方法 |
|------|----------|
| 页面标题 | 原型 heading 文本 vs 实现 heading 文本 |
| 返回链接 | 原型 back-link 文本 vs 实现 back-link 文本 |
| 操作按钮 | 原型 button/link 数量和文本 vs 实现 |
| 表头列 | 原型 columnheader 数量+顺序+文本 vs 实现 |
| 状态标签 | 原型 status 相关 StaticText vs 实现 |
| 信息字段 | 原型 StaticText 中的标签值对 vs 实现 |
| 金额汇总 | 原型金额相关 StaticText vs 实现 |
| 表格行数 | 原型数据行数 vs 实现数据行数 |
| 表格数据 | 原型 cell 文本 vs 实现 cell 文本 |

#### 第四步：标记差异

- ✅ 一致
- ❌ 缺失（原型有但实现没有）
- ➕ 多余（实现有但原型没有）
- ⚠️ 偏差（两者都有但不完全一致——文本不同、数量不同、顺序不同等）

对比中发现差异时，**记录到差异列表，继续对比下一项**。不要停下来修复。

差异记录格式：
```
页面 | 维度 | 状态 | 原型值 | 实现值 | 预估原因
```

---

### 第二阶段：代码定位（针对差异项读源码）

**只读不改**——只为每个差异项定位代码位置，确认修复方式。

#### 第五步：读取源码定位差异

对第一阶段的每个差异项：
1. 找到对应的实现文件（`abt-web/src/pages/xx.rs`）
2. 读取相关代码片段
3. 确认修复方式（添加缺失元素 / 修改 CSS 类 / 调整结构）
4. 如需新增 CSS 类，读取 `static/base.css` 确认是否已有定义
5. 如需查数据模型，读取 `abt-core/src/` 下对应 model 文件

多页面差异时，按页面分组，每个页面的源码定位可并行（subagent）。

---

### 第三阶段：写修复计划 + 执行修复

#### 第六步：写入修复计划文档

将第一、二阶段的结果写入 `docs/plans/` 目录，文件名格式：`YYYY-MM-DD-<scope>-align-fix-plan.md`。

**文档结构**：

```markdown
# 页面对齐修复计划

**日期**：YYYY-MM-DD | **范围**：XX 页面 | **待修复项**：XX

## 总览

| 页面 | 原型 | 实现 | 🔴 | 🟡 |
|------|------|------|-----|-----|
| ... | ... | ... | ... | ... |

**整体匹配度：XX%** | **待修复：XX 项**

## 逐页修复清单

### 1. 页面名称（原型：xx.html → 实现：xx.rs）

| # | 严重度 | 检查项 | 问题描述 | 原型值 | 实现值 | 修复方式 |
|---|--------|--------|----------|--------|--------|----------|
| 1 | 🔴 | D3 信息字段 | 缺少联系人等 3 项 | 8 项 | 5 项 | handler 查联系人数据，模板添加 3 个 info-item |
| 2 | 🟡 | D7 金额汇总 | 缺少成本合计和预估利润 | 3 行 | 1 行 | 模板添加 2 行 amount-row |

**涉及文件**：`abt-web/src/pages/xx.rs`
```

#### 第七步：用户确认后执行修复

文档写完后，向用户展示修复计划摘要，等待确认后逐项执行。

修复流程（每项）：
1. 从修复计划定位到对应的 Rust 源码位置
2. 修改 Maud 模板代码
3. 运行 `cargo clippy -p abt-web` 验证编译
4. **用 agent-browser 刷新实现页面**：
   ```bash
   agent-browser open "http://localhost:8000/admin/xx/..."
   agent-browser snapshot
   ```
5. 对比修复后 snapshot 确认修复
6. 在修复计划文档中标记该项为 ✅ 已完成

---

## agent-browser 命令速查

| 命令 | 用途 |
|------|------|
| `agent-browser open <url>` | 导航到 URL（支持 `file://` 和 `http://`） |
| `agent-browser snapshot` | DOM 无障碍树快照（含 StaticText，看结构和文本） |
| `agent-browser snapshot -i` | 含交互元素引用的快照（`@e1`, `@e2`...） |
| `agent-browser click @ref` | 点击元素 |
| `agent-browser fill @ref "text"` | 填充输入框 |
| `agent-browser get text @ref` | 获取元素文本 |
| `agent-browser back` | 返回上一页 |

**关键规则**：
- **禁止使用 `_browser` 工具**——所有浏览器操作一律用 `agent-browser` CLI
- **禁止截图**——只用 `snapshot` 和 `snapshot -i`
- **必须打开原型 HTML 获取 snapshot**——不能只看实现页面
- 原型 URL 格式：`file:///C:/Users/weichen/AppData/Roaming/Open Design/namespaces/release-stable-win/data/projects/63ce2980-2f4e-45a7-9b34-8050e32135c2/<filename>.html`
- 实现 URL 格式：`http://localhost:8000/admin/<route>`

---

## 页面类型判定

| 类型 | 判定规则 |
|------|----------|
| **列表页** | 文件名含 `list`、原型含 `data-table` 且无表单 |
| **创建/编辑页** | 文件名含 `create`/`edit`/`form`、原型含表单 |
| **详情页** | 文件名含 `detail`/`view`、原型含 `info-card` |
| **仪表盘** | 文件名含 `dashboard`、原型含 `stat-card` 网格 |

---

## 检查清单

### A. 页面级结构（所有页面通用）

- 页面标题（heading 文本和层级）
- 返回链接（文本、图标）
- 操作按钮（数量、文本）

### B. 列表页

| # | 检查点 | snapshot 对比 |
|---|--------|---------------|
| B1 | 状态标签页 | tab 数量和文本 |
| B2 | 筛选栏 | search input + filter select 数量 |
| B3 | 表头列 | columnheader 数量、顺序、文本 |
| B4 | 数据行 | 行数、各 cell 文本 |
| B5 | 链接列 | mono 标记的产品编码/单号列 |
| B6 | 数字列 | num-right 对齐的金额/数量列 |
| B7 | 状态胶囊 | status 相关文本 |
| B8 | 行操作 | 按钮/链接数量和文本 |
| B9 | 空状态 | 空数据提示文本 |
| B10 | 分页器 | 分页控件文本 |

### C. 创建/编辑页

| # | 检查点 | snapshot 对比 |
|---|--------|---------------|
| C1 | 返回链接 | 文本 |
| C2 | 表单分区 | section title 数量和文本 |
| C3 | 字段排列 | label 数量、顺序、文本 |
| C4 | 必填标记 | required 标记 |
| C5 | 跨列字段 | span-2 布局 |
| C6 | 行项目表 | 列头和列数 |
| C7 | 添加/删除行 | 按钮文本 |
| C8 | 合计栏 | 金额行 |
| C9 | 底部操作栏 | 按钮顺序和文本 |

### D. 详情页

| # | 检查点 | snapshot 对比 |
|---|--------|---------------|
| D1 | 返回链接 | 文本 |
| D2 | 详情头部 | 标题 + 状态 + 操作按钮 |
| D3 | 信息字段 | StaticText 中标签值对的数量和文本 |
| D4 | 关联表格 | 同列表页 B3-B10 |
| D5 | 金额汇总 | amount 行数和文本 |
| D6 | 备注 | 文本内容 |

### E. 交互行为（仅标注，低优先级）

HTMX 属性、模态框开关、分页跳转等行为层偏差记入报告，修复优先级低于结构。

---

## 报告格式

### 单页报告

```markdown
## 页面名称 — 对齐报告

**类型**：详情页 | **原型**：xx-detail.html | **实现**：xx_detail.rs

### 概要
匹配度：XX/XX（XX%）| 🔴 严重 X | 🟡 轻微 X

### 浏览器对比（agent-browser snapshot）
| 检查项 | 状态 | 原型值 | 实现值 |
|--------|------|--------|--------|
| D3 信息字段 | ❌ | 8 项（含联系人、电话、业务员） | 5 项 |
| D5 金额汇总 | ❌ | 3 行（成本+利润+总额） | 1 行（仅总额） |

### 修复清单
1. 🔴 [结构缺失] 缺少联系人、联系电话、业务员字段 → 在 handler 查询联系人/用户数据，模板添加 3 个 info-item
2. 🔴 [结构缺失] 缺少成本合计和预估利润 → 模板添加 2 行 amount-row
```

### 批量总览

```markdown
# 页面对齐总览

| 页面 | 原型 | 实现 | 🔴 | 🟡 |
|------|------|------|-----|-----|
| ... | ... | ... | ... | ... |

**整体匹配度：XX%** | **待修复：XX 项**
```

---

## 多页面并行策略

`agent-browser` 是单实例，同一时间只能有一个操作。因此：

- **浏览器对比**：主线程串行执行（逐页双 snapshot）
- **代码定位**：多 subagent 并行（纯文件读取，无冲突）
- **修复执行**：串行（逐项改代码 + clippy 验证）

### 推荐流程（>5 页面时）

1. **主线程**：Glob 扫描原型 + 实现文件，建立映射
2. **主线程串行**：逐页用 agent-browser 做 snapshot 对比
3. **多 subagent 并行**：针对差异项读源码定位
4. **主线程**：汇总所有差异 → 写 `docs/plans/` 修复计划文档
5. **用户确认** → 主线程串行逐项修复

---

## 注意事项

- **禁止使用 `_browser` 工具**——所有浏览器操作只用 `agent-browser` CLI
- **禁止截图**——只用 `snapshot` 和 `snapshot -i`
- **先浏览器后代码**——先看实际渲染差异，再读代码定位
- **先写计划后修复**——修复计划写入 `docs/plans/` 后，用户确认才改代码
- **不要自己启动服务**——服务由用户管理，只需检查是否可访问
- 全局共享元素（sidebar、header）由 layout 渲染，页面级跳过
- 多页面共享同一偏差时提取为全局修复项
