extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse::Parse, parse::ParseStream, parse_macro_input, parse_quote, FnArg, ItemFn, LitStr,
    PatType, Stmt,
};

/// Arguments for the macro: two string literals — resource code and action.
///
/// Example: `#[require_permission("SHIPPING", "create")]`
struct PermissionArgs {
    resource: LitStr,
    action: LitStr,
}

impl Parse for PermissionArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let resource: LitStr = input.parse()?;
        let _: syn::Token![,] = input.parse()?;
        let action: LitStr = input.parse()?;
        Ok(PermissionArgs { resource, action })
    }
}

/// Attribute macro that generates permission check boilerplate for Axum handlers.
///
/// Accepts two string literal arguments matching `RESOURCE_ACTION_DEFS`:
/// `#[require_permission("SHIPPING", "create")]`
///
/// Expects the handler to have a `RequestContext` parameter. The macro will:
/// 1. Find the `RequestContext` parameter by name convention (`ctx`)
/// 2. Call `crate::permissions::check_permission(&ctx, resource, action)`
/// 3. Return a 403 error if permission is denied
#[proc_macro_attribute]
pub fn require_permission(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as PermissionArgs);
    let resource = args.resource;
    let action = args.action;

    let mut func = parse_macro_input!(item as ItemFn);

    let ctx_ident = extract_ctx_ident(&func).unwrap_or_else(|| {
        panic!(
            "#[require_permission] could not find a `RequestContext` parameter in `{}`. \
             Add a parameter like `ctx: RequestContext`.",
            func.sig.ident
        )
    });

    let resource_val = resource.value();
    let action_val = action.value();

    let check_stmt: Stmt = parse_quote! {
        crate::permissions::check_permission(&#ctx_ident, #resource_val, #action_val).await?;
    };

    let mut new_stmts = vec![check_stmt];
    new_stmts.append(&mut func.block.stmts);
    func.block.stmts = new_stmts;

    TokenStream::from(quote! { #func })
}

/// Extract the identifier of a parameter typed as `RequestContext`.
/// Searches all parameters (no `&self` assumption — Axum handlers are free functions).
fn extract_ctx_ident(func: &ItemFn) -> Option<syn::Ident> {
    for arg in &func.sig.inputs {
        if let FnArg::Typed(PatType { pat, ty, .. }) = arg {
            if let syn::Type::Path(type_path) = ty.as_ref() {
                let segments = &type_path.path.segments;
                if segments.last().is_some_and(|s| s.ident == "RequestContext") {
                    if let syn::Pat::Ident(pat_ident) = pat.as_ref() {
                        return Some(pat_ident.ident.clone());
                    }
                }
            }
        }
    }
    None
}
