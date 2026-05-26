//! 劳务工序 gRPC Handler — 委托给 abt-core BomLaborProcessService

use abt_core::master_data::bom_labor_process::BomLaborProcessService;
use abt_core::shared::types::{PageParams, ServiceContext};
use abt_macros::require_permission;
use common::error;
use rust_decimal::Decimal;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response};

use crate::generated::abt::v1::{
    abt_labor_process_service_server::AbtLaborProcessService as GrpcLaborProcessService, *,
};
use crate::handlers::{domain_to_status, empty_to_none, GrpcResult};
use crate::interceptors::auth::extract_auth;
use crate::permissions::PermissionCode;
use crate::server::AppState;

pub struct LaborProcessHandler;

impl LaborProcessHandler {
    pub fn new() -> Self { Self }
}

impl Default for LaborProcessHandler {
    fn default() -> Self { Self::new() }
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

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;
        let ctx = ServiceContext::system(&mut tx);

        let query = abt_core::master_data::bom_labor_process::BomLaborProcessQuery {
            product_code: if req.product_code.is_empty() { None } else { Some(req.product_code) },
            keyword: req.keyword,
        };
        let page = PageParams::new(req.page.unwrap_or(1), req.page_size.unwrap_or(50));
        let result = srv.list(ctx, query, page).await
            .map_err(domain_to_status)?;

