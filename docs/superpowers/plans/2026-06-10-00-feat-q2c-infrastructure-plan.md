---
title: "Q2C E2E 测试 — 00 测试基础设施搭建"
date: 2026-06-10
type: feat
plan_depth: standard
origin: docs/superpowers/specs/2026-06-10-q2c-e2e-test-overview.md
related_specs:
  - docs/superpowers/specs/2026-06-10-q2c-e2e-test-data-env.md
  - docs/superpowers/specs/2026-06-10-q2c-e2e-test-agent-strategy.md
---

## Summary

搭建 Quote-to-Cash 全链路 E2E 测试的基础设施层：测试目录结构、共享 Shell 工具库（登录/导航/断言/接力）、SQL Fixture 数据预置脚本、Agent 会话初始化。这是所有后续测试计划（Plan 01-04）的前置依赖。

## Problem Frame

ABT 系统目前仅有权限测试（`tests/permission/`），没有业务流程 E2E 测试框架。要运行 87 个 Q2C 测试场景，需要先建立：可复用的测试工具函数、标准化的测试数据、多角色 Agent 会话管理机制。没有这些基础设施，每个测试场景都要重复造轮子，且无法保证数据一致性。

---

## Requirements

- R1. 创建标准化的测试目录结构 `tests/e2e-q2c/`
- R2. 提供共享 Shell 工具库，封装 agent-browser 的常见操作（登录、导航、表单填写、断言、接力数据传递）
- R3. 提供 SQL Fixture 脚本，预置 Q2C 全链路所需的主数据（物料/BOM/客户/供应商/仓库/用户/价格）
- R4. 提供 Agent 会话初始化机制，支持 15 个角色的独立浏览器会话
- R5. 提供环境清理脚本，支持一键重置测试数据
- R6. 提供接力状态文件管理机制，支持跨 Agent 数据传递

---

## Key Technical Decisions

KTD1. **Shell 脚本作为测试载体** — agent-browser 是 CLI 工具，测试脚本采用 Bash 而非 Node.js/Python。Shell 函数库提供可复用的封装，测试场景调用这些函数。理由：agent-browser 原生就是 Shell 调用，无需额外运行时。

KTD2. **CSS 选择器优先于 @e 动态引用** — agent-browser 的 `@e` 编号每次 snapshot 都会变，不可靠。测试工具库优先使用 CSS 选择器（`input[name='field']`）和 JavaScript `eval` 进行元素定位和操作。

KTD3. **SQL 直插主数据 + agent-browser 操作业务数据** — 物料/BOM/客户等主数据通过 SQL 脚本预置（高效、幂等）；报价/SO/PO 等业务数据通过 agent-browser UI 操作创建（真实模拟用户行为）。

KTD4. **共享文件接力** — Agent 间数据传递使用 JSON 文件（`relay-state.json`），每个节点写入输出，下一个节点读取输入。简单可靠，无需额外服务。

KTD5. **Argon2 密码复用** — 沿用 `tests/permission/seed.sql` 中已生成的 Argon2 hash（密码 `test1234`），所有测试用户统一密码。

---

## Implementation Units

### U1. 测试目录结构与配置

**Goal:** 创建 `tests/e2e-q2c/` 目录结构，含配置文件和环境变量模板。

**Requirements:** R1

**Files:**
- Create: `tests/e2e-q2c/config/env.sh` — 环境配置（URL、超时、会话名映射）
- Create: `tests/e2e-q2c/config/agents.sh` — Agent 角色定义（15 个角色的用户名/Session 名/权限范围）
- Create: `tests/e2e-q2c/relay/relay-state.json` — 接力状态文件模板

**Approach:** 目录结构按设计文档 `docs/superpowers/specs/2026-06-10-q2c-e2e-test-agent-strategy.md` 第 4.2 节的定义创建。配置文件提供环境变量和角色映射，所有测试脚本 `source` 这些配置。

**Patterns to follow:** 参考 `tests/permission/` 的组织方式。

**Test scenarios:**
- 加载 `env.sh` 后 `$ABT_URL` 变量正确设置
- 加载 `agents.sh` 后 `AGENT_SALES_USER` 等变量正确设置

