//! FMS 财务块全链路端到端测试
//!
//! 两条链路：
//! - k1 报销付款（待补）：expense.create → submit → approve → generate_payment_journal → CashJournal + Paid
//! - k2 成本核算只读：cost_accounting 各查询 Ok

mod common;
use common::TestApp;
use abt_core::shared::types::ServiceContext;
use abt_core::fms::cost_accounting::CostAccountingService;


// ════════════════════════════════════════════════════════════════════════════
//  k4 成本核算只读：各查询返回 Ok（结果可空）
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn k4_cost_accounting_queries_ok() {
    let app = TestApp::new().await;
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    let svc = app.state.cost_accounting_service();

    let period = format!("{}", chrono::Utc::now().date_naive().format("%Y-%m"));

    // 各查询应 Ok（结果是否非空取决于 dev 库数据，不在此断言）
    let _ = svc.get_product_cost(&ctx, &mut conn, 565, period.clone()).await.unwrap();
    let _ = svc.list_product_costs(&mut conn, &period).await.unwrap();
    let _ = svc.list_work_order_costs(&mut conn).await.unwrap();
    let _ = svc.get_margin_analysis(&ctx, &mut conn, 1).await.unwrap();
}
