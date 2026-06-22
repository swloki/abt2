use axum::extract::Query;
use axum::response::Html;
use axum_extra::routing::TypedPath;
use maud::{html, Markup};
use serde::Deserialize;

use abt_core::fms::ar_ap::enums::AgeingBasis;
use abt_core::fms::ar_ap::model::{AgingReq, AgingRow};
use abt_core::fms::ar_ap::ArApService;
use abt_core::fms::enums::CounterpartyType;
use rust_decimal::Decimal;

use crate::errors::Result;
use crate::layout::page::admin_page;
use crate::routes::fms::ApAgingPath;
use crate::utils::RequestContext;
use abt_macros::require_permission;

#[derive(Deserialize, Debug, Default)]
pub struct AgingQuery {
    pub buckets: Option<String>,
}

fn fmt_amount(amount: Decimal) -> String { format!("{amount:.2}") }

fn aging_table(rows: &[AgingRow], bucket_labels: &[String]) -> Markup {
    html! {
        div id="data-card" class="data-card" {
            div class="overflow-x-auto" {
                table class="data-table" {
                    thead {
                        tr {
                            th class="px-4 py-3 text-left text-xs font-medium text-fg-2 uppercase" {
                                "供应商"
                            }
                            th  class="px-4 py-3 text-right text-xs font-medium text-fg-2 uppercase"
                            { "未清总额" }
                            @for label in bucket_labels {
                                th  class="px-4 py-3 text-right text-xs font-medium text-fg-2 uppercase"
                                { (label) }
                            }
                            th  class="px-4 py-3 text-right text-xs font-medium text-fg-2 uppercase"
                            { "超期" }
                        }
                    }
                    tbody class="divide-y divide-border-soft" {
                        @for row in rows {
                            tr {
                                td class="px-4 py-3 text-sm font-medium" { (row.party_name) }
                                td class="px-4 py-3 text-sm font-mono text-right font-semibold" {
                                    "¥"
                                    (fmt_amount(row.total_outstanding))
                                }
                                @for amt in &row.buckets {
                                    td class="px-4 py-3 text-sm font-mono text-right" {
                                        @if *amt > Decimal::ZERO {
                                            span style="color:var(--danger)" {
                                                "¥"
                                                (fmt_amount(*amt))
                                            }
                                        } @else {
                                            span class="text-fg-3" { "—" }
                                        }
                                    }
                                }
                                td class="px-4 py-3 text-sm font-mono text-right" {
                                    @if row.over_max > Decimal::ZERO {
                                        span style="color:var(--danger);font-weight:600" {
                                            "¥"
                                            (fmt_amount(row.over_max))
                                        }
                                    } @else {
                                        span class="text-fg-3" { "—" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[require_permission("FMS", "read")]
pub async fn get_page(
    _path: ApAgingPath,
    ctx: RequestContext,
    Query(q): Query<AgingQuery>,
) -> Result<Html<String>> {
    let is_htmx = ctx.is_htmx();
    let nav_filter = ctx.nav_filter().await;
    let RequestContext { mut conn, state, service_ctx, claims, .. } = ctx;
    let svc = state.ar_ap_service();

    let buckets: Vec<i32> = q.buckets.as_deref()
        .unwrap_or("30,60,90,120")
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    let bucket_labels: Vec<String> = buckets.iter().enumerate().map(|(i, &d)| {
        if i == 0 { format!("0-{}天", d) } else { format!("{}-{}天", buckets[i - 1] + 1, d) }
    }).collect();

    let today = chrono::Utc::now().date_naive();
    let rows = svc.ap_aging(&service_ctx, &mut conn, AgingReq {
        party_type: CounterpartyType::Supplier,
        as_of_date: today,
        ageing_based_on: AgeingBasis::DueDate,
        buckets: buckets.clone(),
        party_ids: None,
    }).await.unwrap_or_default();

    let content = html! {
        div {
            h1 class="text-xl font-bold text-fg tracking-tight mb-5" { "应付账龄分析" }
            (aging_table(&rows, &bucket_labels))
        }
    };

    let page_html = admin_page(is_htmx, "应付账龄分析", &claims, "finance", ApAgingPath::PATH, "财务管理", None, content, &nav_filter);
    Ok(Html(page_html.into_string()))
}
