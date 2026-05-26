---
title: "feat: Declarative RBAC via Handler Permission Macro"
type: feat
status: active
date: 2026-04-05
origin: docs/brainstorms/2026-04-04-rbac-interceptor-macro-requirements.md
---

# feat: Declarative RBAC via Handler Permission Macro

## Overview

Replace 101 manual `extract_auth` + `check_permission` call sites across 12+ handler files with a `#[require_permission("resource", "action")]` proc macro. The macro generates auth extraction and permission check boilerplate, making `auth` available in the handler body. The macro provides **declarative convenience** — it co-locates the permission declaration with the handler, making omissions more visible during code review. However, the annotation is opt-in per method; a developer adding a new RPC can still omit it without a compile error. True enforcement would require a separate lint rule or CI check (noted as future work).

## Problem Frame

Every gRPC handler method repeats the same auth + permission check pattern: `extract_auth(&request)?` + `auth.check_permission("resource", "action").map_err(error::forbidden)?`. With 101 call sites across 12 handler files, new RPCs can easily miss the check — a security risk. tonic's interceptor layer cannot see the RPC method path, so the permission check must live at the handler level. A proc macro co-locates the permission declaration with the handler, similar to Spring's `@PreAuthorize`. *(see origin: docs/brainstorms/2026-04-04-rbac-interceptor-macro-requirements.md)*

## Requirements Trace

- R1. Create `#[require_permission("resource", "action")]` attribute proc macro
- R2. Macro auto-generates: `extract_auth(&request)?` + `check_permission(resource, action).map_err(error::forbidden)?`
- R3. Generated `auth` variable (`AuthContext`) is visible to handler method body
- R4. Macro errors use existing AIP-193 rich error format (`error::forbidden(resource, action)`)
- R5. AuthService handler excluded — no macro needed (no `auth_interceptor` registered)
- R6. `is_super_admin` bypass handled by existing `AuthContext::check_permission()` — no macro logic needed
- R7. `*:*` wildcard: The origin document claims this is handled by existing `check_permission()`, but the actual implementation (`abt/src/models/auth.rs:33`) performs only exact string matching (`self.permissions.contains(&format!("{}:{}", resource, action))`). There is no wildcard expansion logic. Full access is controlled via `is_super_admin` (R6), not `*:*` wildcard permissions. This is inherited behavior — the macro does not change it, but R7 as stated is inaccurate. If wildcard permissions are a business requirement, `check_permission()` itself needs modification (out of scope for this plan).
- R8. Migrate all 101 manual call sites to macro annotations
- R9. Remove handler-layer calls to `extract_auth` after migration (function retained for compatibility)

## Scope Boundaries

- **No** interceptor-level unified checking (tonic limitation)
- **No** audit logging (separate concern)
- **No** department-level data visibility filtering (separate requirement)
- **No** modifications to `AuthContext::check_permission()` logic
- **No** modifications to `RESOURCES` static array or permission model

## Context & Research

### Relevant Code and Patterns

- **Handler method pattern** (`abt-grpc/src/handlers/*.rs`): Every handler method follows `extract_auth(&request)?` → `auth.check_permission(...)` → `request.into_inner()` → business logic. The parameter is named `request` in most handlers; `permission.rs` uses `_request` for two methods with empty request bodies.
- **`extract_auth<T>`** (`abt-grpc/src/interceptors/auth.rs:41`): Retrieves `AuthContext` from `request.extensions()`. Returns `Status::internal` if missing.
- **`AuthContext::check_permission`** (`abt/src/models/auth.rs:33`): Returns `Result<(), String>`. Super admin short-circuits to `Ok(())`.
- **`error::forbidden`** (`common/src/error.rs:122`): Returns `tonic::Status` with `PermissionDenied` code, AIP-193 `ErrorInfo` with resource/action metadata.
- **AuthService exception** (`abt-grpc/src/server.rs:140`): Only service registered without `auth_interceptor`. Uses `extract_user_id_from_header` instead.
- **`GrpcResult<T>`** (`abt-grpc/src/handlers/mod.rs:35`): Type alias `Result<tonic::Response<T>, tonic::Status>`.

### Institutional Learnings

- No `docs/solutions/` directory exists. No prior proc-macro work in this codebase.
- Department isolation plan (`docs/plans/2026-04-04-001`) notes that `AuthContext::check_permission()` stays unchanged for department filtering — confirming the macro won't need updates for that feature.

