//! 供应商服务接口
//!
//! 定义供应商管理的业务逻辑接口。

use anyhow::Result;
use async_trait::async_trait;

use crate::models::{Supplier, SupplierDetail, SupplierQuery};
use crate::repositories::{Executor, PaginatedResult};

/// 供应商服务接口
#[async_trait]
pub trait SupplierService: Send + Sync {
    /// 创建供应商（含联系人和银行账户），返回 supplier_id
    async fn create(
        &self,
        supplier_code: String,
        supplier_name: String,
        short_name: Option<String>,
        classification: String,
        remark: Option<String>,
        operator_id: Option<i64>,
        contacts: Vec<SupplierContactInput>,
        bank_accounts: Vec<SupplierBankAccountInput>,
        executor: Executor<'_>,
    ) -> Result<i64>;

    /// 更新供应商（含联系人和银行账户）
    async fn update(
        &self,
        supplier_id: i64,
        supplier_name: String,
        short_name: Option<String>,
        classification: String,
        remark: Option<String>,
        contacts: Vec<SupplierContactInput>,
        bank_accounts: Vec<SupplierBankAccountInput>,
        executor: Executor<'_>,
    ) -> Result<()>;

    /// 删除供应商（软删除）
    async fn delete(&self, supplier_id: i64, executor: Executor<'_>) -> Result<()>;

    /// 根据 ID 获取供应商详情（含联系人和银行账户）
    async fn get_by_id(&self, supplier_id: i64) -> Result<Option<SupplierDetail>>;

    /// 分页查询供应商列表
    async fn list(&self, query: SupplierQuery) -> Result<PaginatedResult<Supplier>>;

    /// 更新供应商状态
    async fn update_status(
        &self,
        supplier_id: i64,
        status: i16,
        executor: Executor<'_>,
    ) -> Result<()>;
}

/// 联系人输入参数
#[derive(Debug, Clone)]
pub struct SupplierContactInput {
    pub contact_name: String,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub position: Option<String>,
    pub is_primary: bool,
}

/// 银行账户输入参数
#[derive(Debug, Clone)]
pub struct SupplierBankAccountInput {
    pub bank_name: String,
    pub account_name: String,
    pub account_no: String,
    pub is_default: bool,
}
