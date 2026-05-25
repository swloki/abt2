use chrono::{DateTime, Utc};

use crate::om::enums::TrackingNodeType;

// ---------------------------------------------------------------------------
// Entity struct
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct OutsourcingTracking {
    pub id: i64,
    pub outsourcing_id: i64,
    pub node_type: TrackingNodeType,
    pub tracked_at: Option<DateTime<Utc>>,
    pub planned_at: Option<DateTime<Utc>>,
    pub remark: Option<String>,
    pub operator_id: i64,
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Query struct
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct OverdueTrackingQuery {
    pub supplier_id: Option<i64>,
    pub node_type: Option<TrackingNodeType>,
    pub overdue_before: Option<DateTime<Utc>>,
}

// ---------------------------------------------------------------------------
// Request struct
// ---------------------------------------------------------------------------

pub struct RecordNodeReq {
    pub outsourcing_id: i64,
    pub node_type: TrackingNodeType,
    pub tracked_at: Option<DateTime<Utc>>,
    pub remark: Option<String>,
}
