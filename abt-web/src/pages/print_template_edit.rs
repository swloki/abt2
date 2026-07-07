use axum::extract::Query;
use axum::response::{Html, IntoResponse};
use axum_extra::routing::TypedPath;
use maud::{html, Markup, PreEscaped, Render};
use serde::Deserialize;

use abt_core::master_data::print_template::{
    CreatePrintTemplateReq, PrintTemplateService, RenderVars, UpdatePrintTemplateReq,
};

use crate::components::icon;
use crate::layout::page::admin_page;
use crate::routes::print_template::*;
use crate::utils::RequestContext;

use abt_macros::require_permission;

// ── Document type options ──

const DOCUMENT_TYPES: &[(&str, &str)] = &[
    ("delivery_note", "送货单"),
    ("quotation", "报价单"),
    ("sales_order", "销售订单"),
    ("purchase_order", "采购订单"),
];

// ── Form data ──

#[derive(Debug, Deserialize)]
pub struct PrintTemplateForm {
    pub name: String,
    pub document_type: String,
    #[serde(default)]
    pub description: Option<String>,
    pub html_content: String,
    #[serde(default)]
    pub is_default: bool,
}

// ── Handlers ──

#[require_permission("USER", "read")]
pub async fn create_page(
    _path: PrintTemplateCreatePath,
    ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
    let nav_filter = ctx.nav_filter().await;
    let claims = ctx.claims.clone();

    let content = edit_form(None, None, None, None, true).render();
    let page_html = admin_page(
        false,
        "新建打印模板",
        &claims,
        "system",
        PrintTemplateListPath::PATH,
        "系统管理",
        Some("打印模板"),
        content,
        &nav_filter,
    );
    Ok(Html(page_html.into_string()))
}

#[require_permission("USER", "create")]
pub async fn create(
    _path: PrintTemplateCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<PrintTemplateForm>,
) -> crate::errors::Result<impl IntoResponse> {
    let RequestContext {
        mut conn, state, service_ctx, ..
    } = ctx;

    let req = CreatePrintTemplateReq {
        name: form.name,
        document_type: form.document_type,
        description: form.description,
        html_content: form.html_content,
        is_default: form.is_default,
    };

    state
        .print_template_service()
        .create(&service_ctx, &mut conn, req)
        .await?;

    Ok([("HX-Redirect", PrintTemplateListPath::PATH.to_string())])
}

#[require_permission("USER", "read")]
pub async fn edit_page(
    path: PrintTemplateEditPath,
    ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
    let nav_filter = ctx.nav_filter().await;
    let claims = ctx.claims.clone();
    let mut conn = ctx.conn;

    let svc = ctx.state.print_template_service();
    let template = svc.get(&mut conn, path.id).await?;

    let content = edit_form(
        Some(template.id),
        Some(&template.name),
        Some(&template.document_type),
        template.description.as_deref(),
        template.is_default,
    )
    .with_html_content(&template.html_content)
    .render();

    let page_html = admin_page(
        false,
        "编辑打印模板",
        &claims,
        "system",
        PrintTemplateListPath::PATH,
        "系统管理",
        Some("打印模板"),
        content,
        &nav_filter,
    );
    Ok(Html(page_html.into_string()))
}

#[require_permission("USER", "create")]
pub async fn update(
    path: PrintTemplateEditPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<PrintTemplateForm>,
) -> crate::errors::Result<impl IntoResponse> {
    let RequestContext {
        mut conn, state, service_ctx, ..
    } = ctx;

    let req = UpdatePrintTemplateReq {
        name: Some(form.name),
        document_type: Some(form.document_type),
        description: form.description,
        html_content: Some(form.html_content),
        is_default: Some(form.is_default),
    };

    state
        .print_template_service()
        .update(&service_ctx, &mut conn, path.id, req)
        .await?;

    // 不跳列表页：add_toast 入全局 toast 队列 + 空 body。
    // 编辑页 pte 监听 form 的 afterRequest（200+空响应=保存成功），拉 /api/toast 显示到 #pte-toast-area。
    crate::toast::add_toast(
        service_ctx.operator_id,
        "已保存",
        crate::toast::ToastType::Success,
    );
    Ok(Html(String::new()))
}

