/// 19 种领域事件
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
            19 => Some(Self::WriteOffCompleted), _ => None,
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
    fn type_info() -> sqlx::postgres::PgTypeInfo { sqlx::postgres::PgTypeInfo::with_name("smallint") }
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
    fn type_info() -> sqlx::postgres::PgTypeInfo { sqlx::postgres::PgTypeInfo::with_name("smallint") }
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
