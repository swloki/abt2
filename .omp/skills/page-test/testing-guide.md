# 测试流程详细指南

## 整体流程：测完→收集→统一修→回归验证

```
阶段 A：逐页串行测试
  一个页面测完再测下一个 → 用默认 session → 发现问题记录到临时文档

阶段 B：统一修复
  所有页面测完 → 读取临时文档 → 按优先级批量修复 → 一次编译重启

阶段 C：回归验证
  逐页验证所有修复项 → 确认通过 → 更新测试报告
```

---

## 阶段 A：逐页串行测试收集问题

### 登录准备

开始测试前先登录，使用默认 session：

```bash
agent-browser open https://localhost:8000/login
agent-browser snapshot -i
agent-browser fill @e1 "admin"
agent-browser fill @e2 "chenxi0514"
agent-browser click @e3
sleep 2
```

### 环境重置

登录后、测试前，清理可能影响测试的缓存状态：

```bash
agent-browser eval "
  localStorage.clear();
  sessionStorage.clear();
"

### 对每个页面：先计划后执行

每开始测试一个新页面之前，先输出该页面的测试计划，列出要逐项验证的功能点：

```
## 测试页面：/admin/mes/plans（生产计划列表）
测试项：
  1. 页面加载 — 访问页面，确认无 500 错误
  2. 页面标题和面包屑 — "生产管理 > 生产计划"
  3. 筛选栏 — 日期选择器、状态下拉、搜索框是否存在
  4. 表格表头 — 列名正确
  5. 表格数据 — 数据行显示产品名称而非 ID
  6. 状态标签 — 各状态颜色和文字正确
  7. 搜索功能 — 输入关键词后列表正确过滤
  8. 分页 — 翻页后数据刷新
  9. 点击行跳转详情 — URL 正确跳转
```

### 发现问题时的处理

```
测试 → 发现问题？
  ├─ 是 → 记录到临时文档（页面、问题、文件、优先级）→ 继续下一项
  └─ 否 → 标记 ✅ → 继续下一项
```

**不要停下来修代码。** 所有问题记录到临时文档，等全部测完再统一修。

### 临时问题文档

测试过程中将发现的问题写入 `docs/plans/<date>-<module>-issues.md`：

```markdown
# <模块名> 测试问题清单

| # | 页面 | 测试项 | 问题描述 | 涉及文件 | 优先级 | 状态 |
|---|------|--------|---------|----------|--------|------|
| 1 | 发货新建 | 提交 | 客户 select 缺 name 导致 422 | shipping_create.rs | P1 | 🔲 |
```

---

## 阶段 B：统一修复

所有页面测完后，按临时文档中的问题列表逐个修复：

```
读取问题文档 → 按优先级排序（P0→P1→P2→P3）→ 逐个修复 → cargo clippy 验证 → 编译重启
```

### 每个问题的修复流程

```
1. 定位问题 → 读涉及文件，找到 bug 根因
2. 修改代码 → 最小化修改，不改无关逻辑
3. 检查同类 → 同一 bug 模式是否存在于其他表单
4. 标记完成 → 更新问题文档状态为 🔧
```

### 常见 bug 模式（一次发现，全部排查）

| Bug 模式 | 排查方式 | 影响范围 |
|---------|---------|---------|
| IIFE 闭包函数未暴露全局 | `eval "typeof 函数名"` 逐个检查 | 所有用 IIFE 内联 JS 的页面 |
| 表单重复 `name` 字段 | `eval` 枚举 FormData 查重 | 所有有多处输入同名 name 的表单 |
| `me('#id')` 在回调中不可靠 | 搜索代码中 `me('#` 在函数/回调内的用法 | 所有用 surreal.js 的提交按钮 |
| HTMX swap 后属性丢失 | 检查服务端返回的 HTML 片段是否包含必要属性 | 所有 `hx-swap="outerHTML"` 的组件 |
| 客户 select 缺 `name` 属性 | 检查 select 是否有 `name="customer_id"` | 所有新建表单 |

全部修完后一次编译重启：`./scripts/restart-abt.sh --clippy`

---

## 阶段 C：回归验证

修复 + 编译 + 重启完成后，**必须对每个修复项逐个回归测试**：

```
对每个修复项：
  1. agent-browser 打开对应页面
  2. 执行与阶段 A 相同的测试操作（重现原始问题场景）
  3. 验证问题已修复 → 更新问题文档状态为 ✅
  4. 顺带检查该页面的其他功能是否受影响（修复副作用检查）
```

**如果回归验证发现修复不完整或引入新问题：**
- 追加到问题文档
- 立即修复 → 再次编译重启 → 针对性回归
- 直到所有项都是 ✅

回归验证完成后，汇总生成最终测试报告到 `docs/plans/`。

---

## 测试操作方式

```bash
agent-browser open <url>           # 打开页面
agent-browser snapshot -i          # 获取无障碍树 + 交互元素引用 (@e1, @e2, ...)
# ... 执行交互操作 ...
agent-browser snapshot -i          # 操作后重新获取元素
agent-browser errors               # 检查页面是否有 JS 错误
```

**验证手段**：功能验证与数据断言使用 `snapshot -i` 的无障碍树输出。截图仅作为 UI 交互异常的兜底排查手段（元素不可见、点击无响应、白屏、Modal 层级错误等），不作为常规验证手段。

### 细粒度检查要点

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

---

## 测试类型速查

| 测试类型 | 操作步骤 | 验证要点 |
|---------|---------|---------|
| **页面加载** | `open <url> && snapshot -i` | 无 500 错误，页面标题正确 |
| **列表页渲染** | `open <url> && snapshot -i` | 表格行、表头列名正确、筛选栏存在 |
| **搜索/筛选** | `fill @eN "query" && snapshot -i` | 结果行数变化、无结果时有提示 |
| **创建表单** | 见 form-test.md | 完整 fill → submit → verify 流程 |
| **详情页** | `open <detail_url> && snapshot -i` | 关联字段显示名称而非数字 |
| **状态流转** | `click 状态按钮 && snapshot -i` | 状态标签文字变化正确 |
| **分页** | `click 下一页 && snapshot -i` | 页码变化、数据行刷新 |
| **表单验证** | `fill 无效值 && submit && snapshot -i` | 出现错误提示文本 |
| **空数据** | `open <无数据页面> && snapshot -i` | 包含"暂无数据"类提示 |

**关键规则**：
- 每次页面导航或 DOM 变更后，必须重新 `snapshot -i`
- 用 `agent-browser errors --clear` 清空后再检查

---

## 测试数据准备

### 优先使用现有脚本

检查 `scripts/` 目录下是否有对应模块的测试数据 SQL：
- `scripts/sales-test-data.sql`
- `scripts/mes_test_data.sql`
- `scripts/wms-test-data.sql`

```bash
psql "$DATABASE_URL" -f scripts/sales-test-data.sql
```

### 没有现成脚本时自动生成

1. 查看 `abt-core/migrations/` 中该模块的建表 SQL
2. 查看该模块的 `model.rs` 和 `repo.rs`
3. 生成 INSERT SQL，确保满足外键约束
4. 保存到 `scripts/<module>-test-data.sql`

**数据依赖链**：
```
products → BOMs → production_plans → ...
products → warehouses → zones → bins → inventory → ...
```
