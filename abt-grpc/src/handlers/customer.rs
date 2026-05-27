//! Customer gRPC Handler — 委托给 abt-core CustomerService

use abt_core::master_data::customer::CustomerService;
use abt_core::shared::types::{PageParams, ServiceContext};
use crate::error;
use tonic::{Request, Response};

use crate::generated::abt::v1::{
    customer_service_server::CustomerService as GrpcCustomerService,
    *,
};
use crate::handlers::{domain_to_status, GrpcResult};
use crate::interceptors::auth::extract_auth;
use crate::server::AppState;

pub struct CustomerHandler;

impl CustomerHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CustomerHandler {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_decimal(s: &str) -> rust_decimal::Decimal {
    s.parse().unwrap_or(rust_decimal::Decimal::ZERO)
}

fn decimal_to_string(d: Option<rust_decimal::Decimal>) -> String {
    d.map(|d| d.to_string()).unwrap_or_default()
}

fn category_to_proto(c: abt_core::master_data::customer::model::CustomerCategory) -> CustomerCategory {
    match c {
        abt_core::master_data::customer::model::CustomerCategory::Distributor => CustomerCategory::Distributor,
        abt_core::master_data::customer::model::CustomerCategory::DirectCustomer => CustomerCategory::Direct,
        abt_core::master_data::customer::model::CustomerCategory::OEM => CustomerCategory::Oem,
        abt_core::master_data::customer::model::CustomerCategory::Retailer => CustomerCategory::Retailer,
    }
}

fn status_to_proto(s: abt_core::master_data::customer::model::CustomerStatus) -> CustomerStatus {
    match s {
        abt_core::master_data::customer::model::CustomerStatus::Prospective => CustomerStatus::Prospective,
        abt_core::master_data::customer::model::CustomerStatus::Active => CustomerStatus::Active,
        abt_core::master_data::customer::model::CustomerStatus::Inactive => CustomerStatus::Inactive,
        abt_core::master_data::customer::model::CustomerStatus::Blacklisted => CustomerStatus::Blacklisted,
    }
}

fn contact_to_proto(c: &abt_core::master_data::customer::model::CustomerContact) -> CustomerContact {
    CustomerContact {
        contact_id: c.id,
        customer_id: c.customer_id,
        contact_name: c.name.clone(),
        position: c.position.clone().unwrap_or_default(),
        phone: c.phone.clone().unwrap_or_default(),
        email: c.email.clone().unwrap_or_default(),
        is_primary: c.is_primary,
    }
}

fn address_to_proto(a: &abt_core::master_data::customer::model::CustomerAddress) -> CustomerAddress {
    CustomerAddress {
        address_id: a.id,
        customer_id: a.customer_id,
        address_type: a.address_type.clone(),
        province: a.province.clone(),
        city: a.city.clone(),
        district: a.district.clone().unwrap_or_default(),
        detail: a.detail.clone(),
        contact_name: a.contact_name.clone().unwrap_or_default(),
        contact_phone: a.contact_phone.clone().unwrap_or_default(),
        is_default: a.is_default,
    }
}

fn customer_to_proto(c: &abt_core::master_data::customer::model::Customer) -> Customer {
    Customer {
        customer_id: c.id,
        customer_code: c.code.clone(),
        customer_name: c.name.clone(),
        short_name: c.short_name.clone().unwrap_or_default(),
        category: category_to_proto(c.category) as i32,
        status: status_to_proto(c.status) as i32,
        tax_number: c.tax_number.clone().unwrap_or_default(),
        invoice_title: c.invoice_title.clone().unwrap_or_default(),
        credit_limit: decimal_to_string(c.credit_limit),
        payment_terms: c.payment_terms.clone().unwrap_or_default(),
        receivable_account: c.receivable_account.clone().unwrap_or_default(),
        owner_id: c.owner_id.unwrap_or(0),
        department_id: c.department_id.unwrap_or(0),
        remark: c.remark.clone(),
        operator_id: c.operator_id,
        created_at: c.created_at.timestamp(),
        updated_at: c.updated_at.timestamp(),
        contacts: vec![],
        addresses: vec![],
    }
}

#[tonic::async_trait]
impl GrpcCustomerService for CustomerHandler {
    async fn create_customer(
        &self,
        request: Request<CreateCustomerRequest>,
    ) -> GrpcResult<U64Response> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.customer_core_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;
        let ctx = ServiceContext::new(&mut tx, auth.user_id);

        let category = abt_core::master_data::customer::model::CustomerCategory::from_i16(req.category as i16)
            .unwrap_or(abt_core::master_data::customer::model::CustomerCategory::DirectCustomer);

