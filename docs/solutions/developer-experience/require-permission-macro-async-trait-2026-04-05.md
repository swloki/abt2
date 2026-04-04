---
module: "abt-grpc"
date: "2026-04-05"
problem_type: "developer_experience"
component: "tooling"
severity: "medium"
applies_when:
  - "Creating proc-macro attributes that must work with #[async_trait] or #[tonic::async_trait]"
  - "Generating code inside async functions transformed by external macros"
  - "Need to detect Box::pin(async move { ... }) pattern in proc-macro expansion"
tags: ["proc-macro", "async-trait", "rust", "tonic", "grpc", "code-generation", "rbac"]
---

# Proc-Macro Attribute for Auth Boilerplate with async_trait Compatibility

## Context

A tonic-based gRPC server had 101 handler methods each repeating the same two-line auth+permission pattern. Tonic interceptors cannot see RPC method paths, so permission checks must live at the handler level. A proc-macro attribute `#[require_permission("resource", "action")]` was chosen over a helper function because it co-locates the permission declaration with the method signature, making security posture auditable at a glance.

The core challenge: `#[tonic::async_trait]` on impl blocks transforms `async fn` into `fn -> Pin<Box<dyn Future>>` by wrapping the body in `Box::pin(async move { ... })`. Method-level attributes expand BEFORE impl-level attributes, so the macro sees the original `async fn` — but must inject code that ends up inside the transformed async block.

## Guidance

### Detect and penetrate the Box::pin wrapper

The macro must check whether the function body has been transformed into `Box::pin(async move { ... })` and, if so, prepend statements INSIDE the async block rather than outside it.

```rust
fn prepend_inside_async_block(stmts: &mut Vec<Stmt>, to_prepend: Vec<Stmt>) -> bool {
    if stmts.len() != 1 {
        return false;
    }

    let expr_stmt = match &mut stmts[0] {
        Stmt::Expr(expr, None) => expr,
        _ => return false,
    };

    let call = match expr_stmt {
        Expr::Call(call) => call,
        _ => return false,
    };

    if !is_box_pin(call) {
        return false;
    }

    let async_expr = match call.args.first_mut() {
        Some(Expr::Async(async_block)) => async_block,
        _ => return false,
    };

    let mut new_stmts = to_prepend;
    new_stmts.extend(async_expr.block.stmts.drain(..));
    async_expr.block.stmts = new_stmts;
    true
}

fn is_box_pin(call: &ExprCall) -> bool {
    if let Expr::Path(path) = call.func.as_ref() {
        let segments = &path.path.segments;
        if segments.len() == 2 {
            return segments[0].ident == "Box" && segments[1].ident == "pin";
        }
    }
    false
}
```

### Always provide a fallback path

When `Box::pin` is NOT detected (e.g., the function is not inside an `async_trait` impl), prepend directly to the function body:

```rust
if prepend_inside_async_block(&mut func.block.stmts, stmts_to_prepend.clone()) {
    // Successfully prepended inside the async block
} else {
    // Normal case: prepend directly to function body
    let mut new_stmts = stmts_to_prepend;
    new_stmts.extend(func.block.stmts.drain(..));
    func.block.stmts = new_stmts;
}
```

### Extract parameter ident from the function signature

Hardcoding `request` fails for methods using `_request` (e.g., `permission.rs`). Extract the actual ident:

```rust
fn extract_request_ident(func: &ItemFn) -> Option<syn::Ident> {
    let mut params = func.sig.inputs.iter();
    params.next()?; // Skip &self
    let second = params.next()?;
    match second {
        FnArg::Typed(PatType { pat, .. }) => {
            if let syn::Pat::Ident(pat_ident) = pat.as_ref() {
                Some(pat_ident.ident.clone())
            } else {
                None
            }
        }
        _ => None,
    }
}
```

### Generate call-site-resolved code, not fully-qualified paths

The macro generates short names (`extract_auth(...)`, `error::forbidden(...)`) that resolve via existing `use` statements in handler files. This keeps generated code clean but means the `extract_auth` and `error` imports MUST be retained after migration — they're used by the expanded macro code, not the hand-written source.

### Suppress unused variable warnings selectively

The generated `let auth = ...` binding is unused in ~83% of handlers (only permission check, no `auth.user_id`). Use `#[allow(unused_variables)]` on the binding:

```rust
let auth_stmt: Stmt = parse_quote! {
    #[allow(unused_variables)]
    let auth = extract_auth(&#request_ident)?;
};
```

## Why This Matters

- **101 boilerplate sites** reduced to declarative annotations
- **Security posture becomes auditable** at a glance — grep for `#[require_permission]`
- **Copy-paste errors eliminated** in critical security code
- **Future handler methods** have a clear, one-line pattern to follow

## When to Apply

- Repetitive auth/permission boilerplate across many handler/controller methods
- Tonic gRPC servers where interceptors can't handle method-level permission checks
- Any proc-macro that needs to inject code into methods inside `#[async_trait]` impl blocks
- Pattern is generalizable to any cross-cutting concern injected via proc-macro (logging, tracing, metrics)

## Examples

**Before — manual boilerplate repeated 101 times:**
```rust
async fn create_department(&self, request: Request<CreateDepartmentRequest>) -> GrpcResult<DepartmentResponse> {
    let auth = extract_auth(&request)?;
    auth.check_permission("department", "write").map_err(|_e| error::forbidden("department", "write"))?;
    let req = request.into_inner();
    // ... business logic
}
```

**After — declarative annotation:**
```rust
#[require_permission("department", "write")]
async fn create_department(&self, request: Request<CreateDepartmentRequest>) -> GrpcResult<DepartmentResponse> {
    let req = request.into_inner();
    // ... business logic (auth handled automatically, auth variable still available)
}
```

**Key risk:** The `Box::pin` detection is coupled to async_trait's internals. If async_trait changes its desugaring, the macro will silently fall back to prepending outside the async block, which would surface as a compile error — not a silent auth bypass. This is an acceptable tradeoff documented in the implementation plan.

**Related files:**
- `abt-macros/src/lib.rs` — macro implementation
- `abt-grpc/src/handlers/*.rs` — 12 handler files using the macro
- `docs/plans/2026-04-05-001-feat-rbac-permission-macro-plan.md` — implementation plan
