#![allow(deprecated)]
//! RSX for constructing ViewBuilders
use quote::quote;
use syn::Error;

mod tokens;
use tokens::{AttributeToken, ViewToken};

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

fn node_to_builder_token_stream(
    view_token: &ViewToken,
) -> Result<proc_macro2::TokenStream, Error> {
    #[cfg(feature = "dom")]
    let mogwai_path = quote! { mogwai_dom::core };
    #[cfg(feature = "pxy")]
    let mogwai_path = quote! { pxy_mogwai::core };
    #[cfg(not(any(feature = "dom", feature = "pxy")))]
    let mogwai_path = quote! { mogwai };
    match view_token {
        ViewToken::Element {
            name,
            name_span: _,
            attributes,
            children,
        } => {
            let may_xmlns = attributes.iter().find_map(|att| match att {
                AttributeToken::Xmlns(expr) => Some(expr),
                _ => None,
            });

            let mut errs = vec![];
            let (attribute_tokens, attribute_errs) =
                partition_unzip(attributes.iter(), AttributeToken::try_builder_token_stream);
            errs.extend(attribute_errs);

            let (child_tokens, child_errs) = partition_unzip(children.iter(), |token| {
                node_to_builder_token_stream(token)
            });
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
                let create = if let Some(ns) = may_xmlns {
                    quote! {#mogwai_path::view::ViewBuilder::element_ns(#name, #ns)}
                } else {
                    quote! {#mogwai_path::view::ViewBuilder::element(#name)}
                };
                Ok(quote! {{
                    #create
                        #(#attribute_tokens)*
                        #(#child_tokens)*
                }})
            }
        }
        ViewToken::Text(expr) => Ok(quote! {#mogwai_path::view::ViewBuilder::text(#expr)}),
        ViewToken::Block(expr) => Ok(quote! {
            #[allow(unused_braces)]
            #expr
        }),
    }
}
#[deprecated(since = "0.6", note = "Use `html` or convert to `rsx` instead")]
#[proc_macro]
/// Uses an html description to construct a `ViewBuilder`.
///
/// ## Deprecated since 0.6
/// Use [`html!`] instead, or convert to [`rsx!`].
///
/// ```rust, ignore
/// let my_div = mogwai_dom::html! {
///     <div id="main">
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
/// Looks like real HTML, but some editors have a hard time
/// formatting the mixture of Rust and HTML. If that seems to be
/// the case, try converting to [`rsx!`], which editors tend to format
/// well.
///
/// ```rust, ignore
/// let my_div = mogwai_dom::html! {
///     <div id="main">
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
/// This is easier for editors to format than [`html!`], leading to more natural
/// authoring.
///
/// ```rust, ignore
/// let my_div = mogwai_dom::rsx! {
///     div(id="main") {
///         p() {"Trolls are real"}
///     }
/// };
/// ```
pub fn rsx(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    rsx::parse_with(input, rsx::parse_fn)
}

#[deprecated(
    since = "0.6",
    note = "Use `html!{...}.try_into().unwrap()` or `rsx!{...}.try_into().unwrap()`instead"
)]
#[proc_macro]
/// Uses an html description to construct a `View`.
///
/// This is the same as the following:
/// ```rust, ignore
/// use mogwai_dom::prelude::*;
///
/// let my_view = SsrDom::try_from(html! {
///     <div id="main">
///         <p>"Trolls are real"</p>
///     </div>
/// }).unwrap();
/// ```
pub fn view(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let builder: proc_macro2::TokenStream = builder(input).into();
    let token = quote! {{
        {#builder}.try_into().unwrap()
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
