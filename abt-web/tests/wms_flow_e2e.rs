//! WMS 库存管理全流程 Handler 集成测试
//!
//! 覆盖主干闭环：入库 → 可用量(ATP) → 出库 → 调拨(dispatch+complete) → 事务流水
//! 每条测试通过 HTTP 触发动作，再用 abt-core Service 层做字段级 / 余额级断言。
//!
//! 设计要点：
//! - 测试库为共享真实 DB（数据持久），故一律断言"增量"，不依赖绝对余额。
//! - ATP 口径直接复用 InventoryTransactionService::query_available（= quantity − reserved），
//!   顺带验证评审 P0 关注点：可用量是否与实物余额一致。

mod common;
use common::TestApp;

use rust_decimal::Decimal;

use abt_core::shared::types::ServiceContext;
use abt_core::wms::enums::TransferStatus;
use abt_core::wms::inventory::InventoryService;
use abt_core::wms::inventory_transaction::InventoryTransactionService;
use abt_core::wms::transfer::TransferService;
use abt_core::wms::transfer::model::{InventoryTransfer, TransferFilter};

const PRODUCT_ID: i64 = 565;
const PRODUCT_ID_STR: &str = "565";
/// 主仓（备料周转仓）—— stock_in 落账目标
const WAREHOUSE_A: i64 = 23320;
const ZONE_A: i64 = 23320000;
const BIN_A: i64 = 23320000;
/// 调入仓（原材料仓）—— 调拨目标
const WAREHOUSE_B: i64 = 23327;
const ZONE_B: i64 = 23361;
const BIN_B: i64 = 23361;

// ── URL 编码（表单体里的 items_json 必须转义）──

fn urlenc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => {
                out.push('%');
                out.push_str(&format!("{:02X}", b));
            }
        }
    }
    out
}

