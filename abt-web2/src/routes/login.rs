use axum::extract::State;
use axum::http::header::SET_COOKIE;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Redirect};
use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use crate::components::icon::*;
use crate::layout::base::base_html;
use crate::state::AppState;

// ── Typed Paths ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/login")]
pub struct LoginPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/logout")]
pub struct LogoutPath;

// ── Module Router ──

pub fn router(state: AppState) -> Router {
    Router::new()
        .route(LoginPath::PATH, get(get_login).post(post_login))
        .route(LogoutPath::PATH, post(post_logout))
        .with_state(state)
}

// ── Handlers ──

pub async fn get_login(_path: LoginPath) -> axum::response::Html<String> {
    let page = base_html("登录", login_page(None));
    axum::response::Html(page.into_string())
}

#[derive(Deserialize)]
pub struct LoginForm {
    username: String,
    password: String,
}

pub async fn post_login(
    _path: LoginPath,
    State(state): State<AppState>,
    axum::Form(form): axum::Form<LoginForm>,
) -> impl IntoResponse {
    use abt_core::shared::identity::AuthService;
    use crate::routes::dashboard::DashboardPath;

    let auth = state.auth_service();

    match auth.login(&form.username, &form.password).await {
        Ok((token, _claims)) => {
            let cookie = format!(
                "token={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age={}",
                state.jwt_expiration_hours * 3600
            );
            let mut headers = HeaderMap::new();
            headers.insert(SET_COOKIE, cookie.parse().unwrap());
            (headers, Redirect::to(DashboardPath::PATH)).into_response()
        }
        Err(_) => {
            let page = base_html("登录", login_page(Some("用户名或密码错误")));
            axum::response::Html(page.into_string()).into_response()
        }
    }
}

pub async fn post_logout(_path: LogoutPath) -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    headers.insert(
        SET_COOKIE,
        "token=; Path=/; HttpOnly; Max-Age=0".parse().unwrap(),
    );
    (headers, Redirect::to(LoginPath::PATH))
}

// ── Component ──

