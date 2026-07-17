/// 领域事件
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum DomainEventType {
    // Sales
    SalesOrderConfirmed = 1,
    SalesOrderShipped = 2,
    SalesReturnReceived = 3,
    // Purchase
    PurchaseOrderConfirmed = 4,
    ArrivalReceived = 5,
    PaymentPaid = 6,
    // MES
    PlanReleased = 7,
    WOReleased = 8,
    WOCompleted = 9,
    ReceiptConfirmed = 10,
    // OM
    OutsourcingSent = 11,
    OutsourcingReceived = 12,
    ConvertedToInternal = 13,
    // QMS
    InspectionPassed = 14,
    InspectionFailed = 15,
    MRBDispositioned = 16,
    RMACreated = 17,
    // FMS
    CashJournalConfirmed = 18,
    WriteOffCompleted = 19,
    // Master Data — BOM
    BomPublished = 20,
    BomUnpublished = 21,
    BomNodeAdded = 22,
    BomNodeUpdated = 23,
    BomNodeDeleted = 24,
    BomSubstituted = 25,
    // Master Data — Product
    ProductStatusChanged = 26,
    // Master Data — Customer
    CustomerCreated = 27,
    CustomerBlacklisted = 28,
    CustomerTransferred = 29,
    // Master Data — Supplier
    SupplierCreated = 30,
    SupplierBlacklisted = 31,
    SupplierBankAccountChanged = 32,
    // Sales — Quotation
    QuotationCreated = 33,
    QuotationSubmitted = 34,
    QuotationAccepted = 35,
    QuotationRejected = 36,
    QuotationExpired = 37,
    // Sales — Order
    SalesOrderCreated = 38,
    SalesOrderCancelled = 39,
    // Sales — Shipping
    ShipmentShipped = 40,
    // Purchase — additional events
    PurchaseQuotationActivated = 41,
    MiscellaneousRequestApproved = 42,
    PurchaseReturnConfirmed = 43,
    PurchaseReconciliationConfirmed = 44,
    PaymentRequestApproved = 45,
    PurchaseReturnSettled = 46,
    // OM — Outsourcing
    OutsourcingCancelled = 47,
    // Master Data — 工艺路线子域
    RoutingCreated = 49,
    RoutingUpdated = 50,
    RoutingDeleted = 51,
    BomRoutingChanged = 52,
    LaborProcessDictCreated = 53,
    LaborProcessDictUpdated = 54,
    LaborProcessDictDeleted = 55,
    // Master Data — Product lifecycle
    ProductCreated = 56,
    ProductUpdated = 57,
    ProductDeleted = 58,
    // H3Yun — Inventory sync
    H3YunInventorySync = 59,
    // Sales — soft delete
    SalesOrderDeleted = 60,
    QuotationDeleted = 61,
    // Purchase — order cancelled
    PurchaseOrderCancelled = 62,
    // Purchase — return cancelled
    PurchaseReturnCancelled = 63,
    // Sales — Demand
    DemandCreated = 64,
    DemandConfirmed = 65,
    DemandRejected = 66,
    // MES — Work Order lifecycle
    WOUnreleased = 67,
    // WMS — Arrival notice inspection passed
    ArrivalInspected = 68,
    // WMS — 盘点差异超阈值，请求审批
    CycleCountReviewRequested = 69,
    // WMS — 安全库存预警
    LowStockAlert = 70,
    // FMS — 应收应付调整单过账
    ArApAdjustmentPosted = 71,
    // Sales — Demand 释放回池（下游工单/采购单取消，需求重新可用）
    DemandReleased = 72,
    // OM — 委外单创建（跨模块联动挂载点：采购/仓库作业中心待办聚合）
    OutsourcingCreated = 73,
}