#[require_permission("USER", "read")]
pub async fn preview(
    path: PrintPreviewPath,
    Query(params): Query<std::collections::HashMap<String, String>>,
    ctx: RequestContext,
) -> crate::errors::Result<Html<String>> {
    let mut conn = ctx.conn;
    let svc = ctx.state.print_template_service();
    // query 参数作为模板变量（minijinja 渲染，中文 key 直接可用）
    let vars: RenderVars = serde_json::to_value(&params).unwrap_or(serde_json::Value::Null);
    let html = svc.render(&mut conn, path.template_id, vars).await?;
    Ok(Html(html))
}

#[derive(Debug, Deserialize)]
pub struct RenderPreviewForm {
    pub html_content: String,
    pub document_type: String,
}

/// 编辑器实时预览：不入库，用 minijinja + mock 数据渲染用户正在编辑的内容。
/// 渲染失败返回带错误提示的 HTML（让用户在预览里看到 Jinja 语法错，而非整页 500）。
#[require_permission("USER", "read")]
pub async fn render_preview(
    _path: RenderPreviewPath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<RenderPreviewForm>,
) -> crate::errors::Result<Html<String>> {
    let _ = ctx; // mock 渲染不访问 DB；权限由 require_permission 校验
    let mock_ctx = abt_core::master_data::print_template::mock_context(&form.document_type);
    match abt_core::master_data::print_template::implt::PrintTemplateServiceImpl::render_html(
        &form.html_content,
        &mock_ctx,
    ) {
        Ok(html) => Ok(Html(html)),
        Err(e) => Ok(Html(format!(
            r#"<div style="padding:24px;font-family:sans-serif;color:#b91c1c;">
                <h3>模板渲染错误</h3>
                <pre style="white-space:pre-wrap;background:#f5f5f5;padding:12px;border-radius:4px;font-size:13px;">{e}</pre>
            </div>"#
        ))),
    }
}

// ── UI ──

struct EditFormState {
    id: Option<i64>,
    name: Option<String>,
    document_type: Option<String>,
    description: Option<String>,
    is_default: bool,
    html_content: Option<String>,
}

fn edit_form(
    id: Option<i64>,
    name: Option<&str>,
    document_type: Option<&str>,
    description: Option<&str>,
    is_default: bool,
) -> EditFormState {
    EditFormState {
        id,
        name: name.map(|s| s.to_string()),
        document_type: document_type.map(|s| s.to_string()),
        description: description.map(|s| s.to_string()),
        is_default,
        html_content: None,
    }
}

impl EditFormState {
    fn with_html_content(mut self, html: &str) -> Self {
        self.html_content = Some(html.to_string());
        self
    }
}

