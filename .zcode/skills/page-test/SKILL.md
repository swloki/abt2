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

## 核心流程

```
准备（数据+登录）→ 阶段 A（并行测试收集问题）→ 阶段 B（统一修复）→ 阶段 C（回归验证）→ 报告
```

**支持并行测试**：使用 `agent-browser --session <name>` 创建独立浏览器实例，每个 subagent 用不同 session 并行测试不同页面。

**⚠️ 所有缺陷都必须修复，不管 P 几级。** 优先级仅用于修复顺序和报告分类。

### 分层测试

根据用户需求选择测试深度：

| 层级 | 范围 | 适用场景 |
|------|------|---------|
| **Smoke** | 页面加载无 500 + 关键表格有数据 + 主按钮存在 | 快速验证、每次提交后 |
| **Full** | Smoke + 筛选/搜索 + 新建表单完整流程 + 业务逻辑验证 | 模块完整测试 |
| **Regression** | 只测上次 issue 相关的页面和功能 | 修复后验证 |

用户未指定时默认 **Full**。

## 快速开始

```bash
# 1. 准备数据（如需要）
psql "$DATABASE_URL" -f scripts/sales-test-data.sql

# 2. 登录
agent-browser open http://localhost:8000/login
agent-browser fill @e1 "admin" && agent-browser fill @e2 "admin123"
agent-browser click @e3 && sleep 2

# 3. 测试页面
agent-browser open http://localhost:8000/admin/orders && agent-browser snapshot -i

# 4. 修改后重启
./scripts/restart-abt.sh --clippy
```

## 详细文档

| 文档 | 内容 |
|------|------|
| [testing-guide.md](testing-guide.md) | 测试流程（阶段 A/B/C）、测试类型、检查要点 |
| [form-test.md](form-test.md) | 新建表单深度测试 + 业务逻辑与异常输入测试 |
| [commands.md](commands.md) | agent-browser 命令库、eval 技巧、轻量断言、HTMX 错误捕获、常见问题排查 |
| [report-template.md](report-template.md) | 报告格式、问题清单模板、状态标记 |

## 准备检查项

1. **确认测试范围** — 用户指定页面/模块/全部
2. **准备测试数据** — 优先用 `scripts/*.sql`，没有则自动生成（见 testing-guide.md）
3. **登录认证** — 确保已登录（cookie 有效期内可跳过）
4. **服务重启** — 代码变更后用 `./scripts/restart-abt.sh`
