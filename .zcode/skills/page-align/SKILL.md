---
name: page-align
description: 逐元素对比设计原型与页面实现的对齐审查。当用户要求对齐页面、检查设计还原度、对比原型与实现、审查 UI 一致性时触发。适用于所有包含 "对齐"、"原型"、"还原"、"对比"、"页面审查"、"CSS 对齐"、"设计一致性" 关键词的请求，即使用户没有明确提到 "page-align"。支持单页、单模块或全部页面的批量审查。
---

# 页面对齐设计原型审查技能

分三个阶段：**浏览器快照对比**（`agent-browser snapshot` 快速发现差异）→ **代码层定位**（只看差异涉及的代码）→ **写修复计划文档**（用户确认后再执行修复）。三阶段缺一不可，**禁止在写完修复计划前动手改代码**。

## 核心原则

**浏览器优先——用运行时渲染结果驱动审查，而不是从源码猜测。**

- 第一阶段只看浏览器渲染结果，不读源码，快速定位所有差异
- 第二阶段只针对已发现的差异去读对应代码，精准定位修复点
- 这样避免逐行对比大量代码，大幅提升效率

## 文件位置

| 资源 | 路径 |
|------|------|
| 原型 HTML | `C:\Users\weichen\AppData\Roaming\Open Design\namespaces\release-stable-win\data\projects\63ce2980-2f4e-45a7-9b34-8050e32135c2\` |
| 原型 CSS | 同上 `css\app.css` |
| 实现页面 | `abt-web/src/pages/` (子目录按域划分) |
| 实现 CSS | `static/base.css` + `static/app.css` |
| 运行中服务 | `http://localhost:3000` |

---

## 执行流程

### 第一阶段：浏览器快照对比（快速发现差异）

**核心理念：先看结果，再查原因。** 浏览器 snapshot 直接反映用户看到的页面，用它对比原型和实现，快速列出所有差异。

#### 第一步：发现与映射

1. **扫描原型目录**所有 `.html` 文件（排除 `index.html` 等非页面入口）
2. **扫描实现目录**所有 `.rs` 文件
3. **语义匹配**原型与实现（去掉编号前缀、连字符转下划线），输出映射表：
   - ✅ 已匹配（原型 + 实现都存在）
   - ⚠️ 仅原型（有设计但未实现）
   - ➕ 仅实现（已实现但无原型参考）
4. 根据用户范围（单页 / 模块 / 全部）筛选待审查页面

#### 第二步：双 snapshot 对比

对每个待审查页面，严格执行以下流程：

**2.1 打开原型 HTML，获取原型 snapshot**

```bash
agent-browser --allow-file-access open "file:///C:/Users/weichen/AppData/Roaming/Open Design/namespaces/release-stable-win/data/projects/63ce2980-2f4e-45a7-9b34-8050e32135c2/quotation-detail.html" && agent-browser wait --load networkidle && agent-browser snapshot -i
```

记录原型的关键结构元素：
- heading 层级和文本（如 `heading "QT-2026-0042" [level=1]`）
- back-link 文本
- 表格 columnheader 数量和文本
- status-pill 文本
- button / link 数量和文本
- info-label + info-value 对数

**2.2 打开实现页面，获取实现 snapshot**

```bash
agent-browser open "http://localhost:3000/admin/quotations/42" && agent-browser wait --load networkidle && agent-browser snapshot -i
```

> 注意：如果实现页面需要登录，先执行登录流程。

**2.3 逐项对比两个 snapshot**

| 维度 | 对比方法 |
|------|----------|
| **页面标题** | 原型 heading 文本 vs 实现 heading 文本 |
| **返回链接** | 原型 back-link 文本 vs 实现 back-link 文本 |
| **表头列** | 原型所有 columnheader 文本（数量+顺序+内容）vs 实现 |
| **状态标签** | 原型 status-pill 文本 vs 实现 status-pill 文本 |
| **操作按钮** | 原型 button/link 数量和文本 vs 实现 |
| **信息卡片** | 原型 info-label 数量和文本 vs 实现 |
| **金额汇总** | 原型 amount-label 文本 vs 实现 |
| **表格行数** | 原型数据行数 vs 实现数据行数 |
| **空状态** | 原型空数据提示 vs 实现 |

