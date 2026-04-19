//! 劳务工序 gRPC Handler

use abt::LaborProcessService;
use abt_macros::require_permission;
use common::error;
use crate::permissions::PermissionCode;
use rust_decimal::Decimal;
use tonic::{Request, Response};

use crate::generated::abt::v1::{
    abt_labor_process_service_server::AbtLaborProcessService as GrpcLaborProcessService, *,
};
use crate::handlers::GrpcResult;
use crate::interceptors::auth::extract_auth;
use crate::server::AppState;

pub struct LaborProcessHandler;

impl LaborProcessHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LaborProcessHandler {
    fn default() -> Self {
        Self::new()
    }
}

fn empty_to_none(s: String) -> Option<String> {
    if s.is_empty() { None } else { Some(s) }
}

fn parse_decimal(value: &str, field: &str) -> Result<Decimal, tonic::Status> {
    value.parse().map_err(|_| error::validation(field, "Invalid decimal format"))
}

fn group_with_members_to_proto(g: abt::LaborProcessGroupWithMembers) -> LaborProcessGroupProto {
    LaborProcessGroupProto {
        id: g.group.id,
        name: g.group.name,
        remark: g.group.remark.unwrap_or_default(),
        members: g
            .members
            .into_iter()
            .map(|m| ProcessGroupMemberProto {
                process_id: m.process_id,
                sort_order: m.sort_order,
            })
            .collect(),
        created_at: g.group.created_at.timestamp(),
        updated_at: g.group.updated_at.map(|t| t.timestamp()).unwrap_or(0),
    }
}

fn proto_members_to_inputs(members: Vec<ProcessGroupMemberProto>) -> Vec<abt::LaborProcessGroupMemberInput> {
    members
        .into_iter()
        .map(|m| abt::LaborProcessGroupMemberInput {
            process_id: m.process_id,
            sort_order: m.sort_order,
        })
        .collect()
}

#[tonic::async_trait]
impl GrpcLaborProcessService for LaborProcessHandler {
    #[require_permission(Resource::LaborProcess, Action::Read)]
    async fn list_labor_processes(
        &self,
        request: Request<ListLaborProcessesRequest>,
    ) -> GrpcResult<LaborProcessListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.labor_process_service();

        let query = abt::LaborProcessQuery {
            keyword: req.keyword,
            page: req.page.unwrap_or(1),
            page_size: req.page_size.unwrap_or(50),
        };

