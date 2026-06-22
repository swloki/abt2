use maud::{html, Markup};

/// UnoCSS presetIcons 图标 wrapper。`ic` 为完整的图标原子类（如 lucide 图标 class），
/// `c` 透传尺寸/颜色等附加原子类。颜色由 mask 模式的 `background-color: currentColor`
/// 跟随 `text-*` 继承，与旧手写 `stroke="currentColor"` 语义一致。
///
/// 注意：UnoCSS 按需生成——图标 class 必须以**完整字面**出现在源码里才会被内容
/// 扫描器提取并生成 CSS。因此下面的每个 pub fn 都直接写出完整图标 class 字面，
/// 调用方零改动即可。
fn icon(ic: &str, c: &str) -> Markup {
    html! {
        i class=(format!("{ic} {c}")) {}
    }
}

/// 渲染任意图标（传完整 class 字面）。供页面渲染下方未预定义的图标用；
/// 调用方需写完整图标 class 以确保 UnoCSS 能提取生成。
pub fn raw(ic: &str, c: &str) -> Markup {
    icon(ic, c)
}

// ── Brand / Navigation ──

pub fn box_icon(c: &str) -> Markup {
    icon("i-lucide-box", c)
}

pub fn home_icon(c: &str) -> Markup {
    icon("i-lucide-house", c)
}

pub fn users_icon(c: &str) -> Markup {
    icon("i-lucide-users", c)
}

pub fn building_icon(c: &str) -> Markup {
    icon("i-lucide-building-2", c)
}

pub fn grid_icon(c: &str) -> Markup {
    icon("i-lucide-layout-grid", c)
}

pub fn return_arrow_icon(c: &str) -> Markup {
    icon("i-lucide-undo-2", c)
}

pub fn clipboard_document_icon(c: &str) -> Markup {
    icon("i-lucide-clipboard-check", c)
}

pub fn payment_icon(c: &str) -> Markup {
    icon("i-lucide-credit-card", c)
}

pub fn sliders_icon(c: &str) -> Markup {
    icon("i-lucide-sliders-horizontal", c)
}

pub fn clipboard_module_icon(c: &str) -> Markup {
    icon("i-lucide-clipboard-list", c)
}

pub fn question_icon(c: &str) -> Markup {
    icon("i-lucide-circle-help", c)
}

pub fn sidebar_toggle_icon(c: &str) -> Markup {
    icon("i-lucide-panel-left", c)
}

pub fn trending_up_icon(c: &str) -> Markup {
    icon("i-lucide-trending-up", c)
}

pub fn clipboard_list_icon(c: &str) -> Markup {
    icon("i-lucide-clipboard-list", c)
}

pub fn package_icon(c: &str) -> Markup {
    icon("i-lucide-package", c)
}

// ── Auth / User ──

pub fn user_icon(c: &str) -> Markup {
    icon("i-lucide-user", c)
}

pub fn lock_icon(c: &str) -> Markup {
    icon("i-lucide-lock", c)
}

pub fn eye_icon(c: &str) -> Markup {
    icon("i-lucide-eye", c)
}

#[allow(dead_code)]
pub fn eye_off_icon(c: &str) -> Markup {
    icon("i-lucide-eye-off", c)
}

// ── Actions ──

pub fn arrow_right_icon(c: &str) -> Markup {
    icon("i-lucide-arrow-right", c)
}

pub fn arrow_left_icon(c: &str) -> Markup {
    icon("i-lucide-arrow-left", c)
}

pub fn plus_icon(c: &str) -> Markup {
    icon("i-lucide-plus", c)
}

pub fn search_icon(c: &str) -> Markup {
    icon("i-lucide-search", c)
}

#[allow(dead_code)]
pub fn more_horizontal_icon(c: &str) -> Markup {
    icon("i-lucide-ellipsis", c)
}

pub fn dots_vertical_icon(c: &str) -> Markup {
    icon("i-lucide-ellipsis-vertical", c)
}

pub fn trash_icon(c: &str) -> Markup {
    icon("i-lucide-trash-2", c)
}

pub fn edit_icon(c: &str) -> Markup {
    icon("i-lucide-square-pen", c)
}

#[allow(dead_code)]
pub fn copy_icon(c: &str) -> Markup {
    icon("i-lucide-copy", c)
}

// ── Feedback ──

