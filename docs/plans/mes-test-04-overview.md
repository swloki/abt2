# MES 模块 4 个待实现页面 — 总览计划

**日期**: 2026-06-07
**目标**: 实现排程看板、物料消耗追踪、生产异常、流转卡查询 4 个 MES 页面

## 实施顺序（由简到难）

| # | 页面 | 路径 | 计划文件 | 复杂度 |
|---|------|------|----------|--------|
| 1 | 流转卡查询 | /admin/mes/cards | `mes-test-04-card-query.md` | ★☆☆ |
| 2 | 排程看板 | /admin/mes/schedule | `mes-test-05-schedule-board.md` | ★★☆ |
| 3 | 物料消耗追踪 | /admin/mes/material-usage | `mes-test-06-material-usage.md` | ★★☆ |
| 4 | 生产异常 | /admin/mes/exceptions | `mes-test-07-exception.md` | ★★★ |

## 当前状态

- 4 个页面均已有**占位 stub**，路由和 TypedPath 已定义
- 后端已有完整的 `ProductionBatchService`、`BackflushService`、`MesDashboardService`
- 异常页面路由目前硬编码在 `routes/mod.rs` 而非独立 route 模块（需规范化）
- 原型设计文件在 `Open Design/.../04-*.html`，功能已完整定义

## 涉及的文件层级

### abt-core 层（后端）
- `mes/production_batch/repo.rs` — 添加 `find_by_card_sn` 查询
- `mes/production_batch/service.rs` — trait 新增 `find_by_card_sn`
- `mes/production_batch/implt.rs` — 实现 `find_by_card_sn`
- `mes/dashboard/service.rs` — 新增排程看板查询方法
- `mes/dashboard/repo.rs` — 实现排程看板 SQL
- `mes/dashboard/model.rs` — 新增看板数据模型
- `mes/production_exception/` — **新子模块**（migration + model/repo/service/implt/mod）

### abt-web 层（前端）
- `pages/mes_card_query.rs` — 重写为完整搜索+结果页
- `pages/mes_schedule_board.rs` — 重写为看板视图
- `pages/mes_material_usage.rs` — 重写为物料对比分析页
- `pages/mes_exception_list.rs` — 重写为异常列表页
- `pages/mes_exception_detail.rs` — **新增**异常详情页
- `routes/mes_exception.rs` — **新增**独立路由模块（替换 mod.rs 硬编码）
- `routes/mes_batch.rs` — 新增 CardQuerySearchPath
- `routes/mod.rs` — 移除异常硬编码路由

### Migration
- `migrations/xxx_create_production_exceptions.sql` — 新建异常表

## 共用模式

- 所有页面遵循 `#[require_permission("MES", "read")]` 守卫
- 页面布局使用 `admin_page(is_htmx, title, &claims, "production", PATH, "生产管理", parent_path, content)`
- 后端 service 结构体只持 `PgPool`，共享服务通过工厂函数按需获取
- 前端禁止内联 `style`，使用 uno.config.ts 定义的 CSS 类
- HTMX 处理服务端交互，Surreal.js 处理纯前端 UI 状态

## 验证方式

每个页面完成后：
1. `cargo clippy` 通过
2. 浏览器访问页面验证渲染
3. 原型设计对比验证功能完整性
