use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect};
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use tower_sessions::Session;

use crate::auth::session::CURRENT_USER_KEY;
use abt_core::shared::identity::AuthService;
use crate::components::icon::*;
use crate::layout::page::standalone_page;
use crate::routes::auth::{LoginPath, LogoutPath, RefreshTokenPath};
use crate::routes::dashboard::DashboardPath;
use crate::state::AppState;

// ── Handlers ──

pub async fn get_login(session: Session) -> impl IntoResponse {
    if let Ok(Some(_)) = session
        .get::<abt_core::shared::identity::model::Claims>(CURRENT_USER_KEY)
        .await
    {
        return Redirect::to(DashboardPath::PATH).into_response();
    }
    let page = standalone_page("登录", login_page(None, ""));
    axum::response::Html(page.into_string()).into_response()
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct LoginForm {
    pub username: String,
    pub password: String,
}

pub async fn post_login(
    _path: LoginPath,
    State(state): State<AppState>,
    session: Session,
    axum::Form(form): axum::Form<LoginForm>,
) -> impl IntoResponse {
    let claims = {
        use abt_core::shared::identity::AuthService;
        let auth = state.auth_service();
        match auth.login(&form.username, &form.password).await {
            Ok((_token, claims)) => claims,
            Err(_) => {
                let html = login_form_area(Some("用户名或密码错误"), &form.username);
                return axum::response::Html(html.into_string()).into_response();
            }
        }
    };

    if let Err(e) = session.insert(CURRENT_USER_KEY, &claims).await {
        tracing::error!("Failed to save session: {e}");
    }
    (StatusCode::OK, [("HX-Redirect", DashboardPath::PATH)]).into_response()
}

pub async fn post_logout(
    _path: LogoutPath,
    session: Session,
) -> impl IntoResponse {
    let _ = session.remove::<abt_core::shared::identity::model::Claims>(CURRENT_USER_KEY).await;
    (
        StatusCode::OK,
        [("HX-Redirect", LoginPath::PATH), ("HX-Refresh", "true")],
        "",
    )
}

// ── Refresh Token (API endpoint, no session required) ──

#[derive(Debug, serde::Deserialize)]
pub(crate) struct RefreshTokenForm {
    pub token: String,
}

pub async fn post_refresh_token(
    _path: RefreshTokenPath,
    State(state): State<AppState>,
    axum::Form(form): axum::Form<RefreshTokenForm>,
) -> impl IntoResponse {
    match state.auth_service().refresh_token(&form.token).await {
        Ok(new_token) => (
            StatusCode::OK,
            [("Content-Type", "application/json")],
            format!("{{\"token\":\"{new_token}\"}}"),
        )
            .into_response(),
        Err(_) => (StatusCode::UNAUTHORIZED, "Token refresh failed").into_response(),
    }
}

// ── Components ──

fn login_page(error: Option<&str>, username: &str) -> Markup {
    html! {
        div class="grid grid-cols-2 min-h-screen max-[920px]:grid-cols-1" {
            div class="flex flex-col justify-center items-center relative overflow-hidden" {
                div class="relative z-1 max-w-[420px]" {
                    div class="flex items-center gap-[14px] mb-12" {
                        div class="w-11 h-11 rounded-md bg-gradient-to-br from-accent to-accent-hover grid place-items-center shadow-[0_4px_16px_rgba(22,119,255,0.35)]" {
                            (box_icon("w-[22px] h-[22px] text-white"))
                        }
                        div class="text-[22px] font-extrabold text-white tracking-tight" { "ABT ERP" }
                    }

                    h1 class="text-3xl font-extrabold text-[#fff]" {
                        "智能化"
                        br;
                        span { "企业管理平台" }
                    }
                    p class="brand-desc" { "统一管理销售、采购、库存全流程，实时掌控业务数据，让决策更高效。" }

                    div class="flex flex-col gap-5" {
                        (brand_feature(trending_up_icon("w-[18px] h-[18px] text-[var(--accent)]"), "全链路销售管理", "报价 → 订单 → 发货 → 对账，一站式闭环"))
                        (brand_feature(clipboard_list_icon("w-[18px] h-[18px] text-[var(--accent)]"), "采购协同", "供应商管理、采购订单、付款全流程数字化"))
                        (brand_feature(package_icon("w-[18px] h-[18px] text-[var(--accent)]"), "实时库存", "多仓库、多品类库存实时可视，自动预警"))
                    }
                }
            }

            div class="flex flex-col justify-center items-center px-12 py-16 bg-white relative max-[920px]:p-12 max-[920px]:px-7 max-[480px]:p-9 max-[480px]:px-5" {
                div class="w-full max-w-[380px]" {
                    div class="text-[13px] font-medium text-accent mb-2 tracking-wide" { "欢迎回来" }
                    h2 class="text-[28px] font-extrabold text-fg tracking-tight mb-1.5" { "登录您的账户" }
                    p class="text-sm text-text-muted mb-9" { "请输入账号和密码以继续使用系统" }

                    (login_form_area(error, username))

                    div class="flex items-center gap-[16px]" {
                        span class="text-xs text-text-muted whitespace-nowrap" { "其他登录方式" }
                    }

                    button class="w-full bg-bg text-fg-2 border border-border rounded text-sm font-medium cursor-pointer flex items-center justify-center gap-[10px]" {
                        (monitor_icon("w-5 h-5"))
                        "企业 SSO 单点登录"
                    }

                    div class="mt-10 text-center text-xs text-text-muted leading-relaxed" {
                        "登录即表示您同意 "
                        a href="#" class="text-accent font-medium hover-underline" { "服务条款" }
                        " 和 "
                        a href="#" class="text-accent font-medium hover-underline" { "隐私政策" }
                    }
                }

                div class="absolute bottom-6 left-1/2 -translate-x-1/2 text-[11px] text-text-muted opacity-60" { "ABT ERP v2.1.0" }
            }
        }
    }
}

fn login_form_area(error: Option<&str>, username: &str) -> Markup {
    html! {
        div id="login-form-area" {
            @if let Some(msg) = error {
                div class="mb-5 p-3 rounded-md bg-danger-bg text-danger text-sm flex items-center gap-2" {
                    (circle_alert_icon("w-4 h-4 shrink-0"))
                    (msg)
                }
            }

            form hx-post=(LoginPath::PATH)
                 hx-target="#login-form-area"
                 hx-select="#login-form-area"
                 hx-swap="outerHTML"
                 onsubmit="this.classList.add('loading')" {

                div class="mb-5" {
                    div class="flex items-center justify-between mb-[7px]" {
                        label for="username" class="text-[13px] font-semibold text-fg-2" { "账号" }
                    }
                    div class="relative" {
                        input type="text" name="username" id="username" required
                               class="w-full py-[11px] px-[14px] pl-[42px] border border-border rounded-md bg-white text-sm text-fg outline-none transition-all duration-150 hover:border-[#c4cdd5] focus:border-accent focus:shadow-[0_0_0_3px_rgba(22,119,255,0.15)]"
                               placeholder="请输入用户名或手机号" autocomplete="username"
                               value=(username);
                        (user_icon("field-icon"))
                    }
                }

                div class="mb-5" {
                    div class="flex items-center justify-between mb-[7px]" {
                        label for="password" class="text-[13px] font-semibold text-fg-2" { "密码" }
                        a href="javascript:void(0)" class="text-xs text-accent font-medium hover:text-accent-hover transition-colors duration-150" { "忘记密码？" }
                    }
                    div class="relative" {
                        input type="password" name="password" id="password" required
                               class="w-full py-[11px] px-[14px] pl-[42px] border border-border rounded-md bg-white text-sm text-fg outline-none transition-all duration-150 hover:border-[#c4cdd5] focus:border-accent focus:shadow-[0_0_0_3px_rgba(22,119,255,0.15)]" style="padding-right: 44px"
                               placeholder="请输入密码" autocomplete="current-password";
                        (lock_icon("field-icon"))
                        button type="button" class="absolute w-[32px] h-[32px] border-none grid place-items-center cursor-pointer text-text-muted rounded-sm" aria-label="显示密码" _="on click toggle .pw-visible on closest <div/> then if (closest <div/>) matches .pw-visible set #password's type to 'text' else set #password's type to 'password'" {
                            (eye_icon("w-[18px] h-[18px]"))
                        }
                    }
                }

                div class="flex items-center justify-between mb-7" {
                    label class="flex items-center gap-2 cursor-pointer" {
                        input type="checkbox" checked class="w-[16px] h-[16px] bg-bg cursor-pointer relative";
                        span class="text-[13px] text-fg-2 select-none" { "记住我" }
                    }
                }

                button type="submit" class="inline-flex items-center gap-2 rounded-sm text-sm font-medium cursor-pointer whitespace-nowrap relative-login" {
                    span { "登 录" }
                    span class="inline-block w-[18px] h-[18px]" {
                        (arrow_right_icon("w-[18px] h-[18px]"))
                    }
                }
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