pub fn circle_alert_icon(c: &str) -> Markup {
    icon("i-lucide-circle-alert", c)
}

pub fn check_circle_icon(c: &str) -> Markup {
    icon("i-lucide-circle-check", c)
}

pub fn bell_icon(c: &str) -> Markup {
    icon("i-lucide-bell", c)
}

// ── Layout / UI ──

pub fn monitor_icon(c: &str) -> Markup {
    icon("i-lucide-monitor", c)
}

#[allow(dead_code)]
pub fn chevron_down_icon(c: &str) -> Markup {
    icon("i-lucide-chevron-down", c)
}

#[allow(dead_code)]
pub fn chevron_right_icon(c: &str) -> Markup {
    icon("i-lucide-chevron-right", c)
}

pub fn chevron_left_icon(c: &str) -> Markup {
    icon("i-lucide-chevron-left", c)
}

pub fn x_icon(c: &str) -> Markup {
    icon("i-lucide-x", c)
}

pub fn menu_icon(c: &str) -> Markup {
    icon("i-lucide-menu", c)
}

#[allow(dead_code)]
pub fn log_out_icon(c: &str) -> Markup {
    icon("i-lucide-log-out", c)
}

// ── Sales Module ──

pub fn file_text_icon(c: &str) -> Markup {
    icon("i-lucide-file-text", c)
}

pub fn printer_icon(c: &str) -> Markup {
    icon("i-lucide-printer", c)
}

pub fn truck_icon(c: &str) -> Markup {
    icon("i-lucide-truck", c)
}

pub fn chart_bar_icon(c: &str) -> Markup {
    icon("i-lucide-chart-column", c)
}

#[allow(dead_code)]
pub fn refresh_icon(c: &str) -> Markup {
    icon("i-lucide-refresh-cw", c)
}

pub fn phone_icon(c: &str) -> Markup {
    icon("i-lucide-phone", c)
}

pub fn mail_icon(c: &str) -> Markup {
    icon("i-lucide-mail", c)
}

pub fn download_icon(c: &str) -> Markup {
    icon("i-lucide-download", c)
}

#[allow(dead_code)]
pub fn upload_icon(c: &str) -> Markup {
    icon("i-lucide-upload", c)
}

pub fn save_icon(c: &str) -> Markup {
    icon("i-lucide-save", c)
}

pub fn send_icon(c: &str) -> Markup {
    icon("i-lucide-send", c)
}

#[allow(dead_code)]
pub fn filter_icon(c: &str) -> Markup {
    icon("i-lucide-filter", c)
}

pub fn link_icon(c: &str) -> Markup {
    icon("i-lucide-link", c)
}

pub fn currency_icon(c: &str) -> Markup {
    icon("i-lucide-circle-dollar-sign", c)
}

pub fn comment_icon(c: &str) -> Markup {
    icon("i-lucide-message-square", c)
}

pub fn clock_icon(c: &str) -> Markup {
    icon("i-lucide-clock", c)
}

pub fn activity_icon(c: &str) -> Markup {
    icon("i-lucide-activity", c)
}

pub fn bolt_icon(c: &str) -> Markup {
    icon("i-lucide-zap", c)
}

pub fn rocket_icon(c: &str) -> Markup {
    icon("i-lucide-rocket", c)
}

/// 4-square grid icon (物料汇总 view toggle)
pub fn grid_4_icon(c: &str) -> Markup {
    icon("i-lucide-grid-2x2", c)
}

/// Horizontal lines icon (订单行明细 view toggle)
pub fn rows_icon(c: &str) -> Markup {
    icon("i-lucide-rows-3", c)
}

/// 3D cube icon
pub fn cube_icon(c: &str) -> Markup {
    icon("i-lucide-box", c)
}

pub fn tool_icon(c: &str) -> Markup {
    icon("i-lucide-wrench", c)
}

pub fn briefcase_icon(c: &str) -> Markup {
    icon("i-lucide-briefcase", c)
}

pub fn calendar_icon(c: &str) -> Markup {
    icon("i-lucide-calendar", c)
}

pub fn dollar_icon(c: &str) -> Markup {
    icon("i-lucide-dollar-sign", c)
}

pub fn alert_triangle_icon(c: &str) -> Markup {
    icon("i-lucide-triangle-alert", c)
}

pub fn info_icon(c: &str) -> Markup {
    icon("i-lucide-info", c)
}
