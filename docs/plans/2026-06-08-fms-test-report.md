# FMS 模块测试报告

**测试日期**: 2026-06-08
**测试范围**: FMS（财务管理）模块（9 个页面）
**测试数据**: `scripts/fms-test-data.sql`, `scripts/fms-fix-source-type.sql`, `scripts/fms-fix-counterparty.sql`

## 测试总览

| 页面 | 路径 | 状态 | 备注 |
|------|------|------|------|
| 财务总览 Dashboard | /admin/fms | ✅ | 动态数据，从 DB 加载 |
| 出纳日记账列表 | /admin/fms/journals | ✅ | 含筛选、分页 |
| 日记账详情 | /admin/fms/journals/{id} | ✅ | 显示关联名称 |
| 新建日记账 | /admin/fms/journals/create | ✅ | 表单可提交 |
| 费用报销列表 | /admin/fms/expenses | ✅ | 含筛选、分页 |
| 报销单详情 | /admin/fms/expenses/{id} | ✅ | 含明细项 |
| 新建费用报销 | /admin/fms/expenses/create | ✅ | 动态行项 |
| 核销管理 | /admin/fms/writeoffs | ✅ | 含列表数据 |
| 成本核算分析 | /admin/fms/cost-analysis | ✅ | 4 个 Tab 面板 |

## 本次改动摘要

### 核心改动：Dashboard 从硬编码改为真实数据

**之前**: Dashboard 的统计卡片（本月流入/流出/净现金流/待核销/待审报销）、类型分布、月度趋势全部硬编码。
**之后**: 所有数据从数据库通过 Service trait 查询聚合。

#### 新增 Service 方法

| 文件 | 方法 | 功能 |
|------|------|------|
| `cash_journal/service.rs` | `distribution_by_type(period)` | 按类型分组的当月金额 |
| `cash_journal/service.rs` | `monthly_trend(months_back)` | 近 N 月每月流入/流出 |
| `expense/service.rs` | `pending_summary()` | 待审报销数量和总金额 |

#### 新增 Repo SQL 查询

| 方法 | SQL 逻辑 |
|------|------|
| `CashJournalRepo::distribution_by_type` | `SELECT journal_type, SUM(amount) FROM cash_journals WHERE period=$1 AND status=2 GROUP BY journal_type` |
| `CashJournalRepo::monthly_trend` | `SELECT period, SUM(direction=1), SUM(direction=2) FROM cash_journals WHERE status=2 AND period >= ... GROUP BY period ORDER BY period` |
| `ExpenseReimbursementRepo::pending_summary` | `SELECT COUNT(*), SUM(total_amount) FROM expense_reimbursements WHERE status=2` |

#### Dashboard 数据验证

| 统计项 | 页面显示 | 数据库值 | 计算 | ✅ |
|--------|---------|---------|------|---|
| 本月流入 | ¥38.4万 | 384,500 | 384500/10000=38.45→38.4 | ✅ |
| 本月流出 | ¥49.7万 | 497,130 | 497130/10000=49.713→49.7 | ✅ |
| 净现金流 | -¥11.3万 | -112,630 | -112630/10000=-11.263→-11.3 | ✅ |
| 待审报销 | 1 | status=2: 1条 | — | ✅ |
| 待审金额 | ¥0.5万 | 4,650 | 4650/10000=0.465→0.5 | ✅ |
| 分布-销售回款 | ¥38.4万 | 384,500 | journal_type=1, direction=1 | ✅ |
| 分布-采购付款 | ¥7.5万 | 75,250 | journal_type=2, direction=2 | ✅ |
| 分布-费用报销 | ¥0.3万 | 3,280 | journal_type=3, direction=2 | ✅ |
| 分布-工资支付 | ¥41.9万 | 418,600 | journal_type=4, direction=2 | ✅ |
| 趋势-5月 | +24.8万 | 283200-35600=247600 | 247600/10000=24.76→24.8 | ✅ |
| 趋势-6月 | -11.3万 | 384500-497130=-112630 | -112630/10000=-11.263→-11.3 | ✅ |

### CSS Premium 样式

所有 FMS Dashboard premium 样式通过 `.fms-dashboard` 作用域限定，不影响其他页面：

- `backdrop-filter: blur(12px)` glassmorphism
- `mes-stat-icon::after` gloss overlay
- `mes-stat-value` gradient text
- `quick-card` glassmorphism + hover lift + icon scale
- `section-card/head` gradient background
- `flow-row` hover highlight
- `mini-avatar::after` gloss overlay
- `progress-bar-fill::after` gloss
- `btn-primary` triple gradient
- `page-title` gradient text (24px, 800)

## 缺陷记录

### P0 阻塞
无。

### P1 严重
无。

### P2 一般
无。

### P3 轻微
1. 待核销金额卡片暂时显示 ¥0万 — 需要接入核销统计查询后补充真实数据

## 编译状态

- `cargo clippy -p abt-core -p abt-web`: ✅ 通过（仅有 warnings，无 errors）
- 所有 9 个 FMS 页面: ✅ 零 JS 错误加载
