//! 数据模型层
//!
//! 包含 BOM、产品、分类等业务实体的定义。

mod api;
mod auth;
mod bom;
mod department;
mod dept_role;
mod inventory;
mod labor_process;
mod location;
mod permission;
mod product;
pub mod resources;
mod role;
mod term;
mod user;
mod warehouse;

pub use api::*;
pub use auth::*;
pub use bom::*;
pub use department::*;
pub use dept_role::*;
pub use inventory::*;
pub use labor_process::*;
pub use location::*;
pub use permission::*;
pub use product::*;
pub use resources::*;
pub use role::*;
pub use term::*;
pub use user::*;
pub use warehouse::*;
