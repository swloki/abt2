//! Supplier gRPC Handler

use common::error;
use tonic::{Request, Response};

use crate::generated::abt::v1::{
    supplier_service_server::SupplierService as GrpcSupplierService,
    *,
};
use crate::handlers::GrpcResult;
use crate::interceptors::auth::extract_auth;
use crate::server::AppState;
use abt_macros::require_permission;
use crate::permissions::PermissionCode;

use abt::SupplierService;

pub struct SupplierHandler;

impl SupplierHandler {
    pub fn new() -> Self { Self }
}

impl Default for SupplierHandler {
    fn default() -> Self { Self::new() }
}

fn classification_to_i16(cls: i32) -> String {
    match SupplierClassification::try_from(cls).unwrap_or(SupplierClassification::Unspecified) {
        SupplierClassification::A => "A".to_string(),
        SupplierClassification::B => "B".to_string(),
        SupplierClassification::C => "C".to_string(),
        _ => "UNSPECIFIED".to_string(),
    }
}

fn classification_to_proto(cls: &str) -> i32 {
    match cls {
        "A" => SupplierClassification::A as i32,
        "B" => SupplierClassification::B as i32,
        "C" => SupplierClassification::C as i32,
        _ => SupplierClassification::Unspecified as i32,
    }
}

fn supplier_status_to_proto(status: i16) -> i32 {
    match status {
        1 => SupplierStatus::Pending as i32,
        2 => SupplierStatus::Qualified as i32,
        3 => SupplierStatus::Disabled as i32,
        _ => SupplierStatus::Unspecified as i32,
    }
}

fn proto_status_to_i16(status: i32) -> i16 {
    match SupplierStatus::try_from(status).unwrap_or(SupplierStatus::Unspecified) {
        SupplierStatus::Pending => 1,
        SupplierStatus::Qualified => 2,
        SupplierStatus::Disabled => 3,
        _ => 0,
    }
}

fn supplier_to_proto(detail: &abt::models::SupplierDetail) -> Supplier {
    let s = &detail.supplier;
    Supplier {
        supplier_id: s.supplier_id,
        supplier_code: s.supplier_code.clone(),
        supplier_name: s.supplier_name.clone(),
        short_name: s.short_name.clone().unwrap_or_default(),
        classification: classification_to_proto(&s.classification),
        status: supplier_status_to_proto(s.status),
        remark: s.remark.clone().unwrap_or_default(),
        operator_id: s.operator_id.unwrap_or(0),
        created_at: s.created_at.timestamp(),
        updated_at: s.updated_at.timestamp(),
        contacts: detail.contacts.iter().map(|c| SupplierContact {
            contact_id: c.contact_id,
            supplier_id: c.supplier_id,
            contact_name: c.contact_name.clone(),
            phone: c.phone.clone().unwrap_or_default(),
            email: c.email.clone().unwrap_or_default(),
            position: c.position.clone().unwrap_or_default(),
            is_primary: c.is_primary,
        }).collect(),
        bank_accounts: detail.bank_accounts.iter().map(|b| SupplierBankAccount {
            bank_account_id: b.bank_account_id,
            supplier_id: b.supplier_id,
            bank_name: b.bank_name.clone(),
            account_name: b.account_name.clone(),
            account_no: b.account_no.clone(),
            is_default: b.is_default,
        }).collect(),
    }
}

fn supplier_brief_to_proto(s: &abt::models::Supplier) -> Supplier {
    Supplier {
        supplier_id: s.supplier_id,
        supplier_code: s.supplier_code.clone(),
        supplier_name: s.supplier_name.clone(),
        short_name: s.short_name.clone().unwrap_or_default(),
        classification: classification_to_proto(&s.classification),
        status: supplier_status_to_proto(s.status),
        remark: s.remark.clone().unwrap_or_default(),
        operator_id: s.operator_id.unwrap_or(0),
        created_at: s.created_at.timestamp(),
        updated_at: s.updated_at.timestamp(),
        contacts: vec![],
        bank_accounts: vec![],
    }
}

#[tonic::async_trait]
impl GrpcSupplierService for SupplierHandler {
    #[require_permission(Resource::Supplier, Action::Write)]
    async fn create_supplier(&self, request: Request<CreateSupplierRequest>) -> GrpcResult<U64Response> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.supplier_service();
        let mut tx = state.begin_transaction().await.map_err(error::err_to_status)?;

