//! Issue #98 回归测试：合并销售单生成的采购单入库后流水丢失。
//!
//! 根因：record() 的「一库位一产品」硬校验与默认库位多产品混放的现实冲突——
//! 用户按 suggest_bins 选中混放默认库位入库 → 校验失败 → 整个入库事务回滚 → 零流水。
//! 修复：同物料合并放行（目标产品已在该 bin 有库存时不拒绝），仅阻止新料混入。
//!
//! - i1 同物料合并放行：bin 已混放多产品（含目标产品），继续入库目标产品 → 放行（修复前会失败）
//! - i2 新料混入拒绝：bin 已被其他产品占用，入库全新产品 → 拒绝（保持一库位一产品排他）
//! - i3 入库流水查询：doc_number 搜索同时匹配 RK 号和 PO 来源号（修复前只匹配 RK）

mod common;
use common::TestApp;

use rust_decimal::Decimal;
use abt_core::shared::types::ServiceContext;
use abt_core::wms::enums::TransactionType;
use abt_core::wms::inventory_transaction::{
    model::{RecordTransactionReq, TransactionFilter},
    InventoryTransactionService,
};

/// 找一个"多产品混放"的 bin，返回 (bin_id, warehouse_id, zone_id, prod_in_bin, prod_not_in_bin)。
/// prod_in_bin 在该 bin 有库存（同物料合并测试用）；prod_not_in_bin 不在该 bin（新料混入测试用）。
async fn find_mixed_bin(app: &TestApp) -> (i64, i64, i64, i64, i64) {
    let mut conn = app.state.pool.acquire().await.unwrap();
    // 混放 bin：同一 bin 下 ≥2 个不同产品（默认库位 DEFAULT-* 常见此情况）
    let (bin_id, wh_id, zone_id, prod_in_bin): (i64, i64, i64, i64) = sqlx::query_as(
        r#"
        SELECT s.bin_id, s.warehouse_id, COALESCE(s.zone_id, 0), s.product_id
        FROM stock_ledger s
        WHERE s.quantity > 0
          AND (SELECT COUNT(DISTINCT s2.product_id) FROM stock_ledger s2
               WHERE s2.bin_id = s.bin_id AND s2.quantity > 0) >= 2
        LIMIT 1
        "#,
    )
    .fetch_one(&mut *conn)
    .await
    .expect("❌ dev DB 无混放 bin（需预置默认库位多产品混放数据）");
    // 找一个不在该 bin 的产品（任意其他有库存的产品）
    let prod_not_in_bin: i64 = sqlx::query_scalar(
        "SELECT product_id FROM stock_ledger WHERE quantity > 0 AND bin_id <> $1 LIMIT 1",
    )
    .bind(bin_id)
    .fetch_one(&mut *conn)
    .await
    .expect("❌ 找不到该 bin 之外的产品");
    (bin_id, wh_id, zone_id, prod_in_bin, prod_not_in_bin)
}

// ════════════════════════════════════════════════════════════════════════════
//  i1 同物料合并放行（Issue #98 根因修复验证）
//  bin 已混放多产品（含 prod_in_bin），继续入库 prod_in_bin → 应放行
//  修复前：find_other_occupant_in_bin 命中其他产品 → BusinessRule → 失败
//  修复后：has_stock_in_bin(prod_in_bin)=true → 跳过占用校验 → 放行
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
#[serial_test::serial]
async fn i1_same_product_merge_into_mixed_bin_allowed() {
    let app = TestApp::new().await;
    let (bin_id, wh_id, zone_id, prod_in_bin, _) = find_mixed_bin(&app).await;
    let svc = app.state.inventory_transaction_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();

    let zone_arg = (zone_id != 0).then_some(zone_id);
    let result = svc
        .record(
            &ctx,
            &mut conn,
            RecordTransactionReq {
                doc_number: Some(format!("RK-TEST-98I1-{}", chrono::Utc::now().timestamp())),
                delivery_no: None,
                source_doc_number: Some(format!("PO-TEST-98I1-{}", chrono::Utc::now().timestamp())),
                transaction_type: TransactionType::PurchaseReceipt,
                product_id: prod_in_bin,
                warehouse_id: wh_id,
                zone_id: zone_arg,
                bin_id: Some(bin_id),
                batch_no: None,
                quantity: Decimal::from(1),
                unit_cost: None,
                source_type: "test_i1".into(),
                source_id: 0,
                remark: None,
            },
        )
        .await;

    assert!(
        result.is_ok(),
        "❌ 同物料合并入库到混放 bin 应放行（Issue #98 修复），实际: {}",
        result
            .err()
            .map(|e| format!("{e:?}"))
            .unwrap_or_default()
    );
}