**Verification:** 在 Shell 中 `source config/env.sh && echo $ABT_URL` 返回 `http://localhost:8000`。

---

### U2. 共享 Shell 工具库 — 登录与会话

**Goal:** 封装 agent-browser 的登录、会话管理、页面导航为可复用 Shell 函数。

**Requirements:** R2, R4

**Dependencies:** U1

**Files:**
- Create: `tests/e2e-q2c/lib/login.sh` — 登录/登出/会话初始化函数
- Create: `tests/e2e-q2c/lib/session.sh` — Agent 会话管理（初始化所有角色会话）

**Approach:**

登录函数 `abt_login` 接受 session 名、用户名、密码参数，执行：打开登录页 → 填写用户名/密码 → 点击提交 → 等待 → 验证仪表盘加载。

会话初始化函数 `init_all_sessions` 批量为 15 个角色创建独立浏览器会话并完成登录，生成 `session-ready` 标记文件。

关键函数签名：
- `abt_login <session> <username> <password>` — 登录
- `abt_logout <session>` — 登出
- `abt_navigate <session> <url>` — 导航到指定页面
- `init_all_sessions` — 初始化全部 15 个 Agent 会话
- `cleanup_all_sessions` — 清理全部会话

**Patterns to follow:** 参考 `.claude/skills/page-test/SKILL.md` 中的多 Session 并行测试模式和 `testing-guide.md` 的 Session 管理命令。

**Test scenarios:**
- 以 admin 账号登录 → 验证页面包含仪表盘元素
- 以 sales_user 登录 → 验证可以访问报价列表页
- 以 guest 用户登录 → 验证被正确限制权限
- 重复登录（已有 Session）→ 不报错，复用 Session

**Verification:** 运行 `source lib/login.sh && abt_login test_session admin admin123` 成功，浏览器在仪表盘页面。

---

### U3. 共享 Shell 工具库 — 表单操作与断言

**Goal:** 封装表单填写、下拉选择、日期选择、按钮点击、页面断言等操作。

**Requirements:** R2

**Dependencies:** U2

**Files:**
- Create: `tests/e2e-q2c/lib/form.sh` — 表单操作函数
- Create: `tests/e2e-q2c/lib/assert.sh` — 页面断言函数

**Approach:**

表单操作使用 JavaScript `eval` 注入，避免依赖动态 `@e` 引用：

- `abt_fill <session> <css_selector> <value>` — 填写文本字段
- `abt_select <session> <css_selector> <value>` — 选择下拉选项（通过 `dispatchEvent(new Event('change'))` 触发 HTMX）
- `abt_click <session> <css_selector>` — 点击按钮
- `abt_click_by_text <session> <button_text>` — 按文字内容查找并点击按钮

断言函数：

- `abt_assert_visible <session> <css_selector> <error_msg>` — 断言元素可见
- `abt_assert_text <session> <css_selector> <expected>` — 断言文本内容匹配
- `abt_assert_url_contains <session> <path>` — 断言当前 URL 包含指定路径
- `abt_assert_toast <session> <expected_text>` — 断言页面出现成功/错误提示

**Patterns to follow:** 参考 `.claude/skills/page-test/SKILL.md` 中 `form-test.md` 的表单操作模式和 `commands.md` 的 eval 技巧。

**Test scenarios:**
- `abt_fill` 填写输入框 → 验证值已设置
- `abt_select` 选择下拉项 → 验证 HTMX 联动触发
- `abt_assert_text` 断言页面标题 → 通过/失败正确返回
- `abt_assert_toast` 验证操作成功提示 → 正确识别

**Verification:** 创建一个简单的测试脚本调用各函数，验证行为正确。

---

### U4. 共享 Shell 工具库 — 接力状态管理

**Goal:** 实现 Agent 间数据传递的接力状态文件管理。

**Requirements:** R6

**Dependencies:** U1

**Files:**
- Create: `tests/e2e-q2c/lib/relay.sh` — 接力状态文件读写函数

**Approach:**

接力状态使用 JSON 文件存储，通过 `jq` 命令读写：

