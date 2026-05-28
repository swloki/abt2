use crate::shared::types::PgExecutor;
use crate::shared::types::Result;

use super::model::*;
use crate::shared::types::{PageParams, PaginatedResult};

const SUPPLIER_COLUMNS: &str = "supplier_id, supplier_code, supplier_name, short_name, category, status, tax_number, lead_time_days, payment_terms, remark, operator_id, created_at, updated_at, deleted_at";

// ===========================================================================
// SupplierRepo
// ===========================================================================

pub struct SupplierRepo;

impl SupplierRepo {
    pub async fn create(
        &self,
        executor: PgExecutor<'_>,
        supplier_code: &str,
        req: &CreateSupplierReq,
        operator_id: i64,
    ) -> Result<i64> {
        let row = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"INSERT INTO suppliers (supplier_code, supplier_name, short_name, category, status, tax_number, lead_time_days, payment_terms, remark, operator_id)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
               RETURNING supplier_id"#,
        )
        .bind(supplier_code)
        .bind(&req.supplier_name)
        .bind(&req.short_name)
        .bind(req.category.as_i16())
        .bind(SupplierStatus::Prospective.as_i16())
        .bind(&req.tax_number)
        .bind(req.lead_time_days.unwrap_or(0))
        .bind(&req.payment_terms)
        .bind(req.remark.as_deref().unwrap_or(""))
        .bind(operator_id)
        .fetch_one(executor)
        .await?;
        Ok(row)
    }

    #[allow(unused_assignments)]
    pub async fn update(
        &self,
        executor: PgExecutor<'_>,
        id: i64,
        req: &UpdateSupplierReq,
    ) -> Result<()> {
        let mut sets = Vec::new();
        let mut param_idx = 2u32;

        if req.supplier_name.is_some() { sets.push(format!("supplier_name = ${param_idx}")); param_idx += 1; }
        if req.short_name.is_some() { sets.push(format!("short_name = ${param_idx}")); param_idx += 1; }
        if req.category.is_some() { sets.push(format!("category = ${param_idx}")); param_idx += 1; }
        if req.status.is_some() { sets.push(format!("status = ${param_idx}")); param_idx += 1; }
        if req.tax_number.is_some() { sets.push(format!("tax_number = ${param_idx}")); param_idx += 1; }
        if req.lead_time_days.is_some() { sets.push(format!("lead_time_days = ${param_idx}")); param_idx += 1; }
        if req.payment_terms.is_some() { sets.push(format!("payment_terms = ${param_idx}")); param_idx += 1; }
        if req.remark.is_some() { sets.push(format!("remark = ${param_idx}")); param_idx += 1; }

        if sets.is_empty() {
            return Ok(());
        }

        sets.push("updated_at = NOW()".to_string());
        let sql = format!("UPDATE suppliers SET {} WHERE supplier_id = $1 AND deleted_at IS NULL", sets.join(", "));
        let mut q = sqlx::query(sqlx::AssertSqlSafe(sql)).bind(id);

        if let Some(ref v) = req.supplier_name { q = q.bind(v); }
        if let Some(ref v) = req.short_name { q = q.bind(v); }
        if let Some(v) = req.category { q = q.bind(v.as_i16()); }
        if let Some(v) = req.status { q = q.bind(v.as_i16()); }
        if let Some(ref v) = req.tax_number { q = q.bind(v); }
        if let Some(v) = req.lead_time_days { q = q.bind(v); }
        if let Some(ref v) = req.payment_terms { q = q.bind(v); }
        if let Some(ref v) = req.remark { q = q.bind(v); }

        q.execute(executor).await?;
        Ok(())
    }

    pub async fn delete(&self, executor: PgExecutor<'_>, id: i64) -> Result<()> {
        sqlx::query("UPDATE suppliers SET deleted_at = NOW() WHERE supplier_id = $1 AND deleted_at IS NULL")
            .bind(id)
            .execute(executor)
            .await?;
        Ok(())
    }

    pub async fn find_by_id(&self, executor: PgExecutor<'_>, id: i64) -> Result<Option<Supplier>> {
        let supplier = sqlx::query_as::<sqlx::Postgres, Supplier>(
            sqlx::AssertSqlSafe(format!("SELECT {SUPPLIER_COLUMNS} FROM suppliers WHERE supplier_id = $1 AND deleted_at IS NULL")),
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;
        Ok(supplier)
    }

    #[allow(unused_assignments)]
    pub async fn query(
        &self,
        executor: PgExecutor<'_>,
        filter: &SupplierQuery,
        page: &PageParams,
    ) -> Result<PaginatedResult<Supplier>> {
        let mut conditions = vec!["deleted_at IS NULL".to_string()];
        let mut param_idx = 0u32;

        let name_param = if let Some(ref name) = filter.name {
            param_idx += 1;
            conditions.push(format!("supplier_name ILIKE ${param_idx}"));
            Some(format!("%{name}%"))
        } else { None };

        let status_param = if let Some(status) = filter.status {
            param_idx += 1;
            conditions.push(format!("status = ${param_idx}"));
            Some(status.as_i16())
        } else { None };

        let category_param = if let Some(category) = filter.category {
            param_idx += 1;
            conditions.push(format!("category = ${param_idx}"));
            Some(category.as_i16())
        } else { None };

        let where_clause = conditions.join(" AND ");

        let count_sql = format!("SELECT COUNT(*) FROM suppliers WHERE {where_clause}");
        let mut count_q = sqlx::query_scalar::<sqlx::Postgres, i64>(sqlx::AssertSqlSafe(count_sql));
        if let Some(ref v) = name_param { count_q = count_q.bind(v); }
        if let Some(v) = status_param { count_q = count_q.bind(v); }
        if let Some(v) = category_param { count_q = count_q.bind(v); }
        let total = count_q.fetch_one(&mut *executor).await? as u64;

        param_idx += 1;
        let limit_idx = param_idx;
        param_idx += 1;
        let offset_idx = param_idx;
        let data_sql = format!(
            "SELECT {SUPPLIER_COLUMNS} FROM suppliers WHERE {where_clause} ORDER BY supplier_id DESC LIMIT ${limit_idx} OFFSET ${offset_idx}",
        );
        let mut data_q = sqlx::query_as::<sqlx::Postgres, Supplier>(sqlx::AssertSqlSafe(data_sql));
        if let Some(ref v) = name_param { data_q = data_q.bind(v); }
        if let Some(v) = status_param { data_q = data_q.bind(v); }
        if let Some(v) = category_param { data_q = data_q.bind(v); }
        data_q = data_q.bind(page.page_size as i64).bind(page.offset() as i64);
        let items = data_q.fetch_all(executor).await?;

        Ok(PaginatedResult::new(items, total, page.page, page.page_size))
    }

    /// Check if tax_number exists across suppliers and customers tables (dedup).
    pub async fn check_tax_number_exists(&self, executor: PgExecutor<'_>, tax_number: &str) -> Result<bool> {
        let count: i64 = sqlx::query_scalar(
            "SELECT (SELECT COUNT(*) FROM suppliers WHERE tax_number = $1 AND deleted_at IS NULL) + \
                    (SELECT COUNT(*) FROM customers WHERE tax_number = $1 AND deleted_at IS NULL)",
        )
        .bind(tax_number)
        .fetch_one(executor)
        .await?;
        Ok(count > 0)
    }
}