// ════════════════════════════════════════════════════════════════════════════
//  i2 新料混入拒绝（一库位一产品排他仍生效）
//  bin 已被其他产品占用，入库 prod_not_in_bin → 应拒绝
//  确保「同物料合并放行」没有过度放宽到允许新料混入
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
#[serial_test::serial]
async fn i2_new_product_into_occupied_bin_rejected() {
    let app = TestApp::new().await;
    let (bin_id, wh_id, zone_id, _, prod_not_in_bin) = find_mixed_bin(&app).await;
    let svc = app.state.inventory_transaction_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();

    let zone_arg = (zone_id != 0).then_some(zone_id);
    let result = svc
        .record(
            &ctx,
            &mut conn,
            RecordTransactionReq {
                doc_number: Some(format!("RK-TEST-98I2-{}", chrono::Utc::now().timestamp())),
                delivery_no: None,
                source_doc_number: Some(format!("PO-TEST-98I2-{}", chrono::Utc::now().timestamp())),
                transaction_type: TransactionType::PurchaseReceipt,
                product_id: prod_not_in_bin,
                warehouse_id: wh_id,
                zone_id: zone_arg,
                bin_id: Some(bin_id),
                batch_no: None,
                quantity: Decimal::from(1),
                unit_cost: None,
                source_type: "test_i2".into(),
                source_id: 0,
                remark: None,
            },
        )
        .await;

    let err_dbg = format!("{:?}", result.as_ref().err());
    assert!(
        result.is_err(),
        "❌ 新料混入已占用 bin 应被拒绝（保持一库位一产品排他）"
    );
    assert!(
        err_dbg.contains("库位已被其他产品占用") || err_dbg.contains("BusinessRule"),
        "❌ 应为库位占用 BusinessRule，实际: {err_dbg}"
    );
}

// ════════════════════════════════════════════════════════════════════════════
//  i3 入库流水查询匹配来源 PO 号（Issue #98 叠加查询 bug 修复验证）
//  入库一条 source_doc_number=PO-TEST-98I3-xxx 的流水，用该 PO 号作为搜索词查询 → 应命中
//  修复前：doc_number ILIKE 只匹配 RK 入库单号，PO 号搜不到
//  修复后：(doc_number ILIKE OR source_doc_number ILIKE)，PO 号可搜
// ════════════════════════════════════════════════════════════════════════════

#[tokio::test]
#[serial_test::serial]
async fn i3_stock_in_query_matches_source_doc_number() {
    let app = TestApp::new().await;
    let svc = app.state.inventory_transaction_service();
    let ctx = ServiceContext::new(1);
    let mut conn = app.state.pool.acquire().await.unwrap();

    // 用 prod_in_bin 入库（同物料合并放行，确保入库成功）
    let (bin_id, wh_id, zone_id, prod_in_bin, _) = find_mixed_bin(&app).await;
    let zone_arg = (zone_id != 0).then_some(zone_id);
    let po_doc = format!("PO-TEST-98I3-{}", chrono::Utc::now().timestamp());
    svc.record(
        &ctx,
        &mut conn,
        RecordTransactionReq {
            doc_number: Some(format!("RK-TEST-98I3-{}", chrono::Utc::now().timestamp())),
            delivery_no: None,
            source_doc_number: Some(po_doc.clone()),
            transaction_type: TransactionType::PurchaseReceipt,
            product_id: prod_in_bin,
            warehouse_id: wh_id,
            zone_id: zone_arg,
            bin_id: Some(bin_id),
            batch_no: None,
            quantity: Decimal::from(1),
            unit_cost: None,
            source_type: "test_i3".into(),
            source_id: 0,
            remark: None,
        },
    )
    .await
    .expect("setup 入库 FAIL");

    // 用 PO 号（source_doc_number）作为 doc_number 搜索词 → 应命中刚插入的流水
    let result = svc
        .query(
            &ctx,
            &mut conn,
            TransactionFilter {
                doc_number: Some(po_doc.clone()),
                ..Default::default()
            },
            1,
            20,
        )
        .await
        .expect("query FAIL");

    let found = result
        .items
        .iter()
        .any(|t| t.source_doc_number.as_deref() == Some(po_doc.as_str()));
    assert!(
        found,
        "❌ 用 PO 号搜索应匹配 source_doc_number（Issue #98 查询 bug 修复）"
    );
}
