use proc_macro2::Span;
use quote::quote;
use syn::{spanned::Spanned, Error};
use syn_rsx::{Node, NodeName, NodeType};

fn node_name_span(name: &NodeName) -> Span {
    match name {
        NodeName::Path(expr_path) => expr_path.span(),
        NodeName::Dash(dash) => dash.span(),
        NodeName::Colon(colon) => colon.span(),
    }
}

fn cast_type_attribute(node: &Node) -> Option<proc_macro2::TokenStream> {
    let key = node.name_as_string()?;
    let expr = node.value.as_ref()?;
    match key.split(':').collect::<Vec<_>>().as_slice() {
        ["cast", "type"] => Some(quote! {#expr}),
        _ => None,
    }
}

fn attribute_to_token_stream(node: Node) -> Result<proc_macro2::TokenStream, Error> {
    let span: Span = node
        .name
        .as_ref()
        .map(node_name_span)
        .unwrap_or(Span::call_site());
    if let Some(key) = node.name_as_string() {
        if let Some(expr) = node.value {
            match key.split(':').collect::<Vec<_>>().as_slice() {
                ["style", name] => Ok(quote! {
                    __mogwai_node.style(#name, #expr);
                }),
                ["on", event] => Ok(quote! {
                    __mogwai_node.on(#event, #expr);
                }),
                ["window", event] => Ok(quote! {
                    __mogwai_node.window_on(#event, #expr);
                }),
                ["document", event] => Ok(quote! {
                    __mogwai_node.document_on(#event, #expr);
                }),
                ["post", "build"] => Ok(quote! {
                    __mogwai_node.post_build(#expr);
                }),
                ["boolean", name] => Ok(quote! {
                    __mogwai_node.boolean_attribute(#name, #expr);
                }),
                ["this", "later"] => Ok(quote! {
                    __mogwai_node.this_later(#expr);
                }),
                ["patch", "children"] => Ok(quote! {
                    __mogwai_node.patch(#expr);
                }),
                ["cast", "type"] => Ok(quote! {}),
                [attribute_name] => Ok(quote! {
                    __mogwai_node.attribute(#attribute_name, #expr);
                }),
                keys => Err(Error::new(
                    span,
                    format!(
                        "expected a valid attribute key/value pair. Got `{}`",
                        keys.join(":")
                    ),
                )),
            }
        } else {
            Ok(quote! {
                __mogwai_node.boolean_attribute(#key, true);
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
                let mut type_is = quote! { web_sys::HtmlElement };
                'find_type: for att_node in node.attributes.iter() {
                    if let Some(cast_type) = cast_type_attribute(att_node) {
                        type_is = cast_type;
                        break 'find_type;
                    }
                }

                let mut errs: Vec<Error> = vec![];

                let (attribute_tokens, attribute_errs) =
                    partition_unzip(node.attributes, attribute_to_token_stream);
                errs.extend(attribute_errs);

                let (child_tokens, child_errs) = partition_unzip(node.children, node_fn);
                let child_tokens = child_tokens
                    .into_iter()
                    .map(|child| quote! { __mogwai_node.with(#child); });
                errs.extend(child_errs);

                let may_error = combine_errors(errs);
                if let Some(error) = may_error {
                    Err(error)
                } else {
                    Ok(quote! {
                        {
                            let mut __mogwai_node = (#view_path::element(#tag) as #view_path<#type_is>);
                            #(#attribute_tokens)*
                            #(#child_tokens)*
                            __mogwai_node
                        }
                    })
                }
            }
            _ => Err(Error::new(Span::call_site(), "node is missing a name")),
        },
        NodeType::Text => {
            if let Some(value) = node.value {
                Ok(quote! {#view_path::from(#value)})
            } else {
                Err(Error::new(Span::call_site(), "dom child text node value error"))
            }
        },
        NodeType::Block => {
            if let Some(value) = node.value {
                Ok(quote! {#view_path::try_from(#value).ok()})
            } else {
                Err(Error::new(Span::call_site(), "dom child expr node value error"))
            }
        }

        _ => Err(Error::new(
            Span::call_site(),
            "attribute in unsupported position",
        )),
    }
}

fn node_to_view_token_stream(node: Node) -> Result<proc_macro2::TokenStream, Error> {
    walk_node(
        quote! { mogwai::prelude::View },
        node_to_view_token_stream,
        node,
    )
}

fn node_to_hydrateview_token_stream(node: Node) -> Result<proc_macro2::TokenStream, Error> {
    walk_node(
        quote! { mogwai_hydrator::Hydrator },
        node_to_hydrateview_token_stream,
        node,
    )
}

fn node_to_builder_token_stream(node: Node) -> Result<proc_macro2::TokenStream, Error> {
    walk_node(
        quote! { mogwai::prelude::ViewBuilder },
        node_to_builder_token_stream,
        node,
    )
}

fn walk_dom(
    input: proc_macro::TokenStream,
    f: impl Fn(Node) -> Result<proc_macro2::TokenStream, Error>,
) -> proc_macro2::TokenStream {
    match syn_rsx::parse(input, None) {
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
        Err(msg) => {
            let msg = format!("{}", msg);
            quote! {
                compile_error!(#msg)
            }
        }
    }
}

#[proc_macro]
/// Uses an html description to construct a [`View`].
pub fn view(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    proc_macro::TokenStream::from(walk_dom(input, node_to_view_token_stream))
}

#[proc_macro]
/// Uses an html description to construct a [`Hydrator`], which can then be converted
/// into a [`View`] with [`std::convert::TryFrom`].
pub fn hydrate(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    proc_macro::TokenStream::from(walk_dom(input, node_to_hydrateview_token_stream))
}

#[proc_macro]
/// Uses an html description to construct a [`ViewBuilder`].
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
