# ABT E2E 回归测试套件

基于 [Playwright](https://playwright.dev) 的端到端回归测试，连本地运行的 ABT 服务（默认 `127.0.0.1:8000`），覆盖 SSR（Axum + Maud + HTMX + Hyperscript）页面的渲染、HTMX 局部刷新、CRUD 与状态流转。

## 前置

- ABT 服务在跑（`WEB_PORT=8000`）。⚠️ CLAUDE.md 约束：不用 `cargo run` 启动，服务由用户保持运行。
- 本地登录凭据可用（默认 `admin` / `chenxi0514`）。

## 安装

```bash
cd e2e
npm install
npx playwright install chromium
```

## 跑测试

```bash
cp .env.example .env          # 或直接用默认值
npm test                      # 全量
npm test -- smoke             # 只跑 smoke（15 路由可达性，无副作用）
npm test -- work-center       # 只跑作业中心 tab 切换（纯 GET，无副作用）
npm test -- bin-create        # 库位创建（写操作示范）
npm run test:ui               # 交互式 UI 模式
npm run report                # 看 HTML 报告
```

失败时 trace/screenshot/video 自动留存在 `test-results/`，`npm run report` 可查。

## 架构

| 文件 | 职责 |
|---|---|
| `playwright.config.ts` | baseURL、globalSetup、`workers:1` 串行、trace、storageState |
| `global-setup.ts` | API 登录 `/login` → `storageState` 到 `.auth/admin.json`（cookie `id=...`） |
| `fixtures/auth.ts` | 登录态断言 helper |
| `fixtures/wms.ts` | dev DB 硬编码常量 + `uniqueTag()` / `genIdempotencyKey()` |
| `tests/*.spec.ts` | spec 文件 |

每条 test 默认带登录态（`use.storageState`）。登录态 72h 过期，globalSetup 每次重登，无惧过期。

## 隔离纪律（抄 `abt-web/tests/wms_flow_e2e.rs`）

连真实共享 dev DB，**不回滚**——隔离全靠以下纪律：

1. **串行不并行**：`workers:1`（config）。dev DB 共享，并行必污染。
2. **断言增量不绝对值**：`after - base == delta`，禁绝对余额/数量断言。
3. **临时实体唯一后缀**：`uniqueTag()` → `T-PLAY-{timestamp}`，避免与历史数据撞名。
4. **写操作每次新 `idempotency_key`**：`genIdempotencyKey()` → `crypto.randomUUID()`。重复 key 第二次被幂等协议吞掉返回空 200（`wms_stock_in_create.rs:506`），跨 run 会假绿。
5. **复用 dev DB 硬编码常量**：`PRODUCT_ID=565`、`WAREHOUSE_A=23320`、`BIN_A=23320000`（见 `fixtures/wms.ts`），不自创。

## HTMX 等待策略（Playwright 专属）

ABT 不是 SPA，是 HTMX + Hyperscript。关键规则：

- **禁裸 `waitForTimeout`**（flaky 源头）。用 `waitForResponse` / `waitForSelector` / `waitForURL`。
- **drawer 关闭**：提交后关闭走 hyperscript `on 'htmx:afterRequest'[responseText 空] remove .open`，response 早于 DOM 关闭 → 等 `page.waitForSelector('.drawer-overlay:not(.open)')`，而非 timeout。
- **HTMX 局部刷新**（点 tab / 搜索）：`Promise.all([page.waitForResponse(url 匹配), 触发动作])` + `waitForSelector(目标区稳定文案)` 组合最稳。
- **搜索框 `keyup` 触发坑**：ABT 列表页 filter form 监听 `keyup changed delay:300ms from:.search-input`（防抖）。Playwright `fill()` 只触发 `input`/`change`、**不发 keyup** → HTMX 收不到、不会发起搜索请求。必须 `fill()` 后再 `dispatchEvent('keyup')`（合成 keyup 触发 HTMX），或用 `pressSequentially()` 逐字符（慢，仅必要时）。范本：`tests/work-center.spec.ts` 关键词搜索 spec。
- **提交按钮 selector 要限定**：header 的「退出登录」是 `<button type="submit">`（`form[hx-post=/logout]`），全站都有 → `button[type=submit]` 全页匹配到多个会触发 strict mode 报错。限定到具体 form（`#xxx-form button[type=submit]`）或用 `getByRole('button', { name: '保存库位' })` 按文本。范本：`tests/bin-create.spec.ts`。

## 加新 spec

1. **读类 spec**（列表 / 详情）：`page.goto(url)` + 断言 `#app-wrapper` / `#status-tabs` / `[id$="-data-card"]`。无副作用，可高频跑。
2. **写类 spec**：抄 `tests/bin-create.spec.ts` 的隔离模板（唯一标识 → 填表单 → HTMX 提交 → 等列表刷新 → 断言新增行存在）。
3. **跨域 smoke**：在 `tests/smoke.spec.ts` 的 `ROUTES` 数组加路由。

## 路线图（后续扩展，非首批）

- `stock-in-create`（采购入库端到端，需 `beforeAll` 用 API 建 Confirmed PO 作为来源——入库 JS 强制每行有 `source_doc_number`）
- `shipping-create`（发货申请，需前置建销售订单）
- `work-center-receive-po` / `work-center-batch-ship`（作业中心就地操作 drawer）
- sales / purchase / mes 各域同模式铺开

写操作 spec 分两类跑：**读类高频回归**（smoke + tabs，无副作用随便跑），**写类流程冒烟**（创建类，低频跑、隔离执行）。
