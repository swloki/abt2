#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum SequenceStrategy {
    Sequential = 1,
    Timestamp = 2,
}

impl SequenceStrategy {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Sequential), 2 => Some(Self::Timestamp), _ => None,
        }
    }
    pub fn as_i16(self) -> i16 { self as i16 }
}

impl sqlx::Type<sqlx::Postgres> for SequenceStrategy {
    fn type_info() -> sqlx::postgres::PgTypeInfo { <i16 as sqlx::Type<sqlx::Postgres>>::type_info() }
}
impl sqlx::Encode<'_, sqlx::Postgres> for SequenceStrategy {
    fn encode_by_ref(&self, buf: &mut sqlx::postgres::PgArgumentBuffer) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        <i16 as sqlx::Encode<'_, sqlx::Postgres>>::encode_by_ref(&self.as_i16(), buf)
    }
}
impl sqlx::Decode<'_, sqlx::Postgres> for SequenceStrategy {
    fn decode(value: sqlx::postgres::PgValueRef<'_>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let v = <i16 as sqlx::Decode<'_, sqlx::Postgres>>::decode(value)?;
        Self::from_i16(v).ok_or_else(|| format!("unknown SequenceStrategy: {v}").into())
    }
}

impl serde::Serialize for SequenceStrategy {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> { s.serialize_i16(self.as_i16()) }
}
impl<'de> serde::Deserialize<'de> for SequenceStrategy {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = i16::deserialize(d)?;
        Self::from_i16(v).ok_or_else(|| serde::de::Error::custom(format!("unknown SequenceStrategy: {v}")))
    }
}
