//! 数据模型层
//!
//! 包含 BOM、产品、分类等业务实体的定义。

mod api;
mod bom;
mod inventory;
mod labor_process;
mod location;
mod permission;
mod product;
mod role;
mod term;
mod user;
mod warehouse;

pub use api::*;
pub use bom::*;
pub use inventory::*;
pub use labor_process::*;
pub use location::*;
pub use permission::*;
pub use product::*;
pub use role::*;
pub use term::*;
pub use user::*;
pub use warehouse::*;
