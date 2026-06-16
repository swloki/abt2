use axum_extra::routing::TypedPath;
use axum::response::{Html, IntoResponse};
use maud::{Markup, html, PreEscaped};
use serde::Deserialize;

use abt_core::master_data::category::{CategoryService, model::CategoryTree};
use abt_core::master_data::product::model::{CreateProductReq, Product, ProductMeta, ProductStatus, AcquireChannel};
use abt_core::master_data::product::ProductService;
use abt_core::shared::types::DomainError;
use abt_macros::require_permission;

use crate::components::icon;
use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::product::{ProductCopyPath, ProductCreatePath, ProductDetailPath, ProductListPath};
use crate::utils::RequestContext;

// ── Query Params ──

#[derive(Debug, Deserialize)]
pub struct CreateQueryParams {
    #[serde(default)]
    pub copy_from: Option<i64>,
}

// ── Form Data ──

#[derive(Debug, Deserialize)]
pub struct ProductCreateForm {
    pub name: String,
    pub unit: String,
    pub specification: String,
    pub acquire_channel: Option<String>,
    pub external_code: Option<String>,
    pub owner_department_id: Option<String>,
    pub category_id: Option<String>,
    pub old_code: Option<String>,
    pub remark: Option<String>,
}

// ── Handlers ──

#[require_permission("PRODUCT", "create")]
pub async fn get_product_create(
    _path: ProductCreatePath,
    axum::extract::Query(params): axum::extract::Query<CreateQueryParams>,
    ctx: RequestContext,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;

    let copy_source = if let Some(id) = params.copy_from {
        let svc = state.product_service();
        Some(svc.get(&service_ctx, &mut conn, id).await?)
    } else {
        None
    };

    let cat_svc = state.category_service();
    let categories = cat_svc.get_tree(&service_ctx, &mut conn, None, None).await?;

    let title = if copy_source.is_some() { "复制产品" } else { "新建产品" };
    let content = product_create_page(copy_source.as_ref(), &categories);
    let page_html = admin_page(
        is_htmx,
        title,
        &claims,
        "md",
        ProductCreatePath::PATH,
        "主数据管理",
        Some(title),
        content, &nav_filter,    );

    Ok(Html(page_html.into_string()))
}

#[require_permission("PRODUCT", "create")]
pub async fn post_product_create(
    _path: ProductCreatePath,
    ctx: RequestContext,
    axum::Form(form): axum::Form<ProductCreateForm>,
) -> Result<impl IntoResponse> {
    let RequestContext {
        mut conn,
        state,
        service_ctx,
        ..
    } = ctx;
    let svc = state.product_service();

    // 解析并校验 category_id
    let category_id = form.category_id
        .as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<i64>().ok())
        .ok_or_else(|| DomainError::validation("请选择所属分类"))?;

    let owner_department_id = form
        .owner_department_id
        .as_deref()
        .and_then(|s| if s.is_empty() { None } else { s.parse::<i64>().ok() });

    // 将中文获取途径映射为枚举值
    let acquire_channel = match form.acquire_channel.as_deref() {
        Some("自制") => AcquireChannel::SelfProduced,
        Some("采购") => AcquireChannel::Purchased,
        Some("委外") => AcquireChannel::Outsourced,
        _ => AcquireChannel::Legacy, // 默认值，用于历史遗留数据
    };

    let create_req = CreateProductReq {
        name: form.name,
        unit: form.unit,
        status: ProductStatus::Active,
        acquire_channel,
        external_code: form.external_code.filter(|s| !s.is_empty()),
        owner_department_id,
        meta: ProductMeta {
            specification: form.specification,
            old_code: form.old_code.filter(|s| !s.is_empty()),
            remark: form.remark.filter(|s| !s.is_empty()),
            material_consumption_mode: Default::default(),
            over_completion_tolerance: None,
        },
    };

    let id = svc.create(&service_ctx, &mut conn, create_req).await?;

    // 关联产品分类
    let cat_svc = state.category_service();
    cat_svc.assign_products(&service_ctx, &mut conn, category_id, vec![id]).await?;

    let redirect = ProductDetailPath { id }.to_string();
    Ok(([("HX-Redirect", redirect)], Html(String::new())))
}

// ── Components ──