### External References

- Standard Rust proc-macro attribute pattern using `syn` + `quote`.

## Key Technical Decisions

- **Separate workspace crate (`abt-macros/`)**: Proc macros must be in their own crate with `proc-macro = true`. Cannot live in `abt-grpc`. *(resolves origin question 1)*
- **Proc macro over helper function**: A simpler alternative — `fn require_permission<T>(req: &Request<T>, resource: &str, action: &str) -> Result<AuthContext, Status>` — would reduce boilerplate from two lines to one without proc macro complexity. The proc macro was chosen because it co-locates the permission declaration with the method signature (visible in `cargo expand` and IDE hover), making security posture auditable at a glance. The helper function would require scanning method bodies. This tradeoff (proc macro crate overhead vs declarative visibility) is accepted.
- **Call-site import resolution**: The macro generates `extract_auth(...)` and `error::forbidden(...)` using short names that resolve via existing `use` statements in handler files. This keeps generated code clean and consistent with hand-written style. No fully-qualified `crate::` paths in generated code.
- **`let auth` prepended to body**: The macro inserts `let auth = extract_auth(&request)?;` and `auth.check_permission(...).map_err(...)?;` at the top of the method body. The `auth` variable remains in scope for the rest of the handler. If a handler already has `let auth = extract_auth(...)`, this produces a compile error — preventing double extraction.
- **Detect parameter name from signature**: The macro reads the second parameter's identifier (after `&self`) rather than hardcoding `request`. This is necessary because `permission.rs` uses `_request` for two methods. The macro must preserve the exact identifier from the signature.
- **Streaming method compatibility**: Three methods have non-standard signatures: `download_bom` (bom.rs) and `download_export_file` (excel.rs) return `Result<Response<Self::XxxStream>, Status>` instead of `GrpcResult<T>`, and `upload_file` (excel.rs) takes `Request<Streaming<UploadFileRequest>>` instead of `Request<T>`. The macro must handle these return types generically — it only prepends statements to the body and does not depend on the return type. Test coverage for these methods is mandatory.
- **Pilot-first migration**: Migrate 1-2 simple handlers first to validate the macro, then batch the rest.
- **Opt-in, not enforced**: The macro is declarative convenience. It does not produce a compile error if a new handler method omits the annotation. A CI-level check (e.g., grep for un-annotated handler methods, or a custom clippy lint) would be needed for true enforcement. This is noted as future work outside this plan's scope.

## Open Questions

### Resolved During Planning

- **Proc macro crate location**: Separate `abt-macros/` workspace member. Proc macros cannot be defined in the crate that uses them.
- **`auth` visibility mechanism**: `let auth = ...` prepended to method body — standard Rust scoping makes it visible throughout.
- **Migration order**: By handler file, pilot-first. Start with a simple handler (e.g., `warehouse.rs` or `term.rs`), validate, then migrate the rest file-by-file.

### Deferred to Implementation

- **Exact parameter detection logic**: The macro must handle `&self, request: Request<T>` signatures and also `&self, _request: Request<Empty>` as used in `permission.rs`. Edge cases should be discovered via compilation errors during migration.
- **Unused `auth` variable warnings**: 84 of 101 handler methods use `auth` only for `check_permission` and never reference `auth.user_id` or other fields afterward. This is not a minor edge case — it affects 83% of handlers. Decide at implementation time whether to suppress with `#[allow(unused_variables)]` or accept the warning at scale. The most pragmatic solution is likely `#[allow(unused_variables)]` on the generated `let auth` binding.

## High-Level Technical Design

> *This illustrates the intended approach and is directional guidance for review, not implementation specification. The implementing agent should treat it as context, not code to reproduce.*

**Macro expansion sketch:**

Input:
```
#[require_permission("product", "create")]
async fn create_product(&self, request: Request<CreateProductRequest>) -> GrpcResult<ProductResponse> {
    let req = request.into_inner();
    // business logic
}
```

Output:
```
async fn create_product(&self, request: Request<CreateProductRequest>) -> GrpcResult<ProductResponse> {
    let auth = extract_auth(&request)?;
    auth.check_permission("product", "create").map_err(|_e| error::forbidden("product", "create"))?;
    let req = request.into_inner();
    // business logic
}
```

