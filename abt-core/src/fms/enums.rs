// 财务管理模块专属枚举 — 全部 `#[repr(i16)]`

use serde::{Deserialize, Serialize};

// ---------- Boilerplate macros ----------

macro_rules! impl_sqlx_traits {
    ($name:ident) => {
        impl sqlx::Type<sqlx::Postgres> for $name {
            fn type_info() -> sqlx::postgres::PgTypeInfo {
                <i16 as sqlx::Type<sqlx::Postgres>>::type_info()
            }
        }

        impl sqlx::Encode<'_, sqlx::Postgres> for $name {
            fn encode_by_ref(
                &self,
                buf: &mut sqlx::postgres::PgArgumentBuffer,
            ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
                <i16 as sqlx::Encode<'_, sqlx::Postgres>>::encode_by_ref(&self.as_i16(), buf)
            }
        }

        impl sqlx::Decode<'_, sqlx::Postgres> for $name {
            fn decode(
                value: sqlx::postgres::PgValueRef<'_>,
            ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
                let v = <i16 as sqlx::Decode<'_, sqlx::Postgres>>::decode(value)?;
                Self::from_i16(v).ok_or_else(|| {
                    let msg = format!("unknown {}: {v}", stringify!($name));
                    msg.into()
                })
            }
        }
    };
}

macro_rules! impl_serde_traits {
    ($name:ident) => {
        impl Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_i16(self.as_i16())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let v = i16::deserialize(d)?;
                Self::from_i16(v).ok_or_else(|| {
                    let msg = format!("unknown {}: {v}", stringify!($name));
                    serde::de::Error::custom(msg)
                })
            }
        }
    };
}

// ---------- JournalType ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum JournalType {
    SalesReceipt = 1,
    PurchasePayment = 2,
    Expense = 3,
    Payroll = 4,
    Other = 5,
}

impl JournalType {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::SalesReceipt),
            2 => Some(Self::PurchasePayment),
            3 => Some(Self::Expense),
            4 => Some(Self::Payroll),
            5 => Some(Self::Other),
            _ => None,
        }
    }
    pub fn as_i16(self) -> i16 {
        self as i16
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SalesReceipt => "SalesReceipt",
            Self::PurchasePayment => "PurchasePayment",
            Self::Expense => "Expense",
            Self::Payroll => "Payroll",
            Self::Other => "Other",
        }
    }
}

impl_sqlx_traits!(JournalType);
impl_serde_traits!(JournalType);

// ---------- CashDirection ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum CashDirection {
    Inflow = 1,
    Outflow = 2,
}

impl CashDirection {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Inflow),
            2 => Some(Self::Outflow),
            _ => None,
        }
    }
    pub fn as_i16(self) -> i16 {
        self as i16
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Inflow => "Inflow",
            Self::Outflow => "Outflow",
        }
    }
}

impl_sqlx_traits!(CashDirection);
impl_serde_traits!(CashDirection);

// ---------- CounterpartyType ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum CounterpartyType {
    Customer = 1,
    Supplier = 2,
    Employee = 3,
    Other = 4,
}

impl CounterpartyType {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Customer),
            2 => Some(Self::Supplier),
            3 => Some(Self::Employee),
            4 => Some(Self::Other),
            _ => None,
        }
    }
    pub fn as_i16(self) -> i16 {
        self as i16
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Customer => "Customer",
            Self::Supplier => "Supplier",
            Self::Employee => "Employee",
            Self::Other => "Other",
        }
    }
}

impl_sqlx_traits!(CounterpartyType);
impl_serde_traits!(CounterpartyType);

// ---------- JournalStatus ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum JournalStatus {
    Draft = 1,
    Confirmed = 2,
    Cancelled = 3,
}

impl JournalStatus {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Draft),
            2 => Some(Self::Confirmed),
            3 => Some(Self::Cancelled),
            _ => None,
        }
    }
    pub fn as_i16(self) -> i16 {
        self as i16
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "Draft",
            Self::Confirmed => "Confirmed",
            Self::Cancelled => "Cancelled",
        }
    }
}

impl_sqlx_traits!(JournalStatus);
impl_serde_traits!(JournalStatus);

// ---------- WriteOffType ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum WriteOffType {
    SalesReceipt = 1,
    PurchasePayment = 2,
}

