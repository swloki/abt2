use anyhow::Result;
use common::PgExecutor;

use super::model::*;
use crate::shared::types::{DataScope, PageParams, PaginatedResult};

const CUSTOMER_COLUMNS: &str = "customer_id, customer_code, customer_name, short_name, category, status, tax_number, invoice_title, credit_limit, payment_terms, receivable_account, owner_id, department_id, remark, operator_id, created_at, updated_at, deleted_at";

// ---------------------------------------------------------------------------
// CustomerRepo
// ---------------------------------------------------------------------------

pub struct CustomerRepo;

impl CustomerRepo {
    pub async fn create(
        &self,
        executor: PgExecutor<'_>,
        customer_code: &str,
        req: &CreateCustomerReq,
        operator_id: i64,
    ) -> Result<i64> {
        let row = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"INSERT INTO customers (customer_code, customer_name, short_name, category, status, tax_number, invoice_title, credit_limit, payment_terms, receivable_account, remark, operator_id)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
               RETURNING customer_id"#,
        )
        .bind(customer_code)
        .bind(&req.customer_name)
        .bind(&req.short_name)
        .bind(req.category.as_i16())
        .bind(CustomerStatus::Prospective.as_i16())
        .bind(&req.tax_number)
        .bind(&req.invoice_title)
        .bind(req.credit_limit)
        .bind(&req.payment_terms)
        .bind(&req.receivable_account)
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
        req: &UpdateCustomerReq,
    ) -> Result<()> {
        let mut sets = Vec::new();
        let mut param_idx = 2u32;

        if req.customer_name.is_some() {
            sets.push(format!("customer_name = ${param_idx}"));
            param_idx += 1;
        }
        if req.short_name.is_some() {
            sets.push(format!("short_name = ${param_idx}"));
            param_idx += 1;
        }
        if req.category.is_some() {
            sets.push(format!("category = ${param_idx}"));
            param_idx += 1;
        }
        if req.status.is_some() {
            sets.push(format!("status = ${param_idx}"));
            param_idx += 1;
        }
        if req.tax_number.is_some() {
            sets.push(format!("tax_number = ${param_idx}"));
            param_idx += 1;
        }
        if req.invoice_title.is_some() {
            sets.push(format!("invoice_title = ${param_idx}"));
            param_idx += 1;
        }
        if req.credit_limit.is_some() {
            sets.push(format!("credit_limit = ${param_idx}"));
            param_idx += 1;
        }
        if req.payment_terms.is_some() {
            sets.push(format!("payment_terms = ${param_idx}"));
            param_idx += 1;
        }
        if req.receivable_account.is_some() {
            sets.push(format!("receivable_account = ${param_idx}"));
            param_idx += 1;
        }
        if req.remark.is_some() {
            sets.push(format!("remark = ${param_idx}"));
            param_idx += 1;
        }

        if sets.is_empty() {
            return Ok(());
        }

        sets.push("updated_at = NOW()".to_string());
        let sql = format!(
            "UPDATE customers SET {} WHERE customer_id = $1 AND deleted_at IS NULL",
            sets.join(", ")
        );
        let mut q = sqlx::query(&sql).bind(id);

        if let Some(ref v) = req.customer_name {
            q = q.bind(v);
        }
        if let Some(ref v) = req.short_name {
            q = q.bind(v);
        }
        if let Some(v) = req.category {
            q = q.bind(v.as_i16());
        }
        if let Some(v) = req.status {
            q = q.bind(v.as_i16());
        }
        if let Some(ref v) = req.tax_number {
            q = q.bind(v);
        }
        if let Some(ref v) = req.invoice_title {
            q = q.bind(v);
        }
        if let Some(v) = req.credit_limit {
            q = q.bind(v);
        }
        if let Some(ref v) = req.payment_terms {
            q = q.bind(v);
        }
        if let Some(ref v) = req.receivable_account {
            q = q.bind(v);
        }
        if let Some(ref v) = req.remark {
            q = q.bind(v);
        }

        q.execute(executor).await?;
        Ok(())
    }

    pub async fn delete(&self, executor: PgExecutor<'_>, id: i64) -> Result<()> {
        sqlx::query("UPDATE customers SET deleted_at = NOW() WHERE customer_id = $1 AND deleted_at IS NULL")
            .bind(id)
            .execute(executor)
            .await?;
        Ok(())
    }

    pub async fn find_by_id(
        &self,
        executor: PgExecutor<'_>,
        id: i64,
    ) -> Result<Option<Customer>> {
        let customer = sqlx::query_as::<sqlx::Postgres, Customer>(
            &format!("SELECT {CUSTOMER_COLUMNS} FROM customers WHERE customer_id = $1 AND deleted_at IS NULL"),
        )
        .bind(id)
        .fetch_optional(executor)
        .await?;
        Ok(customer)
    }

    #[allow(unused_assignments)]
    pub async fn query(
        &self,
        executor: PgExecutor<'_>,
        filter: &CustomerQuery,
        page: &PageParams,
        data_scope: DataScope,
        scope_operator_id: i64,
        scope_department_id: Option<i64>,
    ) -> Result<PaginatedResult<Customer>> {
        let mut conditions = vec!["deleted_at IS NULL".to_string()];
        let mut param_idx = 0u32;

        let name_param = if let Some(ref name) = filter.name {
            param_idx += 1;
            conditions.push(format!("customer_name ILIKE ${param_idx}"));
            Some(format!("%{name}%"))
        } else {
            None
        };

        let status_param = if let Some(status) = filter.status {
            param_idx += 1;
            conditions.push(format!("status = ${param_idx}"));
            Some(status.as_i16())
        } else {
            None
        };

        let category_param = if let Some(category) = filter.category {
            param_idx += 1;
            conditions.push(format!("category = ${param_idx}"));
            Some(category.as_i16())
        } else {
            None
        };

        let owner_param = if let Some(owner_id) = filter.owner_id {
            param_idx += 1;
            conditions.push(format!("owner_id = ${param_idx}"));
            Some(owner_id)
        } else {
            None
        };

        // DataScope row-level filtering
        let scope_dept_param = match data_scope {
            DataScope::All => None,
            DataScope::Department => {
                param_idx += 1;
                let dept_id = scope_department_id.unwrap_or(0);
                conditions.push(format!(
                    "(department_id = ${param_idx} OR owner_id IS NULL)"
                ));
                Some(dept_id)
            }
            DataScope::SelfOnly => {
                param_idx += 1;
                conditions.push(format!(
                    "(owner_id = ${param_idx} OR owner_id IS NULL)"
                ));
                Some(scope_operator_id)
            }
        };

        let where_clause = conditions.join(" AND ");

        // Count query
        let count_sql = format!("SELECT COUNT(*) FROM customers WHERE {where_clause}");
        let mut count_q = sqlx::query_scalar::<sqlx::Postgres, i64>(&count_sql);
        if let Some(ref v) = name_param {
            count_q = count_q.bind(v);
        }
        if let Some(v) = status_param {
            count_q = count_q.bind(v);
        }
        if let Some(v) = category_param {
            count_q = count_q.bind(v);
        }
        if let Some(v) = owner_param {
            count_q = count_q.bind(v);
        }
        if let Some(v) = scope_dept_param {
            count_q = count_q.bind(v);
        }
        let total = count_q.fetch_one(&mut *executor).await? as u64;

        // Data query
        param_idx += 1;
        let limit_idx = param_idx;
        param_idx += 1;
        let offset_idx = param_idx;
        let data_sql = format!(
            "SELECT {CUSTOMER_COLUMNS} FROM customers WHERE {where_clause} ORDER BY customer_id DESC LIMIT ${limit_idx} OFFSET ${offset_idx}",
        );
        let mut data_q = sqlx::query_as::<sqlx::Postgres, Customer>(&data_sql);
        if let Some(ref v) = name_param {
            data_q = data_q.bind(v);
        }
        if let Some(v) = status_param {
            data_q = data_q.bind(v);
        }
        if let Some(v) = category_param {
            data_q = data_q.bind(v);
        }
        if let Some(v) = owner_param {
            data_q = data_q.bind(v);
        }
        if let Some(v) = scope_dept_param {
            data_q = data_q.bind(v);
        }
        data_q = data_q
            .bind(page.page_size as i64)
            .bind(page.offset() as i64);
        let items = data_q.fetch_all(executor).await?;

        Ok(PaginatedResult::new(items, total, page.page, page.page_size))
    }

    /// Check if tax_number already exists in customers or suppliers tables.
    /// Returns true if the tax number is already taken.
    pub async fn check_tax_number_exists(
        &self,
        executor: PgExecutor<'_>,
        tax_number: &str,
    ) -> Result<bool> {
        let count: i64 = sqlx::query_scalar(
            r#"SELECT (
                (SELECT COUNT(*) FROM customers WHERE tax_number = $1 AND deleted_at IS NULL) +
                (SELECT COUNT(*) FROM suppliers WHERE tax_number = $1 AND deleted_at IS NULL)
               )"#,
        )
        .bind(tax_number)
        .fetch_one(executor)
        .await?;
        Ok(count > 0)
    }

    pub async fn set_owner(
        &self,
        executor: PgExecutor<'_>,
        id: i64,
        owner_id: Option<i64>,
        department_id: Option<i64>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE customers SET owner_id = $2, department_id = $3, updated_at = NOW() WHERE customer_id = $1 AND deleted_at IS NULL",
        )
        .bind(id)
        .bind(owner_id)
        .bind(department_id)
        .execute(executor)
        .await?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// CustomerContactRepo