impl DomainEventType {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::SalesOrderConfirmed), 2 => Some(Self::SalesOrderShipped),
            3 => Some(Self::SalesReturnReceived), 4 => Some(Self::PurchaseOrderConfirmed),
            5 => Some(Self::ArrivalReceived), 6 => Some(Self::PaymentPaid),
            7 => Some(Self::PlanReleased), 8 => Some(Self::WOReleased),
            9 => Some(Self::WOCompleted), 10 => Some(Self::ReceiptConfirmed),
            11 => Some(Self::OutsourcingSent), 12 => Some(Self::OutsourcingReceived),
            13 => Some(Self::ConvertedToInternal), 14 => Some(Self::InspectionPassed),
            15 => Some(Self::InspectionFailed), 16 => Some(Self::MRBDispositioned),
            17 => Some(Self::RMACreated), 18 => Some(Self::CashJournalConfirmed),
            19 => Some(Self::WriteOffCompleted),
            20 => Some(Self::BomPublished), 21 => Some(Self::BomUnpublished),
            22 => Some(Self::BomNodeAdded), 23 => Some(Self::BomNodeUpdated),
            24 => Some(Self::BomNodeDeleted), 25 => Some(Self::BomSubstituted),
            26 => Some(Self::ProductStatusChanged),
            27 => Some(Self::CustomerCreated), 28 => Some(Self::CustomerBlacklisted),
            29 => Some(Self::CustomerTransferred),
            30 => Some(Self::SupplierCreated), 31 => Some(Self::SupplierBlacklisted),
            32 => Some(Self::SupplierBankAccountChanged),
            33 => Some(Self::QuotationCreated), 34 => Some(Self::QuotationSubmitted),
            35 => Some(Self::QuotationAccepted), 36 => Some(Self::QuotationRejected),
            37 => Some(Self::QuotationExpired),
            38 => Some(Self::SalesOrderCreated), 39 => Some(Self::SalesOrderCancelled),
            40 => Some(Self::ShipmentShipped),
            41 => Some(Self::PurchaseQuotationActivated),
            42 => Some(Self::MiscellaneousRequestApproved),
            43 => Some(Self::PurchaseReturnConfirmed),
            44 => Some(Self::PurchaseReconciliationConfirmed),
            45 => Some(Self::PaymentRequestApproved),
            46 => Some(Self::PurchaseReturnSettled),
            47 => Some(Self::OutsourcingCancelled),
            49 => Some(Self::RoutingCreated), 50 => Some(Self::RoutingUpdated),
            51 => Some(Self::RoutingDeleted), 52 => Some(Self::BomRoutingChanged),
            53 => Some(Self::LaborProcessDictCreated), 54 => Some(Self::LaborProcessDictUpdated),
            55 => Some(Self::LaborProcessDictDeleted),
            56 => Some(Self::ProductCreated), 57 => Some(Self::ProductUpdated),
            58 => Some(Self::ProductDeleted), 59 => Some(Self::H3YunInventorySync),
            60 => Some(Self::SalesOrderDeleted), 61 => Some(Self::QuotationDeleted),
            62 => Some(Self::PurchaseOrderCancelled), 63 => Some(Self::PurchaseReturnCancelled),
            64 => Some(Self::DemandCreated),
            65 => Some(Self::DemandConfirmed),
            66 => Some(Self::DemandRejected),
            67 => Some(Self::WOUnreleased),
            68 => Some(Self::ArrivalInspected),
            69 => Some(Self::CycleCountReviewRequested),
            70 => Some(Self::LowStockAlert),
            71 => Some(Self::ArApAdjustmentPosted),
            72 => Some(Self::DemandReleased),
            73 => Some(Self::OutsourcingCreated),
            _ => None,
        }
    }
    pub fn as_i16(self) -> i16 { self as i16 }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum EventStatus {
    Pending = 1,
    Processing = 2,
    Processed = 3,
    Failed = 4,
    DeadLetter = 5,
}

impl EventStatus {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Pending), 2 => Some(Self::Processing),
            3 => Some(Self::Processed), 4 => Some(Self::Failed),
            5 => Some(Self::DeadLetter), _ => None,
        }
    }
    pub fn as_i16(self) -> i16 { self as i16 }
}

impl sqlx::Type<sqlx::Postgres> for DomainEventType {
    fn type_info() -> sqlx::postgres::PgTypeInfo { <i16 as sqlx::Type<sqlx::Postgres>>::type_info() }
}
impl sqlx::Encode<'_, sqlx::Postgres> for DomainEventType {
    fn encode_by_ref(&self, buf: &mut sqlx::postgres::PgArgumentBuffer) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        <i16 as sqlx::Encode<'_, sqlx::Postgres>>::encode_by_ref(&self.as_i16(), buf)
    }
}
impl sqlx::Decode<'_, sqlx::Postgres> for DomainEventType {
    fn decode(value: sqlx::postgres::PgValueRef<'_>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let v = <i16 as sqlx::Decode<'_, sqlx::Postgres>>::decode(value)?;
        Self::from_i16(v).ok_or_else(|| format!("unknown DomainEventType: {v}").into())
    }
}

impl sqlx::Type<sqlx::Postgres> for EventStatus {
    fn type_info() -> sqlx::postgres::PgTypeInfo { <i16 as sqlx::Type<sqlx::Postgres>>::type_info() }
}
impl sqlx::Encode<'_, sqlx::Postgres> for EventStatus {
    fn encode_by_ref(&self, buf: &mut sqlx::postgres::PgArgumentBuffer) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        <i16 as sqlx::Encode<'_, sqlx::Postgres>>::encode_by_ref(&self.as_i16(), buf)
    }
}
impl sqlx::Decode<'_, sqlx::Postgres> for EventStatus {
    fn decode(value: sqlx::postgres::PgValueRef<'_>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let v = <i16 as sqlx::Decode<'_, sqlx::Postgres>>::decode(value)?;
        Self::from_i16(v).ok_or_else(|| format!("unknown EventStatus: {v}").into())
    }
}

impl serde::Serialize for DomainEventType {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> { s.serialize_i16(self.as_i16()) }
}
impl<'de> serde::Deserialize<'de> for DomainEventType {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = i16::deserialize(d)?;
        Self::from_i16(v).ok_or_else(|| serde::de::Error::custom(format!("unknown DomainEventType: {v}")))
    }
}

impl serde::Serialize for EventStatus {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> { s.serialize_i16(self.as_i16()) }
}
impl<'de> serde::Deserialize<'de> for EventStatus {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = i16::deserialize(d)?;
        Self::from_i16(v).ok_or_else(|| serde::de::Error::custom(format!("unknown EventStatus: {v}")))
    }
}
