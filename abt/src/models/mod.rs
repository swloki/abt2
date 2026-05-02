//! 数据模型层
//!
//! 包含 BOM、产品、分类等业务实体的定义。

mod api;
mod auth;
mod bom;
mod bom_category;
mod bom_node;
mod department;
mod inventory;
mod labor_process;
mod labor_process_dict;
mod location;
mod permission;
mod product;
pub mod resources;
mod role;
mod routing;
mod term;
mod user;
mod warehouse;

pub use api::*;
pub use auth::*;
pub use bom::*;
pub use bom_category::*;
pub use bom_node::*;
pub use inventory::*;
pub use labor_process::*;
pub use labor_process_dict::*;
pub use location::*;
pub use permission::*;
pub use product::*;
pub use resources::*;
pub use role::*;
pub use routing::*;
pub use term::*;
pub use user::*;
pub use warehouse::*;