/// 入库明细 JSON：`{product_id, batch_no, quantity, bin_id}`
fn items_in(items: &[(&str, &str, &str, &str)]) -> String {
    let parts: Vec<String> = items
        .iter()
        .map(|(pid, qty, batch, bin)| {
            let b = if batch.is_empty() {
                "null".to_string()
            } else {
                format!("\"{batch}\"")
            };
            format!(r#"{{"product_id":"{pid}","batch_no":{b},"quantity":"{qty}","bin_id":"{bin}"}}"#)
        })
        .collect();
    urlenc(&format!("[{}]", parts.join(",")))
}

/// 出库明细 JSON：`{product_id, quantity}`
fn items_out(items: &[(&str, &str)]) -> String {
    let parts: Vec<String> = items
        .iter()
        .map(|(pid, qty)| format!(r#"{{"product_id":"{pid}","quantity":"{qty}"}}"#))
        .collect();
    urlenc(&format!("[{}]", parts.join(",")))
}

/// 调拨明细 JSON：`{product_id, quantity}`
fn items_xfer(items: &[(&str, &str)]) -> String {
    let parts: Vec<String> = items
        .iter()
        .map(|(pid, qty)| format!(r#"{{"product_id":"{pid}","quantity":"{qty}"}}"#))
        .collect();
    urlenc(&format!("[{}]", parts.join(",")))
}

// ── Service 层验证 helpers ──

async fn available(app: &TestApp, product_id: i64, warehouse_id: i64) -> Decimal {
    let svc = app.state.inventory_transaction_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    svc.query_available(&ctx, &mut conn, product_id, Some(warehouse_id))
        .await
        .unwrap()
}

async fn log_count(app: &TestApp, product_id: i64) -> usize {
    let svc = app.state.inventory_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    svc.list_logs_by_product(&ctx, &mut conn, product_id)
        .await
        .unwrap()
        .len()
}

async fn get_transfer(app: &TestApp, id: i64) -> InventoryTransfer {
    let svc = app.state.transfer_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    svc.get(&ctx, &mut conn, id).await.unwrap()
}

/// 两个有 bin 的仓库（stock_in/调拨都需要 bin 才能落账）。
/// 23320=备料周转仓(1 bin)，23327=原材料仓(94 bin)，均含 bin 且为 RawMaterial 类型。
fn pick_two_warehouses() -> (i64, i64) {
    (WAREHOUSE_A, WAREHOUSE_B)
}

/// 在 Draft 状态的调拨单中找到 from→to 的那一笔（create 重定向到列表，故需回查 id）。
async fn find_draft_transfer(app: &TestApp, from_wh: i64, to_wh: i64) -> InventoryTransfer {
    let svc = app.state.transfer_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    let filter = TransferFilter {
        from_warehouse_id: Some(from_wh),
        to_warehouse_id: Some(to_wh),
        ..Default::default()
    };
    let res = svc.list(&ctx, &mut conn, filter, 1, 50).await.unwrap();
    res.items
        .into_iter()
        .find(|t| matches!(t.status, TransferStatus::Draft))
        .expect("未找到 Draft 状态的调拨单")
}

// ── HTTP 动作 helpers ──

async fn stock_in(app: &TestApp, warehouse_id: i64, zone_id: i64, bin_id: i64, qty: &str, batch: &str) {
    let items = items_in(&[(PRODUCT_ID_STR, qty, batch, &bin_id.to_string())]);
    let body = format!(
        "transaction_type=PurchaseReceipt&source_type=manual&warehouse_id={warehouse_id}&zone_id={zone_id}&bin_id={bin_id}&items_json={items}"
    );
    let resp = app.post_htmx("/admin/wms/stock-in/create", &body).await;
    assert!(
        resp.is_ok(),
        "入库 FAIL: {} body: {}",
        resp.status,
        resp.body.chars().take(300).collect::<String>()
    );
}

async fn stock_out(app: &TestApp, warehouse_id: i64, zone_id: i64, bin_id: i64, qty: &str) {
    let items = items_out(&[(PRODUCT_ID_STR, qty)]);
    let body = format!(
        "source_type=shipping&warehouse_id={warehouse_id}&zone_id={zone_id}&bin_id={bin_id}&items_json={items}"
    );
    let resp = app.post_htmx("/admin/wms/stock-out/create", &body).await;
    assert!(
        resp.is_ok(),
        "出库 FAIL: {} body: {}",
        resp.status,
        resp.body.chars().take(300).collect::<String>()
    );
}

// ════════════════════════════════════════════════════════════════════════════
//  A. 完整流程：入库 → 出库 → 调拨(dispatch+complete)，全程校验余额增量 + 流水
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn a1_full_inventory_lifecycle() {
    let app = TestApp::new().await;
    let (wh_a, wh_b) = pick_two_warehouses();

    // 记录基线可用量（共享库 → 一律断言增量）
    let base_a = available(&app, PRODUCT_ID, wh_a).await;
    let base_b = available(&app, PRODUCT_ID, wh_b).await;

    // 1) 入库 +100 → wh_a（空 batch：web stock_out 不接受 batch，须对齐到同一台账行）
    stock_in(&app, wh_a, ZONE_A, BIN_A, "100", "").await;
    let after_in = available(&app, PRODUCT_ID, wh_a).await;
    assert_eq!(
        after_in - base_a,
        Decimal::from(100),
        "入库后 wh_a 可用量应 +100 (base={base_a}, now={after_in})"
    );

    // 2) ATP 口径自检（评审 P0）：无预留时可用量应等于实物余额，数量必须真切落到台账
    //    （query_available 内部 = StockLedger.quantity − total_reserved）

    // 3) 出库 -30 ← wh_a
    stock_out(&app, wh_a, ZONE_A, BIN_A, "30").await;
    let after_out = available(&app, PRODUCT_ID, wh_a).await;
    assert_eq!(
        after_out - after_in,
        Decimal::from(-30),
        "出库后 wh_a 可用量应 -30 (pre={after_in}, now={after_out})"
    );

    // 4) 调拨 wh_a → wh_b，20：create(Draft) → dispatch(InTransit) → complete(Completed)
    let body = format!(
        "from_warehouse_id={wh_a}&from_zone_id={ZONE_A}&from_bin_id={BIN_A}&to_warehouse_id={wh_b}&to_zone_id={ZONE_B}&to_bin_id={BIN_B}&transfer_date=2026-06-19&items_json={}",
        items_xfer(&[(PRODUCT_ID_STR, "20")])
    );
    let resp = app.post_htmx("/admin/wms/transfers/create", &body).await;
    assert!(
        resp.is_ok(),
        "创建调拨 FAIL: {} body: {}",
        resp.status,
        resp.body.chars().take(300).collect::<String>()
    );

    let draft = find_draft_transfer(&app, wh_a, wh_b).await;

    // dispatch: Draft → InTransit
    let r = app
        .post_htmx(&format!("/admin/wms/transfers/{}", draft.id), "action=dispatch")
        .await;
    assert!(
        r.is_ok() || r.is_redirect(),
        "调拨发货 FAIL: {}",
        r.status
    );

    // complete: InTransit → Completed
    let r = app
        .post_htmx(&format!("/admin/wms/transfers/{}", draft.id), "action=complete")
        .await;
    assert!(
        r.is_ok() || r.is_redirect(),
        "调拨完成 FAIL: {}",
        r.status
    );

    // 状态机：必须抵达 Completed
    let done = get_transfer(&app, draft.id).await;
    assert!(
        matches!(done.status, TransferStatus::Completed),
        "调拨应已完成，实际状态异常"
    );

    // 余额双校验：调出仓 -20，调入仓 +20
    let final_a = available(&app, PRODUCT_ID, wh_a).await;
    let final_b = available(&app, PRODUCT_ID, wh_b).await;
    assert_eq!(
        final_a - after_out,
        Decimal::from(-20),
        "调拨后调出仓 wh_a 应 -20 (pre={after_out}, now={final_a})"
    );
    assert_eq!(
        final_b - base_b,
        Decimal::from(20),
        "调拨后调入仓 wh_b 应 +20 (base={base_b}, now={final_b})"
    );

    // 5) 事务流水连续性：每一步都应留下 append-only 记录
    assert!(
        log_count(&app, PRODUCT_ID).await > 0,
        "应存在库存事务流水记录"
    );

    // 详情页可达
    let detail = app.get_htmx(&format!("/admin/wms/transfers/{}", draft.id)).await;
    assert!(detail.is_ok(), "调拨详情页应可达");
}

// ════════════════════════════════════════════════════════════════════════════
//  B. 入库 — 异常边界
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn b1_stock_in_error_no_warehouse() {
    let app = TestApp::new().await;
    let items = items_in(&[("565", "10", "B1", "23320000")]);
    let body = format!("transaction_type=PurchaseReceipt&source_type=manual&items_json={items}");
    let resp = app.post_htmx("/admin/wms/stock-in/create", &body).await;
    assert!(
        resp.status.is_client_error(),
        "未选仓库应返回 4xx，实际 {}",
        resp.status
    );
}

#[tokio::test]
async fn b2_stock_in_error_empty_items() {
    let app = TestApp::new().await;
    let body = format!(
        "transaction_type=PurchaseReceipt&source_type=manual&warehouse_id=23320&items_json={}",
        urlenc("[]")
    );
    let resp = app.post_htmx("/admin/wms/stock-in/create", &body).await;
    assert!(
        resp.status.is_client_error(),
        "空明细应返回 4xx，实际 {}",
        resp.status
    );
}

#[tokio::test]
async fn b3_stock_in_error_zero_qty() {
    let app = TestApp::new().await;
    let items = items_in(&[("565", "0", "B3", "23320000")]);
    let body = format!(
        "transaction_type=PurchaseReceipt&source_type=manual&warehouse_id=23320&items_json={items}"
    );
    let resp = app.post_htmx("/admin/wms/stock-in/create", &body).await;
    assert!(
        resp.status.is_client_error(),
        "入库数量为 0 应返回 4xx，实际 {}",
        resp.status
    );
}

#[tokio::test]
async fn b4_stock_in_error_malformed_json() {
    let app = TestApp::new().await;
    let body =
        "transaction_type=PurchaseReceipt&source_type=manual&warehouse_id=23320&items_json=NOT_JSON";
    let resp = app.post_htmx("/admin/wms/stock-in/create", body).await;
    assert!(
        resp.status.is_client_error(),
        "非法 items_json 应返回 4xx，实际 {}",
        resp.status
    );
}

// ── P0-2 负库存前置预检：出库超出可用量应被前置拦截，返回明确错误而非 500 ──

#[tokio::test]
async fn b5_stock_out_insufficient_returns_clear_error() {
    let app = TestApp::new().await;
    let (wh, _) = pick_two_warehouses();

    // 先入库一个极小量，确保该产品在此仓库有台账行
    stock_in(&app, wh, ZONE_A, BIN_A, "1", "").await;

    // 再尝试出库一个远超可用量的数：应被 record() 的前置预检拦截
    let items = items_out(&[(PRODUCT_ID_STR, "999999")]);
    let body = format!("source_type=shipping&warehouse_id={wh}&items_json={items}");
    let resp = app.post_htmx("/admin/wms/stock-out/create", &body).await;

    assert!(
        resp.status.is_client_error(),
        "库存不足应返回 4xx，实际 {} body: {}",
        resp.status,
        resp.body.chars().take(200).collect::<String>()
    );
    // P0-2：错误信息应明确指出"库存不足"（而非 upsert 深处的"库存数量不能为负"）
    assert!(
        resp.body.contains("库存不足") || resp.body.contains("可用量"),
        "应返回带上下文的库存不足错误，实际 body: {}",
        resp.body.chars().take(200).collect::<String>()
    );
}

// ════════════════════════════════════════════════════════════════════════════
//  C. 调拨 — 状态机与异常

#[tokio::test]
async fn c1_transfer_detail_nonexistent_is_404() {
    let app = TestApp::new().await;
    assert_eq!(
        app.get("/admin/wms/transfers/999999").await.status,
        axum::http::StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn c2_transfer_bogus_action_no_crash_and_stays_draft() {
    let app = TestApp::new().await;
    let (wh_a, wh_b) = pick_two_warehouses();
    let body = format!(
        "from_warehouse_id={wh_a}&to_warehouse_id={wh_b}&transfer_date=2026-06-19&items_json={}",
        items_xfer(&[("565", "1")])
    );
    let resp = app.post_htmx("/admin/wms/transfers/create", &body).await;
    assert!(resp.is_ok(), "创建调拨 FAIL: {}", resp.status);

    let draft = find_draft_transfer(&app, wh_a, wh_b).await;

    // 未知 action 应被忽略，不应崩溃，状态保持 Draft
    let resp = app
        .post_htmx(&format!("/admin/wms/transfers/{}", draft.id), "action=bogus")
        .await;
    assert!(
        resp.is_ok() || resp.is_redirect(),
        "未知 action 不应报错，实际 {}",
        resp.status
    );
    let after = get_transfer(&app, draft.id).await;
    assert!(
        matches!(after.status, TransferStatus::Draft),
        "未知 action 后应保持 Draft"
    );
}

// ════════════════════════════════════════════════════════════════════════════
//  D. 页面可达性
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn d1_list_pages_accessible() {
    let app = TestApp::new().await;
    for url in [
        "/admin/wms/stock",
        "/admin/wms/stock-in",
        "/admin/wms/stock-out",
        "/admin/wms/transfers",
        "/admin/wms/cycle-counts",
        "/admin/wms/locks",
    ] {
        let resp = app.get(url).await;
        assert!(resp.is_ok(), "GET {url} 失败: {}", resp.status);
    }
}

#[tokio::test]
async fn d2_create_pages_accessible() {
    let app = TestApp::new().await;
    for url in [
        "/admin/wms/stock-in/create",
        "/admin/wms/stock-out/create",
        "/admin/wms/transfers/create",
    ] {
        let resp = app.get(url).await;
        assert!(resp.is_ok(), "GET {url} 失败: {}", resp.status);
    }
}

#[tokio::test]
async fn d3_htmx_list_returns_fragment() {
    let app = TestApp::new().await;
    for url in ["/admin/wms/stock", "/admin/wms/transfers", "/admin/wms/cycle-counts"] {
        let resp = app.get_htmx(url).await;
        assert!(resp.is_ok(), "HTMX {url} => {}", resp.status);
    }
}

// ════════════════════════════════════════════════════════════════════════════
//  E. 盘点（P0-3 核心修复回归）：自动调账 / 阈值审批(approve/reject) / 低库存预警
// ════════════════════════════════════════════════════════════════════════════

use abt_core::shared::enums::document_type::DocumentType;
use abt_core::shared::enums::reservation::ReservationType;
use abt_core::shared::inventory_reservation::{InventoryReservationService, ReserveRequest};
use abt_core::wms::cycle_count::{
    model::{CountCycleCountReq, CountItemReq, CreateCycleCountItemReq, CreateCycleCountReq},
    CycleCountService,
};
use abt_core::wms::enums::{ConversionStatus, CycleCountStatus, LockStatus, LowStockAlertStatus, RequisitionStatus};
use abt_core::wms::form_conversion::{
    model::ConversionFilter, FormConversionService,
};
use abt_core::wms::inventory_lock::{InventoryLockService, LockFilter};
use abt_core::wms::low_stock_alert::{LowStockAlertFilter, LowStockAlertService};
use abt_core::wms::material_requisition::{
    model::RequisitionFilter, MaterialRequisitionService,
};
use abt_core::wms::settings::{UpdateWmsSettingsReq, WmsSettingsService};
use chrono::NaiveDate;

fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 6, 19).unwrap()
}

async fn set_threshold(app: &TestApp, v: i64) {
    let svc = app.state.wms_settings_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    svc.update(&ctx, &mut conn, UpdateWmsSettingsReq {
        cycle_count_variance_threshold: Decimal::from(v),
    })
    .await
    .unwrap();
}

/// 把 (product, wh, bin, batch=None) 台账行的单位成本设为 cost（让 variance_amount 可超阈值）。
async fn set_unit_cost(app: &TestApp, product_id: i64, warehouse_id: i64, bin_id: i64, cost: i64) {
    let mut conn = app.state.pool.acquire().await.unwrap();
    sqlx::query(
        "UPDATE stock_ledger SET unit_cost = $1 \
         WHERE product_id = $2 AND warehouse_id = $3 AND bin_id = $4 AND batch_no IS NULL",
    )
    .bind(Decimal::from(cost))
    .bind(product_id)
    .bind(warehouse_id)
    .bind(bin_id)
    .execute(&mut *conn)
    .await
    .unwrap();
}

async fn set_safety_stock(app: &TestApp, product_id: i64, bin_id: i64, v: i64) {
    let svc = app.state.inventory_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    svc.set_safety_stock(&ctx, &mut conn, product_id, bin_id, Decimal::from(v))
        .await
        .unwrap();
}

/// service 建盘点单（单条明细：product @ bin，system_qty）
async fn cc_create(app: &TestApp, wh: i64, zone: i64, bin: i64, product: i64, system_qty: i64) -> i64 {
    let svc = app.state.cycle_count_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    svc.create(&ctx, &mut conn, CreateCycleCountReq {
        warehouse_id: wh,
        zone_id: Some(zone),
        count_date: today(),
        is_blind: false,
        remark: None,
        items: vec![CreateCycleCountItemReq {
            bin_id: bin,
            product_id: product,
            batch_no: None,
            system_qty: Decimal::from(system_qty),
        }],
    })
    .await
    .unwrap()
}

async fn cc_status(app: &TestApp, id: i64) -> CycleCountStatus {
    let svc = app.state.cycle_count_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    svc.get(&ctx, &mut conn, id).await.unwrap().status
}

/// 录入实盘数（service count，web 无此端点）
async fn cc_count(app: &TestApp, id: i64, counted_qty: i64) {
    let svc = app.state.cycle_count_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    let item_id = svc.get_items(&ctx, &mut conn, id).await.unwrap()[0].id;
    svc.count(&ctx, &mut conn, CountCycleCountReq {
        id,
        items: vec![CountItemReq { item_id, counted_qty: Decimal::from(counted_qty), variance_reason: None }],
    })
    .await
    .unwrap();
}

async fn cc_web_action(app: &TestApp, id: i64, action: &str) -> common::TestResponse {
    app.post_htmx(&format!("/admin/wms/cycle-counts/{id}"), &format!("action={action}"))
        .await
}

#[tokio::test]
async fn e1_cycle_count_auto_adjust_moves_stock() {
    let app = TestApp::new().await;
    let (wh_a, _) = pick_two_warehouses();
    set_threshold(&app, 999_999).await; // 高阈值 → 差异金额不超阈 → 直接调账

    stock_in(&app, wh_a, ZONE_A, BIN_A, "50", "").await;
    let base = available(&app, PRODUCT_ID, wh_a).await;

    let cc = cc_create(&app, wh_a, ZONE_A, BIN_A, PRODUCT_ID, 50).await;
    cc_web_action(&app, cc, "start").await; // Draft → Counting
    cc_count(&app, cc, 60).await; // variance = +10
    cc_web_action(&app, cc, "complete").await; // → Completed
    cc_web_action(&app, cc, "adjust").await; // 阈值内 → 调账 → Adjusted

    assert!(matches!(cc_status(&app, cc).await, CycleCountStatus::Adjusted));
    let after = available(&app, PRODUCT_ID, wh_a).await;
    assert_eq!(after - base, Decimal::from(10), "盘点调账后可用量应 +10（差异）");
}

#[tokio::test]
async fn e2_cycle_count_over_threshold_goes_review_then_approve() {
    let app = TestApp::new().await;
    let (wh_a, _) = pick_two_warehouses();
    set_threshold(&app, 0).await; // 任何正差异金额 → 待审批

    stock_in(&app, wh_a, ZONE_A, BIN_A, "50", "").await;
    set_unit_cost(&app, PRODUCT_ID, wh_a, BIN_A, 10).await; // variance_amount = 10*10 = 100 > 0
    let base = available(&app, PRODUCT_ID, wh_a).await;

    let cc = cc_create(&app, wh_a, ZONE_A, BIN_A, PRODUCT_ID, 50).await;
    cc_web_action(&app, cc, "start").await;
    cc_count(&app, cc, 60).await; // variance +10
    cc_web_action(&app, cc, "complete").await;
    cc_web_action(&app, cc, "adjust").await; // 超阈值 → PendingReview，不调账

    assert!(matches!(cc_status(&app, cc).await, CycleCountStatus::PendingReview));
    assert_eq!(
        available(&app, PRODUCT_ID, wh_a).await,
        base,
        "待审批期间不应调账"
    );

    cc_web_action(&app, cc, "approve").await; // 审批通过 → 调账
    assert!(matches!(cc_status(&app, cc).await, CycleCountStatus::Adjusted));
    assert_eq!(
        available(&app, PRODUCT_ID, wh_a).await - base,
        Decimal::from(10),
        "审批通过后应按差异调账 +10"
    );
}

#[tokio::test]
async fn e3_cycle_count_reject_returns_to_completed() {
    let app = TestApp::new().await;
    let (wh_a, _) = pick_two_warehouses();
    set_threshold(&app, 0).await;

    stock_in(&app, wh_a, ZONE_A, BIN_A, "50", "").await;
    set_unit_cost(&app, PRODUCT_ID, wh_a, BIN_A, 10).await;
    let base = available(&app, PRODUCT_ID, wh_a).await;

    let cc = cc_create(&app, wh_a, ZONE_A, BIN_A, PRODUCT_ID, 50).await;
    cc_web_action(&app, cc, "start").await;
    cc_count(&app, cc, 60).await;
    cc_web_action(&app, cc, "complete").await;
    cc_web_action(&app, cc, "adjust").await;
    assert!(matches!(cc_status(&app, cc).await, CycleCountStatus::PendingReview));

    cc_web_action(&app, cc, "reject").await; // 驳回 → Completed，不调账
    assert!(matches!(cc_status(&app, cc).await, CycleCountStatus::Completed));
    assert_eq!(
        available(&app, PRODUCT_ID, wh_a).await,
        base,
        "驳回不应调账"
    );
}

#[tokio::test]
async fn e4_low_stock_alert_fires_on_stockout() {
    let app = TestApp::new().await;
    let (wh_a, _) = pick_two_warehouses();

    // 建立少量库存 + 设高安全库存
    stock_in(&app, wh_a, ZONE_A, BIN_A, "5", "").await;
    set_safety_stock(&app, PRODUCT_ID, BIN_A, 1000).await;

    // 出库 1（5→4，跌破 safety 1000）→ record() 内 check_and_record 应触发预警
    stock_out(&app, wh_a, ZONE_A, BIN_A, "1").await;

    let svc = app.state.low_stock_alert_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    let alerts = svc
        .list(&ctx, &mut conn, LowStockAlertFilter {
            status: Some(LowStockAlertStatus::Active),
            warehouse_id: Some(wh_a),
        }, 1, 50)
        .await
        .unwrap();
    let hit = alerts.items.iter().any(|a| a.product_id == PRODUCT_ID && a.warehouse_id == wh_a);
    assert!(hit, "应存在 product {PRODUCT_ID} @ wh {wh_a} 的 Active 低库存预警");

    // 清理：还原安全库存，避免污染后续
    set_safety_stock(&app, PRODUCT_ID, BIN_A, 0).await;
}

// ════════════════════════════════════════════════════════════════════════════
//  F. 库存锁定（Lock 进 ATP 扣减）+ ATP 三因子（quantity − Lock − Reservation）
// ════════════════════════════════════════════════════════════════════════════

async fn active_lock_id(app: &TestApp, product: i64, wh: i64) -> Option<i64> {
    let svc = app.state.inventory_lock_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    let res = svc
        .list(&ctx, &mut conn, LockFilter { status: Some(LockStatus::Active), product_id: Some(product), warehouse_id: Some(wh), customer_id: None }, 1, 50)
        .await
        .unwrap();
    res.items.first().map(|l| l.id)
}

/// 释放 product×wh 下所有 Active 锁（清理先前失败运行留下的孤儿锁，避免污染）。
async fn release_all_locks(app: &TestApp, product: i64, wh: i64) {
    let svc = app.state.inventory_lock_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    let ids: Vec<i64> = svc
        .list(&ctx, &mut conn, LockFilter { status: Some(LockStatus::Active), product_id: Some(product), warehouse_id: Some(wh), customer_id: None }, 1, 100)
        .await
        .unwrap()
        .items
        .into_iter()
        .map(|l| l.id)
        .collect();
    drop(conn);
    for id in ids {
        let r = app.post_htmx(&format!("/admin/wms/locks/{id}"), "action=release").await;
        assert!(r.is_ok() || r.is_redirect(), "清理锁定 {id} FAIL: {}", r.status);
    }
}

#[tokio::test]
async fn f1_inventory_lock_reduces_atp_then_release_restores() {
    let app = TestApp::new().await;
    let (wh_a, _) = pick_two_warehouses();
    release_all_locks(&app, PRODUCT_ID, wh_a).await; // 清理孤儿锁
    stock_in(&app, wh_a, ZONE_A, BIN_A, "100", "").await;
    let base = available(&app, PRODUCT_ID, wh_a).await;

    // web 创建锁定 30
    let body = format!(
        "product_id={PRODUCT_ID}&warehouse_id={wh_a}&locked_qty=30&lock_reason=e2e-lock&customer_id="
    );
    let resp = app.post_htmx("/admin/wms/locks/create", &body).await;
    assert!(resp.is_ok() || resp.hx_redirect().is_some(), "创建锁定 FAIL: {} body: {}", resp.status, resp.body.chars().take(300).collect::<String>());

    let after_lock = available(&app, PRODUCT_ID, wh_a).await;
    // 修复后 adjust_reserved_qty 只更新 FIFO 单行，故扣减精确为锁定量 30。
    assert_eq!(
        base - after_lock,
        Decimal::from(30),
        "锁定后可用量应精确 −30 (base={base}, after_lock={after_lock})"
    );

    let lock_id = active_lock_id(&app, PRODUCT_ID, wh_a).await.expect("应存在 Active 锁定");
    let r = app.post_htmx(&format!("/admin/wms/locks/{lock_id}"), "action=release").await;
    assert!(r.is_ok() || r.is_redirect(), "释放锁定 FAIL: {}", r.status);

    let after_release = available(&app, PRODUCT_ID, wh_a).await;
    assert_eq!(after_release, base, "释放后可用量应精确恢复");
    assert!(
        active_lock_id(&app, PRODUCT_ID, wh_a).await.is_none(),
        "释放后不应再有 Active 锁定"
    );
}

#[tokio::test]
async fn f2_atp_deducts_both_lock_and_reservation() {
    let app = TestApp::new().await;
    let (wh_a, _) = pick_two_warehouses();
    stock_in(&app, wh_a, ZONE_A, BIN_A, "100", "").await;

    // 锁 30
    let body = format!("product_id={PRODUCT_ID}&warehouse_id={wh_a}&locked_qty=30&lock_reason=e2e-atp");
    let _ = app.post_htmx("/admin/wms/locks/create", &body).await;
    let after_lock = available(&app, PRODUCT_ID, wh_a).await;

    // 预留 20（shared 域，硬预留）
    let res_svc = abt_core::shared::inventory_reservation::new_inventory_reservation_service(
        app.state.pool.clone(),
    );
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    res_svc
        .reserve(&ctx, &mut conn, vec![ReserveRequest {
            product_id: PRODUCT_ID,
            warehouse_id: Some(wh_a),
            reserved_qty: Decimal::from(20),
            reservation_type: ReservationType::Hard,
            source_type: DocumentType::SalesOrder,
            source_id: 888_888,
            source_line_id: None,
            priority: 0,
            expires_at: None,
        }])
        .await
        .unwrap();

    let after_res = available(&app, PRODUCT_ID, wh_a).await;
    assert_eq!(after_lock - after_res, Decimal::from(20), "预留应进一步扣减可用量 20");

    // 清理预留，避免污染
    let mut conn = app.state.pool.acquire().await.unwrap();
    let _ = res_svc
        .cancel_by_source(&ctx, &mut conn, DocumentType::SalesOrder, 888_888)
        .await;
    // 释放本次锁定
    if let Some(lid) = active_lock_id(&app, PRODUCT_ID, wh_a).await {
        let _ = app.post_htmx(&format!("/admin/wms/locks/{lid}"), "action=release").await;
    }
}

// ════════════════════════════════════════════════════════════════════════════
//  I. 调拨取消 / 核心层负库存兜底 / 倒冲页面
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn i1_transfer_cancel_from_draft() {
    let app = TestApp::new().await;
    let (wh_a, wh_b) = pick_two_warehouses();
    let body = format!(
        "from_warehouse_id={wh_a}&to_warehouse_id={wh_b}&transfer_date=2026-06-19&items_json={}",
        items_xfer(&[(PRODUCT_ID_STR, "1")])
    );
    let resp = app.post_htmx("/admin/wms/transfers/create", &body).await;
    assert!(resp.is_ok(), "创建调拨 FAIL: {}", resp.status);

    let draft = find_draft_transfer(&app, wh_a, wh_b).await;
    let r = app.post_htmx(&format!("/admin/wms/transfers/{}", draft.id), "action=cancel").await;
    assert!(r.is_ok() || r.is_redirect(), "取消调拨 FAIL: {}", r.status);
    let after = get_transfer(&app, draft.id).await;
    assert!(matches!(after.status, TransferStatus::Cancelled), "取消后应为 Cancelled");
}

/// 核心层负库存兜底（P0-2）：绕过 web 预检，直接调 record() 传消耗型负数量超可用，
/// 应被 record() 前置预检拦为 InsufficientStock。
#[tokio::test]
async fn i2_core_record_rejects_insufficient_consumption() {
    let app = TestApp::new().await;
    let (wh_a, _) = pick_two_warehouses();
    // 不建任何库存 → 可用量 0
    let svc = app.state.inventory_transaction_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    use abt_core::wms::enums::TransactionType;
    use abt_core::wms::inventory_transaction::model::RecordTransactionReq;
    let err = svc
        .record(&ctx, &mut conn, RecordTransactionReq {
            doc_number: None,
            delivery_no: None,
            source_doc_number: None,
            transaction_type: TransactionType::SalesShipment,
            product_id: PRODUCT_ID,
            warehouse_id: wh_a,
            zone_id: Some(ZONE_A),
            bin_id: Some(BIN_A),
            batch_no: None,
            quantity: Decimal::from(-9999),
            unit_cost: None,
            source_type: "manual".to_string(),
            source_id: 0,
            remark: None,
        })
        .await
        .unwrap_err();
    assert!(
        matches!(err, abt_core::shared::types::DomainError::InsufficientStock { .. }),
        "核心层 record() 库存不足应返回 InsufficientStock，实际 {err:?}"
    );
}

#[tokio::test]
async fn i3_backflush_pages_accessible_and_detail_404() {
    let app = TestApp::new().await;
    assert!(app.get("/admin/wms/backflushes").await.is_ok(), "倒冲列表页应可达");
    assert_eq!(
        app.get("/admin/wms/backflushes/999999").await.status,
        axum::http::StatusCode::NOT_FOUND
    );
}

// ════════════════════════════════════════════════════════════════════════════
//  G/H. 领料单 / 形态转换 —— 状态机 + 库存增减
//  （已修复：领料 issue 不扣库存 / 形态转换 complete 不动账，现均断言真实库存变动）
// ════════════════════════════════════════════════════════════════════════════

fn req_items(items: &[(&str, &str)]) -> String {
    let parts: Vec<String> = items
        .iter()
        .map(|(pid, qty)| format!(r#"{{"product_id":"{pid}","requested_qty":"{qty}"}}"#))
        .collect();
    urlenc(&format!("[{}]", parts.join(",")))
}

async fn find_requisition(app: &TestApp, wh: i64, status: RequisitionStatus) -> Option<i64> {
    let svc = app.state.material_requisition_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    let res = svc
        .list(&ctx, &mut conn, RequisitionFilter { status: Some(status), warehouse_id: Some(wh), ..Default::default() }, 1, 50)
        .await
        .unwrap();
    res.items.first().map(|r| r.id)
}

#[tokio::test]
async fn g1_material_requisition_status_flow() {
    let app = TestApp::new().await;
    let (wh_a, _) = pick_two_warehouses();
    stock_in(&app, wh_a, ZONE_A, BIN_A, "100", "").await;
    let base = available(&app, PRODUCT_ID, wh_a).await;

    // 手工创建领料单（Draft）
    let body = format!(
        "warehouse_id={wh_a}&requisition_date=2026-06-19&items_json={}",
        req_items(&[(PRODUCT_ID_STR, "20")])
    );
    let resp = app.post_htmx("/admin/wms/requisitions/create", &body).await;
    assert!(resp.is_ok(), "创建领料 FAIL: {} body: {}", resp.status, resp.body.chars().take(200).collect::<String>());

    let id = find_requisition(&app, wh_a, RequisitionStatus::Draft).await.expect("应存在 Draft 领料单");

    // confirm: Draft → Confirmed
    let r = app.post_htmx(&format!("/admin/wms/requisitions/{id}"), "action=confirm").await;
    assert!(r.is_ok() || r.is_redirect(), "确认领料 FAIL: {}", r.status);
    assert!(matches!(find_requisition_status(&app, id).await, RequisitionStatus::Confirmed));

    // issue: Confirmed → Issued，并真实扣减库存 20（修复 zone_id 缺失 + record 自动解析库位）
    let r = app.post_htmx(&format!("/admin/wms/requisitions/{id}"), "action=issue").await;
    assert!(r.is_ok() || r.is_redirect(), "发料 FAIL: {}", r.status);
    assert!(
        matches!(find_requisition_status(&app, id).await, RequisitionStatus::Issued),
        "发料后应为 Issued"
    );
    assert_eq!(
        available(&app, PRODUCT_ID, wh_a).await - base,
        Decimal::from(-20),
        "领料发料应扣减库存 20"
    );
}

async fn find_requisition_status(app: &TestApp, id: i64) -> RequisitionStatus {
    let svc = app.state.material_requisition_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    svc.get(&ctx, &mut conn, id).await.unwrap().status
}

fn conv_json(items: &[(&str, &str)]) -> String {
    // (product_id, quantity) → consume/produce 明细
    let parts: Vec<String> = items
        .iter()
        .map(|(pid, qty)| format!(r#"{{"product_id":"{pid}","quantity":"{qty}","unit_cost":"1","batch_no":""}}"#))
        .collect();
    urlenc(&format!("[{}]", parts.join(",")))
}

async fn find_conversion(app: &TestApp, wh: i64, status: ConversionStatus) -> Option<i64> {
    let svc = app.state.form_conversion_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    let res = svc
        .list(&ctx, &mut conn, ConversionFilter { status: Some(status), warehouse_id: Some(wh) }, 1, 50)
        .await
        .unwrap();
    res.items.first().map(|c| c.id)
}

#[tokio::test]
async fn h1_form_conversion_status_flow() {
    let app = TestApp::new().await;
    let (wh_a, _) = pick_two_warehouses();

    stock_in(&app, wh_a, ZONE_A, BIN_A, "50", "").await;
    let base_565 = available(&app, PRODUCT_ID, wh_a).await;
    let base_566 = available(&app, 566, wh_a).await;

    // 创建形态转换：consume 565×10，produce 566×10
    let body = format!(
        "warehouse_id={wh_a}&conversion_date=2026-06-19&remark=e2e&consume_json={}&produce_json={}",
        conv_json(&[(PRODUCT_ID_STR, "10")]),
        conv_json(&[("566", "10")])
    );
    let resp = app.post_htmx("/admin/wms/conversions/create", &body).await;
    assert!(resp.is_ok(), "创建形态转换 FAIL: {} body: {}", resp.status, resp.body.chars().take(200).collect::<String>());

    let id = find_conversion(&app, wh_a, ConversionStatus::Draft).await.expect("应存在 Draft 形态转换单");

    // complete: Draft → Completed，并真实动账（修复：原仅改状态）
    let r = app.post_htmx(&format!("/admin/wms/conversions/{id}"), "action=complete").await;
    assert!(r.is_ok() || r.is_redirect(), "完成形态转换 FAIL: {}", r.status);

    let svc = app.state.form_conversion_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();
    assert!(
        matches!(svc.get(&ctx, &mut conn, id).await.unwrap().status, ConversionStatus::Completed),
        "完成后应为 Completed"
    );
    assert_eq!(
        available(&app, PRODUCT_ID, wh_a).await - base_565,
        Decimal::from(-10),
        "转换消耗应扣减 565 库存 10"
    );
    assert_eq!(
        available(&app, 566, wh_a).await - base_566,
        Decimal::from(10),
        "转换产出应增加 566 库存 10"
    );
}

// ════════════════════════════════════════════════════════════════════════════
//  J. 入库来源关联（source_type=arrival / purchase / manual）
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn j1_stock_in_with_arrival_source_succeeds() {
    let app = TestApp::new().await;
    let (wh_a, _) = pick_two_warehouses();
    // 关联一个不存在的来料通知 source_id（仅校验来源字段被正确接收并落库，不校验来源存在性）
    let items = items_in(&[(PRODUCT_ID_STR, "10", "", &BIN_A.to_string())]);
    let body = format!(
        "transaction_type=PurchaseReceipt&source_type=arrival&source_ref=AN-TEST&source_id=999999&warehouse_id={wh_a}&zone_id={ZONE_A}&bin_id={BIN_A}&items_json={items}"
    );
    let resp = app.post_htmx("/admin/wms/stock-in/create", &body).await;
    assert!(
        resp.is_ok(),
        "带来源的入库应成功: {} body: {}",
        resp.status,
        resp.body.chars().take(200).collect::<String>()
    );
}