impl maud::Render for EditFormState {
    fn render(&self) -> Markup {
        let is_create = self.id.is_none();
        let form_action = if let Some(id) = self.id {
            PrintTemplateEditPath { id }.to_string()
        } else {
            PrintTemplateCreatePath::PATH.to_string()
        };
        let back_url = PrintTemplateListPath::PATH.to_string();
        let html_content = self.html_content.clone().unwrap_or_default();

        html! {
            div class="flex flex-col gap-5" {
                // ── Breadcrumb ──
                div class="flex items-center gap-2 text-sm text-muted" {
                    a href=(back_url) class="text-accent hover:text-accent-hover no-underline" { "打印模板" }
                    span { "/" }
                    span class="text-fg" {
                        @if is_create { "新建" } @else { "编辑" }
                    }
                }

                // ── Form ──
                form
                    class="flex flex-col gap-5"
                    hx-post=(form_action)
                    hx-target="this"
                    hx-swap="none"
                {
                    // ── Basic info ──
                    div class="data-card" {
                        h3 class="text-sm font-semibold text-fg mb-4" { "基本信息" }
                        div class="grid grid-cols-3 gap-4" {
                            // Name
                            div class="form-field" {
                                label class="block text-xs font-medium text-fg-2 mb-1" { "模板名称" }
                                input
                                    type="text"
                                    name="name"
                                    class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg focus:border-accent focus:shadow-[var(--shadow-focus)]"
                                    required
                                    value=(self.name.as_deref().unwrap_or(""))
                                    placeholder="例如：标准送货单"
                                ;
                            }
                            // Document type
                            div class="form-field" {
                                label class="block text-xs font-medium text-fg-2 mb-1" { "单据类型" }
                                select
                                    name="document_type"
                                    class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg focus:border-accent focus:shadow-[var(--shadow-focus)]"
                                {
                                    @for (value, label) in DOCUMENT_TYPES {
                                        option
                                            value=(*value)
                                            selected[self.document_type.as_deref() == Some(*value)]
                                        { (label) }
                                    }
                                }
                            }
                            // Default checkbox
                            div class="form-field flex items-end" {
                                label class="flex items-center gap-2 cursor-pointer" {
                                    input
                                        type="checkbox"
                                        name="is_default"
                                        value="true"
                                        checked[self.is_default]
                                        class="w-4 h-4 rounded border-border text-accent focus:ring-accent"
                                    ;
                                    span class="text-sm text-fg-2" { "设为默认模板" }
                                }
                            }
                        }
                        // Description
                        div class="form-field mt-3" {
                            label class="block text-xs font-medium text-fg-2 mb-1" { "备注" }
                            input
                                type="text"
                                name="description"
                                class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg focus:border-accent focus:shadow-[var(--shadow-focus)]"
                                value=(self.description.as_deref().unwrap_or(""))
                                placeholder="模板用途说明"
                            ;
                        }
                    }

                    // ── CodeMirror 5 资源（可视化编辑器）──
                    link rel="stylesheet" href="/codemirror.css" {};
                    link rel="stylesheet" href="/theme/material-darker.css" {};

                    // ── Editor：变量面板（常驻）+ 源码/预览 tab ──
                    div class="data-card flex flex-col" style="min-height: 620px;"
                        _=(PreEscaped("on load set s to document.createElement('script') then set s's src to '/print-template-edit.js?v=20260723' then call document.head.appendChild(s)"))
                    {
                        div class="flex items-center justify-between mb-3" {
                            div class="flex items-center border border-border rounded-sm overflow-hidden" {
                                button
                                    type="button"
                                    id="tab-source"
                                    class="px-3 py-1 text-xs font-medium cursor-pointer border-none bg-accent text-white transition-colors"
                                    _=(PreEscaped("on click remove .bg-accent .text-white from #tab-preview then add .bg-white .text-fg-2 to #tab-preview then add .bg-accent .text-white to me then remove .bg-white .text-fg-2 from me then remove .hidden from #editor-pane then add .hidden to #html-preview"))
                                { "源码编辑" }
                                button
                                    type="button"
                                    id="tab-preview"
                                    class="px-3 py-1 text-xs font-medium cursor-pointer border-none bg-white text-fg-2 transition-colors"
                                    _=(PreEscaped("on click remove .bg-accent .text-white from #tab-source then add .bg-white .text-fg-2 to #tab-source then add .bg-accent .text-white to me then remove .bg-white .text-fg-2 from me then call updatePreview() then remove .hidden from #html-preview then add .hidden to #editor-pane"))
                                { "实时预览" }
                            }
                            div class="flex items-center gap-3" {
                                button
                                    type="button"
                                    class="inline-flex items-center gap-1 text-xs text-accent hover:text-accent-hover cursor-pointer border-none bg-transparent"
                                    _=(PreEscaped("on click call printTemplate()"))
                                { (icon::printer_icon("w-3.5 h-3.5")) "打印预览" }
                                button
                                    type="submit"
                                    class="inline-flex items-center gap-1.5 px-4 py-1.5 rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-xs font-medium cursor-pointer transition-colors shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                                { (icon::check_circle_icon("w-3.5 h-3.5")) "保存" }
                            }
                        }
                        // 左右两栏：变量面板 + 编辑器/预览（tab 互斥）
                        div class="flex gap-4 flex-1 min-h-0" {
                            div class="w-56 shrink-0 overflow-y-auto border-r border-border-soft pr-2" {
                                (variable_panel(self.document_type.as_deref()))
                            }
                            div id="editor-pane" class="flex-1 flex flex-col min-h-0" {
                                textarea
                                    name="html_content"
                                    id="html-editor"
                                    class="flex-1 w-full px-3 py-3 border border-border rounded-sm text-sm bg-[#1e1e2e] text-[#cdd6f4] font-mono resize-none focus:border-accent focus:shadow-[var(--shadow-focus)]"
                                    style="min-height: 420px; tab-size: 2;"
                                    spellcheck="false"
                                { (html_content) }
                            }
                            iframe
                                id="html-preview"
                                class="flex-1 w-full border border-border rounded-sm bg-white hidden"
                                style="min-height: 420px;"
                            ;
                        }
                    }

                    // ── Actions ──
                    div class="flex items-center gap-3 pt-2" {
                        button
                            type="submit"
                            class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-accent text-accent-on border-none hover:bg-accent-hover text-sm font-medium cursor-pointer transition-all duration-150 shadow-[0_1px_2px_rgba(37,99,235,0.2)]"
                        { (icon::check_circle_icon("w-4 h-4")) "保存" }
                        a
                            href=(back_url)
                            class="inline-flex items-center gap-2 py-[9px] px-[18px] rounded-sm bg-white text-fg-2 border border-border hover:bg-surface hover:border-[rgba(37,99,235,0.3)] hover:text-accent text-sm font-medium cursor-pointer transition-all duration-150 shadow-xs no-underline"
                        { "取消" }
                    }
                    // toast 容器由 pte 运行时动态创建（初始 HTML 渲染的在编辑页会被剥）
                }
                // 编辑器由上方 data-card 的 hyperscript（on load）加载 print-template-edit.js 到 <head>。
                // body 内 <script src> 在本系统会被移除不执行（codemirror.js 根本不发网络请求），
                // head 的 script 不受影响，故由 head 的 hyperscript 触发 head 注入。
                // #pte-toast-area 放在下方 form 内（form 外的元素在编辑页会被剥）
            }
        }
    }
}