**Crate dependency graph:**

```
abt-macros (proc-macro, standalone — no dependencies on abt/common)
    ↑
abt-grpc (depends on abt-macros, abt, common)
```

`abt-macros` only needs `syn`, `quote`, and `proc-macro2`. It does not depend on `abt` or `common` — it generates token streams that resolve at the call site.

## Implementation Units

- [ ] **Unit 1: Create `abt-macros` proc-macro crate**

**Goal:** Set up the workspace crate and implement the `#[require_permission]` attribute macro.

**Requirements:** R1, R2, R3, R4

**Dependencies:** None

**Files:**
- Create: `abt-macros/Cargo.toml`
- Create: `abt-macros/src/lib.rs`
- Modify: `Cargo.toml` (add `abt-macros` to workspace members)

**Approach:**
- Create `abt-macros/` with `edition = "2021"`, `[lib] proc-macro = true`
- Dependencies: `syn` (v2, `full` feature), `quote` (v1), `proc-macro2` (v1)
- Implement `#[proc_macro_attribute] fn require_permission(attr, item) -> TokenStream`
- Parse two string literals from `attr` (resource, action)
- Parse the method signature with `syn::ItemFn`
- Extract the second parameter ident (after `&self`) for the request variable name — must handle both `request` and `_request` (underscore-prefixed) as present in `permission.rs`
- Generate: `let auth = extract_auth(&<request_ident>)?;` + `auth.check_permission(<resource>, <action>).map_err(|_e| error::forbidden(<resource>, <action>))?;`
- Prepend to the method body, keeping everything else unchanged

**Patterns to follow:**
- Standard `syn` + `quote` attribute proc macro pattern
- `abt-grpc` uses edition 2021; match that for the macro crate

**Test scenarios:**
- Happy path: `#[require_permission("product", "create")]` on a method with `&self, request: Request<T>` signature expands to valid code with `auth` variable accessible
- Happy path: Method with `&self, _request: Request<Empty>` signature (as in `permission.rs`) — macro reads `_request` from signature and generates `extract_auth(&_request)?`
- Happy path: Server-streaming method returning `Result<Response<Self::XxxStream>, Status>` (as in `bom.rs:download_bom`, `excel.rs:download_export_file`) — macro handles arbitrary return types
- Happy path: Client-streaming method with `Request<Streaming<T>>` parameter (as in `excel.rs:upload_file`) — macro reads parameter ident and generates correct code
- Error path: Attribute with wrong argument count or non-string arguments produces a clear compile error
- Edge case: Method with non-standard parameter names still works (macro reads ident from signature)

**Verification:**
- `cargo build -p abt-macros` compiles without errors
- `cargo build -p abt-grpc` still compiles (no changes to abt-grpc yet, just the new crate)

---

- [ ] **Unit 2: Wire dependency and validate on pilot handlers**

**Goal:** Add `abt-macros` as a dependency to `abt-grpc`, annotate 2-3 pilot handler methods, and verify identical behavior.

**Requirements:** R1, R2, R3, R4, R5, R6, R7

**Dependencies:** Unit 1

**Files:**
- Modify: `abt-grpc/Cargo.toml` (add `abt-macros` dependency)
- Modify: `abt-grpc/src/handlers/product.rs` (pilot migration)
- Modify: `abt-grpc/src/handlers/term.rs` (pilot migration — if it has auth checks)

**Approach:**
- Add `abt-macros = { path = "../abt-macros" }` to `abt-grpc/Cargo.toml`
- In pilot handler files: add `use abt_macros::require_permission;`
- For 2-3 methods: add `#[require_permission(...)]` annotation, remove manual `extract_auth` + `check_permission` lines
- Verify `auth` variable is still usable in the method body (e.g., `auth.user_id` for `operator_id`)
- Keep `use crate::interceptors::auth::extract_auth;` import for now (other methods still use it)
- Confirm AuthService handler (`auth.rs`) remains untouched

**Patterns to follow:**
- Existing handler structure: unit struct handler, `AppState::get().await`, service factory calls
- Existing import set in handler files

**Test scenarios:**
- Happy path: Migrated methods compile and `cargo test -p abt-grpc` passes
- Happy path: `auth.user_id` is still accessible in migrated method bodies
- Happy path: Methods with `_request` parameter name (from `permission.rs`) expand correctly
- Happy path: Streaming-response method (if a pilot includes one) compiles and runs identically
- Integration: Permission-denied response format unchanged (AIP-193 with resource/action metadata)
- Integration: Super admin bypass still works (no macro-level logic, verified through existing `check_permission`)

