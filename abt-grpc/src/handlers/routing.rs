//! 工艺路线 gRPC Handler

use abt::RoutingService;
use abt_macros::require_permission;
use common::error;
use tonic::{Request, Response};

use crate::generated::abt::v1::{
    abt_routing_service_server::AbtRoutingService as GrpRoutingService, *,
};
use crate::handlers::{empty_to_none, GrpcResult};
use crate::interceptors::auth::extract_auth;
use crate::permissions::PermissionCode;
use crate::server::AppState;

pub struct RoutingHandler;

impl RoutingHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RoutingHandler {
    fn default() -> Self {
        Self::new()
    }
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

        let query = abt::ListRoutingQuery {
            keyword: req.keyword,
            page: req.page.unwrap_or(1),
            page_size: req.page_size.unwrap_or(50),
        };

        let (items, total) = srv.list(query).await.map_err(error::err_to_status)?;

        Ok(Response::new(RoutingListResponse {
            items: items
                .into_iter()
                .map(|r| RoutingProto {
                    id: r.id,
                    name: r.name,
                    description: r.description.unwrap_or_default(),
                })
                .collect(),
            total: total as u64,
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

        let (routing, steps) = srv
            .get_detail(req.id)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(RoutingDetailResponse {
            routing: Some(RoutingProto {
                id: routing.id,
                name: routing.name,
                description: routing.description.unwrap_or_default(),
            }),
            steps: steps
                .into_iter()
                .map(|s| RoutingStepProto {
                    id: s.id,
                    routing_id: s.routing_id,
                    process_code: s.process_code,
                    step_order: s.step_order,
                    is_required: s.is_required,
                    remark: s.remark.unwrap_or_default(),
                })
                .collect(),
        }))
    }

    #[require_permission(Resource::Routing, Action::Write)]
    async fn create_routing(
        &self,
        request: Request<CreateRoutingRequest>,
    ) -> GrpcResult<U64Response> {
        let req = request.into_inner();

        if req.name.is_empty() {
            return Err(error::validation("name", "路线名称不能为空"));
        }

        let state = AppState::get().await;
        let srv = state.routing_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let id = srv
            .create(
                abt::CreateRoutingReq {
                    name: req.name,
                    description: empty_to_none(req.description),
                    steps: req
                        .steps
                        .into_iter()
                        .map(|s| abt::RoutingStepInput {
                            process_code: s.process_code,
                            step_order: s.step_order,
                            is_required: s.is_required,
                            remark: empty_to_none(s.remark),
                        })
                        .collect(),
                },
                &mut tx,
            )
            .await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    #[require_permission(Resource::Routing, Action::Write)]
    async fn update_routing(
        &self,
        request: Request<UpdateRoutingRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();

        if req.id <= 0 {
            return Err(error::validation("id", "ID 无效"));
        }
        if req.name.is_empty() {
            return Err(error::validation("name", "路线名称不能为空"));
        }

        let state = AppState::get().await;
        let srv = state.routing_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        srv.update(
            abt::UpdateRoutingReq {
                id: req.id,
                name: req.name,
                description: empty_to_none(req.description),
                steps: req
                    .steps
                    .into_iter()
                    .map(|s| abt::RoutingStepInput {
                        process_code: s.process_code,
                        step_order: s.step_order,
                        is_required: s.is_required,
                        remark: empty_to_none(s.remark),
                    })
                    .collect(),
            },
            &mut tx,
        )
        .await
        .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::Routing, Action::Delete)]
    async fn delete_routing(
        &self,
        request: Request<DeleteRoutingRequest>,
    ) -> GrpcResult<U64Response> {
        let req = request.into_inner();

        if req.id <= 0 {
            return Err(error::validation("id", "ID 无效"));
        }

        let state = AppState::get().await;
        let srv = state.routing_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let affected = srv
            .delete(req.id, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: affected }))
    }

    #[require_permission(Resource::Routing, Action::Write)]
    async fn set_bom_routing(
        &self,
        request: Request<SetBomRoutingRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();

        if req.product_code.is_empty() {
            return Err(error::validation("product_code", "产品编码不能为空"));
        }
        if req.routing_id <= 0 {
            return Err(error::validation("routing_id", "路线 ID 无效"));
        }

        let state = AppState::get().await;
        let srv = state.routing_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        srv.set_bom_routing(&req.product_code, req.routing_id, &mut tx)
            .await
            .map_err(error::err_to_status)?;

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

        let result = srv
            .get_bom_routing(&req.product_code)
            .await
            .map_err(error::err_to_status)?;

        match result {
            Some((routing_id, routing_name, steps)) => Ok(Response::new(GetBomRoutingResponse {
                routing_id: Some(routing_id),
                routing_name: Some(routing_name),
                steps: steps
                    .into_iter()
                    .map(|s| RoutingStepProto {
                        id: s.id,
                        routing_id: s.routing_id,
                        process_code: s.process_code,
                        step_order: s.step_order,
                        is_required: s.is_required,
                        remark: s.remark.unwrap_or_default(),
                    })
                    .collect(),
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

        let (items, total) = srv
            .list_boms_by_routing(
                req.routing_id,
                req.page.unwrap_or(1),
                req.page_size.unwrap_or(12),
            )
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(BomListByRoutingResponse {
            items: items
                .into_iter()
                .map(|b| BomBriefProto {
                    bom_id: b.bom_id,
                    bom_name: b.bom_name,
                    created_at: b.created_at.to_rfc3339(),
                })
                .collect(),
            total: total as u64,
        }))
    }
}
