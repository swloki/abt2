//! 工序字典 gRPC Handler

use abt::LaborProcessDictService;
use abt_macros::require_permission;
use common::error;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response};

use crate::generated::abt::v1::{
    abt_labor_process_dict_service_server::AbtLaborProcessDictService as GrpcLaborProcessDictService, *,
};
use crate::handlers::{empty_to_none, GrpcResult};
use crate::interceptors::auth::extract_auth;
use crate::permissions::PermissionCode;
use crate::server::AppState;
use abt::{ExcelExportService, ExportRequest};

pub struct LaborProcessDictHandler;

impl LaborProcessDictHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LaborProcessDictHandler {
    fn default() -> Self {
        Self::new()
    }
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

        let query = abt::ListLaborProcessDictQuery {
            keyword: req.keyword,
            page: req.page.unwrap_or(1),
            page_size: req.page_size.unwrap_or(50),
        };

        let (items, total) = srv
            .list(query)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(LaborProcessDictListResponse {
            items: items
                .into_iter()
                .map(|d| LaborProcessDictProto {
                    id: d.id,
                    code: d.code,
                    name: d.name,
                    description: d.description.unwrap_or_default(),
                    sort_order: d.sort_order,
                })
                .collect(),
            total: total as u64,
        }))
    }

    #[require_permission(Resource::LaborProcessDict, Action::Write)]
    async fn create_labor_process_dict(
        &self,
        request: Request<CreateLaborProcessDictRequest>,
    ) -> GrpcResult<U64Response> {
        let req = request.into_inner();

        if req.name.is_empty() {
            return Err(error::validation("name", "工序名称不能为空"));
        }

        let state = AppState::get().await;
        let srv = state.labor_process_dict_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let id = srv
            .create(
                abt::CreateLaborProcessDictReq {
                    name: req.name,
                    description: empty_to_none(req.description),
                    sort_order: req.sort_order,
                },
                &mut tx,
            )
            .await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    #[require_permission(Resource::LaborProcessDict, Action::Write)]
    async fn update_labor_process_dict(
        &self,
        request: Request<UpdateLaborProcessDictRequest>,
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();

        if req.id <= 0 {
            return Err(error::validation("id", "ID 无效"));
        }
        if req.name.is_empty() {
            return Err(error::validation("name", "工序名称不能为空"));
        }

        let state = AppState::get().await;
        let srv = state.labor_process_dict_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        srv.update(
            abt::UpdateLaborProcessDictReq {
                id: req.id,
                name: req.name,
                description: empty_to_none(req.description),
                sort_order: req.sort_order,
            },
            &mut tx,
        )
        .await
        .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::LaborProcessDict, Action::Delete)]
    async fn delete_labor_process_dict(
        &self,
        request: Request<DeleteLaborProcessDictRequest>,
    ) -> GrpcResult<U64Response> {
        let req = request.into_inner();

        if req.id <= 0 {
            return Err(error::validation("id", "ID 无效"));
        }

        let state = AppState::get().await;
        let srv = state.labor_process_dict_service();

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

    type ExportLaborProcessDictsStream = ReceiverStream<Result<DownloadFileResponse, tonic::Status>>;

    #[require_permission(Resource::LaborProcessDict, Action::Read)]
    async fn export_labor_process_dicts(
        &self,
        _request: Request<ExportLaborProcessDictsRequest>,
    ) -> Result<Response<Self::ExportLaborProcessDictsStream>, tonic::Status> {
        let state = AppState::get().await;

        let exporter = abt::excel::LaborProcessDictExporter::new(state.pool());
        let bytes = exporter
            .export(ExportRequest { params: () })
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(crate::handlers::stream_excel_bytes(
            "labor_process_dict.xlsx",
            bytes,
        )))
    }
}