        let contacts: Vec<abt::SupplierContactInput> = req.contacts
            .into_iter()
            .map(|c| abt::SupplierContactInput {
                contact_name: c.contact_name,
                phone: if c.phone.is_empty() { None } else { Some(c.phone) },
                email: if c.email.is_empty() { None } else { Some(c.email) },
                position: if c.position.is_empty() { None } else { Some(c.position) },
                is_primary: c.is_primary,
            })
            .collect();

        let bank_accounts: Vec<abt::SupplierBankAccountInput> = req.bank_accounts
            .into_iter()
            .map(|b| abt::SupplierBankAccountInput {
                bank_name: b.bank_name,
                account_name: b.account_name,
                account_no: b.account_no,
                is_default: b.is_default,
            })
            .collect();

        let id = srv.create(
            req.supplier_code,
            req.supplier_name,
            if req.short_name.is_empty() { None } else { Some(req.short_name) },
            classification_to_i16(req.classification),
            if req.remark.is_empty() { None } else { Some(req.remark) },
            Some(auth.user_id),
            contacts,
            bank_accounts,
            &mut tx,
        ).await.map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    #[require_permission(Resource::Supplier, Action::Write)]
    async fn update_supplier(&self, request: Request<UpdateSupplierRequest>) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.supplier_service();
        let mut tx = state.begin_transaction().await.map_err(error::err_to_status)?;

        let contacts: Vec<abt::SupplierContactInput> = req.contacts
            .into_iter()
            .map(|c| abt::SupplierContactInput {
                contact_name: c.contact_name,
                phone: if c.phone.is_empty() { None } else { Some(c.phone) },
                email: if c.email.is_empty() { None } else { Some(c.email) },
                position: if c.position.is_empty() { None } else { Some(c.position) },
                is_primary: c.is_primary,
            })
            .collect();

        let bank_accounts: Vec<abt::SupplierBankAccountInput> = req.bank_accounts
            .into_iter()
            .map(|b| abt::SupplierBankAccountInput {
                bank_name: b.bank_name,
                account_name: b.account_name,
                account_no: b.account_no,
                is_default: b.is_default,
            })
            .collect();

        srv.update(
            req.supplier_id,
            req.supplier_name,
            if req.short_name.is_empty() { None } else { Some(req.short_name) },
            classification_to_i16(req.classification),
            if req.remark.is_empty() { None } else { Some(req.remark) },
            contacts,
            bank_accounts,
            &mut tx,
        ).await.map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::Supplier, Action::Delete)]
    async fn delete_supplier(&self, request: Request<DeleteRequest>) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.supplier_service();
        let mut tx = state.begin_transaction().await.map_err(error::err_to_status)?;

        srv.delete(req.id, &mut tx).await.map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::Supplier, Action::Read)]
    async fn get_supplier(&self, request: Request<GetSupplierRequest>) -> GrpcResult<SupplierResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.supplier_service();

        let detail = srv.get_by_id(req.supplier_id).await
            .map_err(error::err_to_status)?
            .ok_or_else(|| error::not_found("Supplier", &req.supplier_id.to_string()))?;

        Ok(Response::new(SupplierResponse {
            supplier: Some(supplier_to_proto(&detail)),
        }))
    }

    #[require_permission(Resource::Supplier, Action::Read)]
    async fn list_suppliers(&self, request: Request<ListSuppliersRequest>) -> GrpcResult<SupplierListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.supplier_service();

        let pagination = req.pagination.unwrap_or(PaginationParams { page: 1, page_size: 20 });

        let query = abt::models::SupplierQuery {
            keyword: req.keyword,
            classification: req.classification.map(classification_to_i16),
            status: req.status.map(proto_status_to_i16),
            page: Some(pagination.page as i64),
            page_size: Some(pagination.page_size as i64),
        };

        let result = srv.list(query).await.map_err(error::err_to_status)?;

        Ok(Response::new(SupplierListResponse {
            items: result.items.into_iter().map(|s| supplier_brief_to_proto(&s)).collect(),
            pagination: Some(PaginationInfo {
                total: result.total,
                page: result.page,
                page_size: result.page_size,
                total_pages: result.total_pages,
            }),
        }))
    }

    #[require_permission(Resource::Supplier, Action::Write)]
    async fn update_supplier_status(&self, request: Request<UpdateSupplierStatusRequest>) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.supplier_service();
        let mut tx = state.begin_transaction().await.map_err(error::err_to_status)?;

        let status = proto_status_to_i16(req.status);
        srv.update_status(req.supplier_id, status, &mut tx).await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }
}