**2.4 标记差异**

- ✅ 一致（原型和实现的 snapshot 匹配）
- ❌ 缺失（原型 snapshot 有但实现 snapshot 没有）
- ➕ 多余（实现 snapshot 有但原型 snapshot 没有）
- ⚠️ 偏差（两者都有但不完全一致）

**2.5 记录差异，继续对比**

对比过程中发现任何 ❌ 或 ⚠️ 差异，**记录到差异清单中，继续对比下一项**。不要停下来修复——先把所有页面的所有差异全部找完，再统一写修复计划。

记录内容包括：
- 差异所在的页面、检查维度
- 原型 snapshot 对应片段
- 实现 snapshot 对应片段
- 预估修复方式（结构缺失 / 样式偏差 / CSS 类名不对等）

#### 第三步：生成浏览器层差异报告

多页面时用 **subagent 并行**——每个 subagent 负责若干页面的双 snapshot 对比，全部完成后汇总。

> ⚠️ 注意：`agent-browser` 是单实例浏览器，**同一时间只能有一个 agent 操作浏览器**。因此浏览器 snapshot 必须串行执行。推荐的并行策略：
> 1. 主线程串行执行所有页面的浏览器 snapshot 对比
> 2. 或者将页面分组，每个 subagent 依次做自己的页面（使用 `--session` 隔离），但各组之间仍需串行
>
> 实际操作中，主线程串行逐页做双 snapshot 对比是最可靠的方式。

每页一个结构化差异清单。全部页面完成后追加总览表。

---

### 第二阶段：代码层定位（只看差异涉及的代码）

**这一阶段只针对第一阶段已发现的差异。** 不做全量代码审查，只定位差异的代码根因。

#### 第四步：读取原型 CSS + 差异涉及的源码

对第一阶段发现的每个差异项：
1. **读取原型 CSS**：从 `css/app.css` 提取差异项涉及的 CSS 类定义
2. **读取原型 HTML**：只读差异项对应的结构片段（不是整个文件）
3. **读取实现源码**：定位差异项在 Rust/Maud 代码中的位置
4. **读取实现 CSS**：检查 `static/base.css` + `static/app.css` 中对应样式是否存在

多页面差异可用 **subagent 并行**——每个 subagent 负责若干差异项的代码定位。

#### 第五步：确认代码根因

对每个差异项确定根因分类：
- **结构缺失**：实现中缺少某个 HTML 元素（需添加 Maud 代码）
- **CSS 类名不匹配**：实现用了不同的类名（需改为原型标准类名）
- **CSS 定义缺失**：实现 CSS 中缺少某个样式定义（需从原型 CSS 补充）
- **包裹层级不对**：实现缺少必要的包裹 div（需调整嵌套结构）
- **数据绑定问题**：结构正确但数据未正确渲染（需检查 service 层）

#### 第六步：生成最终报告

合并浏览器层 + 代码层定位结果，输出最终报告。

---

### 第三阶段：写修复计划文档（先写文档，后动手）

**这一阶段是强制性的——不能跳过。** 对比完成后必须先把修复计划写入文档，用户确认后才能开始修改代码。

#### 第七步：写入修复计划文档

将第六步的最终报告写入 `docs/plans/` 目录，文件名格式：`YYYY-MM-DD-<scope>-align-fix-plan.md`。

**文档结构**：

