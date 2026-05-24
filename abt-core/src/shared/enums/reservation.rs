#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum ReservationType {
    Hard = 1,
    Soft = 2,
    SafetyStock = 3,
}

impl ReservationType {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Hard),
            2 => Some(Self::Soft),
            3 => Some(Self::SafetyStock),
            _ => None,
        }
    }
    pub fn as_i16(self) -> i16 { self as i16 }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum ReservationStatus {
    Active = 1,
    Fulfilled = 2,
    Cancelled = 3,
    Expired = 4,
}

impl ReservationStatus {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Active),
            2 => Some(Self::Fulfilled),
            3 => Some(Self::Cancelled),
            4 => Some(Self::Expired),
            _ => None,
        }
    }
    pub fn as_i16(self) -> i16 { self as i16 }
}

impl sqlx::Type<sqlx::Postgres> for ReservationType {
    fn type_info() -> sqlx::postgres::PgTypeInfo { sqlx::postgres::PgTypeInfo::with_name("smallint") }
}
impl sqlx::Encode<'_, sqlx::Postgres> for ReservationType {
    fn encode_by_ref(&self, buf: &mut sqlx::postgres::PgArgumentBuffer) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        <i16 as sqlx::Encode<'_, sqlx::Postgres>>::encode_by_ref(&self.as_i16(), buf)
    }
}
impl sqlx::Decode<'_, sqlx::Postgres> for ReservationType {
    fn decode(value: sqlx::postgres::PgValueRef<'_>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let v = <i16 as sqlx::Decode<'_, sqlx::Postgres>>::decode(value)?;
        Self::from_i16(v).ok_or_else(|| format!("unknown ReservationType: {v}").into())
    }
}

impl sqlx::Type<sqlx::Postgres> for ReservationStatus {
    fn type_info() -> sqlx::postgres::PgTypeInfo { sqlx::postgres::PgTypeInfo::with_name("smallint") }
}
impl sqlx::Encode<'_, sqlx::Postgres> for ReservationStatus {
    fn encode_by_ref(&self, buf: &mut sqlx::postgres::PgArgumentBuffer) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        <i16 as sqlx::Encode<'_, sqlx::Postgres>>::encode_by_ref(&self.as_i16(), buf)
    }
}
impl sqlx::Decode<'_, sqlx::Postgres> for ReservationStatus {
    fn decode(value: sqlx::postgres::PgValueRef<'_>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let v = <i16 as sqlx::Decode<'_, sqlx::Postgres>>::decode(value)?;
        Self::from_i16(v).ok_or_else(|| format!("unknown ReservationStatus: {v}").into())
    }
}

impl serde::Serialize for ReservationType {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> { s.serialize_i16(self.as_i16()) }
}
impl<'de> serde::Deserialize<'de> for ReservationType {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = i16::deserialize(d)?;
        Self::from_i16(v).ok_or_else(|| serde::de::Error::custom(format!("unknown ReservationType: {v}")))
    }
}

impl serde::Serialize for ReservationStatus {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> { s.serialize_i16(self.as_i16()) }
}
impl<'de> serde::Deserialize<'de> for ReservationStatus {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = i16::deserialize(d)?;
        Self::from_i16(v).ok_or_else(|| serde::de::Error::custom(format!("unknown ReservationStatus: {v}")))
    }
}
