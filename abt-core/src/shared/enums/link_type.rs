#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum LinkType {
    DerivedFrom = 1,
    Triggers = 2,
    References = 3,
    Reconciles = 4,
    Inspects = 5,
    Fulfills = 6,
    Allocates = 7,
}

impl LinkType {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::DerivedFrom),
            2 => Some(Self::Triggers),
            3 => Some(Self::References),
            4 => Some(Self::Reconciles),
            5 => Some(Self::Inspects),
            6 => Some(Self::Fulfills),
            7 => Some(Self::Allocates),
            _ => None,
        }
    }

    pub fn as_i16(self) -> i16 {
        self as i16
    }
}

impl sqlx::Type<sqlx::Postgres> for LinkType {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("smallint")
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for LinkType {
    fn encode_by_ref(&self, buf: &mut sqlx::postgres::PgArgumentBuffer) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        <i16 as sqlx::Encode<'_, sqlx::Postgres>>::encode_by_ref(&self.as_i16(), buf)
    }
}

impl sqlx::Decode<'_, sqlx::Postgres> for LinkType {
    fn decode(value: sqlx::postgres::PgValueRef<'_>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let v = <i16 as sqlx::Decode<'_, sqlx::Postgres>>::decode(value)?;
        Self::from_i16(v).ok_or_else(|| format!("unknown LinkType: {v}").into())
    }
}

impl serde::Serialize for LinkType {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> { s.serialize_i16(self.as_i16()) }
}

impl<'de> serde::Deserialize<'de> for LinkType {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = i16::deserialize(d)?;
        Self::from_i16(v).ok_or_else(|| serde::de::Error::custom(format!("unknown LinkType: {v}")))
    }
}