// ---------------------------------------------------------------------------

const CONTACT_COLUMNS: &str = "contact_id, customer_id, contact_name, position, phone, email, is_primary";

pub struct CustomerContactRepo;

impl CustomerContactRepo {
    pub async fn create(
        &self,
        executor: PgExecutor<'_>,
        customer_id: i64,
        req: &CreateContactReq,
    ) -> Result<i64> {
        let row = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"INSERT INTO customer_contacts (customer_id, contact_name, phone, email, position, is_primary)
               VALUES ($1, $2, $3, $4, $5, $6)
               RETURNING contact_id"#,
        )
        .bind(customer_id)
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
        req: &UpdateContactReq,
    ) -> Result<()> {
        let mut sets = Vec::new();
        let mut param_idx = 2u32;

        if req.contact_name.is_some() {
            sets.push(format!("contact_name = ${param_idx}"));
            param_idx += 1;
        }
        if req.phone.is_some() {
            sets.push(format!("phone = ${param_idx}"));
            param_idx += 1;
        }
        if req.email.is_some() {
            sets.push(format!("email = ${param_idx}"));
            param_idx += 1;
        }
        if req.position.is_some() {
            sets.push(format!("position = ${param_idx}"));
            param_idx += 1;
        }
        if req.is_primary.is_some() {
            sets.push(format!("is_primary = ${param_idx}"));
            param_idx += 1;
        }

        if sets.is_empty() {
            return Ok(());
        }

        let sql = format!(
            "UPDATE customer_contacts SET {} WHERE contact_id = $1",
            sets.join(", ")
        );
        let mut q = sqlx::query(&sql).bind(contact_id);

        if let Some(ref v) = req.contact_name {
            q = q.bind(v);
        }
        if let Some(ref v) = req.phone {
            q = q.bind(v);
        }
        if let Some(ref v) = req.email {
            q = q.bind(v);
        }
        if let Some(ref v) = req.position {
            q = q.bind(v);
        }
        if let Some(v) = req.is_primary {
            q = q.bind(v);
        }

        q.execute(executor).await?;
        Ok(())
    }

    pub async fn delete(&self, executor: PgExecutor<'_>, contact_id: i64) -> Result<()> {
        sqlx::query("DELETE FROM customer_contacts WHERE contact_id = $1")
            .bind(contact_id)
            .execute(executor)
            .await?;
        Ok(())
    }

    pub async fn find_by_id(
        &self,
        executor: PgExecutor<'_>,
        contact_id: i64,
    ) -> Result<Option<CustomerContact>> {
        let contact = sqlx::query_as::<sqlx::Postgres, CustomerContact>(
            &format!("SELECT {CONTACT_COLUMNS} FROM customer_contacts WHERE contact_id = $1"),
        )
        .bind(contact_id)
        .fetch_optional(executor)
        .await?;
        Ok(contact)
    }

    pub async fn find_by_customer_id(
        &self,
        executor: PgExecutor<'_>,
        customer_id: i64,
    ) -> Result<Vec<CustomerContact>> {
        let contacts = sqlx::query_as::<sqlx::Postgres, CustomerContact>(
            &format!("SELECT {CONTACT_COLUMNS} FROM customer_contacts WHERE customer_id = $1 ORDER BY is_primary DESC, contact_id"),
        )
        .bind(customer_id)
        .fetch_all(executor)
        .await?;
        Ok(contacts)
    }
}

