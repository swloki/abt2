extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse::Parser, parse_macro_input, parse_quote, Expr, ExprCall, FnArg, ItemFn, LitStr, PatType, Stmt};

/// Attribute macro that generates auth extraction and permission check boilerplate.
///
/// Works correctly with `#[tonic::async_trait]` on the impl block by detecting
/// the `Box::pin(async move { ... })` transformation and prepending inside the async block.
#[proc_macro_attribute]
pub fn require_permission(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse two comma-separated string literal arguments
    let args_parsed = syn::punctuated::Punctuated::<LitStr, syn::Token![,]>::parse_terminated
        .parse2(attr.into())
        .unwrap_or_else(|e| {
            panic!(
                "#[require_permission] expects two string literal arguments: \"resource\", \"action\".\nError: {}",
                e
            )
        });

    let args: Vec<LitStr> = args_parsed.into_iter().collect();

    if args.len() != 2 {
        panic!(
            "#[require_permission] expects exactly two string literal arguments (resource, action), got {}",
            args.len()
        );
    }

    let resource = &args[0];
    let action = &args[1];

    // Parse the function item
    let mut func = parse_macro_input!(item as ItemFn);

    // Find the request parameter name (second parameter after &self)
    let request_ident = extract_request_ident(&func)
        .unwrap_or_else(|| {
            panic!(
                "#[require_permission] could not find a typed parameter in function `{}`. \
                 Expected signature: `fn name(&self, request: Request<T>) -> ...`",
                func.sig.ident
            )
        });

    let resource_val = resource.value();
    let action_val = action.value();

    let auth_stmt: Stmt = parse_quote! {
        #[allow(unused_variables)]
        let auth = extract_auth(&#request_ident)?;
    };
    let check_stmt: Stmt = parse_quote! {
        auth.check_permission(#resource_val, #action_val).map_err(|_e| error::forbidden(#resource_val, #action_val))?;
    };

    let stmts_to_prepend = vec![auth_stmt, check_stmt];

    // Check if the body has been transformed by async_trait into Box::pin(async move { ... })
    if prepend_inside_async_block(&mut func.block.stmts, stmts_to_prepend.clone()) {
        // Successfully prepended inside the async block
    } else {
        // Normal case: prepend directly to function body (original async fn)
        let mut new_stmts = stmts_to_prepend;
        new_stmts.extend(func.block.stmts.drain(..));
        func.block.stmts = new_stmts;
    }

    TokenStream::from(quote! { #func })
}

/// Try to find `Box::pin(async move { ... })` as the only statement in the body
/// and prepend statements inside the async block.
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

    // Get the first argument which should be the async block
    let async_expr = match call.args.first_mut() {
        Some(Expr::Async(async_block)) => async_block,
        _ => return false,
    };

    // Prepend statements to the async block
    let mut new_stmts = to_prepend;
    new_stmts.extend(async_expr.block.stmts.drain(..));
    async_expr.block.stmts = new_stmts;

    true
}

/// Check if an expression is `Box::pin(...)`
fn is_box_pin(call: &ExprCall) -> bool {
    if let Expr::Path(path) = call.func.as_ref() {
        let segments = &path.path.segments;
        if segments.len() == 2 {
            return segments[0].ident == "Box" && segments[1].ident == "pin";
        }
    }
    false
}

/// Extract the identifier of the second parameter (after &self) from the function signature.
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
