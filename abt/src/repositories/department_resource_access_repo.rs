use anyhow::Result;
use sqlx::PgPool;

use crate::models::{is_business_resource, is_system_resource, BUSINESS_RESOURCE_CODES};
use crate::repositories::{DepartmentRepo, Executor};
use std::collections::HashSet;

pub struct DepartmentResourceAccessRepo;

impl DepartmentResourceAccessRepo {
    /// Get resource codes accessible to a single department
    pub async fn get_department_resources(
        pool: &PgPool,
        department_id: i64,
    ) -> Result<Vec<String>> {
        let codes: Vec<String> = sqlx::query_scalar(
            r#"
            SELECT resource_code
            FROM department_resource_access
            WHERE department_id = $1
            ORDER BY resource_code
            "#,
        )
        .bind(department_id)
        .fetch_all(pool)
        .await?;

        Ok(codes)
    }

    /// Get UNION of resource codes accessible across multiple departments (R8)
    pub async fn get_departments_accessible_resources(
        pool: &PgPool,
        department_ids: &[i64],
    ) -> Result<Vec<String>> {
        if department_ids.is_empty() {
            return Ok(vec![]);
        }

        let codes: Vec<String> = sqlx::query_scalar(
            r#"
            SELECT DISTINCT resource_code
            FROM department_resource_access
            WHERE department_id = ANY($1)
            ORDER BY resource_code
            "#,
        )
        .bind(department_ids)
        .fetch_all(pool)
        .await?;

        Ok(codes)
    }

    /// Get the default department ID (where is_default = true)
    pub async fn get_default_department_id(pool: &PgPool) -> Result<Option<i64>> {
        let id: Option<i64> = sqlx::query_scalar(
            r#"
            SELECT department_id
            FROM departments
            WHERE is_default = true AND is_active = true
            LIMIT 1
            "#,
        )
        .fetch_optional(pool)
        .await?;

        Ok(id)
    }

    /// Set department resource codes with full-overwrite semantics (R11)
    pub async fn set_department_resources(
        executor: Executor<'_>,
        department_id: i64,
        resource_codes: &[String],
    ) -> Result<()> {
        // Delete existing
        sqlx::query(
            "DELETE FROM department_resource_access WHERE department_id = $1",
        )
        .bind(department_id)
        .execute(&mut *executor)
        .await?;

        // Insert new
        if !resource_codes.is_empty() {
            sqlx::query(
                r#"
                INSERT INTO department_resource_access (department_id, resource_code)
                SELECT $1, unnest($2::varchar[])
                "#,
            )
            .bind(department_id)
            .bind(resource_codes)
            .execute(&mut *executor)
            .await?;
        }

        Ok(())
    }

    /// Resolve the accessible resource codes for a user, accounting for
    /// department membership, default department fallback, and fail-closed.
    /// Returns a HashSet for efficient lookup during permission filtering.
    pub async fn resolve_user_accessible_resources(
        pool: &PgPool,
        user_id: i64,
    ) -> Result<HashSet<String>> {
        // 1. Get user's explicit department memberships
        let mut dept_ids = DepartmentRepo::get_user_department_ids(pool, user_id).await?;

        // 2. Fallback to default department if no memberships (R6)
        if dept_ids.is_empty()
            && let Some(default_id) = Self::get_default_department_id(pool).await? {
                dept_ids = vec![default_id];
            }

        // 3. Fail-closed if still no departments (R5b)
        if dept_ids.is_empty() {
            return Ok(HashSet::new());
        }

        // 4. Get union of accessible resource codes (R8)
        let codes = Self::get_departments_accessible_resources(pool, &dept_ids).await?;
        Ok(codes.into_iter().collect())
    }

    /// Filter role permissions by department-accessible resource codes.
    /// System resource permissions always pass through (R4).
    /// Business resource permissions are kept only if the resource code
    /// is in the user's department-accessible set.
    pub fn filter_permissions_by_department(
        permissions: Vec<String>,
        accessible_resources: &HashSet<String>,
    ) -> Vec<String> {
        permissions
            .into_iter()
            .filter(|perm| {
                let resource_code = perm.split(':').next().unwrap_or("");
                is_system_resource(resource_code) || accessible_resources.contains(resource_code)
            })
            .collect()
    }

    /// Validate and filter resource codes for SetDepartmentResources (R11b).
    /// Returns Ok(business_codes) if valid, Err if any unknown code found.
    /// System codes are silently filtered out.
    pub fn validate_resource_codes(codes: &[String]) -> Result<Vec<String>> {
        let mut business_codes = Vec::new();
        for code in codes {
            if is_system_resource(code) {
                continue; // silently ignore system codes
            }
            if is_business_resource(code) {
                business_codes.push(code.clone());
            } else {
                return Err(anyhow::anyhow!("Unknown resource code: {}", code));
            }
        }
        Ok(business_codes)
    }

    /// Returns the list of all known business resource codes
    pub fn get_business_resource_codes() -> &'static [&'static str] {
        BUSINESS_RESOURCE_CODES
    }
}