```markdown
# 页面对齐修复计划

**日期**：YYYY-MM-DD | **范围**：销售模块 / XX 页面 | **待修复项**：XX

## 总览

| 页面 | 类型 | 原型 | 实现 | 浏览器差异 | 代码定位 | 🔴 | 🟡 |
|------|------|------|------|-----------|---------|-----|-----|
| ... | ... | ... | ... | ... | ... | ... | ... |

**整体匹配度：XX%** | **待修复：XX 项**

## 逐页修复清单

### 1. 页面名称（原型：xx.html → 实现：xx.rs）

| # | 严重度 | 检查项 | 问题描述 | 代码位置 | 修复方式 |
|---|--------|--------|----------|---------|----------|
| 1 | 🔴 | B4 表头列 | 缺少"来源报价"列 | `xx.rs:L42` | 在 thead 中添加 th |
| 2 | 🟡 | D5 标签值对 | `.info-label` 缺少 | `xx.rs:L78` | 改用 `.info-label` 类名 |

**涉及文件**：`abt-web/src/pages/xx.rs`, `static/app.css`

### 2. 下一个页面...
```

#### 第八步：用户确认后执行修复

文档写完后，**向用户展示修复计划摘要**（总览表 + 待修复项数），等待用户确认：

- 用户说"开始修复"或"执行" → 按文档中的修复清单逐项执行
- 用户说"先修 XX 页" → 按指定页面优先修复
- 用户说"跳过 XX 项" → 从清单中划掉对应项

修复流程（每项）：
1. 从修复计划定位到对应的 Rust 源码位置
2. 修改 Maud 模板代码
3. 运行 `cargo clippy -p abt-web` 验证编译
4. 刷新实现页面（`agent-browser open` 同一 URL）
5. 重新 `agent-browser snapshot` 确认修复
6. 在修复计划文档中标记该项为 ✅ 已完成

---

## agent-browser 常用命令速查

| 命令 | 用途 |
|------|------|
| `agent-browser open <url>` | 导航到指定 URL（支持 `file://` 和 `http://`） |
| `agent-browser snapshot -i` | 获取含交互元素引用的快照（`@e1`, `@e2`...） |
| `agent-browser click @ref` | 点击指定元素 |
| `agent-browser fill @ref "text"` | 填充输入框 |
| `agent-browser back` | 返回上一页 |
| `agent-browser screenshot --annotate` | 带标注的截图（可视化元素位置） |
| `agent-browser diff snapshot` | 对比当前与上次 snapshot 的差异 |
| `agent-browser wait --load networkidle` | 等待页面完全加载 |

**关键规则**：
- **必须打开原型 HTML 获取 snapshot**，不能只看实现页面
- 两个 snapshot 必须是同一页面类型（列表 vs 列表，详情 vs 详情）
- 原型使用 `file:///` 协议：`file:///C:/Users/weichen/AppData/Roaming/Open Design/namespaces/release-stable-win/data/projects/63ce2980-2f4e-45a7-9b34-8050e32135c2/<filename>.html`
- 实现使用 `http://localhost:3000/admin/<route>`
- 对比时重点关注 `heading`、`columnheader`、`link`、`button`、`status-pill`、`info-label`、`info-value`
- 每次 `open` 后跟 `wait --load networkidle` 确保页面完全渲染

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

- 页面是否在 `.page-content` 内
- `.page-header > .page-title + .page-actions` 是否存在且结构正确
- 面包屑由 layout 统一渲染，页面级跳过

### B. 列表页

| # | 检查点 | snapshot 对比关注点 |
|---|--------|---------------------|
| B1 | 状态标签页 | heading 或 tablist 中的标签页文本 |
| B2 | 筛选栏 | search input + select 元素的数量和 placeholder |
| B3 | 数据卡片 | table 结构是否存在 |
| B4 | 表头列 | columnheader 文本和顺序 |
| B5 | 链接列 | link 元素在表格中的位置 |
| B6 | 数字列 | 数值文本的对齐（通过 CSS 确认） |
| B7 | 状态胶囊 | status 文本和颜色类名 |
| B8 | 行操作 | button/link 在每行的数量和文本 |
| B9 | 空状态 | 空数据提示文本 |
| B10 | 分页器 | navigation/pagination 元素 |

### C. 创建/编辑页