// ===========================================================================
// SupplierContactRepo
// ===========================================================================

const CONTACT_COLUMNS: &str = "contact_id, supplier_id, contact_name, phone, email, position, is_primary";

pub struct SupplierContactRepo;

impl SupplierContactRepo {
    pub async fn create(
        &self,
        executor: PgExecutor<'_>,
        supplier_id: i64,
        req: &CreateContactReq,
    ) -> Result<i64> {
        let row = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"INSERT INTO supplier_contacts (supplier_id, contact_name, phone, email, position, is_primary)
               VALUES ($1, $2, $3, $4, $5, $6)
               RETURNING contact_id"#,
        )
        .bind(supplier_id)
        .bind(&req.contact_name)
        .bind(&req.phone)
        .bind(&req.email)
        .bind(&req.position)
        .bind(req.is_primary)
        .fetch_one(executor)
        .await?;
        Ok(row)
    }

    #[allow(unused_assignments)]
    pub async fn update(
        &self,
        executor: PgExecutor<'_>,
        contact_id: i64,
        supplier_id: i64,
        req: &UpdateContactReq,
    ) -> Result<()> {
        let mut sets = Vec::new();
        let mut param_idx = 3u32;

        if req.contact_name.is_some() { sets.push(format!("contact_name = ${param_idx}")); param_idx += 1; }
        if req.phone.is_some() { sets.push(format!("phone = ${param_idx}")); param_idx += 1; }
        if req.email.is_some() { sets.push(format!("email = ${param_idx}")); param_idx += 1; }
        if req.position.is_some() { sets.push(format!("position = ${param_idx}")); param_idx += 1; }
        if req.is_primary.is_some() { sets.push(format!("is_primary = ${param_idx}")); param_idx += 1; }

        if sets.is_empty() {
            return Ok(());
        }

        let sql = format!(
            "UPDATE supplier_contacts SET {} WHERE contact_id = $1 AND supplier_id = $2",
            sets.join(", "),
        );
        let mut q = sqlx::query(sqlx::AssertSqlSafe(sql)).bind(contact_id).bind(supplier_id);

        if let Some(ref v) = req.contact_name { q = q.bind(v); }
        if let Some(ref v) = req.phone { q = q.bind(v); }
        if let Some(ref v) = req.email { q = q.bind(v); }
        if let Some(ref v) = req.position { q = q.bind(v); }
        if let Some(v) = req.is_primary { q = q.bind(v); }

        q.execute(executor).await?;
        Ok(())
    }

    pub async fn delete(&self, executor: PgExecutor<'_>, contact_id: i64, supplier_id: i64) -> Result<()> {
        sqlx::query("DELETE FROM supplier_contacts WHERE contact_id = $1 AND supplier_id = $2")
            .bind(contact_id)
            .bind(supplier_id)
            .execute(executor)
            .await?;
        Ok(())
    }

    pub async fn find_by_id(&self, executor: PgExecutor<'_>, contact_id: i64) -> Result<Option<SupplierContact>> {
        let contact = sqlx::query_as::<sqlx::Postgres, SupplierContact>(
            sqlx::AssertSqlSafe(format!("SELECT {CONTACT_COLUMNS} FROM supplier_contacts WHERE contact_id = $1")),
        )
        .bind(contact_id)
        .fetch_optional(executor)
        .await?;
        Ok(contact)
    }

    pub async fn find_by_supplier_id(&self, executor: PgExecutor<'_>, supplier_id: i64) -> Result<Vec<SupplierContact>> {
        let contacts = sqlx::query_as::<sqlx::Postgres, SupplierContact>(
            sqlx::AssertSqlSafe(format!("SELECT {CONTACT_COLUMNS} FROM supplier_contacts WHERE supplier_id = $1 ORDER BY is_primary DESC, contact_id ASC")),
        )
        .bind(supplier_id)
        .fetch_all(executor)
        .await?;
        Ok(contacts)
    }
}

