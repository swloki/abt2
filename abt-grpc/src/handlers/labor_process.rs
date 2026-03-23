//! BOM 人工工序 gRPC Handler

use crate::generated::abt::v1::*;
use crate::handlers::GrpcResult;
use crate::server::AppState;
use abt::LaborProcessService;
use rust_decimal::Decimal;
use tonic::{Response, Status};

/// 辅助函数：列出人工工序（按产品编码查询）
pub async fn list_labor_processes_internal(
    req: ListLaborProcessesRequest,
) -> GrpcResult<BomLaborProcessListResponse> {
    let state = AppState::get().await;
    let service = state.labor_process_service();

    let page = req.page.unwrap_or(1).max(1);
    let page_size = req.page_size.unwrap_or(50).clamp(1, 100);

    let (items, total) = service
        .list(abt::ListLaborProcessRequest {
            product_code: req.product_code,
            page: Some(page),
            page_size: Some(page_size),
        })
        .await
        .map_err(|e| Status::internal(e.to_string()))?;

    let items = items
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
        .collect();

    Ok(Response::new(BomLaborProcessListResponse {
        items,
        total: total as u64,
    }))
}

/// 辅助函数：创建人工工序
pub async fn create_labor_process_internal(
    req: CreateLaborProcessRequest,
) -> GrpcResult<U64Response> {
    let state = AppState::get().await;
    let service = state.labor_process_service();

    let mut tx = state
        .begin_transaction()
        .await
        .map_err(|e| Status::internal(e.to_string()))?;

    let unit_price: Decimal = req.unit_price
        .parse()
        .map_err(|e| Status::invalid_argument(format!("invalid unit_price: {}", e)))?;
    let quantity: Decimal = req.quantity
        .parse()
        .map_err(|e| Status::invalid_argument(format!("invalid quantity: {}", e)))?;

    let id = service
        .create(
            abt::CreateLaborProcessRequest {
                product_code: req.product_code,
                name: req.name,
                unit_price,
                quantity,
                sort_order: req.sort_order,
                remark: if req.remark.is_empty() {
                    None
                } else {
                    Some(req.remark)
                },
            },
            &mut tx,
        )
        .await
        .map_err(|e| Status::internal(e.to_string()))?;

    tx.commit()
        .await
        .map_err(|e| Status::internal(e.to_string()))?;

    Ok(Response::new(U64Response { value: id as u64 }))
}

/// 辅助函数：更新人工工序
pub async fn update_labor_process_internal(
    req: UpdateLaborProcessRequest,
) -> GrpcResult<BoolResponse> {
    let state = AppState::get().await;
    let service = state.labor_process_service();

    let mut tx = state
        .begin_transaction()
        .await
        .map_err(|e| Status::internal(e.to_string()))?;

    let unit_price: Decimal = req.unit_price
        .parse()
        .map_err(|e| Status::invalid_argument(format!("invalid unit_price: {}", e)))?;
    let quantity: Decimal = req.quantity
        .parse()
        .map_err(|e| Status::invalid_argument(format!("invalid quantity: {}", e)))?;

    service
        .update(
            abt::UpdateLaborProcessRequest {
                id: req.id,
                product_code: req.product_code,
                name: req.name,
                unit_price,
                quantity,
                sort_order: req.sort_order,
                remark: if req.remark.is_empty() {
                    None
                } else {
                    Some(req.remark)
                },
            },
            &mut tx,
        )
        .await
        .map_err(|e| Status::internal(e.to_string()))?;

    tx.commit()
        .await
        .map_err(|e| Status::internal(e.to_string()))?;

    Ok(Response::new(BoolResponse { value: true }))
}

/// 辅助函数：删除人工工序
pub async fn delete_labor_process_internal(
    req: DeleteLaborProcessRequest,
) -> GrpcResult<U64Response> {
    let state = AppState::get().await;
    let service = state.labor_process_service();

    let mut tx = state
        .begin_transaction()
        .await
        .map_err(|e| Status::internal(e.to_string()))?;

    let deleted = service
        .delete(req.id, &req.product_code, &mut tx)
        .await
        .map_err(|e| Status::internal(e.to_string()))?;

    tx.commit()
        .await
        .map_err(|e| Status::internal(e.to_string()))?;

    Ok(Response::new(U64Response { value: deleted }))
}

/// 辅助函数：导入人工工序
///
/// Excel 格式：
/// - 第一列 (A): 产品编码 (product_code) - 用于匹配 BOM
/// - 第二列 (B): 工序名称 (name)
/// - 第三列 (C): 单价 (unit_price)
/// - 第四列 (D): 数量 (quantity)
/// - 第五列 (E): 排序 (sort_order)
/// - 第六列 (F): 备注 (remark)
///
/// 导入策略：按产品编码分组，每个产品编码对应一个 BOM，先删除该产品编码的所有现有工序，再批量插入新工序
pub async fn import_labor_processes_internal(
    req: ImportLaborProcessRequest,
) -> GrpcResult<ImportLaborProcessResponse> {
    let state = AppState::get().await;
    let service = state.labor_process_service();

    let mut tx = state
        .begin_transaction()
        .await
        .map_err(|e| Status::internal(e.to_string()))?;

    let result = service
        .import(&req.file_path, &mut tx)
        .await
        .map_err(|e| Status::internal(e.to_string()))?;

    tx.commit()
        .await
        .map_err(|e| Status::internal(e.to_string()))?;

    Ok(Response::new(ImportLaborProcessResponse {
        success_count: result.success_count,
        fail_count: result.fail_count,
        errors: result.errors,
    }))
}
