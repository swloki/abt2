use axum::routing::{get, post};
use axum::Router;
use axum_extra::routing::TypedPath;
use serde::Deserialize;

use crate::pages::fms_work_center;
use crate::state::AppState;

// ── TypedPath definitions ──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/fms/work-center")]
pub struct FmsWorkCenterPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/fms/work-center/receivables")]
pub struct FcReceivablesPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/fms/work-center/payables")]
pub struct FcPayablesPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/fms/work-center/ar-adjustments")]
pub struct FcArAdjustmentsPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/fms/work-center/ap-adjustments")]
pub struct FcApAdjustmentsPath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/fms/work-center/settlements")]
pub struct FcSettlementsPath;

// ── Drawer body（GET）──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/fms/work-center/receivables/{id}/receipt-drawer")]
pub struct FcReceiptDrawerPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/fms/work-center/payables/{id}/payment-drawer")]
pub struct FcPaymentDrawerPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/fms/work-center/ledger/{id}/detail-drawer")]
pub struct FcLedgerDetailDrawerPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/fms/work-center/settle/{party_type}/{party_id}/drawer")]
pub struct FcSettleDrawerPath {
    pub party_type: i16,
    pub party_id: i64,
}

// ── 写操作（POST）──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/fms/work-center/receivables/{id}/receipt")]
pub struct FcJournalReceiptPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/fms/work-center/payables/{id}/payment")]
pub struct FcJournalPaymentPath {
    pub id: i64,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/fms/work-center/settlements/settle")]
pub struct FcSettlePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/fms/work-center/unsettle/{id}")]
pub struct FcSettlementUnsettlePath {
    pub id: i64,
}

// ── 调整创建 drawer（#190：a href 改 drawer 就地操作）──

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/fms/work-center/adjustment/{party_type}/drawer")]
pub struct FcAdjustmentDrawerPath {
    pub party_type: i16,
}

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/fms/work-center/adjustment/create")]
pub struct FcAdjustmentCreatePath;

#[derive(TypedPath, Deserialize, Clone)]
#[typed_path("/admin/fms/work-center/adjustment/balance")]
pub struct FcAdjustmentBalancePath;

// ── Router ──

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            FmsWorkCenterPath::PATH,
            get(fms_work_center::get_work_center),
        )
        .route(
            FcReceivablesPath::PATH,
            get(fms_work_center::get_receivables_card),
        )
        .route(FcPayablesPath::PATH, get(fms_work_center::get_payables_card))
        .route(
            FcArAdjustmentsPath::PATH,
            get(fms_work_center::get_ar_adjustments_card),
        )
        .route(
            FcApAdjustmentsPath::PATH,
            get(fms_work_center::get_ap_adjustments_card),
        )
        .route(
            FcSettlementsPath::PATH,
            get(fms_work_center::get_settlements_card),
        )
        // drawer body（GET）
        .route(
            FcReceiptDrawerPath::PATH,
            get(fms_work_center::get_receipt_drawer),
        )
        .route(
            FcPaymentDrawerPath::PATH,
            get(fms_work_center::get_payment_drawer),
        )
        .route(
            FcLedgerDetailDrawerPath::PATH,
            get(fms_work_center::get_ledger_detail_drawer),
        )
        .route(
            FcSettleDrawerPath::PATH,
            get(fms_work_center::get_settle_drawer),
        )
        .route(
            FcAdjustmentDrawerPath::PATH,
            get(fms_work_center::get_adjustment_drawer),
        )
        .route(
            FcAdjustmentBalancePath::PATH,
            get(fms_work_center::get_adjustment_balance),
        )
        // 写操作（POST）
        .route(
            FcJournalReceiptPath::PATH,
            post(fms_work_center::create_receipt),
        )
        .route(
            FcJournalPaymentPath::PATH,
            post(fms_work_center::create_payment),
        )
        .route(FcSettlePath::PATH, post(fms_work_center::settle))
        .route(
            FcSettlementUnsettlePath::PATH,
            post(fms_work_center::unsettle),
        )
        .route(
            FcAdjustmentCreatePath::PATH,
            post(fms_work_center::create_adjustment),
        )
}
