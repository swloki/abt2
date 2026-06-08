---
name: page-test
description: >
  ABT 项目页面功能测试技能。使用 agent-browser 对 SSR 页面进行端到端功能验证，覆盖页面渲染、数据展示、
  CRUD 操作、筛选/搜索、状态流转、分页、导航等场景。当用户提到"测试页面"、"页面测试"、"功能测试"、
  "测一下这个页面"、"跑一遍测试"、"验证页面"、"review 页面"、"页面 QA"、"帮我测试"、"test page"、
  "test this page"、"page test"、"functional test"、"E2E test"、"端到端测试" 时触发。
  也在以下场景触发：用户提到某个模块需要测试、用户想验证某个页面是否正常、用户刚完成功能开发想确认页面工作、
  用户提到测试计划或测试报告、用户要求对某个路由做回归测试。即使用户只是说"看看这个页面对不对"也应触发。
allowed-tools: Bash(agent-browser:*), Bash(npx agent-browser:*), Bash(psql:*), Bash(bun:*)
---

# ABT 页面功能测试

使用 `agent-browser` CLI 对 ABT 系统（Axum + Maud + HTMX SSR）进行端到端页面功能测试。

## 环境信息

- **应用地址**: `http://localhost:8000`
- **测试账号**: `admin` / `admin123`
- **项目约束**: 使用中文沟通，不要用 `curl` 测试页面
- **数据库**: PostgreSQL `abt_v2`，连接串在 `.env` 的 `DATABASE_URL`

## 测试流程

整个测试分为三个阶段：**准备 → 执行 → 报告**。

### 阶段一：准备

#### 1. 询问原型设计

向用户确认原型设计文件的访问方式。问用户：

> "请提供原型设计地址或文件路径，用于测试时对比验证。"

原型可能是一个 URL、本地 HTML 文件路径、或者 Open Design 项目。拿到原型后，测试时用 `snapshot -i` 获取页面的无障碍树结构，与原型的 DOM 结构做对比。如果用户说没有原型或不需要对比，则跳过此步骤，仅做功能验证。

#### 2. 确认测试范围

用户可能指定具体页面、模块或说"全部测一遍"。如果是模块级别，先用 `read` 查看对应的路由文件和页面文件，了解有哪些路由需要覆盖。

#### 3. 自动准备测试数据

数据库默认没有数据，测试前必须自动插入测试数据。

**优先使用现有数据脚本**：检查 `scripts/` 目录下是否有对应模块的测试数据 SQL：
- `scripts/mes_test_data.sql` — MES 模块（生产计划、工单、批次、报工、报检、入库）
- `scripts/wms-test-data.sql` — WMS 模块（仓库、库区、库位、库存）
- `scripts/mes-test-data.sql` — MES 补充数据（工序路线等）

**插入方式**：
```bash
# 通过 psql 执行 SQL 脚本
psql "$DATABASE_URL" -f scripts/mes_test_data.sql
```

**没有现成脚本时，自动生成数据**：

如果目标模块没有现成的测试数据脚本，需要根据模块的数据库表结构自动生成 INSERT 语句。步骤如下：

1. 查看 `abt-core/migrations/` 中该模块相关的建表 SQL，了解表结构和约束
2. 查看该模块的 `model.rs` 和 `repo.rs`，了解必填字段和业务含义
3. 生成 INSERT SQL，确保：
   - 满足所有外键约束（先插主表再插明细表）
   - 状态值使用正确的枚举（`#[repr(i16)]` 对应的数字）
   - `operator_id` 使用 `1`（admin 用户）
   - 时间字段使用 `NOW()`
   - 不与已有数据冲突
4. 将生成的 SQL 保存到 `scripts/<module>-test-data.sql` 供后续复用
5. 执行插入并确认成功

**数据依赖链**：大部分业务数据依赖基础主数据，确保插入顺序正确：
```
products → BOMs → production_plans → production_plan_items → work_orders → production_batches → ...
products → warehouses → zones → bins → inventory → ...
```

#### 4. 登录认证

每次测试会话开始时需要先登录：

```bash
agent-browser --session-name abt open http://localhost:3000/login
agent-browser snapshot -i
agent-browser fill @e<username_input> "admin"
agent-browser fill @e<password_input> "admin123"
agent-browser click @e<login_button>
agent-browser wait 2000
```