// ── 变量面板（按 document_type 渲染，中文 Jinja 占位符）──
// chip 点击调用 print-template-edit.js 的 insertPrintSnippet / insertDetailLoop。

fn variable_panel(document_type: Option<&str>) -> Markup {
    match document_type.unwrap_or("delivery_note") {
        "quotation" => quotation_vars(),
        "sales_order" => sales_order_vars(),
        "purchase_order" => purchase_order_vars(),
        _ => delivery_note_vars(),
    }
}

fn var_chip(label: &str, snippet: &str) -> Markup {
    html! {
        button type="button" data-snippet=(snippet)
            class="block w-full text-left px-2 py-1 mb-1 text-xs rounded-sm bg-surface text-fg-2 border border-border-soft hover:bg-accent-bg hover:text-accent cursor-pointer transition-colors"
            _="on click call insertPrintSnippet(my @data-snippet)"
        { (label) }
    }
}

fn chips(items: &[(&str, &str)]) -> Markup {
    html! { @for (label, snippet) in items { (var_chip(label, snippet)) } }
}

fn var_group(title: &str, items: &[(&str, &str)]) -> Markup {
    html! {
        div class="mb-3" {
            div class="text-xs font-semibold text-fg mb-1.5" { (title) }
            (chips(items))
        }
    }
}

fn detail_group(title: &str, items: &[(&str, &str)]) -> Markup {
    html! {
        div class="mb-3" {
            div class="text-xs font-semibold text-fg mb-1.5" { (title) }
            button type="button"
                class="block w-full text-left px-2 py-1.5 mb-1.5 text-xs rounded-sm bg-accent-bg text-accent border border-accent/30 hover:bg-accent hover:text-white cursor-pointer transition-colors font-medium"
                _="on click call insertDetailLoop()"
            { "↧ 插入明细循环块" }
            (chips(items))
        }
    }
}

fn delivery_note_vars() -> Markup {
    html! {
        (var_group("单据头", &[
            ("出库单号", "{{ 出库单号 }}"), ("出库日期", "{{ 出库日期 }}"),
            ("计划日期", "{{ 计划日期 }}"), ("客户全称", "{{ 客户全称 }}"),
            ("收货地址", "{{ 收货地址 }}"), ("联系电话", "{{ 联系电话 }}"),
            ("收货人", "{{ 收货人 }}"), ("客户经理", "{{ 客户经理 }}"),
            ("订单总金额", "{{ 订单总金额 }}"), ("大写金额", "{{ 大写金额 }}"),
            ("备注", "{{ 备注 }}"), ("单据状态", "{{ 单据状态 }}"),
        ]))
        (detail_group("明细行循环（item.）", &[
            ("产品编码", "{{ item.产品编码 }}"), ("产品名称", "{{ item.产品名称 }}"),
            ("规格型号", "{{ item.规格型号 }}"), ("单位", "{{ item.单位 }}"),
            ("本次出库数量", "{{ item.本次出库数量 }}"), ("批次号", "{{ item.批次号 }}"),
            ("单价", "{{ item.单价 }}"), ("金额", "{{ item.金额 }}"), ("行备注", "{{ item.行备注 }}"),
        ]))
        (var_group("系统变量", &[
            ("公司名称", "{{ 公司名称 }}"), ("打印时间", "{{ 打印时间 }}"),
        ]))
    }
}