// ===========================================================================
// SupplierBankAccountRepo
// ===========================================================================

const BANK_ACCOUNT_COLUMNS: &str = "account_id, supplier_id, bank_name, account_name, account_number, is_default";

pub struct SupplierBankAccountRepo;

impl SupplierBankAccountRepo {
    pub async fn create(
        &self,
        executor: PgExecutor<'_>,
        supplier_id: i64,
        req: &CreateBankAccountReq,
    ) -> Result<i64> {
        let row = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"INSERT INTO supplier_bank_accounts (supplier_id, bank_name, account_name, account_number, is_default)
               VALUES ($1, $2, $3, $4, $5)
               RETURNING account_id"#,
        )
        .bind(supplier_id)
        .bind(&req.bank_name)
        .bind(&req.account_name)
        .bind(&req.account_number)
        .bind(req.is_default)
        .fetch_one(executor)
        .await?;
        Ok(row)
    }

    #[allow(unused_assignments)]
    pub async fn update(
        &self,
        executor: PgExecutor<'_>,
        account_id: i64,
        supplier_id: i64,
        req: &UpdateBankAccountReq,
    ) -> Result<Option<SupplierBankAccount>> {
        // Fetch before for diff
        let before = sqlx::query_as::<sqlx::Postgres, SupplierBankAccount>(
            sqlx::AssertSqlSafe(format!("SELECT {BANK_ACCOUNT_COLUMNS} FROM supplier_bank_accounts WHERE account_id = $1 AND supplier_id = $2")),
        )
        .bind(account_id)
        .bind(supplier_id)
        .fetch_optional(&mut *executor)
        .await?;

        let mut sets = Vec::new();
        let mut param_idx = 3u32;

        if req.bank_name.is_some() { sets.push(format!("bank_name = ${param_idx}")); param_idx += 1; }
        if req.account_name.is_some() { sets.push(format!("account_name = ${param_idx}")); param_idx += 1; }
        if req.account_number.is_some() { sets.push(format!("account_number = ${param_idx}")); param_idx += 1; }
        if req.is_default.is_some() { sets.push(format!("is_default = ${param_idx}")); param_idx += 1; }

        if sets.is_empty() {
            return Ok(before);
        }

        let sql = format!(
            "UPDATE supplier_bank_accounts SET {} WHERE account_id = $1 AND supplier_id = $2",
            sets.join(", "),
        );
        let mut q = sqlx::query(sqlx::AssertSqlSafe(sql)).bind(account_id).bind(supplier_id);

        if let Some(ref v) = req.bank_name { q = q.bind(v); }
        if let Some(ref v) = req.account_name { q = q.bind(v); }
        if let Some(ref v) = req.account_number { q = q.bind(v); }
        if let Some(v) = req.is_default { q = q.bind(v); }

        q.execute(executor).await?;
        Ok(before)
    }

    pub async fn delete(&self, executor: PgExecutor<'_>, account_id: i64, supplier_id: i64) -> Result<()> {
        sqlx::query("DELETE FROM supplier_bank_accounts WHERE account_id = $1 AND supplier_id = $2")
            .bind(account_id)
            .bind(supplier_id)
            .execute(executor)
            .await?;
        Ok(())
    }

    pub async fn find_by_id(&self, executor: PgExecutor<'_>, account_id: i64) -> Result<Option<SupplierBankAccount>> {
        let account = sqlx::query_as::<sqlx::Postgres, SupplierBankAccount>(
            sqlx::AssertSqlSafe(format!("SELECT {BANK_ACCOUNT_COLUMNS} FROM supplier_bank_accounts WHERE account_id = $1")),
        )
        .bind(account_id)
        .fetch_optional(executor)
        .await?;
        Ok(account)
    }

    pub async fn find_by_supplier_id(&self, executor: PgExecutor<'_>, supplier_id: i64) -> Result<Vec<SupplierBankAccount>> {
        let accounts = sqlx::query_as::<sqlx::Postgres, SupplierBankAccount>(
            sqlx::AssertSqlSafe(format!("SELECT {BANK_ACCOUNT_COLUMNS} FROM supplier_bank_accounts WHERE supplier_id = $1 ORDER BY is_default DESC, account_id ASC")),
        )
        .bind(supplier_id)
        .fetch_all(executor)
        .await?;
        Ok(accounts)
    }
}