impl WriteOffType {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::SalesReceipt),
            2 => Some(Self::PurchasePayment),
            _ => None,
        }
    }
    pub fn as_i16(self) -> i16 {
        self as i16
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SalesReceipt => "SalesReceipt",
            Self::PurchasePayment => "PurchasePayment",
        }
    }
}

impl_sqlx_traits!(WriteOffType);
impl_serde_traits!(WriteOffType);

// ---------- ExpenseType ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum ExpenseType {
    Travel = 1,
    Office = 2,
    Transport = 3,
    Meal = 4,
    Other = 5,
}

impl ExpenseType {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Travel),
            2 => Some(Self::Office),
            3 => Some(Self::Transport),
            4 => Some(Self::Meal),
            5 => Some(Self::Other),
            _ => None,
        }
    }
    pub fn as_i16(self) -> i16 {
        self as i16
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Travel => "Travel",
            Self::Office => "Office",
            Self::Transport => "Transport",
            Self::Meal => "Meal",
            Self::Other => "Other",
        }
    }
}

impl_sqlx_traits!(ExpenseType);
impl_serde_traits!(ExpenseType);

// ---------- ExpenseStatus ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum ExpenseStatus {
    Draft = 1,
    Submitted = 2,
    Approved = 3,
    Paid = 4,
    Cancelled = 5,
    SupervisorApproved = 6,
    FinanceApproved = 7,
}

impl ExpenseStatus {
    pub fn from_i16(v: i16) -> Option<Self> {
        match v {
            1 => Some(Self::Draft),
            2 => Some(Self::Submitted),
            3 => Some(Self::Approved),
            4 => Some(Self::Paid),
            5 => Some(Self::Cancelled),
            6 => Some(Self::SupervisorApproved),
            7 => Some(Self::FinanceApproved),
            _ => None,
        }
    }
    pub fn as_i16(self) -> i16 {
        self as i16
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "Draft",
            Self::Submitted => "Submitted",
            Self::Approved => "Approved",
            Self::Paid => "Paid",
            Self::Cancelled => "Cancelled",
            Self::SupervisorApproved => "SupervisorApproved",
            Self::FinanceApproved => "FinanceApproved",
        }
    }

    /// 中文标签
    pub fn label(self) -> &'static str {
        match self {
            Self::Draft => "草稿",
            Self::Submitted => "已提交",
            Self::Approved => "已通过",
            Self::Paid => "已付款",
            Self::Cancelled => "已取消",
            Self::SupervisorApproved => "直属上级已批",
            Self::FinanceApproved => "财务已审",
        }
    }

    /// 是否处于待审批状态（Submitted | SupervisorApproved | FinanceApproved）
    pub fn is_pending_approval(self) -> bool {
        matches!(self, Self::Submitted | Self::SupervisorApproved | Self::FinanceApproved)
    }
}

impl_sqlx_traits!(ExpenseStatus);
impl_serde_traits!(ExpenseStatus);

// ---------- CounterpartyRef ----------
// Rust 模型层枚举 — DB 层用 counterparty_type + counterparty_id 两列存储
// Repository 层负责拆包/装包

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CounterpartyRef {
    Customer(i64),
    Supplier(i64),
    Employee(i64),
    Other(i64),
}

impl CounterpartyRef {
    pub fn from_parts(ct: CounterpartyType, id: i64) -> Self {
        match ct {
            CounterpartyType::Customer => Self::Customer(id),
            CounterpartyType::Supplier => Self::Supplier(id),
            CounterpartyType::Employee => Self::Employee(id),
            CounterpartyType::Other => Self::Other(id),
        }
    }

    pub fn to_parts(self) -> (CounterpartyType, i64) {
        match self {
            Self::Customer(id) => (CounterpartyType::Customer, id),
            Self::Supplier(id) => (CounterpartyType::Supplier, id),
            Self::Employee(id) => (CounterpartyType::Employee, id),
            Self::Other(id) => (CounterpartyType::Other, id),
        }
    }

    pub fn id(&self) -> i64 {
        match self {
            Self::Customer(id) | Self::Supplier(id) | Self::Employee(id) | Self::Other(id) => *id,
        }
    }
}
