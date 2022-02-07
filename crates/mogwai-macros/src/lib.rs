#![allow(deprecated)]
//! RSX for building mogwai::core DOM nodes.
use quote::quote;
use syn::Error;

mod tokens;
use tokens::{AttributeToken, ViewToken};

mod relay;
mod rsx;

fn partition_unzip<S, T, F>(items: impl Iterator<Item = S>, f: F) -> (Vec<T>, Vec<Error>)
where
    F: Fn(S) -> Result<T, Error>,
{
    let (tokens, errs): (Vec<Result<_, _>>, _) = items.map(f).partition(Result::is_ok);
    let tokens = tokens
        .into_iter()
        .filter_map(Result::ok)
        .collect::<Vec<_>>();
    let errs = errs.into_iter().filter_map(Result::err).collect::<Vec<_>>();
    (tokens, errs)
}

fn combine_errors(errs: Vec<Error>) -> Option<Error> {
    errs.into_iter()
        .fold(None, |may_prev_error: Option<Error>, err| {
            if let Some(mut prev_error) = may_prev_error {
                prev_error.combine(err);
                Some(prev_error)
            } else {
                Some(err)
            }
        })
}

fn node_to_builder_token_stream(view_token: &ViewToken) -> Result<proc_macro2::TokenStream, Error> {
    let view_path = quote! { mogwai::core::builder::ViewBuilder };
    match view_token {
        ViewToken::Element {
            name,
            name_span: _,
            attributes,
            children,
        } => {
            let may_type = attributes.iter().find_map(|att| match att {
                AttributeToken::CastType(expr) => {
                    Some(quote! { as mogwai::core::builder::ViewBuilder<#expr> })
                }
                _ => None,
            });

            let type_is = may_type
                .unwrap_or_else(|| {
                    if cfg!(feature = "dom") {
                        quote! {as mogwai::core::builder::ViewBuilder<mogwai::dom::view::Dom>}
                    } else {
                        quote! {}
                    }
                });

            let mut errs = vec![];
            let (attribute_tokens, attribute_errs) =
                partition_unzip(attributes.iter(), AttributeToken::try_builder_token_stream);
            errs.extend(attribute_errs);

            let (child_tokens, child_errs) =
                partition_unzip(children.iter(), node_to_builder_token_stream);
            let child_tokens = child_tokens.into_iter().map(|child| {
                quote! {
                        .append(#child)
                }
            });
            errs.extend(child_errs);

            let may_error = combine_errors(errs);
            if let Some(error) = may_error {
                Err(error)
            } else {
                let create = quote! {#view_path::element(#name)};
                Ok(quote! {
                    #create
                        #(#attribute_tokens)*
                        #(#child_tokens)*
                        #type_is
                })
            }
        }
        ViewToken::Text(expr) => Ok(quote! {mogwai::core::builder::ViewBuilder::text(#expr)}),
        ViewToken::Block(expr) => Ok(quote! {
            #[allow(unused_braces)]
            #expr
        }),
    }
}
#[deprecated(
    since = "0.6",
    note = "Use `html` or convert to `rsx` instead"
)]
#[proc_macro]
/// Uses an html description to construct a `ViewBuilder`.
///
/// ```rust
/// let my_div = mogwai::macros::builder! {
///     <div cast:type=mogwai::dom::view::Dom id="main">
///         <p>"Trolls are real"</p>
///     </div>
/// };
/// ```
pub fn builder(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    html(input)
}

#[proc_macro]
/// Uses an html description to construct a `ViewBuilder`.
///
/// ```rust
/// let my_div = mogwai::macros::html! {
///     <div cast:type=mogwai::dom::view::Dom id="main">
///         <p>"Trolls are real"</p>
///     </div>
/// };
/// ```
pub fn html(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    rsx::parse_with(input, rsx::parse_html)
}

#[proc_macro]
/// Uses a function-style description to construct a `ViewBuilder`.
///
/// ```rust
/// let my_div = mogwai::macros::rsx! {
///     div(cast:type=mogwai::dom::view::Dom, id="main") {
///         p() {"Trolls are real"}
///     }
/// };
/// ```
pub fn rsx(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    rsx::parse_with(input, rsx::parse_fn)
}

#[deprecated(
    since = "0.6",
    note = "Use `html!{...}.build().unwrap()` or `rsx!{...}.build().unwrap()`instead"
)]
#[proc_macro]
/// Uses an html description to construct a `View`.
///
/// This is the same as the following:
/// ```rust
/// # use mogwai::prelude::*;
/// let my_view = builder! {
///     <div cast:type=Dom id="main">
///         <p>"Trolls are real"</p>
///     </div>
/// }.build().unwrap();
/// ```
pub fn view(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let builder: proc_macro2::TokenStream = builder(input).into();
    let token = quote! {{
        {#builder}.build().unwrap()
    }};
    proc_macro::TokenStream::from(token)
}

#[proc_macro]
pub fn target_arch_is_wasm32(_: proc_macro::TokenStream) -> proc_macro::TokenStream {
    proc_macro::TokenStream::from(quote! {
        cfg!(target_arch = "wasm32")
    })
}

#[cfg(test)]
mod ssr_tests {
    use std::str::FromStr;

    #[test]
    fn can_parse_rust_closure() {
        let expr: syn::Expr = syn::parse_str(r#"|i:i32| format!("{}", i)"#).unwrap();
        match expr {
            syn::Expr::Closure(_) => {}
            _ => panic!("wrong expr parse, expected closure"),
        }
    }

    #[test]
    fn can_token_stream_from_string() {
        let _ts = proc_macro2::TokenStream::from_str(r#"|i:i32| format!("{}", i)"#).unwrap();
    }

    #[test]
    fn can_parse_from_token_stream() {
        let _ts = proc_macro2::TokenStream::from_str(r#"<div class="any_class" />"#).unwrap();
    }
}