**Verification:**
- `cargo build -p abt-grpc` compiles with macro-annotated pilot methods
- `cargo test -p abt-grpc` passes (or `cargo test` if integration tests exist)
- No behavioral change compared to manual auth extraction — identical gRPC responses

---

- [ ] **Unit 3: Migrate all remaining handlers**

**Goal:** Replace all remaining manual auth + permission checks with `#[require_permission]` annotations across all handler files (excluding `auth.rs`).

**Requirements:** R8

**Dependencies:** Unit 2

**Files:**
- Modify: `abt-grpc/src/handlers/bom.rs`
- Modify: `abt-grpc/src/handlers/department.rs`
- Modify: `abt-grpc/src/handlers/excel.rs`
- Modify: `abt-grpc/src/handlers/inventory.rs`
- Modify: `abt-grpc/src/handlers/location.rs`
- Modify: `abt-grpc/src/handlers/permission.rs`
- Modify: `abt-grpc/src/handlers/price.rs`
- Modify: `abt-grpc/src/handlers/role.rs`
- Modify: `abt-grpc/src/handlers/user.rs`
- Modify: `abt-grpc/src/handlers/warehouse.rs`
- Complete: `abt-grpc/src/handlers/product.rs` (remaining methods from pilot)
- Complete: `abt-grpc/src/handlers/term.rs` (remaining methods from pilot)

**Approach:**
- For each handler file (excluding `auth.rs`):
  1. Add `use abt_macros::require_permission;` if not already present
  2. For each method with manual `extract_auth` + `check_permission`: add `#[require_permission("resource", "action")]`, remove the two boilerplate lines
  3. Methods that only call `extract_auth` without `check_permission` (if any): annotate with `#[require_permission("resource", "read")]` or equivalent
- Special attention for non-standard methods:
  - `excel.rs:upload_file` — client-streaming with `Request<Streaming<UploadFileRequest>>`
  - `excel.rs:download_export_file` — server-streaming with `Result<Response<Self::DownloadExportFileStream>, Status>`
  - `bom.rs:download_bom` — server-streaming with `Result<Response<Self::DownloadBomStream>, Status>`
  - `permission.rs:list_resources` and `permission.rs:list_permissions` — use `_request` parameter name
- Codebase analysis confirms all 101 call sites follow the exact same two-line pattern (`extract_auth` + `check_permission`). No methods call `extract_auth` without `check_permission`.
- `department.rs` uses `|_|` instead of `|_e|` in map_err closures — the macro will normalize this to `|_e|`. This is cosmetic, not behavioral.
- Migrate file-by-file, compiling after each file to catch issues early
- Do NOT touch `auth.rs` — it uses `extract_user_id_from_header`, not `extract_auth`
- Do NOT touch `convert.rs` — confirmed as pure type conversion module, no handler methods
- Do NOT touch `labor_process.rs` — contains only internal helper functions; auth checks for labor process operations are in `bom.rs`
- **Key technical assumption**: `#[require_permission]` is a method-level attribute inside `#[tonic::async_trait]` impl blocks. Rust expands method-level attributes before impl-level attributes, so `#[require_permission]` sees the original `async fn` signature. The pilot migration (Unit 2) validates this assumption on day one. If async_trait's desugaring interferes, it will surface as a compile error on the first migrated method.

**Test scenarios:**
- Happy path: All handler methods compile after migration, including streaming variants (upload, download)
- Happy path: `permission.rs` methods with `_request` parameter name compile correctly
- Integration: `cargo test` passes for full workspace
- Integration: Permission check results identical to pre-migration (same resource:action pairs, same error format)
- Edge case: Methods that use `auth.user_id` after `request.into_inner()` still compile — `auth` remains in scope
- Edge case: Delegation methods in `bom.rs` (labor process list/create/update/delete/import) compile — `auth` is generated but only used for permission check, then `request.into_inner()` passes to internal helpers

**Verification:**
- `cargo build` succeeds across full workspace
- `cargo test` passes
- `grep -r "extract_auth" abt-grpc/src/handlers/` returns zero matches (except `auth.rs` which uses `extract_user_id_from_header`)
- Every non-auth handler method has a `#[require_permission]` annotation