`--session-name abt` 自动保存/恢复 cookie，后续命令可省略此参数。

### 阶段二：逐页测试
测试的核心原则：**先做计划，逐项测试，发现问题立刻修复，修复完再继续下一项。** 不要攒一堆问题最后一起修。
#### 对每个页面：先计划后执行
每开始测试一个新页面之前，先输出该页面的测试计划，列出要逐项验证的功能点。格式示例：
```
## 测试页面：/admin/mes/plans（生产计划列表）
测试项：
  1. 页面加载 — 访问页面，确认无 500 错误
  2. 页面标题和面包屑 — "生产管理 > 生产计划"
  3. 筛选栏 — 日期选择器、状态下拉、搜索框是否存在
  4. 筛选栏样式 — 筛选区域布局对齐，class 正确
  5. 表格表头 — 列名与原型一致
  6. 表格数据 — 数据行显示产品名称而非 ID
  7. 状态标签 — 各状态颜色和文字正确
  8. 搜索功能 — 输入关键词后列表正确过滤
  9. 分页 — 翻页后数据刷新
  10. 点击行跳转详情 — URL 正确跳转
  ...
```
计划列好后，**按顺序逐项测试，一个接一个**。
#### 单个测试项的执行节奏
每个测试项严格遵循这个循环：
```
测试 → 发现问题？
  ├─ 是 → 立刻修代码 → cargo clippy → agent-browser 回归验证 → 确认修复 → 记录到报告 → 继续
  └─ 否 → 标记 ✅ → 继续下一项
```
**绝对不要跳过问题留到后面修。** 每个问题都要当场解决、当场验证通过，再测下一项。这样保证每一步都是建立在正确基础上的。
#### 每项测试的操作方式
```
agent-browser open <url>           # 打开页面
agent-browser snapshot -i          # 获取无障碍树 + 交互元素引用 (@e1, @e2, ...)
# ... 分析 snapshot 输出，检查页面结构 ...
# ... 执行交互操作 ...
agent-browser snapshot -i          # 操作后重新获取元素
agent-browser errors               # 检查页面是否有 JS 错误
```
**验证手段**：统一使用 `snapshot -i` 的无障碍树输出。snapshot 返回页面的结构化无障碍树，包含角色（role）、名称（name）、值（value）、状态（states）等信息，可以验证：
- 表格行数、表头列名
- 按钮和链接的文本
- 输入框的 placeholder 和类型
- 状态标签的文字内容
- 数据值是否显示为可读名称（而非原始 ID）
- CSS class 是否正确（通过 states 中的属性推断）
- 导航结构是否完整
#### 细粒度检查要点
不要只看页面"大概能显示"，要逐个细节检查：
**布局和样式**（通过 snapshot 中的元素层级和属性验证）：
- 筛选栏的 input class 是否正确（`search-input` 而非 `form-input`）
- 日期选择器的 max-width 是否合理
- 表格列宽是否合理，数据是否被截断
- 状态标签的样式是否匹配业务语义
**数据展示**：
- 数值字段是否有多余小数位（200.000000 应显示为 200）
- 金额字段是否有千分位分隔
- 空值字段是否显示为"—"而非空白
- 关联 ID 是否已转为可读名称
**交互行为**：
- 搜索框输入后列表是否实时过滤（HTMX keyup trigger）
- 下拉选择后筛选是否立即生效
- 点击表格行是否正确跳转详情
- 表单提交后是否显示成功提示或跳转
- 删除操作是否有确认弹窗
#### 测试类型速查
| 测试类型 | 操作步骤 | 验证要点 |
|---------|---------|---------|
| **页面加载** | `open <url> && snapshot -i` | 无 500 错误，页面标题正确 |
| **列表页渲染** | `open <url> && snapshot -i` | snapshot 中有表格行、表头列名正确、筛选栏元素存在 |
| **搜索/筛选** | `fill @eN "query" && press Enter && snapshot -i` | snapshot 中结果行数变化、无结果时有提示文本 |
| **创建表单** | `open <create_url> && fill 表单 && click 提交 && snapshot -i` | snapshot 显示提交成功或新数据出现 |
| **详情页** | `open <detail_url> && snapshot -i` | snapshot 中关联字段显示名称而非数字、字段完整 |
| **删除操作** | `click 删除按钮 && snapshot -i && click 确认` | snapshot 中对应行消失、无 errors |
| **状态流转** | `click 状态按钮 && snapshot -i` | snapshot 中状态标签文字变化正确 |
| **分页** | `click 下一页 && snapshot -i` | snapshot 中页码变化、数据行刷新 |
| **侧栏导航** | `click 侧栏菜单项 && wait 1000 && snapshot -i` | snapshot 显示新页面内容、侧栏高亮项变化 |
| **表单验证** | `fill 无效值 && click 提交 && snapshot -i` | snapshot 中出现错误提示文本 |
| **空数据** | `open <无数据的页面> && snapshot -i` | snapshot 中包含"暂无数据"类提示文本 |
| **CSS 样式** | `snapshot -i` 检查元素属性 | class 名称与 `uno.config.ts` 中的 shortcut 匹配 |
**关键规则**：
- 每次页面导航或 DOM 变更后，必须重新 `snapshot -i` 获取新的元素引用
- 用 `agent-browser errors --clear` 在操作前清空错误，操作后用 `errors` 检查
- 不要使用截图（screenshot）做验证，只使用 snapshot 的无障碍树结构
#### 与原型对比
如果用户提供了原型设计：
1. 用 `agent-browser open <原型URL>` 打开原型页面，`snapshot -i` 获取原型的无障碍树
2. 再打开实际页面，`snapshot -i` 获取实际的无障碍树
3. 逐区域对比：元素角色、文本内容、层级结构、表单字段名称、筛选栏组件
4. 发现差异立即修代码，修完验证，再继续对比下一个区域
5. 所有差异记录到测试报告的"与原型对比差异"表格中
如果原型是本地 HTML 文件，用 `read` 读取内容分析其结构。
### 阶段三：报告

