//! RSX for building mogwai DOM nodes.
use proc_macro2::Span;
use quote::quote;
use syn::Error;
use syn_rsx::{Node, NodeType};

fn attribute_to_token_stream(node: Node) -> Result<proc_macro2::TokenStream, Error> {
    let span = node.name_span().unwrap_or(Span::call_site());
    if let Some(key) = node.name_as_string() {
        if let Some(expr) = node.value {
            match key.split(':').collect::<Vec<_>>().as_slice() {
                ["xmlns"] => Ok(quote! {
                    .with_namespace(#expr)
                }),
                ["style"] => Ok(quote! {
                    .with_style_stream(#expr)
                }),
                ["style", name] => Ok(quote! {
                    .with_single_style_stream(#name, #expr)
                }),
                ["on", event] => Ok(quote! {
                    .with_event(#event, #expr)
                }),
                ["window", event] => Ok(quote! {
                    .with_window_event(#event, #expr)
                }),
                ["document", event] => Ok(quote! {
                    .with_document_event(#event, #expr)
                }),
                ["boolean", name] => Ok(quote! {
                    .with_single_bool_attrib_stream(#name, #expr)
                }),
                ["patch", "children"] => Ok(quote! {
                    .with_child_stream(#expr)
                }),
                ["cast", "type"] => Ok(quote! {
                    .with_type::<#expr>()
                }),
                [attribute_name] => Ok(quote! {
                    .with_single_attrib_stream(#attribute_name, #expr)
                }),
                keys => {
                    let attribute_name = keys.join(":");
                    Ok(quote! {
                        .with_attrib_stream(#attribute_name, #expr)
                    })
                }
            }
        } else {
            Ok(quote! {
                .with_single_bool_attrib_stream(#key, true)
            })
        }
    } else {
        Err(Error::new(span, "dom attribute is missing a name"))
    }
}

fn partition_unzip<T, F>(items: Vec<T>, f: F) -> (Vec<proc_macro2::TokenStream>, Vec<Error>)
where
    F: Fn(T) -> Result<proc_macro2::TokenStream, Error>,
{
    let (tokens, errs): (Vec<Result<_, _>>, _) = items.into_iter().map(f).partition(Result::is_ok);
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

fn walk_node<F>(
    view_path: proc_macro2::TokenStream,
    node_fn: F,
    node: Node,
) -> Result<proc_macro2::TokenStream, Error>
where
    F: Fn(Node) -> Result<proc_macro2::TokenStream, Error>,
{
    match node.node_type {
        NodeType::Element => match node.name_as_string() {
            Some(tag) => {
                let mut errs: Vec<Error> = vec![];

                let (attribute_tokens, attribute_errs) =
                    partition_unzip(node.attributes, attribute_to_token_stream);
                errs.extend(attribute_errs);

                let (child_tokens, child_errs) = partition_unzip(node.children, node_fn);
                let child_tokens = child_tokens.into_iter().map(|child| {
                    quote! {
                            .with_child(#child)
                    }
                });
                errs.extend(child_errs);

                let may_error = combine_errors(errs);
                if let Some(error) = may_error {
                    Err(error)
                } else {
                    let create = quote! {#view_path::element(#tag)};
                    Ok(quote! {
                        #create
                            #(#attribute_tokens)*
                            #(#child_tokens)*
                    })
                }
            }
            _ => Err(Error::new(Span::call_site(), "node is missing a name")),
        },
        NodeType::Text => {
            if let Some(value) = node.value {
                Ok(quote! {mogwai::builder::ViewBuilder::text(#value)})
            } else {
                Err(Error::new(
                    Span::call_site(),
                    "dom child text node value error",
                ))
            }
        }
        NodeType::Block => {
            if let Some(value) = node.value {
                Ok(quote! {ViewBuilder::try_from(#value).ok()})
            } else {
                Err(Error::new(
                    Span::call_site(),
                    "dom child expr node value error",
                ))
            }
        }

        _ => Err(Error::new(
            Span::call_site(),
            "attribute in unsupported position",
        )),
    }
}

fn _node_to_view_token_stream(node: Node) -> Result<proc_macro2::TokenStream, Error> {
    walk_node(
        quote! { mogwai::prelude::View },
        _node_to_view_token_stream,
        node,
    )
}

fn _node_to_hydrateview_token_stream(node: Node) -> Result<proc_macro2::TokenStream, Error> {
    walk_node(
        quote! { mogwai_hydrator::Hydrator },
        _node_to_hydrateview_token_stream,
        node,
    )
}

fn node_to_builder_token_stream(node: Node) -> Result<proc_macro2::TokenStream, Error> {
    walk_node(
        quote! { mogwai::builder::ViewBuilder },
        node_to_builder_token_stream,
        node,
    )
}

fn walk_dom(
    input: proc_macro::TokenStream,
    f: impl Fn(Node) -> Result<proc_macro2::TokenStream, Error>,
) -> proc_macro2::TokenStream {
    match syn_rsx::parse(input) {
        Ok(parsed) => {
            let (tokens, errs) = partition_unzip(parsed, f);
            if let Some(error) = combine_errors(errs) {
                error.to_compile_error().into()
            } else {
                match tokens.len() {
                    0 => quote! { compile_error("dom/hydrate macro must not be empty") },
                    1 => {
                        let ts = &tokens[0];
                        quote! { #ts }
                    }
                    _ => quote! { vec![#(#tokens),*] },
                }
            }
        }
        Err(error) => error.to_compile_error(),
    }
}

#[proc_macro]
/// Uses an html description to construct a `ViewBuilder`.
///
/// ```rust
/// # extern crate mogwai;
///
/// let my_div = builder! {
///     <div id="main">
///         <p>"Trolls are real"</p>
///     </div>
/// };
/// ```
pub fn builder(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    proc_macro::TokenStream::from(walk_dom(input, node_to_builder_token_stream))
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
