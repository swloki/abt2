use maud::{html, Markup};

/// Common SVG wrapper. `paths` is the inner SVG elements.
pub(crate) fn svg(paths: &str, class: &str) -> Markup {
    html! {
        svg viewBox="0 0 24 24" fill="none" stroke="currentColor"
            stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"
            class=(class) {
            (maud::PreEscaped(paths))
        }
    }
}

// ── Brand / Navigation ──

pub fn box_icon(c: &str) -> Markup {
    svg(r#"<path d="M20 7l-8-4-8 4m16 0l-8 4m8-4v10l-8 4m0-10L4 7m8 4v10M4 7v10l8 4"/>"#, c)
}

pub fn home_icon(c: &str) -> Markup {
    svg(r#"<path d="M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-4 0a1 1 0 01-1-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 01-1 1h-2z"/>"#, c)
}

pub fn users_icon(c: &str) -> Markup {
    svg(r#"<path d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0zm6 3a2 2 0 11-4 0 2 2 0 014 0zM7 10a2 2 0 11-4 0 2 2 0 014 0z"/>"#, c)
}

pub fn building_icon(c: &str) -> Markup {
    svg(r#"<path d="M19 21V5a2 2 0 00-2-2H7a2 2 0 00-2 2v16m14 0h2m-2 0h-5m-9 0H3m2 0h5M9 7h1m-1 4h1m4-4h1m-1 4h1m-5 10v-5a1 1 0 011-1h2a1 1 0 011 1v5m-4 0h4"/>"#, c)
}

pub fn grid_icon(c: &str) -> Markup {
    svg(r#"<path d="M4 5a1 1 0 011-1h14a1 1 0 011 1v2a1 1 0 01-1 1H5a1 1 0 01-1-1V5zm0 8h16M8 5v16m8-16v16m-8-8h8"/>"#, c)
}

pub fn return_arrow_icon(c: &str) -> Markup {
    svg(r#"<path d="M3 10h10a5 5 0 015 5v2M3 10l4-4M3 10l4 4"/>"#, c)
}

pub fn clipboard_document_icon(c: &str) -> Markup {
    svg(r#"<path d="M16 4h2a2 2 0 012 2v14a2 2 0 01-2 2H6a2 2 0 01-2-2V6a2 2 0 012-2h2m0-2h6a1 1 0 011 1v2a1 1 0 01-1 1H8a1 1 0 01-1-1V3a1 1 0 011-1zm4 10H9m6 4H9"/>"#, c)
}

pub fn payment_icon(c: &str) -> Markup {
    svg(r#"<path d="M17 9V7a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2m2 4h10a2 2 0 002-2v-6a2 2 0 00-2-2H9a2 2 0 00-2 2v6a2 2 0 002 2zm7-5a2 2 0 11-4 0 2 2 0 014 0z"/>"#, c)
}

pub fn sliders_icon(c: &str) -> Markup {
    svg(r#"<path d="M12 6V4m0 2a2 2 0 100 4m0-4a2 2 0 110 4m-6 8a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4m6 6v10m6-2a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4"/>"#, c)
}

pub fn clipboard_module_icon(c: &str) -> Markup {
    svg(r#"<path d="M16 4h2a2 2 0 012 2v14a2 2 0 01-2 2H6a2 2 0 01-2-2V6a2 2 0 012-2h2"/><rect x="8" y="2" width="8" height="4" rx="1" ry="1"/>"#, c)
}

pub fn question_icon(c: &str) -> Markup {
    svg(r#"<path d="M8.228 9c.549-1.165 2.03-2 3.772-2 2.21 0 4 1.343 4 3 0 1.4-1.278 2.575-3.006 2.907-.542.104-.994.54-.994 1.093m0 3h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"/>"#, c)
}

pub fn sidebar_toggle_icon(c: &str) -> Markup {
    svg(r#"<rect x="3" y="3" width="18" height="18" rx="2"/><line x1="9" y1="3" x2="9" y2="21"/>"#, c)
}

pub fn trending_up_icon(c: &str) -> Markup {
    svg(r#"<path d="M23 6l-9.5 9.5-5-5L1 18"/><path d="M17 6h6v6"/>"#, c)
}

pub fn clipboard_list_icon(c: &str) -> Markup {
    svg(r#"<path d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2m-6 9l2 2 4-4"/>"#, c)
}

pub fn package_icon(c: &str) -> Markup {
    svg(r#"<path d="M5 8h14M5 8a2 2 0 110-4h14a2 2 0 110 4M5 8v10a2 2 0 002 2h10a2 2 0 002-2V8m-9 4h4"/>"#, c)
}

// ── Auth / User ──

pub fn user_icon(c: &str) -> Markup {
    svg(r#"<path d="M20 21v-2a4 4 0 00-4-4H8a4 4 0 00-4 4v2"/><circle cx="12" cy="7" r="4"/>"#, c)
}

pub fn lock_icon(c: &str) -> Markup {
    svg(r#"<rect x="3" y="11" width="18" height="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0110 0v4"/>"#, c)
}

pub fn eye_icon(c: &str) -> Markup {
    svg(r#"<path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/><circle cx="12" cy="12" r="3"/>"#, c)
}

#[allow(dead_code)]
pub fn eye_off_icon(c: &str) -> Markup {
    svg(r#"<path d="M17.94 17.94A10.07 10.07 0 0112 20c-7 0-11-8-11-8a18.45 18.45 0 015.06-5.94M9.9 4.24A9.12 9.12 0 0112 4c7 0 11 8 11 8a18.5 18.5 0 01-2.16 3.19m-6.72-1.07a3 3 0 11-4.24-4.24"/><line x1="1" y1="1" x2="23" y2="23"/>"#, c)
}

// ── Actions ──

pub fn arrow_right_icon(c: &str) -> Markup {
    svg(r#"<path d="M5 12h14M12 5l7 7-7 7"/>"#, c)
}

pub fn arrow_left_icon(c: &str) -> Markup {
    svg(r#"<path d="M19 12H5M12 19l-7-7 7-7"/>"#, c)
}

pub fn plus_icon(c: &str) -> Markup {
    svg(r#"<line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/>"#, c)
}

pub fn search_icon(c: &str) -> Markup {
    svg(r#"<circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/>"#, c)
}

#[allow(dead_code)]
pub fn more_horizontal_icon(c: &str) -> Markup {
    svg(r#"<circle cx="12" cy="12" r="1"/><circle cx="19" cy="12" r="1"/><circle cx="5" cy="12" r="1"/>"#, c)
}
pub fn dots_vertical_icon(c: &str) -> Markup {
    svg(r#"<circle cx="12" cy="5" r="1"/><circle cx="12" cy="12" r="1"/><circle cx="12" cy="19" r="1"/>"#, c)
}

pub fn trash_icon(c: &str) -> Markup {
    svg(r#"<polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2"/>"#, c)
}

pub fn edit_icon(c: &str) -> Markup {
    svg(r#"<path d="M11 4H4a2 2 0 00-2 2v14a2 2 0 002 2h14a2 2 0 002-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 013 3L12 15l-4 1 1-4 9.5-9.5z"/>"#, c)
}

#[allow(dead_code)]
pub fn copy_icon(c: &str) -> Markup {
    svg(r#"<rect x="9" y="9" width="13" height="13" rx="2" ry="2"/><path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1"/>"#, c)
}

// ── Feedback ──

pub fn circle_alert_icon(c: &str) -> Markup {
    svg(r#"<circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/>"#, c)
}

pub fn check_circle_icon(c: &str) -> Markup {
    svg(r#"<path d="M22 11.08V12a10 10 0 11-5.93-9.14"/><polyline points="22 4 12 14.01 9 11.01"/>"#, c)
}

pub fn bell_icon(c: &str) -> Markup {
    svg(r#"<path d="M18 8A6 6 0 006 8c0 7-3 9-3 9h18s-3-2-3-9"/><path d="M13.73 21a2 2 0 01-3.46 0"/>"#, c)
}

// ── Layout / UI ──

pub fn monitor_icon(c: &str) -> Markup {
    svg(r#"<rect x="2" y="3" width="20" height="14" rx="2" ry="2"/><line x1="8" y1="21" x2="16" y2="21"/><line x1="12" y1="17" x2="12" y2="21"/>"#, c)
}

#[allow(dead_code)]
pub fn chevron_down_icon(c: &str) -> Markup {
    svg(r#"<polyline points="6 9 12 15 18 9"/>"#, c)
}

#[allow(dead_code)]
pub fn chevron_right_icon(c: &str) -> Markup {
    svg(r#"<polyline points="9 18 15 12 9 6"/>"#, c)
}

pub fn chevron_left_icon(c: &str) -> Markup {
    svg(r#"<polyline points="15 18 9 12 15 6"/>"#, c)
}

pub fn x_icon(c: &str) -> Markup {
    svg(r#"<line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>"#, c)
}

pub fn menu_icon(c: &str) -> Markup {
    svg(r#"<line x1="3" y1="12" x2="21" y2="12"/><line x1="3" y1="6" x2="21" y2="6"/><line x1="3" y1="18" x2="21" y2="18"/>"#, c)
}

#[allow(dead_code)]
pub fn log_out_icon(c: &str) -> Markup {
    svg(r#"<path d="M9 21H5a2 2 0 01-2-2V5a2 2 0 012-2h4"/><polyline points="16 17 21 12 16 7"/><line x1="21" y1="12" x2="9" y2="12"/>"#, c)
}

// ── Sales Module ──

pub fn file_text_icon(c: &str) -> Markup {
    svg(r#"<path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z"/><polyline points="14 2 14 8 20 8"/><line x1="16" y1="13" x2="8" y2="13"/><line x1="16" y1="17" x2="8" y2="17"/><polyline points="10 9 9 9 8 9"/>"#, c)
}

pub fn printer_icon(c: &str) -> Markup {
    svg(r#"<polyline points="6 9 6 2 18 2 18 9"/><path d="M6 18H4a2 2 0 01-2-2v-5a2 2 0 012-2h16a2 2 0 012 2v5a2 2 0 01-2 2h-2"/><rect x="6" y="14" width="12" height="8"/>"#, c)
}

pub fn truck_icon(c: &str) -> Markup {
    svg(r#"<rect x="1" y="3" width="15" height="13"/><polygon points="16 8 20 8 23 11 23 16 16 16 16 8"/><circle cx="5.5" cy="18.5" r="2.5"/><circle cx="18.5" cy="18.5" r="2.5"/>"#, c)
}

#[allow(dead_code)]
pub fn refresh_icon(c: &str) -> Markup {
    svg(r#"<polyline points="23 4 23 10 17 10"/><path d="M20.49 15a9 9 0 11-2.12-9.36L23 10"/>"#, c)
}

pub fn phone_icon(c: &str) -> Markup {
    svg(r#"<path d="M22 16.92v3a2 2 0 01-2.18 2 19.79 19.79 0 01-8.63-3.07 19.5 19.5 0 01-6-6 19.79 19.79 0 01-3.07-8.67A2 2 0 014.11 2h3a2 2 0 012 1.72 12.84 12.84 0 00.7 2.81 2 2 0 01-.45 2.11L8.09 9.91a16 16 0 006 6l1.27-1.27a2 2 0 012.11-.45 12.84 12.84 0 002.81.7A2 2 0 0122 16.92z"/>"#, c)
}

pub fn mail_icon(c: &str) -> Markup {
    svg(r#"<path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z"/><polyline points="22,6 12,13 2,6"/>"#, c)
}

pub fn download_icon(c: &str) -> Markup {
    svg(r#"<path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/>"#, c)
}

#[allow(dead_code)]
pub fn upload_icon(c: &str) -> Markup {
    svg(r#"<path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/><polyline points="17 8 12 3 7 8"/><line x1="12" y1="3" x2="12" y2="15"/>"#, c)
}

#[allow(dead_code)]
pub fn filter_icon(c: &str) -> Markup {
    svg(r#"<polygon points="22 3 2 3 10 12.46 10 19 14 21 14 12.46 22 3"/>"#, c)
}

pub fn link_icon(c: &str) -> Markup {
    svg(r#"<path d="M10 13a5 5 0 007.54.54l3-3a5 5 0 00-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 00-7.54-.54l-3 3a5 5 0 007.07 7.07l1.71-1.71"/>"#, c)
}

pub fn currency_icon(c: &str) -> Markup {
    svg(r#"<path d="M12 8c-1.657 0-3 .895-3 2s1.343 2 3 2 3 .895 3 2-1.343 2-3 2m0-8c1.11 0 2.08.402 2.599 1M12 8V7m0 1v8m0 0v1m0-1c-1.11 0-2.08-.402-2.599-1M21 12a9 9 0 11-18 0 9 9 0 0118 0z"/>"#, c)
}

pub fn comment_icon(c: &str) -> Markup {
    svg(r#"<path d="M7 8h10M7 12h4m1 8l-4-4H5a2 2 0 01-2-2V6a2 2 0 012-2h14a2 2 0 012 2v8a2 2 0 01-2 2h-3l-4 4z"/>"#, c)
}

pub fn clock_icon(c: &str) -> Markup {
    svg(r#"<path d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z"/>"#, c)
}

pub fn bolt_icon(c: &str) -> Markup {
    svg(r#"<path d="M13 10V3L4 14h7v7l9-11h-7z"/>"#, c)
}

pub fn rocket_icon(c: &str) -> Markup {
    svg(r#"<path d="M4.5 16.5c-1.5 1.26-2 5-2 5s3.74-.5 5-2c.71-.84.7-2.13-.09-2.91a2.18 2.18 0 00-2.91-.09z"/><path d="M12 15l-3-3a22 22 0 012-3.95A12.88 12.88 0 0122 2c0 2.72-.78 7.5-6 11a22.35 22.35 0 01-4 2z"/><path d="M9 12H4s.55-3.03 2-4c1.62-1.08 5 0 5 0"/><path d="M12 15v5s3.03-.55 4-2c1.08-1.62 0-5 0-5"/>"#, c)
}
