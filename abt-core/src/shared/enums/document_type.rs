/// 单据类型 — 覆盖 8 个业务模块的 33 种单据
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum DocumentType {
    // Sales CRM
    Quotation = 1,
    SalesOrder = 2,
    ShippingRequest = 3,
    SalesReturn = 4,
    Reconciliation = 5,
    // Purchase SRM
    PurchaseQuotation = 6,
    PurchaseOrder = 7,
    PurchaseReturn = 8,
    MiscellaneousRequest = 9,
    // MES
    WorkOrder = 10,
    OutsourcingOrder = 11,
    ProductionPlan = 12,
    WorkReport = 13,
    ProductionInspection = 14,
    ProductionReceipt = 15,
    // WMS
    ArrivalNotice = 16,
    MaterialRequisition = 17,
    Backflush = 18,
    CycleCount = 19,
    InventoryTransfer = 20,
    FormConversion = 21,
    InventoryLock = 22,
    PaymentRequest = 23,
    Invoice = 24,
    // OM 委外
    OutsourcingTracking = 25,
    // QMS 质量
    InspectionSpecification = 26,
    InspectionResult = 27,
    Mrb = 28,
    Rma = 29,
    // FMS 财务
    CashJournal = 30,
    WriteOff = 31,
    ExpenseReimbursement = 32,
    // Master Data
    Product = 33,
    Customer = 34,
    Supplier = 35,
    Bom = 36,
    // Purchase SRM — Reconciliation (separate from Sales)
    PurchaseReconciliation = 37,
    // Master Data — 工艺路线子域
    LaborProcessDict = 38,
    Routing = 39,
    BomLaborProcess = 40,
    // MES — 生产批次
    ProductionBatch = 41,
    // MES — 流转卡
    FlowCard = 42,
    // WMS — 入库单
    StockReceipt = 43,
    // WMS — 出库单
    StockShipment = 44,
    // FMS 财务 — 台账调整
    LedgerAdjustment = 45,
}

impl DocumentType {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Quotation),
            2 => Some(Self::SalesOrder),
            3 => Some(Self::ShippingRequest),
            4 => Some(Self::SalesReturn),
            5 => Some(Self::Reconciliation),
            6 => Some(Self::PurchaseQuotation),
            7 => Some(Self::PurchaseOrder),
            8 => Some(Self::PurchaseReturn),
            9 => Some(Self::MiscellaneousRequest),
            10 => Some(Self::WorkOrder),
            11 => Some(Self::OutsourcingOrder),
            12 => Some(Self::ProductionPlan),
            13 => Some(Self::WorkReport),
            14 => Some(Self::ProductionInspection),
            15 => Some(Self::ProductionReceipt),
            16 => Some(Self::ArrivalNotice),
            17 => Some(Self::MaterialRequisition),
            18 => Some(Self::Backflush),
            19 => Some(Self::CycleCount),
            20 => Some(Self::InventoryTransfer),
            21 => Some(Self::FormConversion),
            22 => Some(Self::InventoryLock),
            23 => Some(Self::PaymentRequest),
            24 => Some(Self::Invoice),
            25 => Some(Self::OutsourcingTracking),
            26 => Some(Self::InspectionSpecification),
            27 => Some(Self::InspectionResult),
            28 => Some(Self::Mrb),
            29 => Some(Self::Rma),
            30 => Some(Self::CashJournal),
            31 => Some(Self::WriteOff),
            32 => Some(Self::ExpenseReimbursement),
            33 => Some(Self::Product),
            34 => Some(Self::Customer),
            35 => Some(Self::Supplier),
            36 => Some(Self::Bom),
            37 => Some(Self::PurchaseReconciliation),
            38 => Some(Self::LaborProcessDict),
            39 => Some(Self::Routing),
            40 => Some(Self::BomLaborProcess),
            41 => Some(Self::ProductionBatch),
            42 => Some(Self::FlowCard),
            43 => Some(Self::StockReceipt),
            44 => Some(Self::StockShipment),
            45 => Some(Self::LedgerAdjustment),
            _ => None,
        }
    }

    pub fn as_i16(self) -> i16 {
        self as i16
    }

    /// 返回各单据类型的编号前缀
    pub fn prefix(self) -> &'static str {
        match self {
            // Sales CRM
            Self::Quotation => "QUO",
            Self::SalesOrder => "SO",
            Self::ShippingRequest => "SR",
            Self::SalesReturn => "SRT",
            Self::Reconciliation => "REC",
            // Purchase SRM
            Self::PurchaseQuotation => "PQ",
            Self::PurchaseOrder => "PO",
            Self::PurchaseReturn => "PRT",
            Self::MiscellaneousRequest => "MISC",
            // MES
            Self::WorkOrder => "WO",
            Self::OutsourcingOrder => "OO",
            Self::ProductionPlan => "PP",
            Self::WorkReport => "WR",
            Self::ProductionInspection => "PI",
            Self::ProductionReceipt => "PR",
            // WMS
            Self::ArrivalNotice => "AN",
            Self::MaterialRequisition => "MR",
            Self::Backflush => "BF",
            Self::CycleCount => "CC",
            Self::InventoryTransfer => "TRF",
            Self::FormConversion => "FC",
            Self::InventoryLock => "LCK",
            Self::PaymentRequest => "PAY",
            Self::Invoice => "INV",
            // OM 委外
            Self::OutsourcingTracking => "OT",
            // QMS 质量
            Self::InspectionSpecification => "QS",
            Self::InspectionResult => "QR",
            Self::Mrb => "MRB",
            Self::Rma => "RMA",
            // FMS 财务
            Self::CashJournal => "CJ",
            Self::WriteOff => "WOF",
            Self::ExpenseReimbursement => "ER",
            // Master Data — Timestamp 策略
            Self::Product => "x",
            Self::Customer => "CUS",
            Self::Supplier => "SUP",
            Self::Bom => "BOM",
            // Purchase SRM — Reconciliation
            Self::PurchaseReconciliation => "PRC",
            // Master Data — 工艺路线子域
            Self::LaborProcessDict => "LPD",
            Self::Routing => "RTG",
            Self::BomLaborProcess => "BLP",
            // MES — 生产批次
            Self::ProductionBatch => "PB",
            Self::FlowCard => "FC",
            Self::StockReceipt => "RK",
            // WMS — 出库单
            Self::StockShipment => "CK",
            Self::LedgerAdjustment => "ADJ",
        }
    }
}

impl sqlx::Type<sqlx::Postgres> for DocumentType {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <i16 as sqlx::Type<sqlx::Postgres>>::type_info()
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for DocumentType {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        <i16 as sqlx::Encode<'_, sqlx::Postgres>>::encode_by_ref(&self.as_i16(), buf)
    }
}

impl sqlx::Decode<'_, sqlx::Postgres> for DocumentType {
    fn decode(value: sqlx::postgres::PgValueRef<'_>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let v = <i16 as sqlx::Decode<'_, sqlx::Postgres>>::decode(value)?;
        Self::from_i16(v).ok_or_else(|| format!("unknown DocumentType discriminant: {v}").into())
    }
}

impl serde::Serialize for DocumentType {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_i16(self.as_i16())
    }
}

impl<'de> serde::Deserialize<'de> for DocumentType {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = i16::deserialize(d)?;
        Self::from_i16(v).ok_or_else(|| serde::de::Error::custom(format!("unknown DocumentType: {v}")))
    }
}
