use chrono::{DateTime, Utc};

/// 通知类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum NotificationType {
    System = 1,
    Business = 2,
    Alert = 3,
}

impl NotificationType {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::System),
            2 => Some(Self::Business),
            3 => Some(Self::Alert),
            _ => None,
        }
    }

    pub fn as_i16(self) -> i16 {
        self as i16
    }
}

impl sqlx::Type<sqlx::Postgres> for NotificationType {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("smallint")
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for NotificationType {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        <i16 as sqlx::Encode<'_, sqlx::Postgres>>::encode_by_ref(&self.as_i16(), buf)
    }
}

impl sqlx::Decode<'_, sqlx::Postgres> for NotificationType {
    fn decode(value: sqlx::postgres::PgValueRef<'_>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let v = <i16 as sqlx::Decode<'_, sqlx::Postgres>>::decode(value)?;
        Self::from_i16(v).ok_or_else(|| format!("unknown NotificationType: {v}").into())
    }
}

/// 通知实体
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Notification {
    pub notification_id: i64,
    pub user_id: i64,
    pub notification_type: NotificationType,
    pub title: String,
    pub content: Option<String>,
    pub related_type: Option<String>,
    pub related_id: Option<i64>,
    pub is_read: bool,
    pub read_at: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
}

/// 创建通知请求
#[derive(Debug, Clone)]
pub struct CreateNotificationReq {
    pub user_id: i64,
    pub notification_type: NotificationType,
    pub title: String,
    pub content: Option<String>,
    pub related_type: Option<String>,
    pub related_id: Option<i64>,
}

/// 通知查询
#[derive(Debug, Clone, Default)]
pub struct NotificationQuery {
    pub notification_type: Option<NotificationType>,
    pub is_read: Option<bool>,
    pub page: u32,
    pub page_size: u32,
}
