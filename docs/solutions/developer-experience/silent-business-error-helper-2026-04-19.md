---
title: Silent Business Error Helper for gRPC Validation
date: 2026-04-19
category: docs/solutions/developer-experience
module: common
problem_type: developer_experience
component: development_workflow
severity: low
applies_when:
  - Returning business validation errors from gRPC handlers
  - Expected user input failures that should not log to console
  - Validation errors needing structured gRPC response without tracing::error! noise
tags: [grpc, error-handling, business-validation, tonic, developer-experience]
---

# Silent Business Error Helper for gRPC Validation

## Context

Business validation errors (e.g., "工序 X 的数量为 0，备注不能为空") were incorrectly routed through `err_to_status()`, causing `tracing::error!` to fire for expected user input failures. This polluted console logs and conflated infrastructure errors with normal validation feedback.

The `err_to_status()` function is designed for infrastructure errors (database failures, network timeouts) and unconditionally logs via `tracing::error!` with full backtraces. Business validation failures are expected outcomes based on user input — they should return structured errors to the frontend silently.

## Guidance

### Use `business_error()` for expected validation failures

The new `business_error()` helper in `common/src/error.rs` returns a proper gRPC `Status` with `ErrorDetails` (following Google AIP-193 / ConnectRPC standard) so the frontend can extract structured field-level violations via `findDetails()`, but performs zero logging.

```rust
// common/src/error.rs
pub fn business_error(field: &str, message: &str) -> tonic::Status {
    let mut details = ErrorDetails::new();
    details.add_bad_request_violation(field, message);
    tonic::Status::with_error_details(
        Code::InvalidArgument,
        message,
        details,
    )
}
```

### Error helper selection guide

| Helper | Use case | Logs? |
|--------|----------|-------|
| `err_to_status()` / `sqlx_err_to_status()` | Infrastructure errors (DB, IO) | Yes — full backtrace |
| `validation()` / `validation_errors()` | Field-level input validation with warning | Yes — `tracing::warn!` |
| `business_error()` | Expected business rule failures | **No** — silent |

### Keep validation at handler level

Business validation belongs in the gRPC handler (translation layer), not the service layer. Service methods should assume valid input and focus on business logic.

```rust
// abt-grpc/src/handlers/labor_process.rs
async fn set_bom_labor_cost(&self, request: Request<SetBomLaborCostRequest>) -> GrpcResult<BoolResponse> {
    let items = parse_items_from_request(request)?;

    // 业务校验：如果数量为 0，则备注不能为空
    for item in &items {
        if item.quantity.is_zero() && item.remark.as_ref().is_none_or(|r| r.is_empty()) {
            return Err(error::business_error(
                "remark",
                &format!("工序 {} 的数量为 0，备注不能为空", item.process_id),
            ));
        }
    }

    // Service layer receives pre-validated input
    srv.set_bom_labor_cost(req, &mut tx).await?;
    Ok(Response::new(BoolResponse { value: true }))
}
```

## Why This Matters

Conflating infrastructure errors with business validation failures creates noise in console logs, making real problems harder to spot during debugging. It also wastes resources capturing backtraces for routine validation failures that are simply user feedback.

## When to Apply

- When adding validation in a gRPC handler that checks business rules (not just input format)
- When an error is an expected response to the user based on their input
- When the error should propagate to the frontend as a structured field violation

## Examples

**Before**: Validation at service layer using `anyhow::bail`, routed through `err_to_status()`:
```rust
// Service impl — DON'T do this for business validation
async fn set_bom_labor_cost(&self, req: SetBomLaborCostReq, executor: Executor<'_>) -> Result<()> {
    for item in &req.items {
        if item.quantity.is_zero() && item.remark.as_ref().is_none_or(|r| r.is_empty()) {
            anyhow::bail!("工序 {} 的数量为 0，备注不能为空", item.process_id);
        }
    }
    // ...
}
```

**After**: Validation at handler layer using `business_error()`:
```rust
// Handler — validate business rules before calling service
for item in &items {
    if item.quantity.is_zero() && item.remark.as_ref().is_none_or(|r| r.is_empty()) {
        return Err(error::business_error(
            "remark",
            &format!("工序 {} 的数量为 0，备注不能为空", item.process_id),
        ));
    }
}
```

## Related Files

- `common/src/error.rs` — `business_error()` implementation (line 155)
- `abt-grpc/src/handlers/labor_process.rs` — usage example (line 337-345)
- `abt/src/implt/labor_process_service_impl.rs` — validation removed from service layer