// ---------------------------------------------------------------------------
// CustomerAddressRepo
// ---------------------------------------------------------------------------

const ADDRESS_COLUMNS: &str = "address_id, customer_id, address_type, province, city, district, detail, contact_name, contact_phone, is_default";

pub struct CustomerAddressRepo;

impl CustomerAddressRepo {
    pub async fn create(
        &self,
        executor: PgExecutor<'_>,
        customer_id: i64,
        req: &CreateAddressReq,
    ) -> Result<i64> {
        let row = sqlx::query_scalar::<sqlx::Postgres, i64>(
            r#"INSERT INTO customer_addresses (customer_id, address_type, province, city, district, detail, contact_name, contact_phone, is_default)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
               RETURNING address_id"#,
        )
        .bind(customer_id)
        .bind(&req.address_type)
        .bind(&req.province)
        .bind(&req.city)
        .bind(&req.district)
        .bind(&req.detail)
        .bind(&req.contact_name)
        .bind(&req.contact_phone)
        .bind(req.is_default)
        .fetch_one(executor)
        .await?;
        Ok(row)
    }

    #[allow(unused_assignments)]
    pub async fn update(
        &self,
        executor: PgExecutor<'_>,
        address_id: i64,
        req: &UpdateAddressReq,
    ) -> Result<()> {
        let mut sets = Vec::new();
        let mut param_idx = 2u32;

        if req.address_type.is_some() {
            sets.push(format!("address_type = ${param_idx}"));
            param_idx += 1;
        }
        if req.province.is_some() {
            sets.push(format!("province = ${param_idx}"));
            param_idx += 1;
        }
        if req.city.is_some() {
            sets.push(format!("city = ${param_idx}"));
            param_idx += 1;
        }
        if req.district.is_some() {
            sets.push(format!("district = ${param_idx}"));
            param_idx += 1;
        }
        if req.detail.is_some() {
            sets.push(format!("detail = ${param_idx}"));
            param_idx += 1;
        }
        if req.contact_name.is_some() {
            sets.push(format!("contact_name = ${param_idx}"));
            param_idx += 1;
        }
        if req.contact_phone.is_some() {
            sets.push(format!("contact_phone = ${param_idx}"));
            param_idx += 1;
        }
        if req.is_default.is_some() {
            sets.push(format!("is_default = ${param_idx}"));
            param_idx += 1;
        }

        if sets.is_empty() {
            return Ok(());
        }

        let sql = format!(
            "UPDATE customer_addresses SET {} WHERE address_id = $1",
            sets.join(", ")
        );
        let mut q = sqlx::query(&sql).bind(address_id);

        if let Some(ref v) = req.address_type {
            q = q.bind(v);
        }
        if let Some(ref v) = req.province {
            q = q.bind(v);
        }
        if let Some(ref v) = req.city {
            q = q.bind(v);
        }
        if let Some(ref v) = req.district {
            q = q.bind(v);
        }
        if let Some(ref v) = req.detail {
            q = q.bind(v);
        }
        if let Some(ref v) = req.contact_name {
            q = q.bind(v);
        }
        if let Some(ref v) = req.contact_phone {
            q = q.bind(v);
        }
        if let Some(v) = req.is_default {
            q = q.bind(v);
        }

        q.execute(executor).await?;
        Ok(())
    }