        let create_req = abt_core::master_data::customer::model::CreateCustomerReq {
            customer_name: req.customer_name,
            short_name: if req.short_name.is_empty() { None } else { Some(req.short_name) },
            category,
            tax_number: if req.tax_number.is_empty() { None } else { Some(req.tax_number) },
            invoice_title: if req.invoice_title.is_empty() { None } else { Some(req.invoice_title) },
            credit_limit: if req.credit_limit.is_empty() { None } else { Some(parse_decimal(&req.credit_limit)) },
            payment_terms: if req.payment_terms.is_empty() { None } else { Some(req.payment_terms) },
            receivable_account: if req.receivable_account.is_empty() { None } else { Some(req.receivable_account) },
            remark: if req.remark.is_empty() { None } else { Some(req.remark) },
        };

        let id = srv.create(ctx, create_req).await
            .map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    async fn update_customer(
        &self,
        request: Request<UpdateCustomerRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.customer_core_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;
        let ctx = ServiceContext::new(&mut tx, auth.user_id);

        let update_req = abt_core::master_data::customer::model::UpdateCustomerReq {
            customer_name: if req.customer_name.is_empty() { None } else { Some(req.customer_name) },
            short_name: if req.short_name.is_empty() { None } else { Some(req.short_name) },
            category: abt_core::master_data::customer::model::CustomerCategory::from_i16(req.category as i16),
            status: abt_core::master_data::customer::model::CustomerStatus::from_i16(req.status as i16),
            tax_number: if req.tax_number.is_empty() { None } else { Some(req.tax_number) },
            invoice_title: if req.invoice_title.is_empty() { None } else { Some(req.invoice_title) },
            credit_limit: if req.credit_limit.is_empty() { None } else { Some(parse_decimal(&req.credit_limit)) },
            payment_terms: if req.payment_terms.is_empty() { None } else { Some(req.payment_terms) },
            receivable_account: if req.receivable_account.is_empty() { None } else { Some(req.receivable_account) },
            remark: if req.remark.is_empty() { None } else { Some(req.remark) },
        };

        srv.update(ctx, req.customer_id, update_req).await
            .map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    async fn get_customer(
        &self,
        request: Request<GetCustomerRequest>,
    ) -> GrpcResult<CustomerResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.customer_core_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;
        let ctx = ServiceContext::new(&mut tx, 0);

        let c = srv.get(ctx, req.customer_id).await
            .map_err(domain_to_status)?;

        let mut proto = customer_to_proto(&c);

        // Load contacts and addresses
        let ctx2 = ServiceContext::new(&mut tx, 0);
        let contacts = srv.list_contacts(ctx2, c.id).await
            .map_err(domain_to_status)?;
        proto.contacts = contacts.iter().map(contact_to_proto).collect();

        let ctx3 = ServiceContext::new(&mut tx, 0);
        let addresses = srv.list_addresses(ctx3, c.id).await
            .map_err(domain_to_status)?;
        proto.addresses = addresses.iter().map(address_to_proto).collect();

        Ok(Response::new(CustomerResponse { customer: Some(proto) }))
    }

    async fn list_customers(
        &self,
        request: Request<ListCustomersRequest>,
    ) -> GrpcResult<CustomerListResponse> {
        // Extract auth before into_inner() consumes the request
        let (data_scope, operator_id) = match extract_auth(&request) {
            Ok(auth) if auth.is_super_admin() => (abt_core::shared::types::DataScope::All, auth.user_id),
            Ok(auth) => (abt_core::shared::types::DataScope::SelfOnly, auth.user_id),
            Err(_) => (abt_core::shared::types::DataScope::All, 0),
        };

        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.customer_core_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, operator_id)
            .with_data_scope(data_scope);

        let filter = abt_core::master_data::customer::model::CustomerQuery {
            name: if req.keyword.is_empty() { None } else { Some(req.keyword) },
            status: abt_core::master_data::customer::model::CustomerStatus::from_i16(req.status as i16),
            category: abt_core::master_data::customer::model::CustomerCategory::from_i16(req.category as i16),
            owner_id: None,
        };
        let page = PageParams::new(
            req.pagination.as_ref().map(|p| p.page).unwrap_or(1),
            req.pagination.as_ref().map(|p| p.page_size).unwrap_or(20),
        );

        let result = srv.list(ctx, filter, page).await
            .map_err(domain_to_status)?;

        Ok(Response::new(CustomerListResponse {
            items: result.items.iter().map(customer_to_proto).collect(),
            pagination: Some(PaginationInfo {
                total: result.total,
                page: result.page,
                page_size: result.page_size,
                total_pages: result.total_pages,
            }),
        }))
    }

