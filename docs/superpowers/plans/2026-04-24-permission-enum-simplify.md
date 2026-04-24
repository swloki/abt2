# Permission Enum Simplification Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminate manual permission code mapping and API-level case conversion by making proto enums the sole source of truth.

**Architecture:** Two changes: (1) replace the `PermissionCode` manual match with automatic `as_str_name().to_lowercase()` conversion, so adding a new proto enum variant requires zero Rust changes; (2) change `CheckPermissionRequest` from string fields to proto enum fields, eliminating `normalize_code`/`to_proto_code` conversion at the API boundary.

**Tech Stack:** Rust, prost (proto enum generation), tonic (gRPC), proto3

---

### Task 1: Simplify PermissionCode — auto PascalCase→snake_case

The `PermissionCode` trait currently has a manual match for each `Resource` and `Action` variant. Adding a new resource requires adding a match arm here. Replace with automatic conversion using the proto-generated `as_str_name()` method (returns SCREAMING_SNAKE_CASE like `"LABOR_PROCESS_DICT"`) converted to lowercase.

**Files:**
- Modify: `abt-grpc/src/permissions/mod.rs`
- Modify: `abt-macros/src/lib.rs` (add `&` before `.code()` since return type changes to `String`)
- Modify: `abt-grpc/src/permissions/tests.rs`

- [ ] **Step 1: Update PermissionCode trait in `abt-grpc/src/permissions/mod.rs`**

Change the return type from `&'static str` to `String`, replace manual match bodies with `self.as_str_name().to_lowercase()`:

```rust
pub trait PermissionCode {
    fn code(&self) -> String;
}

impl PermissionCode for Resource {
    fn code(&self) -> String {
        self.as_str_name().to_lowercase()
    }
}

impl PermissionCode for Action {
    fn code(&self) -> String {
        self.as_str_name().to_lowercase()
    }
}
```

Delete the entire manual match blocks for both `Resource` and `Action`.

- [ ] **Step 2: Update macro in `abt-macros/src/lib.rs`**

The macro generates calls like `check_permission_for_resource(&auth, Resource::Product.code(), Action::Read.code())`. Since `.code()` now returns `String`, add `&` to create `&str` references via deref coercion.

In the `parse_quote!` block for `check_stmt`, change:

```rust
let check_stmt: Stmt = parse_quote! {
    crate::permissions::check_permission_for_resource(
        &auth,
        &#resource.code(),
        &#action.code(),
    ).map_err(|_e| error::forbidden(&#resource.code(), &#action.code()))?;
};
```

The `&` prefix before `#resource.code()` creates a reference to the temporary `String`, which auto-derefs to `&str` when passed to the function.

- [ ] **Step 3: Update tests in `abt-grpc/src/permissions/tests.rs`**

The tests compare `.code()` with string literals. `assert_eq!` works with `String == &str` (no change needed for those).

For `all_resource_codes_match_resources_rs` and `all_action_codes_exist_in_resources_rs`, the `HashSet<&str>.contains()` call needs updating since `.code()` now returns `String`:

```rust
// Before: defined_resources.contains(code) where code: &str
// After:  defined_resources.contains(code.as_str()) where code: String
```

Update both test functions — change `variant.code()` usage in `contains()` calls to `variant.code().as_str()` or use `defined_resources.contains(&*variant.code())`.

- [ ] **Step 4: Build and test**

Run: `cargo build`
Expected: compiles successfully (all 18 handler files using `require_permission` macro should still work)

Run: `cargo test -p abt-grpc -- permissions`
Expected: all permission tests pass

- [ ] **Step 5: Commit**

```bash
git add abt-grpc/src/permissions/mod.rs abt-macros/src/lib.rs abt-grpc/src/permissions/tests.rs
git commit -m "refactor: auto-generate PermissionCode from proto enum names"
```

---

### Task 2: Change CheckPermissionRequest to use proto enums

Change `CheckPermissionRequest` from string fields (`resource_code`, `action_code`) to enum fields (`resource`, `action`). This eliminates the `normalize_code` function and provides type safety at the API boundary.

**Files:**
- Modify: `proto/abt/v1/permission.proto`
- Modify: `abt-grpc/src/handlers/permission.rs`

- [ ] **Step 1: Update proto definition**

In `proto/abt/v1/permission.proto`, change `CheckPermissionRequest`:

```protobuf
message CheckPermissionRequest {
    int64 user_id = 1;
    Resource resource = 2;
    Action action = 3;
}
```

- [ ] **Step 2: Regenerate proto code**

Run: `cargo build`
This triggers `abt-grpc/build.rs` which regenerates `abt-grpc/src/generated/abt.v1.rs`.

The generated `CheckPermissionRequest` struct will now have `resource: i32` and `action: i32` fields (prost represents proto enums as `i32`).

- [ ] **Step 3: Update handler in `abt-grpc/src/handlers/permission.rs`**

In the `check_permission` method, convert the `i32` fields to enums and use `.code()`:

```rust
async fn check_permission(
    &self,
    request: Request<CheckPermissionRequest>,
) -> GrpcResult<CheckPermissionResponse> {
    let req = request.into_inner();
    let resource = Resource::try_from(req.resource)
        .map_err(|_| error::invalid_argument("resource"))?;
    let action = Action::try_from(req.action)
        .map_err(|_| error::invalid_argument("action"))?;

    let state = AppState::get().await;
    let srv = state.permission_service();

    let has_permission = srv
        .check_permission(req.user_id, &resource.code(), &action.code())
        .await
        .map_err(error::err_to_status)?;

    Ok(Response::new(CheckPermissionResponse { has_permission }))
}
```

Add import for `Resource` and `Action` if not already present (they should be from the `use crate::generated::abt::v1::*` wildcard).

- [ ] **Step 4: Delete `normalize_code` function**

In `abt-grpc/src/handlers/permission.rs`, delete:

```rust
/// Normalize incoming codes from proto enum names (SCREAMING_SNAKE_CASE) to internal lowercase.
fn normalize_code(code: &str) -> String {
    code.to_lowercase()
}
```

- [ ] **Step 5: Verify `to_proto_code` is still needed**

`to_proto_code` (`.to_uppercase()`) is still used in response building for `ResourceInfo` and `PermissionInfo` which use string fields. Keep it — do not delete.

Check all remaining usages of `to_proto_code` are only in response-building functions (`group_resources`, `group_resources_by_refs`, `group_permissions`, `get_user_permissions`). If `normalize_code` has no remaining callers, it's safe to delete.

- [ ] **Step 6: Build and test**

Run: `cargo build`
Expected: compiles successfully

Run: `cargo test -p abt-grpc`
Expected: all tests pass

- [ ] **Step 7: Commit**

```bash
git add proto/abt/v1/permission.proto abt-grpc/src/handlers/permission.rs abt-grpc/src/generated/abt.v1.rs
git commit -m "refactor: use proto enums in CheckPermissionRequest, remove normalize_code"
```