| # | 检查点 | snapshot 对比关注点 |
|---|--------|---------------------|
| C1 | 返回链接 | link 文本（通常是"返回 XX"） |
| C2 | 表单分区 | heading 元素标识的表单区域 |
| C3 | 分区图标 | （snapshot 不直接显示图标，需代码层或截图确认） |
| C4 | 字段排列 | label 文本的顺序、数量 |
| C5 | 必填标记 | label 中是否含 `*` 或 required 标记 |
| C6 | 跨列字段 | textarea 等大字段是否存在 |
| C7 | 行项目表 | table 的 columnheader |
| C8 | 添加/删除行 | button "添加行" / "删除" |
| C9 | 合计栏 | 合计文本和数值 |
| C10 | 底部操作栏 | button 文本和顺序（"保存"/"取消"） |

### D. 详情页

| # | 检查点 | snapshot 对比关注点 |
|---|--------|---------------------|
| D1 | 返回链接 | link 文本 |
| D2 | 详情头部 | heading + button 组合 |
| D3 | 标题行 | heading 中的编号 + status 文本 |
| D4 | 信息卡片 | heading 标识的信息区域 |
| D5 | 标签值对 | label/text 对的数量和内容 |
| D6 | 关联表格 | 同列表页 B3-B10 |
| D7 | 金额汇总 | 金额 label 和 value |

### E. CSS 类严格核验（第二阶段代码定位时执行）

对每个差异项中涉及的 CSS 类名：
1. 从原型 HTML 提取该元素实际使用的类名
2. 从原型 CSS 确认该类名的定义（存在性 + 语义）
3. 从实现代码检查是否使用了**完全相同的类名**

关键规则：
- 类名必须逐字匹配——原型用 `.info-card-title`，实现就不能用 `.detail-card-title`
- 组合类必须完整——按钮要 `.btn.btn-primary` 两个都写，状态胶囊要 `status-pill status-xxx`
- 包裹层级必须一致——原型 `.search-wrap > .search-input`，实现也要有 `.search-wrap`

### F. 交互行为（仅标注，低优先级）

HTMX 属性、模态框开关、分页跳转等行为层偏差记入报告，但修复优先级低于结构和样式。

---

## 报告格式

### 单页报告

```markdown
## 页面名称 — 对齐报告

**类型**：列表页 | **原型**：xx-list.html | **实现**：xx_list.rs

### 概要
匹配度：XX/XX（XX%）| 🔴 严重 X | 🟡 轻微 X

### 浏览器 snapshot 对比（第一阶段）
| 检查项 | 状态 | 原型 snapshot | 实现 snapshot | 说明 |
|--------|------|---------------|---------------|------|
| B4 表头列 | ❌ | 6 个 columnheader | 5 个 columnheader | 缺少"来源报价"列 |
| B7 状态胶囊 | ⚠️ | status-pill "草稿" | badge "draft" | 类名和文本都不同 |

### 代码层定位（第二阶段）
| 差异项 | 代码位置 | 根因 | 修复指引 |
|--------|---------|------|----------|
| B4 表头列 | `xx.rs:L42` | 结构缺失 | 在 thead 中添加 th |
| B7 状态胶囊 | `xx.rs:L67` | CSS 类名不匹配 | 改用 `status-pill status-xxx` |

### 修复清单
1. 🔴 [结构缺失] ...
2. 🟡 [样式偏差] ...
```

### 批量总览（多页时追加）

```markdown
# 页面对齐总览

| 页面 | 类型 | 原型 | 实现 | 浏览器差异 | 代码定位 | 🔴 | 🟡 |
|------|------|------|------|-----------|---------|-----|-----|
| ... | ... | ... | ... | ... | ... | ... | ... |

**整体匹配度：XX%** | **待修复：XX 项**
```

---

## 多 Agent 并行策略

**核心原则：浏览器 snapshot 必须串行（单实例），代码层定位可并行。** 修复阶段必须串行（单 agent 逐项改代码）。

### 角色分工

| 角色 | 数量 | 职责 | 动作 |
|------|------|------|------|
| **主线程** | 1 | 调度 + 浏览器对比 + 汇总 + 写文档 | 发现映射、串行执行双 snapshot 对比、分发代码定位任务、收集结果、写修复计划 |
| **代码定位 Agent** | N（按差异项数拆分） | 代码层定位 | 读原型 HTML 片段 + 读原型 CSS + 读实现源码 + 确认根因，返回定位结果 |
| **修复 Agent** | 1（主线程自身） | 执行修复 | 用户确认后，按修复计划逐项改代码 + clippy + snapshot 验证 |