    // --- Contacts ---

    async fn create_contact(
        &self,
        request: Request<CreateContactRequest>,
    ) -> GrpcResult<U64Response> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.customer_core_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;
        let ctx = ServiceContext::new(&mut tx, auth.user_id);

        let create_req = abt_core::master_data::customer::model::CreateContactReq {
            contact_name: req.contact_name,
            phone: if req.phone.is_empty() { None } else { Some(req.phone) },
            email: if req.email.is_empty() { None } else { Some(req.email) },
            position: if req.position.is_empty() { None } else { Some(req.position) },
            is_primary: req.is_primary,
        };

        let id = srv.add_contact(ctx, req.customer_id, create_req).await
            .map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    async fn update_contact(
        &self,
        request: Request<UpdateContactRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.customer_core_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;
        let ctx = ServiceContext::new(&mut tx, auth.user_id);

        let update_req = abt_core::master_data::customer::model::UpdateContactReq {
            contact_name: if req.contact_name.is_empty() { None } else { Some(req.contact_name) },
            phone: if req.phone.is_empty() { None } else { Some(req.phone) },
            email: if req.email.is_empty() { None } else { Some(req.email) },
            position: if req.position.is_empty() { None } else { Some(req.position) },
            is_primary: Some(req.is_primary),
        };

        srv.update_contact(ctx, req.customer_id, req.contact_id, update_req).await
            .map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    async fn delete_contact(
        &self,
        request: Request<DeleteContactRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.customer_core_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;
        let ctx = ServiceContext::new(&mut tx, auth.user_id);

        srv.delete_contact(ctx, req.customer_id, req.contact_id).await
            .map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    async fn list_contacts(
        &self,
        request: Request<ListContactsRequest>,
    ) -> GrpcResult<ContactListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.customer_core_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;
        let ctx = ServiceContext::new(&mut tx, 0);

        let contacts = srv.list_contacts(ctx, req.customer_id).await
            .map_err(domain_to_status)?;

        Ok(Response::new(ContactListResponse {
            items: contacts.iter().map(contact_to_proto).collect(),
        }))
    }

    // --- Addresses ---

    async fn create_address(
        &self,
        request: Request<CreateAddressRequest>,
    ) -> GrpcResult<U64Response> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.customer_core_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;
        let ctx = ServiceContext::new(&mut tx, auth.user_id);

        let create_req = abt_core::master_data::customer::model::CreateAddressReq {
            address_type: req.address_type,
            province: req.province,
            city: req.city,
            district: if req.district.is_empty() { None } else { Some(req.district) },
            detail: req.detail,
            contact_name: if req.contact_name.is_empty() { None } else { Some(req.contact_name) },
            contact_phone: if req.contact_phone.is_empty() { None } else { Some(req.contact_phone) },
            is_default: req.is_default,
        };

        let id = srv.add_address(ctx, req.customer_id, create_req).await
            .map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    async fn update_address(
        &self,
        request: Request<UpdateAddressRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.customer_core_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;
        let ctx = ServiceContext::new(&mut tx, auth.user_id);

        let update_req = abt_core::master_data::customer::model::UpdateAddressReq {
            address_type: if req.address_type.is_empty() { None } else { Some(req.address_type) },
            province: if req.province.is_empty() { None } else { Some(req.province) },
            city: if req.city.is_empty() { None } else { Some(req.city) },
            district: if req.district.is_empty() { None } else { Some(req.district) },
            detail: if req.detail.is_empty() { None } else { Some(req.detail) },
            contact_name: if req.contact_name.is_empty() { None } else { Some(req.contact_name) },
            contact_phone: if req.contact_phone.is_empty() { None } else { Some(req.contact_phone) },
            is_default: Some(req.is_default),
        };

        srv.update_address(ctx, req.customer_id, req.address_id, update_req).await
            .map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    async fn delete_address(
        &self,
        request: Request<DeleteAddressRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.customer_core_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;
        let ctx = ServiceContext::new(&mut tx, auth.user_id);

        srv.delete_address(ctx, req.customer_id, req.address_id).await
            .map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    async fn list_addresses(
        &self,
        request: Request<ListAddressesRequest>,
    ) -> GrpcResult<AddressListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.customer_core_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;
        let ctx = ServiceContext::new(&mut tx, 0);

        let addresses = srv.list_addresses(ctx, req.customer_id).await
            .map_err(domain_to_status)?;

        Ok(Response::new(AddressListResponse {
            items: addresses.iter().map(address_to_proto).collect(),
        }))
    }
}
