# abt-web2

Rust 全栈前端，Axum + Maud + HTMX + Alpine.js + UnoCSS，直接调用 `abt-core` Service trait。

## Commands

```bash
cargo run                     # 启动服务器 (port 3000)
cargo check                   # 快速编译检查
cargo clippy                  # Lint 检查
```

## Architecture

### SSR + HTMX

- **Axum Handler** 直接调用 `abt-core` Service trait，无 gRPC 中间层
- **Maud** 编译期 HTML 宏渲染完整页面或局部片段
- **HTMX** 处理表单提交、分页、搜索等需要服务器状态的交互
- **Alpine.js** 处理纯前端 UI 状态（dropdown、modal、tab 切换）

### 项目结构

```
src/
├── main.rs              # 入口：加载配置、初始化 PgPool、启动 Axum
├── config.rs            # 环境变量（WEB_PORT, DATABASE_URL, JWT_SECRET）
├── state.rs             # AppState：PgPool + 共享服务实例
├── errors.rs            # DomainError → HTTP 响应
├── auth/
│   ├── middleware.rs     # JWT 验证中间件
│   └── session.rs       # Session 类型
├── layout/
│   ├── base.rs          # HTML 壳（HTMX + Alpine + UnoCSS）
│   ├── admin.rs         # Admin 布局（sidebar + header + content）
│   ├── sidebar.rs       # 侧边栏
│   └── header.rs        # 顶部栏
├── components/          # 共享 UI 组件
└── routes/              # 路由模块（高内聚：TypedPath + Maud + Handler 同文件）
```

### 环境变量

- `DATABASE_URL`（必须，PostgreSQL 连接串）
- `JWT_SECRET`（必须）
- `WEB_PORT`（默认 3000）
- `WEB_HOST`（默认 0.0.0.0）
- `MAX_CONNECTION`（默认 20）

---

## 组件化实现范式 (Component Patterns in Maud)

### 函数式组件 (Functional Components)

适用于无状态、仅依赖入参的轻量级 UI 单元。

```rust
use maud::{html, Markup};

pub fn user_button(name: &str, is_admin: bool) -> Markup {
    html! {
        button class="btn" {
            "用户: " (name)
            @if is_admin {
                span class="badge-admin" { " [管理员]" }
            }
        }
    }
}
```

### 结构体组件与 Render Trait

适用于带有复杂内部数据、需要统一接口的标准组件。实现 `Render` 后可直接在 `html!` 宏中以 `(struct_instance)` 形式挂载。

```rust
use maud::{html, Markup, Render};

pub struct UserCard {
    pub name: String,
    pub avatar_url: String,
}

impl Render for UserCard {
    fn render(&self) -> Markup {
        html! {
            div class="user-card" {
                img src=(self.avatar_url) alt=(self.name);
                h3 { (self.name) }
            }
        }
    }
}
```

### 插槽/容器组件 (Children Slots)

通过传递已渲染的 `Markup` 对象实现父子组件嵌套。

```rust
use maud::{html, Markup};

pub fn layout_container(title: &str, children: Markup) -> Markup {
    html! {
        html {
            head {
                title { (title) }
                script src="https://unpkg.com/htmx.org@1.9.10" {}
            }
            body {
                main class="content" { (children) }
            }
        }
    }
}
```

---

## 抗碎片化工程实践 (Anti-Fragmentation Strategy)

### 强类型路由 + hx-select 局部精准更新

**必须使用 `TypedPath`**，禁止硬编码字符串 URL。同时利用 `hx-select` 和 `hx-target` 共同指向组件自身 ID，实现：

- Handler **始终返回完整组件**（含外层 ID 壳），无需感知请求来源
- htmx 在前端自动从响应中抓取指定 ID 片段并覆盖当前 DOM
- 一个 URL、一个 Handler，**禁止为局部刷新拆出额外的 Handler**

```rust
use axum::response::IntoResponse;
use axum_extra::routing::{RouterExt, TypedPath};
use maud::{html, Markup};
use serde::{Deserialize, Serialize};

// 1. 定义强类型 URL 路径
#[derive(TypedPath, Deserialize, Serialize, Clone)]
#[typed_path("/profile")]
pub struct ProfilePath {
    pub mode: String,
}

// 2. Maud 组件：外层 div 拥有固定 ID，hx-select/hx-target 指向自身
pub fn render_component(username: &str, current_mode: &str) -> Markup {
    let edit_path = ProfilePath { mode: "edit".to_string() };
    let display_path = ProfilePath { mode: "display".to_string() };

    html! {
        div class="profile-card" id="profile-section" {
            @if current_mode == "display" {
                p { "当前用户: " (username) }
                // hx-select + hx-target 指向自身 ID
                // 后端返回完整组件，htmx 只取 #profile-section 覆盖当前 DOM
                button
                    hx-get=(edit_path)
                    hx-target="#profile-section"
                    hx-select="#profile-section"
                    hx-swap="outerHTML" {
                    "编辑"
                }
            } @else {
                form
                    hx-post=(display_path)
                    hx-target="#profile-section"
                    hx-select="#profile-section"
                    hx-swap="outerHTML" {
                    input type="text" name="username" value=(username);
                    button { "保存" }
                }
            }
        }
    }
}

// 3. Handler：无论是初始加载还是 htmx 局部刷新，统一调用完整组件
pub async fn handler(path: ProfilePath) -> impl IntoResponse {
    render_component("Lyra", &path.mode)
}

// 4. 暴露子路由组
pub fn router() -> axum::Router {
    axum::Router::new().route_with_ts(handler)
}
```

### 高内聚设计

路由定义、Handler、Maud 组件收拢在同一个领域模块内，禁止跨文件拆分。

### 约束

- 组件外层必须有稳定的 `id` 属性，作为 `hx-select` 和 `hx-target` 的锚点
- `hx-select` 的值必须是页面上已存在元素的 CSS 选择器
- 禁止为"局部刷新"单独创建返回片段的 Handler

---

## 事件驱动解耦 (HX-Trigger)

当一个前端交互需要同时引发页面上多个互不隶属的局部组件刷新时，避免编写"聚合刷新路由"，采用 htmx 事件响应机制：

1. 主动组件发出 POST 请求（如 `/cart/add`）
2. 后端在响应头附带 `HX-Trigger: "cartUpdated"`，不返回大块 HTML
3. 被动组件声明 `hx-trigger="cartUpdated from:body"` 并指向各自的强类型路径

---

## 混合群岛架构边界 (Hybrid Islands Architecture)

严格界定 htmx 与纯前端交互的边界：

**使用 htmx 的场景**：交互涉及服务器状态流转、数据库读写、权限校验（表单提交、动态分页、条件搜索）。

**禁止使用 htmx 的场景**：纯前端 UI 状态切换（Dropdown 菜单展开、Modal 弹窗显隐、选项卡纯样式切换）。此类交互由 **Alpine.js** (`x-data`) 在前端本地闭环，严禁通过 htmx 向后端发送请求，防止路由碎片化。