fn product_create_page(source: Option<&Product>, categories: &[CategoryTree]) -> Markup {
    let title = if source.is_some() { "复制产品" } else { "新建产品" };
    let btn_label = if source.is_some() { "保存副本" } else { "保存产品" };

    let name_val = source.map(|p| format!("{}-1", p.pdt_name)).unwrap_or_default();
    let spec_val = source.map(|p| p.meta.specification.as_str()).unwrap_or("");
    let unit_val = source.map(|p| p.unit.as_str()).unwrap_or("");
    let acquire_val = source.map(|p| match p.acquire_channel {
        AcquireChannel::SelfProduced => "自制",
        AcquireChannel::Purchased => "采购",
        AcquireChannel::Outsourced => "委外",
        AcquireChannel::NonInventory => "非库存",
        AcquireChannel::Legacy => "历史遗留",
    }).unwrap_or("采购");
    let external_code_val = source.as_ref().and_then(|p| p.external_code.as_deref()).unwrap_or("");
    let old_code_val = source.as_ref().and_then(|p| p.meta.old_code.as_deref()).unwrap_or("");

    html! {
        div {
            // ── Page Header ──
            div class="flex items-center justify-between mb-6" {
                a class="inline-flex items-center gap-2 text-sm text-muted hover:text-accent transition-colors duration-150" href=(format!("{}?restore=true", ProductListPath::PATH)) {
                    (icon::arrow_left_icon("w-4 h-4"))
                    "返回产品列表"
                }
                h1 class="text-xl font-bold text-fg tracking-tight" { (title) }
            }

            form id="product-form"
                  hx-post=(ProductCreatePath::PATH)
                  hx-swap="none" {

                // ── Section: 基本信息 ──
                div class="form-section" {
                    div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" { "基本信息" }
                    div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                        div class="form-field" {
                            label { "产品名称 " span style="color:var(--danger)" { "*" } }
                            input type="text" name="name" required placeholder="请输入产品名称" value=(name_val) {}
                        }
                        div class="form-field" {
                            label { "产品编码" }
                            input type="text" value="自动生成" readonly
                                style="background:var(--surface);color:var(--muted)" {}
                        }
                        div class="form-field" {
                            label { "规格型号" }
                            input type="text" name="specification" placeholder="请输入规格型号" value=(spec_val) {}
                        }
                        div class="form-field" {
                            label { "计量单位 " span style="color:var(--danger)" { "*" } }
                            select name="unit" required {
                                option value="" disabled selected[unit_val.is_empty()] { "请选择" }
                                option value="个" selected[unit_val == "个"] { "个" }
                                option value="件" selected[unit_val == "件"] { "件" }
                                option value="台" selected[unit_val == "台"] { "台" }
                                option value="套" selected[unit_val == "套"] { "套" }
                                option value="批" selected[unit_val == "批"] { "批" }
                                option value="kg" selected[unit_val == "kg" || unit_val == "千克"] { "千克 (kg)" }
                                option value="g" selected[unit_val == "g" || unit_val == "克"] { "克 (g)" }
                                option value="m" selected[unit_val == "m" || unit_val == "米"] { "米 (m)" }
                                option value="cm" selected[unit_val == "cm" || unit_val == "厘米"] { "厘米 (cm)" }
                                option value="L" selected[unit_val == "L" || unit_val == "升"] { "升 (L)" }
                                option value="卷" selected[unit_val == "卷"] { "卷" }
                                option value="包" selected[unit_val == "包"] { "包" }
                                option value="箱" selected[unit_val == "箱"] { "箱" }
                                option value="根" selected[unit_val == "根"] { "根" }
                                option value="块" selected[unit_val == "块"] { "块" }
                                option value="片" selected[unit_val == "片"] { "片" }
                                option value="张" selected[unit_val == "张"] { "张" }
                                option value="条" selected[unit_val == "条"] { "条" }
                            }
                        }
                        div class="form-field" {
                            label { "获取途径" }
                            select name="acquire_channel" {
                                option value="采购" selected[acquire_val == "采购"] { "采购" }
                                option value="自制" selected[acquire_val == "自制"] { "自制" }
                                option value="委外" selected[acquire_val == "委外"] { "委外" }
                            }
                        }
                        div class="form-field" {
                            label { "外部编码" }
                            input type="text" name="external_code" placeholder="请输入外部编码" value=(external_code_val) {}
                        }
                    }
                }

                // ── Section: 分类与归属 ──
                div class="form-section" {
                    div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" { "分类与归属" }
                    div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                        div class="form-field" {
                            label { "所属分类 " span style="color:var(--danger)" { "*" } }
                            input type="hidden" name="category_id" id="selected-category-id" {}
                            button type="button" class="w-full px-3 py-2 border border-border rounded-sm text-sm bg-white text-fg transition-all duration-150 outline-none focus:border-accent focus:shadow-[var(--shadow-focus)] category-select-trigger" id="category-select-btn" _="on click add .is-open to #category-modal" {
                                span id="category-select-label" { "请选择分类" }
                                (icon::chevron_right_icon("w-4 h-4"))
                            }
                        }
                        div class="form-field" {
                            label { "归属部门" }
                            select name="owner_department_id" {
                                option value="" { "-- 请选择 --" }
                            }
                        }
                        div class="form-field" {
                            label { "旧编码" }
                            input type="text" name="old_code" placeholder="请输入旧编码" value=(old_code_val) {}
                        }
                    }
                }

                // ── Section: 其他信息 ──
                div class="form-section" {
                    div class="flex items-center gap-2 text-sm font-semibold text-fg mb-4 pb-2 border-b border-border-soft" { "其他信息" }
                    div class="grid grid-cols-2 gap-4 gap-x-6 mb-6" {
                        div class="form-field field-full" {
                            label { "备注" }
                            textarea name="remark" placeholder="请输入备注信息…"
                                style="width:100%;min-height:80px;resize:vertical" {}
                        }
                    }
                }

                // ── Action Bar ──
                div class="flex items-center justify-end gap-3 pt-4 border-t border-border-soft" {
                    a class="btn bg-white text-fg border border-border hover:bg-surface" href=(format!("{}?restore=true", ProductListPath::PATH)) { "取消" }
                    button type="submit" class="btn bg-accent text-accent-on border-none hover:bg-accent-hover" {
                        (btn_label)
                    }
                }
            }

            // ── Category Select Modal ──
            div id="category-modal" class="modal-overlay" _="on click[me is event.target] remove .is-open" {
                div class="modal" onclick="event.stopPropagation()" {
                    div class="px-6 py-5 border-b border-border-soft flex justify-between items-center shrink-0" {
                        h2 { "选择分类" }
                        button type="button" class="btn-icon" _="on click remove .is-open from #category-modal" {
                            (icon::x_icon("w-4 h-4"))
                        }
                    }
                    div class="overflow-y-auto flex-1 min-h-0 p-6" {
                        div class="category-search-bar" {
                            (icon::search_icon("w-4 h-4"))
                            input type="text" id="category-search-input" class="category-w-full pl-9 pr-3 py-2 border border-border rounded-sm text-sm bg-white text-fg outline-none transition-all duration-150 focus:border-accent" placeholder="搜索分类…" {}
                        }
                        div id="category-list-container" class="category-select-list" {
                            @if categories.is_empty() {
                                div class="category-empty" { "暂无分类数据" }
                            } @else {
                                @for node in categories {
                                    (category_tree_node(node, 0))
                                }
                            }
                        }
                    }
                }
            }

            // ── Category Select Scripts ──
            (PreEscaped(r#"<script>
(function(){
    var searchInput=document.getElementById('category-search-input');
    var container=document.getElementById('category-list-container');
    if(!searchInput||!container)return;

    function filter(q){
        q=(q||'').trim().toLowerCase();
        var items=container.querySelectorAll('.category-select-item');
        if(!q){for(var i=0;i<items.length;i++)items[i].style.display='';return;}
        for(var i=0;i<items.length;i++){
            var name=(items[i].getAttribute('data-name')||'').toLowerCase();
            items[i]._match=(name.indexOf(q)>=0);
        }
        for(var i=0;i<items.length;i++){
            if(items[i]._match){
                var p=items[i].parentElement;
                while(p&&p!==container){
                    if(p.classList&&p.classList.contains('category-select-item'))p._match=true;
                    p=p.parentElement;
                }
            }
        }
        for(var i=0;i<items.length;i++){
            items[i].style.display=items[i]._match?'':'none';
            delete items[i]._match;
        }
    }

    searchInput.addEventListener('input',function(){filter(this.value)});

    container.addEventListener('click',function(e){
        if(e.target.closest('.category-tree-toggle'))return;
        var nameEl=e.target.closest('.category-select-name');
        if(!nameEl)return;
        var id=nameEl.getAttribute('data-id');
        var name=nameEl.getAttribute('data-name');
        document.getElementById('selected-category-id').value=id;
        document.getElementById('category-select-label').textContent=name;
        document.getElementById('category-modal').classList.remove('is-open');
    });
})();
</script>"#))
        }
    }
}

/// 递归渲染分类树节点（用于弹窗选择）
fn category_tree_node(node: &CategoryTree, depth: usize) -> Markup {
    let id = node.category_id;
    let name = &node.category_name;
    let name_lower = name.to_lowercase();
    let pad = format!("padding-left:{}px", depth * 24 + 12);

    let has_children = !node.children.is_empty();

    html! {
        div.category-select-item data-name=(name_lower) {
            div class="category-select-info" style=(pad) {
                @if has_children {
                    span class="category-tree-toggle" _="on click halt the event then toggle .expanded on closest .category-select-item" {
                        (icon::chevron_right_icon("w-3.5 h-3.5"))
                    }
                }
                span class="category-select-name" data-id=(id) data-name=(name) { (name) }
            }
            @if has_children {
                div class="category-select-children" {
                    @for child in &node.children {
                        (category_tree_node(child, depth + 1))
                    }
                }
            }
        }
    }
}

// ── Copy Handler ──

#[require_permission("PRODUCT", "create")]
pub async fn copy_product(path: ProductCopyPath, _ctx: RequestContext) -> crate::errors::Result<impl IntoResponse> {
    Ok(axum::response::Redirect::to(&format!("/admin/md/products/new?copy_from={}", path.id)))
}