fn quotation_vars() -> Markup {
    html! {
        (var_group("单据头", &[
            ("报价单号", "{{ 报价单号 }}"), ("报价日期", "{{ 报价日期 }}"),
            ("有效期至", "{{ 有效期至 }}"), ("客户全称", "{{ 客户全称 }}"),
            ("报价总金额", "{{ 报价总金额 }}"), ("付款条款", "{{ 付款条款 }}"),
            ("交货条款", "{{ 交货条款 }}"), ("报价状态", "{{ 报价状态 }}"),
            ("销售员", "{{ 销售员 }}"),
        ]))
        (detail_group("明细行循环（item.）", &[
            ("行号", "{{ item.行号 }}"), ("产品名称", "{{ item.产品名称 }}"),
            ("数量", "{{ item.数量 }}"), ("单位", "{{ item.单位 }}"),
            ("单价", "{{ item.单价 }}"), ("折扣率", "{{ item.折扣率 }}"), ("金额", "{{ item.金额 }}"),
        ]))
        (var_group("系统变量", &[
            ("公司名称", "{{ 公司名称 }}"), ("打印时间", "{{ 打印时间 }}"),
        ]))
    }
}

fn sales_order_vars() -> Markup {
    html! {
        (var_group("单据头", &[
            ("订单号", "{{ 订单号 }}"), ("订单日期", "{{ 订单日期 }}"),
            ("客户全称", "{{ 客户全称 }}"), ("订单总金额", "{{ 订单总金额 }}"),
            ("交货地址", "{{ 交货地址 }}"), ("付款条款", "{{ 付款条款 }}"),
            ("交货条款", "{{ 交货条款 }}"), ("订单状态", "{{ 订单状态 }}"),
            ("销售员", "{{ 销售员 }}"),
        ]))
        (detail_group("明细行循环（item.）", &[
            ("行号", "{{ item.行号 }}"), ("产品名称", "{{ item.产品名称 }}"),
            ("数量", "{{ item.数量 }}"), ("单位", "{{ item.单位 }}"),
            ("单价", "{{ item.单价 }}"), ("金额", "{{ item.金额 }}"),
            ("已发数量", "{{ item.已发数量 }}"), ("未交数量", "{{ item.未交数量 }}"), ("行状态", "{{ item.行状态 }}"),
        ]))
        (var_group("系统变量", &[
            ("公司名称", "{{ 公司名称 }}"), ("打印时间", "{{ 打印时间 }}"),
        ]))
    }
}

fn purchase_order_vars() -> Markup {
    html! {
        (var_group("单据头", &[
            ("采购单号", "{{ 采购单号 }}"), ("采购日期", "{{ 采购日期 }}"),
            ("供应商全称", "{{ 供应商全称 }}"), ("采购总金额", "{{ 采购总金额 }}"),
            ("要求交期", "{{ 要求交期 }}"), ("交货地址", "{{ 交货地址 }}"),
            ("付款条款", "{{ 付款条款 }}"), ("订单说明", "{{ 订单说明 }}"),
            ("采购员", "{{ 采购员 }}"), ("采购状态", "{{ 采购状态 }}"),
            ("供应商地址", "{{ 供应商地址 }}"), ("部门", "{{ 部门 }}"),
            ("委外工单号", "{{ 委外工单号 }}"), ("采购经理", "{{ 采购经理 }}"),
        ]))
        (var_group("明细变量（取第一条明细，用于硬编码单行模板）", &[
            ("产品编码", "{{ 产品编码 }}"), ("产品名称", "{{ 产品名称 }}"),
            ("单位", "{{ 单位 }}"), ("采购净价", "{{ 采购净价 }}"),
            ("采购数量", "{{ 采购数量 }}"), ("净价合计", "{{ 净价合计 }}"),
            ("备注", "{{ 备注 }}"),
        ]))
        (detail_group("明细行循环（item.，多行用 for 循环）", &[
            ("行号", "{{ item.行号 }}"), ("产品编码", "{{ item.产品编码 }}"),
            ("产品名称", "{{ item.产品名称 }}"), ("数量", "{{ item.数量 }}"),
            ("采购数量", "{{ item.采购数量 }}"), ("单位", "{{ item.单位 }}"),
            ("单价", "{{ item.单价 }}"), ("采购净价", "{{ item.采购净价 }}"),
            ("金额", "{{ item.金额 }}"), ("净价合计", "{{ item.净价合计 }}"),
            ("已收数量", "{{ item.已收数量 }}"), ("备注", "{{ item.备注 }}"),
        ]))
        (var_group("系统变量", &[
            ("公司名称", "{{ 公司名称 }}"), ("打印时间", "{{ 打印时间 }}"),
        ]))
    }
}

