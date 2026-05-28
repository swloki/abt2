use std::collections::{HashMap, HashSet};

use abt_core::master_data::customer::CustomerService;
use abt_core::shared::types::{PgExecutor, ServiceContext};
use serde::{Deserialize, de};

pub fn empty_as_none<'de, D, T>(de: D) -> std::result::Result<Option<T>, D::Error>
where
    D: de::Deserializer<'de>,
    T: std::str::FromStr,
{
    let s: Option<String> = Option::deserialize(de)?;
    match s.as_deref() {
        None | Some("") => Ok(None),
        Some(v) => v.parse::<T>().map(Some).map_err(|_| {
            de::Error::custom(format!("cannot parse '{v}'"))
        }),
    }
}

pub async fn resolve_customer_names<S: CustomerService>(
    svc: &S,
    ctx: &ServiceContext,
    db: PgExecutor<'_>,
    ids: impl IntoIterator<Item = i64>,
) -> HashMap<i64, String> {
    let mut map = HashMap::new();
    let unique: HashSet<i64> = ids.into_iter().collect();
    for id in unique {
        if let Ok(customer) = svc.get(ctx, db, id).await {
            map.insert(id, customer.name);
        }
    }
    map
}