        Ok(Response::new(LaborProcessListResponse {
            items: result.items.into_iter().map(|p| BomLaborProcessProto {
                id: p.id,
                product_code: p.product_code,
                name: p.name,
                unit_price: p.unit_price.to_string(),
                quantity: p.quantity.to_string(),
                sort_order: p.sort_order,
                remark: p.remark.unwrap_or_default(),
                process_code: p.process_code,
            }).collect(),
            total: result.total as u64,
        }))
    }

    #[require_permission(Resource::LaborProcess, Action::Write)]
    async fn create_labor_process(
        &self,
        request: Request<CreateLaborProcessRequest>,
    ) -> GrpcResult<U64Response> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();

        if req.product_code.is_empty() {
            return Err(error::validation("product_code", "产品编码不能为空"));
        }
        if req.name.is_empty() {
            return Err(error::validation("name", "工序名称不能为空"));
        }

        let state = AppState::get().await;
        let srv = state.labor_process_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;
        let ctx = ServiceContext::new(&mut tx, auth.user_id);

        let unit_price = parse_decimal(&req.unit_price, "unit_price")?;
        let quantity = parse_decimal(&req.quantity, "quantity")?;

        let id = srv.create(ctx, abt_core::master_data::bom_labor_process::CreateBomLaborProcessReq {
            product_code: req.product_code,
            labor_process_dict_id: 0,
            process_code: req.process_code,
            name: req.name,
            unit_price,
            quantity,
            sort_order: req.sort_order,
            remark: empty_to_none(req.remark),
        }).await.map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: id as u64 }))
    }

    #[require_permission(Resource::LaborProcess, Action::Write)]
    async fn update_labor_process(
        &self,
        request: Request<UpdateLaborProcessRequest>,
    ) -> GrpcResult<BoolResponse> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();

        if req.id <= 0 {
            return Err(error::validation("id", "ID 无效"));
        }
        if req.product_code.is_empty() {
            return Err(error::validation("product_code", "产品编码不能为空"));
        }
        if req.name.is_empty() {
            return Err(error::validation("name", "工序名称不能为空"));
        }

        let state = AppState::get().await;
        let srv = state.labor_process_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;
        let ctx = ServiceContext::new(&mut tx, auth.user_id);

        let unit_price = parse_decimal(&req.unit_price, "unit_price")?;
        let quantity = parse_decimal(&req.quantity, "quantity")?;

        srv.update(ctx, req.id, abt_core::master_data::bom_labor_process::UpdateBomLaborProcessReq {
            labor_process_dict_id: None,
            process_code: req.process_code,
            name: Some(req.name),
            unit_price: Some(unit_price),
            quantity: Some(quantity),
            sort_order: Some(req.sort_order),
            remark: empty_to_none(req.remark),
        }).await.map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(BoolResponse { value: true }))
    }

    #[require_permission(Resource::LaborProcess, Action::Delete)]
    async fn delete_labor_process(
        &self,
        request: Request<DeleteLaborProcessRequest>,
    ) -> GrpcResult<U64Response> {
        let auth = extract_auth(&request)?;
        let req = request.into_inner();

        if req.id <= 0 {
            return Err(error::validation("id", "ID 无效"));
        }

        let state = AppState::get().await;
        let srv = state.labor_process_service();

        let mut tx = state.begin_core_transaction().await
            .map_err(error::err_to_status)?;
        let ctx = ServiceContext::new(&mut tx, auth.user_id);

        srv.delete(ctx, req.id).await.map_err(domain_to_status)?;

        tx.commit().await.map_err(error::sqlx_err_to_status)?;

        Ok(Response::new(U64Response { value: req.id as u64 }))
    }

    type ExportLaborProcessesStream = ReceiverStream<Result<DownloadFileResponse, tonic::Status>>;

    #[require_permission(Resource::LaborProcess, Action::Read)]
    async fn export_labor_processes(
        &self,
        request: Request<ExportLaborProcessesRequest>,
    ) -> Result<Response<Self::ExportLaborProcessesStream>, tonic::Status> {
        let req = request.into_inner();
        let state = AppState::get().await;

        let exporter = abt_core::shared::excel::labor_process_export::LaborProcessExporter::new(
            state.core_pool(), req.product_code.clone(),
        );
        let bytes = exporter
            .export()
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(crate::handlers::stream_excel_bytes(
            format!("{}_labor_processes.xlsx", req.product_code),
            bytes,
        )))
    }

    type ExportBomsWithoutLaborCostStream = ReceiverStream<Result<DownloadFileResponse, tonic::Status>>;

    #[require_permission(Resource::LaborProcess, Action::Read)]
    async fn export_boms_without_labor_cost(
        &self,
        _request: Request<ExportBomsWithoutLaborCostRequest>,
    ) -> Result<Response<Self::ExportBomsWithoutLaborCostStream>, tonic::Status> {
        let state = AppState::get().await;

        let exporter = abt_core::shared::excel::boms_no_labor_cost_export::BomsNoLaborCostExporter::new(
            state.core_pool(),
        );
        let bytes = exporter
            .export()
            .await
            .map_err(error::err_to_status)?;

        Ok(Response::new(crate::handlers::stream_excel_bytes(
            "boms_without_labor_cost.xlsx",
            bytes,
        )))
    }

    #[require_permission(Resource::LaborProcess, Action::Write)]
    async fn import_labor_processes(
        &self,
        request: Request<ImportLaborProcessesRequest>,
    ) -> GrpcResult<ImportLaborProcessesResponse> {
        let req = request.into_inner();
        let state = AppState::get().await;

        // 路径校验：只允许读取上传目录中的文件
        let path = std::path::Path::new(&req.file_path);
        let upload_dir = std::env::temp_dir().canonicalize()
            .map_err(|e| error::err_to_status(anyhow::anyhow!("无法解析上传目录: {}", e)))?;
        let canonical = path.canonicalize()
            .map_err(|e| error::err_to_status(anyhow::anyhow!("无法解析文件路径: {}", e)))?;
        if !canonical.starts_with(&upload_dir) {
            return Err(error::validation("file_path", "只允许导入上传目录中的文件"));
        }

        let bytes = tokio::fs::read(path)
            .await
            .map_err(|e| error::err_to_status(anyhow::anyhow!("无法读取上传文件: {}", e)))?;

        let tracker = abt_core::shared::excel::helpers::ProgressTracker::new();
        let importer = abt_core::shared::excel::labor_process_import::LaborProcessImporter::new(
            state.core_pool(),
            tracker,
        );

        let result = importer
            .import(abt_core::shared::excel::types::ImportSource::Bytes(bytes))
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
            routing_results: result
                .routing_results
                .into_iter()
                .map(|r| ProductRoutingInfo {
                    product_code: r.product_code,
                    matched_existing_routing: r.matched_existing_routing,
                    routing_name: r.routing_name,
                    routing_id: r.routing_id,
                })
                .collect(),
        }))
    }
}