fn login_page(error: Option<&str>) -> Markup {
    html! {
        div class="login-shell" {
            // Left: Brand Panel
            div class="brand-panel" {
                div class="relative z-1 max-w-[420px]" {
                    // Logo
                    div class="flex items-center gap-[14px] mb-12" {
                        div class="w-11 h-11 rounded-md bg-gradient-to-br from-accent to-accent-hover grid place-items-center shadow-[0_4px_16px_rgba(22,119,255,0.35)]" {
                            (box_icon("w-[22px] h-[22px] text-white"))
                        }
                        div class="text-[22px] font-extrabold text-white tracking-tight" { "ABT ERP" }
                    }

                    h1 class="brand-headline" {
                        "智能化"
                        br;
                        span { "企业管理平台" }
                    }
                    p class="brand-desc" { "统一管理销售、采购、库存全流程，实时掌控业务数据，让决策更高效。" }

                    // Features
                    div class="flex flex-col gap-5" {
                        (brand_feature(trending_up_icon("w-[18px] h-[18px] text-[var(--accent)]"), "全链路销售管理", "报价 → 订单 → 发货 → 对账，一站式闭环"))
                        (brand_feature(clipboard_list_icon("w-[18px] h-[18px] text-[var(--accent)]"), "采购协同", "供应商管理、采购订单、付款全流程数字化"))
                        (brand_feature(package_icon("w-[18px] h-[18px] text-[var(--accent)]"), "实时库存", "多仓库、多品类库存实时可视，自动预警"))
                    }
                }
            }

            // Right: Login Form
            div class="login-panel" {
                div class="w-full max-w-[380px]" {
                    div class="text-[13px] font-medium text-accent mb-2 tracking-wide" { "欢迎回来" }
                    h2 class="text-[28px] font-extrabold text-fg tracking-tight mb-1.5" { "登录您的账户" }
                    p class="text-sm text-muted mb-9" { "请输入账号和密码以继续使用系统" }

                    @if let Some(msg) = error {
                        div class="mb-5 p-3 rounded-md bg-danger-bg text-danger text-sm flex items-center gap-2" {
                            (circle_alert_icon("w-4 h-4 shrink-0"))
                            (msg)
                        }
                    }

                    form method="POST" action=(LoginPath::PATH) x-data=(r#"{"loading": false}"#) x-on:submit="loading = true" {
                        // Username
                        div class="mb-5" {
                            div class="flex items-center justify-between mb-[7px]" {
                                label for="username" class="text-[13px] font-semibold text-fg-2" { "账号" }
                            }
                            div class="relative" {
                                input type="text" name="username" id="username" required
                                       class="field-input"
                                       placeholder="请输入用户名或手机号" autocomplete="username";
                                (user_icon("field-icon"))
                            }
                        }

                        // Password
                        div class="mb-5" {
                            div class="flex items-center justify-between mb-[7px]" {
                                label for="password" class="text-[13px] font-semibold text-fg-2" { "密码" }
                                a href="javascript:void(0)" class="text-xs text-accent font-medium hover:text-accent-hover transition-colors duration-150" { "忘记密码？" }
                            }
                            div class="relative" x-data=(r#"{"show": false}"#) {
                                input type="password" name="password" id="password" required
                                       class="field-input" style="padding-right: 44px"
                                       placeholder="请输入密码" autocomplete="current-password"
                                       x-bind:type="show ? 'text' : 'password'";
                                (lock_icon("field-icon"))
                                button type="button" class="pw-toggle" x-on:click="show = !show" aria-label="显示密码" {
                                    (eye_icon("w-[18px] h-[18px]"))
                                }
                            }
                        }

                        // Remember me
                        div class="flex items-center justify-between mb-7" {
                            label class="flex items-center gap-2 cursor-pointer" {
                                input type="checkbox" checked class="custom-checkbox";
                                span class="text-[13px] text-fg-2 select-none" { "记住我" }
                            }
                        }

                        // Submit
                        button type="submit" class="btn-login" x-bind:disabled="loading" x-bind:class=(r#"{"loading": loading}"#) {
                            span x-show="!loading" { "登 录" }
                            span x-show="!loading" class="inline-block w-[18px] h-[18px]" {
                                (arrow_right_icon("w-[18px] h-[18px]"))
                            }
                            span x-show="loading" class="spinner" { }
                            span x-show="loading" { "登录中..." }
                        }
                    }

                    div class="login-divider" {
                        span class="text-xs text-muted whitespace-nowrap" { "其他登录方式" }
                    }

                    button class="btn-sso" {
                        (monitor_icon("w-5 h-5"))
                        "企业 SSO 单点登录"
                    }

                    div class="mt-10 text-center text-xs text-muted leading-relaxed" {
                        "登录即表示您同意 "
                        a href="#" class="text-accent font-medium hover-underline" { "服务条款" }
                        " 和 "
                        a href="#" class="text-accent font-medium hover-underline" { "隐私政策" }
                    }
                }

                div class="absolute bottom-6 left-1/2 -translate-x-1/2 text-[11px] text-muted opacity-60" { "ABT ERP v2.1.0" }
            }
        }
    }
}

fn brand_feature(icon_markup: Markup, title: &str, desc: &str) -> Markup {
    html! {
        div class="flex items-start gap-[14px]" {
            div class="w-9 h-9 rounded-md bg-[rgba(22,119,255,0.12)] grid place-items-center shrink-0" {
                (icon_markup)
            }
            div {
                div class="text-sm font-semibold text-[rgba(255,255,255,0.9)] mb-[3px]" { (title) }
                div class="text-[13px] text-[rgba(255,255,255,0.4)] leading-normal" { (desc) }
            }
        }
    }
}