### 执行流程

```
主线程: 发现映射 → 串行执行双 snapshot 对比（逐页）
                                    ↓
                          汇总浏览器层差异清单
                                    ↓
            Agent 1: ──→ 报价单差异项代码定位 ──→ 返回定位结果
            Agent 2: ──→ 销售订单差异项代码定位 ──→ 返回定位结果
            Agent 3: ──→ 发货+退货差异项代码定位 ──→ 返回定位结果
                                    ↓
主线程: 合并所有定位结果 → 写 docs/plans/ 修复计划文档 → 等用户确认
                                    ↓
主线程: 按优先级逐项修复 → clippy → snapshot 验证 → 文档标记 ✅
```

### Agent 拆分规则

**第一阶段（浏览器 snapshot）：主线程串行**
- 主线程逐页执行双 snapshot 对比
- agent-browser 是单实例，不能并行操作浏览器

**第二阶段（代码层定位）：多 agent 并行**
- 每个代码定位 Agent 负责 **1 个模块的差异项**（同一模块的页面放同一 agent）
- 每个 Agent 的 prompt 包含：
  1. 待定位的差异清单（来自第一阶段）
  2. 原型 HTML 文件路径（只读差异涉及的部分）
  3. 实现源码文件路径
  4. 输出格式要求（结构化定位结果）
  5. **明确指令："只读不写，只返回代码定位结果"**
- 主线程用 `Agent` 工具 + `run_in_background: true` 并行启动，全部完成后汇总

### Agent 输出格式

每个代码定位 Agent 返回如下结构，主线程直接汇总：

```markdown
## Agent N 代码定位结果

### 页面: quotation-list
| # | 差异项 | 代码位置 | 根因分类 | 原型片段 | 实现片段 | 修复指引 |
|---|--------|---------|---------|----------|----------|----------|
| 1 | B4 表头列 | `xx.rs:L42` | 结构缺失 | `<th>来源报价</th>` | （缺失） | 在 thead 中添加 th |
| 2 | B7 状态胶囊 | `xx.rs:L67` | CSS 类名不匹配 | `status-pill status-draft` | `status-badge draft` | 改用 status-pill |

### 页面: quotation-create
| # | 差异项 | ... |
```

### 批量审查策略

>5 个页面时：
1. **一次性发现**：主线程 Glob 扫描全部文件，生成映射表
2. **浏览器 snapshot 对比**：主线程串行逐页做双 snapshot 对比，记录所有差异
3. **分发代码定位任务**：按模块拆分差异项，每个 subagent 负责一个模块的代码定位（并行）
4. **汇总定位结果**：主线程收集所有 agent 返回的代码定位结果
5. **写修复计划**：合并浏览器差异 + 代码定位结果，写入 `docs/plans/` 修复计划文档
6. **用户确认**：展示计划摘要，等待确认
7. **逐项修复**：按优先级串行修复，每项修复后 clippy + snapshot 验证

---

## 注意事项

- **先写计划后修复**——对比完成后必须先写修复计划文档到 `docs/plans/`，用户确认后才能动代码。**禁止边对比边修复**
- **原型是唯一标准**——所有偏差描述为"实现应如何调整以匹配原型"
- **浏览器优先**——先用 snapshot 发现差异，再针对性看代码，不做全量代码审查
- **三阶段缺一不可**——浏览器快照对比 + 代码层定位 + 写修复计划文档，不能跳过任何阶段
- 全局共享元素（sidebar、header）由 layout 渲染，页面级跳过
- 多页面共享同一偏差时提取为全局修复项
- **不要自己启动服务**——服务由用户管理，只需检查是否可访问
- **snapshot 后必须 wait**——每次 `open` 后跟 `wait --load networkidle`，确保页面完全渲染后再 snapshot
