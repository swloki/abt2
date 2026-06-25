use rust_decimal::Decimal;

/// 采购订单直收入库 — 单行明细（按 order_item_id 精确累加 received_qty）
#[derive(Debug, Clone)]
pub struct PoStockInRow {
    /// 采购订单明细行 id（received_qty 累加定位）
    pub order_item_id: i64,
    pub product_id: i64,
    /// 本次实收量
    pub received_qty: Decimal,
    pub batch_no: Option<String>,
    /// 目标仓库（用户在 drawer 选）
    pub warehouse_id: i64,
    /// 目标库位（None → 仓库默认库位）
    pub bin_id: Option<i64>,
}

/// 采购订单直收入库请求（work-center 收货 drawer / stock-in/create 采购分支共用）
#[derive(Debug, Clone)]
pub struct ReceiveAndStockInReq {
    pub po_id: i64,
    pub rows: Vec<PoStockInRow>,
    pub delivery_note: Option<String>,
    pub remark: Option<String>,
    /// 幂等键（防双击/重试重复入库）；None 跳过幂等
    pub idempotency_key: Option<String>,
}