    pub async fn delete(&self, executor: PgExecutor<'_>, address_id: i64) -> Result<()> {
        sqlx::query("DELETE FROM customer_addresses WHERE address_id = $1")
            .bind(address_id)
            .execute(executor)
            .await?;
        Ok(())
    }

    pub async fn find_by_id(
        &self,
        executor: PgExecutor<'_>,
        address_id: i64,
    ) -> Result<Option<CustomerAddress>> {
        let address = sqlx::query_as::<sqlx::Postgres, CustomerAddress>(
            &format!("SELECT {ADDRESS_COLUMNS} FROM customer_addresses WHERE address_id = $1"),
        )
        .bind(address_id)
        .fetch_optional(executor)
        .await?;
        Ok(address)
    }

    pub async fn find_by_customer_id(
        &self,
        executor: PgExecutor<'_>,
        customer_id: i64,
    ) -> Result<Vec<CustomerAddress>> {
        let addresses = sqlx::query_as::<sqlx::Postgres, CustomerAddress>(
            &format!("SELECT {ADDRESS_COLUMNS} FROM customer_addresses WHERE customer_id = $1 ORDER BY is_default DESC, address_id"),
        )
        .bind(customer_id)
        .fetch_all(executor)
        .await?;
        Ok(addresses)
    }
}