- `relay_init <run_id>` — 初始化接力文件，写入 run_id 和空 artifacts
- `relay_write <key> <value>` — 写入一个键值对到 artifacts
- `relay_read <key>` — 读取一个键值对
- `relay_set_phase <phase>` — 更新当前阶段
- `relay_set_status <status>` — 更新状态（completed/failed/blocked）
- `relay_snapshot <snapshot_point>` — 在关键节点创建数据快照标记

接力文件路径：`tests/e2e-q2c/relay/relay-state.json`

**Test scenarios:**
- 初始化接力文件 → 文件包含 run_id 和空 artifacts
- 写入 quotation_id → 读取返回正确值
- 覆盖已有 key → 新值生效
- 读取不存在的 key → 返回空字符串

**Verification:** `source lib/relay.sh && relay_init test && relay_write so_id "SO-001" && relay_read so_id` 返回 `SO-001`。

---

### U5. SQL Fixture — 用户与角色

**Goal:** 创建 Q2C 测试专用的用户和角色预置 SQL 脚本。

**Requirements:** R3, R5

**Dependencies:** 无

**Files:**
- Create: `tests/e2e-q2c/fixtures/01_users_and_roles.sql` — 15 个测试用户 + 对应角色 + 权限分配

**Approach:**

沿用 `tests/permission/seed.sql` 的模式（Argon2 hash、`ON CONFLICT DO NOTHING`），但扩展到 Q2C 所需的全部 15 个角色：

用户列表（密码统一 `test1234`，hash 复用现有值）：
`q2c_sales`, `q2c_sales_mgr`, `q2c_planner`, `q2c_buyer`, `q2c_buyer_mgr`, `q2c_prod_mgr`, `q2c_operator`, `q2c_qc`, `q2c_qc_mgr`, `q2c_warehouse`, `q2c_accountant`, `q2c_cost_acct`, `q2c_cashier`, `q2c_gl_acct`, `q2c_gm`

每个角色分配对应业务域的 CRUD 权限。角色和权限需要覆盖设计文档 `docs/superpowers/specs/2026-06-10-q2c-e2e-test-data-env.md` 第 1.1 节定义的范围。

脚本必须幂等（`ON CONFLICT DO NOTHING`），可重复执行。

**Patterns to follow:** 参考 `tests/permission/seed.sql` 的用户创建、角色分配、权限映射模式。

**Test scenarios:**
- 执行脚本后 `q2c_sales` 用户存在
- `q2c_sales` 用户可以登录系统
- `q2c_sales` 用户有 SALES_ORDER 的 create 权限
- 重复执行不报错（幂等性）

**Verification:** `psql "$DATABASE_URL" -c "SELECT username FROM users WHERE username LIKE 'q2c_%'"` 返回 15 行。

---

### U6. SQL Fixture — 主数据（物料/BOM/客户/供应商/仓库/价格）

**Goal:** 创建 Q2C 全链路所需的主数据预置脚本。

**Requirements:** R3

**Dependencies:** U5（用户需先存在，用于 operator_id）

**Files:**
- Create: `tests/e2e-q2c/fixtures/02_master_data.sql` — 物料、BOM、工艺路线、客户、供应商、仓库、价格表
- Create: `tests/e2e-q2c/fixtures/03_initial_inventory.sql` — 初始库存

**Approach:**

按设计文档 `docs/superpowers/specs/2026-06-10-q2c-e2e-test-data-env.md` 第 1.1 节的数据清单创建：

物料：PRD-FG-001（成品A）、PRD-SFG-001（半成品B）、PRD-RM-001/002/003（原材料）
BOM：成品A → 半成品B×1 + 原材料D×0.5KG + 辅料E×1；半成品B → 原材料C×2KG
客户：CUS-001（正常）、CUS-002（信用冻结）
供应商：SUP-001、SUP-002
仓库：WH-RAW、WH-WIP、WH-FG、WH-QC、WH-REJ、WH-SCRAP（含仓位）
价格：成品售价 ¥1,500；原材料采购价 ¥50/30/5

初始库存：原材料仓有库存（确保采购和生产可以部分进行），成品仓为空（强制走完生产流程）。

