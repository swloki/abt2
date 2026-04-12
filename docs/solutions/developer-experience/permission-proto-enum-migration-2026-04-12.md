---
title: Migrate permission macro from string literals to proto-generated enums
date: 2026-04-12
category: developer-experience
module: abt-grpc
problem_type: developer_experience
component: tooling
severity: low
resolution_type: migration
applies_when:
  - Using proc macros with hardcoded string arguments that should be type-checked
  - Needing proto enums consumed by both Rust backend and TypeScript frontend
  - Bridging proto SCREAMING_SNAKE_CASE to runtime lowercase strings
tags:
  - proc-macro
  - protobuf
  - permissions
  - type-safety
  - enum-migration
---

# Migrate permission macro from string literals to proto-generated enums

## Context

The `#[require_permission]` proc macro accepted string literals (`#[require_permission("warehouse", "read")]`) for permission checks. This created three independent sources of truth: `resources.rs` static arrays, 101 handler string literals, and proto bare string fields. Typos in resource or action names passed compilation but caused runtime permission denials. The frontend had no compile-time permission constants because proto didn't define enum types for resources and actions.

## Guidance

**Define permission enums in proto, then use enum paths in the macro.**

### 1. Define enums in proto

```protobuf
// proto/abt/v1/permission.proto
enum Resource {
  PRODUCT = 0; TERM = 1; BOM = 2; WAREHOUSE = 3;
  LOCATION = 4; INVENTORY = 5; PRICE = 6; LABOR_PROCESS = 7;
  USER = 8; ROLE = 9; PERMISSION = 10; DEPARTMENT = 11; EXCEL = 12;
}
enum Action { READ = 0; WRITE = 1; DELETE = 2; }
```

### 2. Bridge proto enums to runtime strings

Prost generates `as_str_name()` returning SCREAMING_SNAKE_CASE, but the JWT/runtime permission system uses lowercase strings like `"warehouse"`. Create a trait with hardcoded match arms:

```rust
pub trait PermissionCode {
    fn code(&self) -> &'static str;
}

impl PermissionCode for Resource {
    fn code(&self) -> &'static str {
        match self {
            Self::Product => "product",
            Self::Warehouse => "warehouse",
            Self::LaborProcess => "labor_process",
            // ... compiler enforces exhaustiveness
        }
    }
}
```

Use hardcoded match instead of `as_str_name().to_lowercase()` because:
- No runtime string allocation or transformation
- Compiler enforces exhaustiveness — adding a new proto variant without updating the match is a compile error

### 3. Update the proc macro to accept enum paths

```rust
// Before: parsed LitStr tokens
// After: parse Expr paths, generate .code() calls
let check_stmt: Stmt = parse_quote! {
    auth.check_permission(#resource.code(), #action.code())
        .map_err(|_e| error::forbidden(#resource.code(), #action.code()))?;
};
```

The compiler validates that `Resource::Warehouse` and `Action::Read` exist at the call site.

### 4. Handling crate dependency boundaries

The core `abt` crate cannot reference `abt-grpc` types (dependency: `common <- abt <- abt-grpc`). Place `PermissionCode` in `abt-grpc`, keep `resources.rs` display-name mappings in `abt`, and add consistency tests in `abt-grpc` that cross-validate both.

## Why This Matters

- **Compile-time safety**: `#[require_permission(Resource::Invalid, Action::Read)]` fails to compile
- **Cross-platform**: Proto enums generate both Rust types and TypeScript constants
- **Single source of truth**: Proto is the canonical definition; consistency tests catch drift
- **Zero-cost runtime**: `.code()` returns `&'static str` via match on i32 — no allocation

## When to Apply

- When a proc macro accepts string arguments that correspond to enum variants
- When proto is the shared contract between frontend and backend
- When runtime strings differ from proto naming conventions (case, formatting)
- When crate boundaries prevent direct type sharing between layers

## Examples

**Before (strings):**
```rust
#[require_permission("warehouse", "read")]  // typo passes compilation
#[require_permission("labor_process", "read")]  // inconsistent casing
```

**After (enum paths):**
```rust
use crate::permissions::PermissionCode;

#[require_permission(Resource::Warehouse, Action::Read)]
#[require_permission(Resource::LaborProcess, Action::Read)]
```

**Consistency test pattern:**
```rust
#[test]
fn all_resource_codes_match_resources_rs() {
    use abt::models::resources::collect_all_resources;
    let defined: HashSet<&str> = collect_all_resources().iter().map(|r| r.resource_code).collect();
    for variant in ALL_RESOURCES {
        assert!(defined.contains(variant.code()), "Missing: {:?}", variant);
    }
}
```

## Related

- [Proc-Macro Attribute for Auth Boilerplate with async_trait Compatibility](require-permission-macro-async-trait-2026-04-05.md) — covers the original macro creation and Box::pin penetration for `#[tonic::async_trait]`
