//! 工序字典 gRPC Handler — 委托给 abt-core LaborProcessDictService

use abt_core::master_data::labor_process_dict::LaborProcessDictService;
use abt_core::shared::types::{PageParams, ServiceContext};
use abt_macros::require_permission;
use crate::error;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response};

use crate::generated::abt::v1::{
    abt_labor_process_dict_service_server::AbtLaborProcessDictService as GrpcLaborProcessDictService, *,
};
use crate::handlers::{domain_to_status, empty_to_none, GrpcResult};
use crate::interceptors::auth::extract_auth;
use crate::permissions::PermissionCode;
use crate::server::AppState;

pub struct LaborProcessDictHandler;

impl LaborProcessDictHandler {
    pub fn new() -> Self { Self }
}

impl Default for LaborProcessDictHandler {
    fn default() -> Self { Self::new() }
}

#[tonic::async_trait]
impl GrpcLaborProcessDictService for LaborProcessDictHandler {
    #[require_permission(Resource::LaborProcessDict, Action::Read)]
    async fn list_labor_process_dicts(
        &self,
        request: Request<ListLaborProcessDictsRequest>,
    ) -> GrpcResult<LaborProcessDictListResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.labor_process_dict_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, 0);
        let query = abt_core::master_data::labor_process_dict::LaborProcessDictQuery {
            keyword: req.keyword,
        };
        let page = PageParams::new(req.page.unwrap_or(1), req.page_size.unwrap_or(50));
        let result = srv.list(ctx, query, page).await
            .map_err(domain_to_status)?;

        Ok(Response::new(LaborProcessDictListResponse {
            items: result.items.into_iter().map(|d| LaborProcessDictProto {
                id: d.id,
                code: d.code,
                name: d.name,
                description: d.description.unwrap_or_default(),
                sort_order: d.sort_order,
            }).collect(),
            total: result.total as u64,
        }))
    }

    #[require_permission(Resource::LaborProcessDict, Action::Write)]
    async fn create_labor_process_dict(
        &self,
        request: Request<CreateLaborProcessDictRequest>,
    ) -> GrpcResult<U64Response> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();

        if req.name.is_empty() {
            return Err(error::validation("name", "工序名称不能为空"));
        }

        let state = AppState::get().await;
        let srv = state.labor_process_dict_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        let id = srv.create(ctx, abt_core::master_data::labor_process_dict::CreateLaborProcessDictReq {
            name: req.name,
            description: empty_to_none(req.description),
            sort_order: req.sort_order,
        }).await.map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    #[require_permission(Resource::LaborProcessDict, Action::Write)]
    async fn update_labor_process_dict(
        &self,
        request: Request<UpdateLaborProcessDictRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();

        if req.id <= 0 {
            return Err(error::validation("id", "ID 无效"));
        }
        if req.name.is_empty() {
            return Err(error::validation("name", "工序名称不能为空"));
        }

        let state = AppState::get().await;
        let srv = state.labor_process_dict_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        srv.update(ctx, req.id, abt_core::master_data::labor_process_dict::UpdateLaborProcessDictReq {
            name: Some(req.name),
            description: empty_to_none(req.description),
            sort_order: Some(req.sort_order),
        }).await.map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::LaborProcessDict, Action::Delete)]
    async fn delete_labor_process_dict(
        &self,
        request: Request<DeleteLaborProcessDictRequest>,
    ) -> GrpcResult<U64Response> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();

        if req.id <= 0 {
            return Err(error::validation("id", "ID 无效"));
        }

        let state = AppState::get().await;
        let srv = state.labor_process_dict_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;

        let ctx = ServiceContext::new(&mut tx, auth.user_id);
        srv.delete(ctx, req.id).await.map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: req.id as u64 }))
    }

    type ExportLaborProcessDictsStream = ReceiverStream<Result<DownloadFileResponse, tonic::Status>>;

    #[require_permission(Resource::LaborProcessDict, Action::Read)]
    async fn export_labor_process_dicts(
        &self,
        _request: Request<ExportLaborProcessDictsRequest>,
    ) -> Result<Response<Self::ExportLaborProcessDictsStream>, tonic::Status> {
        // Excel 导出
        let state = AppState::get().await;
        let exporter = abt_core::shared::excel::labor_process_dict_export::LaborProcessDictExporter::new(
            state.core_pool(),
        );
        let bytes = exporter
            .export()
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(crate::handlers::stream_excel_bytes(
            "labor_process_dict.xlsx",
            bytes,
        )))
    }
}
