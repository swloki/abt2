//! 工艺路线 gRPC Handler — 委托给 abt-core RoutingService

use abt_core::master_data::routing::RoutingService;
use abt_core::shared::types::{PageParams, ServiceContext};
use abt_macros::require_permission;
use crate::error;
use tonic::{Request, Response};

use crate::generated::abt::v1::{
    abt_routing_service_server::AbtRoutingService as GrpRoutingService, *,
};
use crate::handlers::{domain_to_status, empty_to_none, GrpcResult};
use crate::interceptors::auth::extract_auth;
use crate::permissions::PermissionCode;
use crate::server::AppState;

pub struct RoutingHandler;

impl RoutingHandler {
    pub fn new() -> Self { Self }
}

impl Default for RoutingHandler {
    fn default() -> Self { Self::new() }
}

#[tonic::async_trait]
impl GrpRoutingService for RoutingHandler {
    #[require_permission(Resource::Routing, Action::Read)]
    async fn list_routings(
        &self,
        request: Request<ListRoutingsRequest>,
    ) -> GrpcResult<RoutingListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.routing_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, 0);
        let query = abt_core::master_data::routing::RoutingQuery {
            keyword: req.keyword,
        };
        let page = PageParams::new(req.page.unwrap_or(1), req.page_size.unwrap_or(50));
        let result = srv.list(ctx, query, page).await.map_err(domain_to_status)?;

        Ok(Response::new(RoutingListResponse {
            items: result.items.into_iter().map(|r| RoutingProto {
                id: r.id,
                name: r.name,
                description: r.description.unwrap_or_default(),
            }).collect(),
            total: result.total as u64,
        }))
    }

    #[require_permission(Resource::Routing, Action::Read)]
    async fn get_routing_detail(
        &self,
        request: Request<GetRoutingDetailRequest>,
    ) -> GrpcResult<RoutingDetailResponse> {
        let req = request.into_inner();
        if req.id <= 0 {
            return Err(error::validation("id", "ID 无效"));
        }

        let state = AppState::get().await;
        let srv = state.routing_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, 0);
        let detail = srv.get_detail(ctx, req.id).await.map_err(domain_to_status)?;

        Ok(Response::new(RoutingDetailResponse {
            routing: Some(RoutingProto {
                id: detail.routing.id,
                name: detail.routing.name,
                description: detail.routing.description.unwrap_or_default(),
            }),
            steps: detail.steps.into_iter().map(|s| RoutingStepProto {
                id: s.id,
                routing_id: s.routing_id,
                process_code: s.process_code,
                step_order: s.step_order,
                is_required: s.is_required,
                remark: s.remark.unwrap_or_default(),
            }).collect(),
        }))
    }

    #[require_permission(Resource::Routing, Action::Write)]
    async fn create_routing(
        &self,
        request: Request<CreateRoutingRequest>,
    ) -> GrpcResult<U64Response> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        if req.name.is_empty() {
            return Err(error::validation("name", "路线名称不能为空"));
        }

        let state = AppState::get().await;
        let srv = state.routing_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        let id = srv.create(ctx, abt_core::master_data::routing::CreateRoutingReq {
            name: req.name,
            description: empty_to_none(req.description),
            steps: req.steps.into_iter().map(|s| abt_core::master_data::routing::RoutingStepInput {
                process_code: s.process_code,
                step_order: s.step_order,
                is_required: s.is_required,
                remark: empty_to_none(s.remark),
            }).collect(),
        }).await.map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    #[require_permission(Resource::Routing, Action::Write)]
    async fn update_routing(
        &self,
        request: Request<UpdateRoutingRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        if req.id <= 0 {
            return Err(error::validation("id", "ID 无效"));
        }
        if req.name.is_empty() {
            return Err(error::validation("name", "路线名称不能为空"));
        }

        let state = AppState::get().await;
        let srv = state.routing_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        srv.update(ctx, req.id, abt_core::master_data::routing::UpdateRoutingReq {
            name: Some(req.name),
            description: empty_to_none(req.description),
            steps: Some(req.steps.into_iter().map(|s| abt_core::master_data::routing::RoutingStepInput {
                process_code: s.process_code,
                step_order: s.step_order,
                is_required: s.is_required,
                remark: empty_to_none(s.remark),
            }).collect()),
        }).await.map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::Routing, Action::Delete)]
    async fn delete_routing(
        &self,
        request: Request<DeleteRoutingRequest>,
    ) -> GrpcResult<U64Response> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        if req.id <= 0 {
            return Err(error::validation("id", "ID 无效"));
        }

        let state = AppState::get().await;
        let srv = state.routing_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        srv.delete(ctx, req.id).await.map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: 1 }))
    }

    #[require_permission(Resource::Routing, Action::Write)]
    async fn set_bom_routing(
        &self,
        request: Request<SetBomRoutingRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();
        if req.product_code.is_empty() {
            return Err(error::validation("product_code", "产品编码不能为空"));
        }
        if req.routing_id <= 0 {
            return Err(error::validation("routing_id", "路线 ID 无效"));
        }

        let state = AppState::get().await;
        let srv = state.routing_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        srv.set_bom_routing(ctx, req.product_code, req.routing_id)
            .await.map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::Routing, Action::Read)]
    async fn get_bom_routing(
        &self,
        request: Request<GetBomRoutingRequest>,
    ) -> GrpcResult<GetBomRoutingResponse> {
        let req = request.into_inner();
        if req.product_code.is_empty() {
            return Err(error::validation("product_code", "产品编码不能为空"));
        }

        let state = AppState::get().await;
        let srv = state.routing_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, 0);
        let result = srv.get_bom_routing(ctx, req.product_code)
            .await.map_err(domain_to_status)?;

        match result {
            Some(detail) => Ok(Response::new(GetBomRoutingResponse {
                routing_id: Some(detail.routing.id),
                routing_name: Some(detail.routing.name),
                steps: detail.steps.into_iter().map(|s| RoutingStepProto {
                    id: s.id,
                    routing_id: s.routing_id,
                    process_code: s.process_code,
                    step_order: s.step_order,
                    is_required: s.is_required,
                    remark: s.remark.unwrap_or_default(),
                }).collect(),
            })),
            None => Ok(Response::new(GetBomRoutingResponse {
                routing_id: None,
                routing_name: None,
                steps: vec![],
            })),
        }
    }

    #[require_permission(Resource::Routing, Action::Read)]
    async fn get_boms_by_routing(
        &self,
        request: Request<GetBomsByRoutingRequest>,
    ) -> GrpcResult<BomListByRoutingResponse> {
        let req = request.into_inner();
        if req.routing_id <= 0 {
            return Err(error::validation("routing_id", "路线 ID 无效"));
        }

        let state = AppState::get().await;
        let srv = state.routing_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, 0);
        let items = srv.list_boms_by_routing(ctx, req.routing_id)
            .await.map_err(domain_to_status)?;

        let total = items.len() as u64;
        Ok(Response::new(BomListByRoutingResponse {
            items: items.into_iter().map(|b| BomBriefProto {
                bom_id: b.id,
                bom_name: b.product_code,
                created_at: b.created_at.map(|t| t.to_rfc3339()).unwrap_or_default(),
            }).collect(),
            total,
        }))
    }
}