        let (items, total) = srv
            .list_processes(query)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(LaborProcessListResponse {
            items: items
                .into_iter()
                .map(|p| LaborProcessProto {
                    id: p.id,
                    name: p.name,
                    unit_price: p.unit_price.to_string(),
                    remark: p.remark.unwrap_or_default(),
                    created_at: p.created_at.timestamp(),
                    updated_at: p.updated_at.map(|t| t.timestamp()).unwrap_or(0),
                })
                .collect(),
            total: total as u64,
        }))
    }

    #[require_permission(Resource::LaborProcess, Action::Write)]
    async fn create_labor_process(
        &self,
        request: Request<CreateLaborProcessRequest>,
    ) -> GrpcResult<U64Response> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.labor_process_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let unit_price = parse_decimal(&req.unit_price, "unit_price")?;

        let id = srv
            .create_process(
                abt::CreateLaborProcessReq {
                    name: req.name,
                    unit_price,
                    remark: empty_to_none(req.remark),
                },
                &mut tx,
            )
            .await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    #[require_permission(Resource::LaborProcess, Action::Write)]
    async fn update_labor_process(
        &self,
        request: Request<UpdateLaborProcessRequest>,
    ) -> GrpcResult<UpdateLaborProcessResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.labor_process_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let unit_price = parse_decimal(&req.unit_price, "unit_price")?;

        let impact = srv
            .update_process(
                abt::UpdateLaborProcessReq {
                    id: req.id,
                    name: req.name,
                    unit_price,
                    remark: empty_to_none(req.remark),
                },
                &mut tx,
            )
            .await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(UpdateLaborProcessResponse {
            success: true,
            affected_bom_count: impact.as_ref().map(|i| i.affected_bom_count).unwrap_or(0),
            affected_item_count: impact.as_ref().map(|i| i.affected_item_count).unwrap_or(0),
        }))
    }

    #[require_permission(Resource::LaborProcess, Action::Delete)]
    async fn delete_labor_process(
        &self,
        request: Request<DeleteLaborProcessRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.labor_process_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        srv.delete_process(req.id, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::LaborProcess, Action::Read)]
    async fn list_labor_process_groups(
        &self,
        request: Request<ListLaborProcessGroupsRequest>,
    ) -> GrpcResult<LaborProcessGroupListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.labor_process_service();

        let query = abt::LaborProcessGroupQuery {
            keyword: req.keyword,
            page: req.page.unwrap_or(1),
            page_size: req.page_size.unwrap_or(50),
        };

        let (groups, total) = srv
            .list_groups(query)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(LaborProcessGroupListResponse {
            items: groups.into_iter().map(group_with_members_to_proto).collect(),
            total: total as u64,
        }))
    }

    #[require_permission(Resource::LaborProcess, Action::Write)]
    async fn create_labor_process_group(
        &self,
        request: Request<CreateLaborProcessGroupRequest>,
    ) -> GrpcResult<U64Response> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.labor_process_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let id = srv
            .create_group(
                abt::CreateLaborProcessGroupReq {
                    name: req.name,
                    remark: empty_to_none(req.remark),
                    members: proto_members_to_inputs(req.members),
                },
                &mut tx,
            )
            .await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    #[require_permission(Resource::LaborProcess, Action::Write)]
    async fn update_labor_process_group(
        &self,
        request: Request<UpdateLaborProcessGroupRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.labor_process_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        srv.update_group(
            abt::UpdateLaborProcessGroupReq {
                id: req.id,
                name: req.name,
                remark: empty_to_none(req.remark),
                members: proto_members_to_inputs(req.members),
            },
            &mut tx,
        )
        .await
        .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::LaborProcess, Action::Delete)]
    async fn delete_labor_process_group(
        &self,
        request: Request<DeleteLaborProcessGroupRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.labor_process_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        srv.delete_group(req.id, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::LaborProcess, Action::Write)]
    async fn set_bom_labor_cost(
        &self,
        request: Request<SetBomLaborCostRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.labor_process_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let items: Vec<abt::BomLaborCostItemInput> = req
            .items
            .into_iter()
            .map(|item| {
                let quantity = parse_decimal(&item.quantity, "quantity")?;
                Ok(abt::BomLaborCostItemInput {
                    process_id: item.process_id,
                    quantity,
                    remark: empty_to_none(item.remark),
                })
            })
            .collect::<Result<_, tonic::Status>>()?;

        // 业务校验：如果数量为 0，则备注不能为空
        for item in &items {
            if item.quantity.is_zero() && item.remark.as_ref().is_none_or(|r| r.is_empty()) {
                return Err(error::business_error(
                    "remark",
                    &format!("工序 {} 的数量为 0，备注不能为空", item.process_id),
                ));
            }
        }

        srv.set_bom_labor_cost(
            abt::SetBomLaborCostReq {
                bom_id: req.bom_id,
                process_group_id: req.process_group_id,
                items,
            },
            &mut tx,
        )
        .await
        .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::LaborProcess, Action::Read)]
    async fn get_bom_labor_cost(
        &self,
        request: Request<GetBomLaborCostRequest>,
    ) -> GrpcResult<BomLaborCostResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.labor_process_service();

        let result = srv
            .get_bom_labor_cost(req.bom_id)
            .await
            .map_err(error::err_to_status)?;

        let (group_with_members, cost_items) = match result {
            Some(r) => r,
            None => {
                return Ok(Response::new(BomLaborCostResponse {
                    bom_id: req.bom_id,
                    process_group: None,
                    items: vec![],
                    total_cost: "0".to_string(),
                    snapshot_total_cost: "0".to_string(),
                }));
            }
        };

        let mut total_cost = Decimal::ZERO;
        let mut snapshot_total_cost = Decimal::ZERO;

        let items: Vec<BomLaborCostItemProto> = cost_items
            .into_iter()
            .map(|item| {
                let subtotal = item.subtotal();
                let snapshot_subtotal = item.snapshot_subtotal().unwrap_or(Decimal::ZERO);

                total_cost += subtotal;
                snapshot_total_cost += snapshot_subtotal;

                BomLaborCostItemProto {
                    id: item.id,
                    process_id: item.process_id,
                    process_name: item.process_name,
                    current_unit_price: item.current_unit_price.to_string(),
                    snapshot_unit_price: item.snapshot_unit_price.map(|p| p.to_string()).unwrap_or_default(),
                    quantity: item.quantity.to_string(),
                    subtotal: subtotal.to_string(),
                    snapshot_subtotal: snapshot_subtotal.to_string(),
                    remark: item.remark.unwrap_or_default(),
                }
            })
            .collect();

        Ok(Response::new(BomLaborCostResponse {
            bom_id: req.bom_id,
            process_group: Some(group_with_members_to_proto(group_with_members)),
            items,
            total_cost: total_cost.to_string(),
            snapshot_total_cost: snapshot_total_cost.to_string(),
        }))
    }
}
