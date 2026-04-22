//! 劳务工序 gRPC Handler

use abt::LaborProcessService;
use abt_macros::require_permission;
use common::error;
use rust_decimal::Decimal;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response};

use crate::generated::abt::v1::{
    abt_labor_process_service_server::AbtLaborProcessService as GrpcLaborProcessService, *,
};
use crate::handlers::{empty_to_none, GrpcResult};
use crate::interceptors::auth::extract_auth;
use crate::permissions::PermissionCode;
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

fn parse_decimal(value: &str, field: &str) -> Result<Decimal, tonic::Status> {
    value.parse().map_err(|_| error::validation(field, "Invalid decimal format"))
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

        let query = abt::ListLaborProcessQuery {
            product_code: req.product_code,
            keyword: req.keyword,
            page: req.page.unwrap_or(1),
            page_size: req.page_size.unwrap_or(50),
        };

        let (items, total) = srv
            .list(query)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(LaborProcessListResponse {
            items: items
                .into_iter()
                .map(|p| BomLaborProcessProto {
                    id: p.id,
                    product_code: p.product_code,
                    name: p.name,
                    unit_price: p.unit_price.to_string(),
                    quantity: p.quantity.to_string(),
                    sort_order: p.sort_order,
                    remark: p.remark.unwrap_or_default(),
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
        let quantity = parse_decimal(&req.quantity, "quantity")?;

        let id = srv
            .create(
                abt::CreateLaborProcessReq {
                    product_code: req.product_code,
                    name: req.name,
                    unit_price,
                    quantity,
                    sort_order: req.sort_order,
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
    ) -> GrpcResult<BoolResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.labor_process_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let unit_price = parse_decimal(&req.unit_price, "unit_price")?;
        let quantity = parse_decimal(&req.quantity, "quantity")?;

        srv.update(
            abt::UpdateLaborProcessReq {
                id: req.id,
                product_code: req.product_code,
                name: req.name,
                unit_price,
                quantity,
                sort_order: req.sort_order,
                remark: empty_to_none(req.remark),
            },
            &mut tx,
        )
        .await
        .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::LaborProcess, Action::Write)]
    async fn delete_labor_process(
        &self,
        request: Request<DeleteLaborProcessRequest>,
    ) -> GrpcResult<U64Response> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.labor_process_service();

        let mut tx = state
            .begin_transaction()
            .await
            .map_err(error::err_to_status)?;

        let affected = srv
            .delete(req.id, &req.product_code, &mut tx)
            .await
            .map_err(error::err_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: affected }))
    }

    type ExportLaborProcessesStream = ReceiverStream<Result<DownloadFileResponse, tonic::Status>>;

    #[require_permission(Resource::LaborProcess, Action::Read)]
    async fn export_labor_processes(
        &self,
        request: Request<ExportLaborProcessesRequest>,
    ) -> Result<Response<Self::ExportLaborProcessesStream>, tonic::Status> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.labor_process_service();

        let bytes = srv
            .export_to_bytes(&req.product_code)
            .await
            .map_err(error::err_to_status)?;

        let file_size = bytes.len() as i64;
        let (tx, rx) = tokio::sync::mpsc::channel(32);

        tokio::spawn(async move {
            let metadata = FileMetadata {
                file_name: format!("{}_labor_processes.xlsx", req.product_code),
                file_size,
                content_type: crate::handlers::EXCEL_MIME_TYPE.to_string(),
            };
            if tx.send(Ok(DownloadFileResponse {
                data: Some(download_file_response::Data::Metadata(metadata)),
            }))
            .await
            .is_err()
            {
                return;
            }

            for chunk in bytes.chunks(crate::handlers::STREAM_CHUNK_SIZE) {
                if tx.send(Ok(DownloadFileResponse {
                    data: Some(download_file_response::Data::Chunk(chunk.to_vec())),
                }))
                .await
                .is_err()
                {
                    break;
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    #[require_permission(Resource::LaborProcess, Action::Write)]
    async fn import_labor_processes(
        &self,
        request: Request<ImportLaborProcessesRequest>,
    ) -> GrpcResult<ImportLaborProcessesResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;
        let srv = state.labor_process_service();

        let result = srv
            .import_from_excel(&req.product_code, &req.file_path)
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(ImportLaborProcessesResponse {
            success_count: result.success_count,
            failure_count: result.failure_count,
            results: result
                .results
                .into_iter()
                .map(|r| ImportLaborProcessResult {
                    row_number: r.row_number,
                    process_name: r.process_name,
                    operation: r.operation,
                    error_message: r.error_message,
                })
                .collect(),
        }))
    }
}