**Patterns to follow:** 参考 `scripts/sales-test-data.sql` 和 `scripts/wms-test-data.sql` 的数据创建模式。

**Test scenarios:**
- 执行后 PRD-FG-001 存在且类型为"成品"
- BOM 展开结果包含 3 个直接子件
- CUS-001 信用额度为 500,000
- WH-RAW 仓库存在且包含仓位 WH-RAW-A01
- 初始库存 PRD-RM-001 在 WH-RAW 中有 500 KG
- PRD-FG-001 在 WH-FG 中库存为 0

**Verification:** SQL 查询验证各主数据存在且字段正确。

---

### U7. 环境清理脚本

**Goal:** 提供一键清理测试数据的脚本，支持环境重置。

**Requirements:** R5

**Dependencies:** U5, U6

**Files:**
- Create: `tests/e2e-q2c/fixtures/99_cleanup.sql` — 清理脚本
- Create: `tests/e2e-q2c/scripts/setup.sh` — 一键环境初始化（清理 → 建用户 → 建主数据 → 建库存）
- Create: `tests/e2e-q2c/scripts/teardown.sh` — 一键环境清理

**Approach:**

清理脚本按依赖关系逆序删除：业务单据 → 库存事务 → 主数据 → 用户。使用 `BEGIN/COMMIT` 事务包裹，确保原子性。

`setup.sh` 流程：执行 99_cleanup → 01_users → 02_master → 03_inventory → 输出 "Environment ready"。
`teardown.sh` 流程：执行 99_cleanup → 输出 "Environment cleaned"。

**Patterns to follow:** 参考 `tests/permission/cleanup.sql` 的逆序删除模式。

**Test scenarios:**
- 执行 setup.sh 后所有主数据和用户存在
- 执行 teardown.sh 后所有 q2c_ 前缀数据被清除
- 连续执行两次 setup.sh 不报错（幂等性）
- teardown 后重新 setup 可以正常运行

**Verification:** `bash scripts/setup.sh` 成功退出，`psql` 查询验证数据存在。

---

### U8. 集成冒烟测试

**Goal:** 验证整个基础设施层协同工作：环境初始化 → Agent 登录 → 页面导航 → 断言 → 接力传递 → 清理。

**Requirements:** R1-R6

**Dependencies:** U1, U2, U3, U4, U5, U6, U7

**Files:**
- Create: `tests/e2e-q2c/tests/smoke-test.sh` — 端到端冒烟测试

**Approach:**

冒烟测试脚本执行以下步骤：
1. `source` 所有 lib/*.sh 和 config/*.sh
2. 运行 `scripts/setup.sh` 初始化环境
3. 以 `q2c_sales` 登录 → 导航到报价列表 → 断言页面加载
4. 以 `q2c_warehouse` 登录 → 导航到库存页面 → 断言初始库存显示
5. 写入接力数据 → 读取并验证
6. 运行 `scripts/teardown.sh` 清理
7. 输出 PASS/FAIL 汇总

**Test scenarios:**
- 全流程冒烟通过（基础设施集成验证）
- 单独测试每个 lib 函数的基本功能

**Verification:** `bash tests/smoke-test.sh` 输出 "ALL PASSED"。

---

## Scope Boundaries

### In Scope
- 测试目录结构、共享 Shell 工具库、SQL Fixture 脚本
- 15 个测试用户的创建和权限配置
- Q2C 全链路所需的基础主数据预置
- 环境初始化和清理脚本
- 基础设施层的冒烟测试

### Deferred to Follow-Up Work
- 具体业务场景的测试脚本（Plan 01-04）
- 异常场景和逆向操作的测试脚本（Plan 04）
- 数据一致性校验脚本（Plan 03）
- 测试报告生成器（Plan 03）
- CI/CD 集成配置

---

## Open Questions

- Q1. `jq` 是否已安装在环境中？接力状态管理依赖它。如果不可用，需要改为 `sed/awk` 解析 JSON 或使用纯文本格式。
- Q2. 系统中是否已存在 Q2C 所需的全部权限资源编码（如 `QUOTATION`, `CONTRACT`, `MRP` 等）？如果不存在，Fixture 脚本需要先创建这些资源编码。
