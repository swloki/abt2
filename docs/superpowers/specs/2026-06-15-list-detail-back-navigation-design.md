# 列表-详情返回导航优化 — 设计文档

> 关联 Issue: [#62](https://github.com/swloki/abt2/issues/62)
> 状态: **已定稿**
> 方案: 服务端 Session URL 快照
> 日期: 2026-06-15

## 一、问题陈述

系统中所有"列表页 → 详情页"跳转的业务模块（29 个详情页），返回按钮采用硬路由跳转到列表根路径，用户在列表页设置的筛选条件、翻页位置全部丢失。

期望：返回时精准还原用户进入详情前的列表状态（筛选参数 + 翻页）。

## 二、方案选型结论

评估了四个候选方案（A: history.back / B: history.back+滚动 / C: from参数 / D: 客户端sessionStorage），最终选定 **服务端 Session URL 快照**。

**选定理由**：
- 纯服务端 SSR，零客户端 JS / Hyperscript，完美契合项目架构
- 服务端 Session 是唯一真相源，不依赖浏览器历史栈 / hx-push-url / 客户端 JS
- 复用现成 `tower-sessions` 基础设施（`RequestContext.session` 已存在）
- 多级导航天然支持（仅列表页写 Session，详情页只读）

## 三、核心机制

### 3.1 状态存储

在服务端 Session 中维护一个映射表：

```
Session key: "list_urls"
value: HashMap<String, String>  // {列表路由路径: 最近完整URL}

示例:
{
  "/admin/md/bom":       "/admin/md/bom?keyword=电源&status=1&page=5",
  "/admin/wms/stock-in": "/admin/wms/stock-in?doc_number=RK&warehouse_id=3&page=2",
}
```

### 3.2 状态记录（列表页）

所有列表页 handler 在返回响应前，将当前请求的完整 URI（路径 + 查询参数）存入 Session。每次筛选、翻页（HTMX 请求）都会更新，始终保留最新状态。

**仅列表页写入**，详情页不触碰 Session 中的 `list_urls`。因此从详情A跳转到详情B后返回，仍一步回到最初带筛选条件的列表页。

### 3.3 返回还原（详情页）

所有详情页 handler 从 Session 读取对应列表的最近 URL，注入模板。若 Session 中无记录（首次直接访问详情），降级为列表根路径。

### 3.4 不恢复的内容

- 页面滚动位置（不实现；翻页已通过 URL page 参数保留，用户回到正确页码）
- 未提交的搜索框文字（HTMX 300ms 自动提交，窗口极短）

## 四、详细设计

### 4.1 RequestContext 增加 original_uri 字段

当前 handler 签名没有 URI 参数。在 `RequestContext` 中直接存原始 URI，所有 handler 通过 `ctx.original_uri` 获取，**不改任何 handler 签名**。

```rust
// abt-web/src/utils.rs

pub struct RequestContext {
    pub claims: Claims,
    pub conn: PgPoolConn,
    pub state: AppState,
    pub service_ctx: ServiceContext,
    pub headers: HeaderMap,
    pub session: Session,
    pub original_uri: Uri,    // 新增
}

// FromRequestParts 实现中
let original_uri = parts.uri.clone();  // 已含 query string
```

`parts.uri`（来自 `http::Parts`）包含当前请求的完整 URI（路径 + query string）。对于列表页筛选/翻页的 HTMX GET 请求，URI 包含所有 hx-include 的表单参数。

### 4.2 辅助函数（utils.rs）

```rust
const LIST_URLS_KEY: &str = "list_urls";

/// 列表页 handler 调用：记录当前列表 URL 到 Session
pub async fn record_list_url(session: &Session, list_path: &str, uri: &Uri) {
    let mut urls: HashMap<String, String> = session
        .get(LIST_URLS_KEY).await.ok().flatten().unwrap_or_default();
    urls.insert(list_path.to_string(), uri.to_string());
    let _ = session.insert(LIST_URLS_KEY, &urls).await;
}

/// 详情页 handler 调用：读取列表最近 URL
pub async fn get_list_url(session: &Session, list_path: &str) -> Option<String> {
    session
        .get::<HashMap<String, String>>(LIST_URLS_KEY).await.ok()?
        ?.get(list_path).cloned()
}
```

### 4.3 列表页 handler 改动（47 处，每处 +1 行）

在 handler 处理完业务逻辑、返回响应前，加一行记录调用：

```rust
// bom_list.rs — get_bom_list
pub async fn get_bom_list(
    _path: BomListPath,
    ctx: RequestContext,
    Query(mut params): Query<BomQueryParams>,
) -> crate::errors::Result<Html<String>> {
    // ... 现有业务逻辑 ...

    // 新增：记录列表 URL（返回前）
    crate::utils::record_list_url(&ctx.session, BomListPath::PATH, &ctx.original_uri).await;

    // 现有的返回
    Ok(/* ... */)
}
```

### 4.4 详情页 handler 改动（29 处，每处 +2 行）

handler 读取 Session 中的列表 URL，传给模板：

```rust
// bom_detail.rs — get_bom_detail
pub async fn get_bom_detail(
    _path: BomDetailPath,
    ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
    // 新增：读取返回 URL
    let back_url = crate::utils::get_list_url(&ctx.session, BomListPath::PATH).await
        .unwrap_or_else(|| BomListPath::PATH.to_string());

    // ... 现有业务逻辑 ...

    // 传给模板渲染函数
    Ok(admin_page(&ctx.claims, ..., bom_detail_content(&bom, ..., &back_url)))
}
```

### 4.5 详情页模板改动（29 处）

返回按钮的 href 从硬编码列表路径改为动态变量：

```rust
// 改动前
a class="back-link" href=(BomListPath::PATH) { "← 返回列表" }

// 改动后
a class="back-link" href=(back_url) { "← 返回列表" }
```

文案保持"返回列表"（语义准确：回到列表页，且保留筛选状态）。

## 五、改动范围

| 项目 | 文件数 | 每处改动 |
|---|---|---|
| RequestContext 加 `original_uri` 字段 | 1 (utils.rs) | ~3 行 |
| 辅助函数 `record_list_url` / `get_list_url` | 1 (utils.rs) | ~20 行 |
| 列表页 handler 加记录调用 | 47 | +1 行 |
| 详情页 handler 读取 + 传变量 | 29 | +2 行 |
| 详情页模板 href 改为变量 | 29 | 改 1 行 |
| **前端 JS** | **0** | — |
| **Hyperscript** | **0** | — |

## 六、边缘情况处理

| 场景 | 行为 |
|---|---|
| 首次直接访问详情（Session 无记录） | 降级为列表根路径 |
| Session 过期（用户长时间不活动） | 降级为列表根路径（需重新登录） |
| 同用户多标签页访问同列表 | Session 存最后一个标签页的状态（合理行为） |
| URL 中参数已失效（如删除了筛选的分类） | 服务器忽略无效参数，返回正常列表 |
| HTMX 局部刷新请求（筛选/翻页） | URI 含完整参数，正常记录 |

## 七、验证策略

1. **单元验证**：`record_list_url` / `get_list_url` 的 Session 读写正确性
2. **端到端验证**（agent-browser）：
   - 列表页筛选 + 翻页 → 进入详情 → 点返回 → 验证列表筛选/翻页保留
   - 多级导航：列表 → 详情A → 详情B → 返回 → 一步回列表（带筛选）
   - 直接打开详情 URL → 返回 → 降级列表根路径
3. **`cargo clippy`** 编译验证

## 八、方案弃选记录

评估后弃选的方案及原因：

- **A: history.back**：依赖浏览器历史栈，多级导航逐级回退，开发者遗漏 hx-push-url 即失效
- **B: history.back + 滚动恢复**：核心缺陷同 A，仅补滚动
- **C: from 参数**：URL 污染（编码参数泄漏到书签/分享），改动面大（47 处列表链接）
- **D: 客户端 sessionStorage**：引入客户端状态管理，偏离 SSR 架构；依赖 JS 加载