---

- [ ] **Unit 4: Final verification**

**Goal:** Verify migration completeness — no hand-written `extract_auth` + `check_permission` boilerplate remains, all handler methods have macro annotations, and the workspace builds clean.

**Requirements:** R9

**Dependencies:** Unit 3

**Files:**
- Modify: `abt-grpc/src/handlers/mod.rs` (if any cleanup needed)

**Approach:**
- **Do NOT remove `use crate::interceptors::auth::extract_auth;` imports** — the macro-generated code calls `extract_auth(...)` and requires this import to be in scope at compile time. The compiler sees the expanded code and needs the import regardless of whether the hand-written call was removed.
- Similarly, `use common::error;` must stay — the macro-generated code calls `error::forbidden(...)`.
- Remove any dead `use` imports that are truly no longer needed (e.g., if a handler file had imports only used by the removed boilerplate lines and nothing else)
- Verify no handler file (except `auth.rs`) contains hand-written `let auth = extract_auth(&request)?;` or `auth.check_permission(...)` lines — these should all be replaced by macro annotations
- Run full workspace build and tests
- Run `cargo clippy` to catch any remaining warnings

**Test scenarios:**
- Happy path: `cargo build` clean — no warnings
- Happy path: `cargo test` passes for full workspace
- Integration: `grep -r "let auth = extract_auth" abt-grpc/src/handlers/` returns zero matches outside `auth.rs`

**Verification:**
- `cargo build --all` succeeds without warnings
- `cargo test --all` passes
- No behavioral regression — all permission checks produce identical results
- Every handler method (except `auth.rs`) has a `#[require_permission]` annotation

## System-Wide Impact

- **Interaction graph:** Every gRPC handler method (except AuthService) gains a macro annotation. The generated code calls `extract_auth` and `check_permission` identically to the current manual pattern. No new middleware, interceptors, or callbacks.
- **Error propagation:** Unchanged — macro generates the same `map_err(error::forbidden)` call. gRPC error format (AIP-193) is preserved.
- **State lifecycle risks:** None. The macro is a compile-time transformation. No runtime state changes.
- **API surface parity:** gRPC wire protocol is unchanged. Clients see identical behavior.
- **Unchanged invariants:** `AuthContext::check_permission()` logic, `auth_interceptor` JWT verification, `error::forbidden` error format, AuthService handler — all remain exactly as they are.

## Risks & Dependencies

| Risk | Mitigation |
|------|------------|
| Macro generates incorrect code for unusual method signatures (streaming input/output, `_request` parameter) | Detect parameter name from signature; handle arbitrary return types (macro only prepends to body); compile after each file migration catches issues immediately |
| `#[require_permission]` + `#[tonic::async_trait]` macro expansion ordering | Method-level attributes expand before impl-level; pilot migration validates on day one. If async_trait interferes, surfaces as compile error immediately |
| `extract_auth` and `error` imports must be retained after migration | Macro-generated code calls these via short names; imports stay in all handler files permanently |
| Unused `auth` variable warnings in 84 of 101 handlers (83%) | Accept the warning or add `#[allow(unused_variables)]` on the generated binding at implementation time |
| `syn`/`quote` version compatibility | Use `syn` v2 + `quote` v1 — current stable ecosystem |
| New RPC methods can omit `#[require_permission]` without compile error (opt-in, not enforced) | Declarative visibility aids code review; future CI check could grep for un-annotated trait impl methods |
| R7 wildcard claim (`*:*`) is inaccurate — `check_permission()` only does exact matching | Inherited behavior, not introduced by the macro. If wildcard permissions are a business need, `check_permission()` itself needs modification (out of scope) |

## Sources & References

- **Origin document:** [docs/brainstorms/2026-04-04-rbac-interceptor-macro-requirements.md](docs/brainstorms/2026-04-04-rbac-interceptor-macro-requirements.md)
- Auth interceptor: `abt-grpc/src/interceptors/auth.rs`
- Error module: `common/src/error.rs`
- AuthContext model: `abt/src/models/auth.rs`
- Handler pattern examples: `abt-grpc/src/handlers/product.rs`, `abt-grpc/src/handlers/role.rs`
- Server registration: `abt-grpc/src/server.rs`