测试完成后生成结构化报告到 `docs/plans/` 目录。

#### 报告格式

```markdown
# <模块名> 模块测试报告

**测试日期**: YYYY-MM-DD
**测试范围**: <模块名> 模块（X 个页面）
**测试数据**: <数据来源 SQL 脚本>

## 测试总览

| 页面 | 路径 | 状态 | 修复项 |
|------|------|------|--------|
| 页面名 | /path | ✅/🐛 | 问题描述 |

## 缺陷记录

### P0 阻塞
| # | 问题 | 修复 | Commit |
|---|------|------|--------|

### P1 严重
...

### P2 一般
...

## 数据验证结果
<具体数据核对记录>
```

#### 状态标记
- ✅ 通过
- ⚠️ 部分实现
- ❌ 未实现
- 🐛 缺陷
- ⏭ 无法测试（依赖未实现模块）

#### 缺陷优先级
- **P0 阻塞** — 页面 500、核心提交失败
- **P1 严重** — 数据不正确、状态流转失败、主要功能异常
- **P2 一般** — UI 偏差、缺少非关键字段
- **P3 轻微** — 美观/体验问题

## 修复后验证

发现缺陷后：
1. 修复代码
2. `cargo clippy` 确认编译通过
3. 用 agent-browser 回归测试确认修复生效
4. 更新报告中的状态

## 进阶用法

### Headed 模式（调试时使用）

加 `--headed` 可看到浏览器窗口实时操作：

```bash
agent-browser --headed open http://localhost:3000/admin/md/products
```

### 批量测试多个页面

可以用命令串联快速验证多个页面的可访问性：

```bash
agent-browser open http://localhost:3000/admin/mes && agent-browser snapshot -i && agent-browser errors
agent-browser open http://localhost:3000/admin/mes/plans && agent-browser snapshot -i && agent-browser errors
# ... 逐页验证
```

## 测试计划模板

如果用户要求先制定测试计划再执行，参照 `docs/plans/` 中已有的测试计划格式（如 `2026-06-07-mes-test-*` 系列），包含：
- 模块范围与功能清单
- 测试文件索引
- 测试工作流（Phase 划分）
- 公共测试项（导航、布局、通用组件）
- 逐页面测试用例（表格形式：测试项 + 操作 + 预期结果）
- 枚举值速查（状态、类型等）
- 测试结果记录规范
