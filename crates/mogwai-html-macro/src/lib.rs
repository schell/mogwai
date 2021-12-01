//! RSX for building mogwai DOM nodes.
use std::convert::TryFrom;

use proc_macro2::Span;
use quote::{format_ident, quote};
use syn::{parse_quote, spanned::Spanned, Error};
use syn_rsx::Node;

mod tokens;
use tokens::{get_ident, AttributeToken, NamedRelay, ViewToken};

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
    let view_path = quote! { mogwai::builder::ViewBuilder };
    match view_token {
        ViewToken::Element {
            name,
            name_span: _,
            attributes,
            children,
        } => {
            let may_type = attributes.iter().find_map(|att| match att {
                AttributeToken::CastType(expr) => {
                    Some(quote! { as mogwai::builder::ViewBuilder<#expr> })
                }
                _ => None,
            });

            let type_is = may_type
                .unwrap_or_else(|| quote! {as mogwai::builder::ViewBuilder<mogwai::view::Dom>});

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
        ViewToken::Text(expr) => Ok(quote! {mogwai::builder::ViewBuilder::text(#expr)}),
        ViewToken::Block(expr) => Ok(quote! {
            #[allow(unused_braces)]
            #expr
        }),
    }
}

#[proc_macro]
/// Uses an html description to construct a `ViewBuilder`.
///
/// ```rust
/// extern crate mogwai;
///
/// let my_div = mogwai::macros::builder! {
///     <div cast:type=mogwai::view::Dom id="main">
///         <p>"Trolls are real"</p>
///     </div>
/// };
/// ```
pub fn builder(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let tokens = match syn_rsx::parse(input) {
        Ok(parsed) => {
            let (view_tokens, errs) = partition_unzip(parsed.into_iter(), ViewToken::try_from);
            if let Some(error) = combine_errors(errs) {
                return error.to_compile_error().into();
            }
            let (tokens, errs) = partition_unzip(view_tokens.iter(), node_to_builder_token_stream);
            if let Some(error) = combine_errors(errs) {
                return error.to_compile_error().into();
            }

            match tokens.len() {
                0 => quote! { compile_error("dom/hydrate macro must not be empty") },
                1 => {
                    let ts = &tokens[0];
                    quote! { #ts }
                }
                _ => quote! { vec![#(#tokens),*] },
            }
        }
        Err(error) => error.to_compile_error(),
    };

    proc_macro::TokenStream::from(tokens)
}

#[proc_macro]
/// Uses an html description to construct a `View`.
///
/// This is the same as the following:
/// ```rust
/// extern crate mogwai;
///
/// let my_div = mogwai::macros::view! {
///     <div cast:type=mogwai::view::Dom id="main">
///         <p>"Trolls are real"</p>
///     </div>
/// };
/// ```
pub fn view(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let builder: proc_macro2::TokenStream = builder(input).into();
    let token = quote! {{
        use std::convert::TryFrom;
        mogwai::view::View::try_from(#builder).unwrap()
    }};
    proc_macro::TokenStream::from(token)
}

struct StructViewField {
    msg_type: proc_macro2::TokenStream,
    fn_name: syn::Ident,
    is_input: bool,
}

impl StructViewField {
    fn from_attribute(attribute: &AttributeToken) -> Option<Self> {
        fn mk(
            expr: &syn::Expr,
            msg_type: proc_macro2::TokenStream,
            is_input: bool,
        ) -> Option<StructViewField> {
            Some(StructViewField {
                fn_name: get_ident(expr, "")?,
                msg_type,
                is_input,
            })
        }
        match attribute {
            AttributeToken::CastType(_) => None,
            AttributeToken::PostBuild(_) => None,
            AttributeToken::CaptureView(expr) => mk(expr, quote! { T }, false),
            AttributeToken::Xmlns(expr) => mk(expr, quote! { String }, true),
            AttributeToken::Style(expr) => mk(expr, quote! { String }, true),
            AttributeToken::StyleSingle(_, expr) => mk(expr, quote! { String }, true),
            AttributeToken::On(_, expr) => mk(expr, quote! { T::Event }, false),
            AttributeToken::Window(_, expr) => mk(expr, quote! { T::Event }, false),
            AttributeToken::Document(_, expr) => mk(expr, quote! { T::Event }, false),
            AttributeToken::BooleanSingle(_, expr) => mk(expr, quote! { bool }, true),
            AttributeToken::BooleanTrue(_) => None,
            AttributeToken::PatchChildren(expr) => {
                mk(expr, quote! { ListPatch<ViewBuilder<T>> }, true)
            }
            AttributeToken::Attrib(_, expr) => mk(expr, quote! { String }, true),
            AttributeToken::Other(name, _) => panic!("unsupported RSX attribute: {}", name),
        }
    }

    fn to_field(&self) -> proc_macro2::TokenStream {
        let StructViewField {
            msg_type,
            fn_name,
            is_input,
        } = self;
        if *is_input {
            let id = format_ident!("tx_{}", fn_name);
            quote! { #id: mogwai::channel::broadcast::Sender<#msg_type> }
        } else {
            let id = format_ident!("{}_chan", fn_name);
            quote! { #id: mogwai::channel::broadcast::Channel<#msg_type> }
        }
    }

    fn to_chan(&self) -> proc_macro2::TokenStream {
        let StructViewField {
            msg_type,
            fn_name,
            is_input,
        } = self;
        if *is_input {
            let tx = format_ident!("tx_{}", fn_name);
            let rx = format_ident!("rx_{}", fn_name);
            quote! {
                let (#tx, #rx) = mogwai::channel::broadcast::bounded::<#msg_type>(1);
            }
        } else {
            let chan = format_ident!("{}_chan", fn_name);
            quote! {
                let mut #chan:mogwai::channel::broadcast::Channel<#msg_type> = mogwai::channel::broadcast::Channel::new(1);
                #chan.set_overflow(true);
            }
        }
    }

    fn to_field_name_only(&self) -> proc_macro2::TokenStream {
        let StructViewField {
            msg_type: _,
            fn_name,
            is_input,
        } = self;
        if *is_input {
            let id = format_ident!("tx_{}", fn_name);
            quote! {#id}
        } else {
            let id = format_ident!("{}_chan", fn_name);
            quote! {#id}
        }
    }

    fn to_impl_fns(&self) -> Result<Vec<proc_macro2::TokenStream>, Error> {
        let StructViewField {
            msg_type,
            fn_name,
            is_input,
        } = self;
        let tokens = if *is_input {
            let tx = format_ident!("tx_{}", fn_name);
            let stream = format_ident!("{}_with_stream", fn_name);
            vec![
                quote! {
                    /// Send an asyncronous update.
                    pub async fn #fn_name(&mut self, s: impl Into<#msg_type>) -> Result<(), ()> {
                        self.#tx.broadcast(s.into()).await
                            .map(|_| ())
                            .map_err(|_| ())
                    }
                },
                quote! {
                    /// Set a stream of asyncronous updates.
                    pub fn #stream(&self, s: impl Streamable<#msg_type> + 'static + Unpin) {
                        let mut tx = self.#tx.clone();
                        mogwai::spawn(async move {
                            use mogwai::futures::StreamExt;
                            mogwai::futures::pin_mut!(s);
                            while let Some(msg) = s.next().await {
                                tx.broadcast(msg).await.unwrap();
                            }
                        });
                    }
                },
            ]
        } else {
            let chan = format_ident!("{}_chan", fn_name);
            let stream = format_ident!("{}_stream", fn_name);
            let sink = format_ident!("{}_sink", fn_name);
            vec![
                quote! {
                    /// Wait for a single event to occur.
                    pub async fn #fn_name(&self) -> Option<#msg_type> {
                        use mogwai::futures::StreamExt;
                        let mut rx = self.#chan.receiver();
                        rx.next().await
                    }
                },
                quote! {
                    /// Return a [`Stream`] to stream events from.
                    pub fn #stream(&self) -> impl mogwai::target::Streamable<#msg_type> + 'static + Unpin {
                        self.#chan.receiver()
                    }
                },
                quote! {
                    /// Return a [`Sink`] to send events into.
                    pub fn #sink(&self) -> impl mogwai::futures::Sink<#msg_type> + 'static + Unpin {
                        use mogwai::futures::IntoSenderSink;
                        self.#chan.sender().sink()
                    }
                },
            ]
        };
        Ok(tokens)
    }
}

fn make_struct_fields_from_attributes(
    attributes: &[AttributeToken],
) -> Result<Vec<StructViewField>, Error> {
    let mut fields: Vec<StructViewField> = vec![];
    for attribute in attributes {
        if let Some(token) = StructViewField::from_attribute(attribute) {
            fields.push(token);
        }
    }
    Ok(fields)
}

fn make_struct_fields(node: &ViewToken) -> Result<Vec<StructViewField>, Error> {
    let mut fields = vec![];
    match node {
        ViewToken::Element {
            name: _,
            name_span: _,
            attributes,
            children,
        } => {
            fields.extend(make_struct_fields_from_attributes(&attributes)?);
            for child in children.iter() {
                fields.extend(make_struct_fields(child)?);
            }
        }
        ViewToken::Text(_) => {}
        ViewToken::Block(expr) => {
            if let Some(id) = get_ident(&expr, "") {
                fields.push(StructViewField {
                    msg_type: quote! { String },
                    fn_name: id,
                    is_input: true,
                });
            } else {
                return Err(Error::new(
                    expr.span(),
                    &format!("expression must be an identifier:\n{:#?}", expr),
                ));
            }
        }
    }
    Ok(fields)
}

fn ident_to_path_expr(ident: syn::Ident) -> syn::Expr {
    let mut segments = syn::punctuated::Punctuated::default();
    segments.push(syn::PathSegment {
        ident,
        arguments: syn::PathArguments::None,
    });
    let path = syn::Path {
        leading_colon: None,
        segments,
    };
    let epath = syn::ExprPath {
        attrs: vec![],
        qself: None,
        path,
    };
    syn::Expr::Path(epath)
}

fn output_chan(expr: syn::Expr) -> syn::Expr {
    if let Some(id) = get_ident(&expr, "") {
        let ident = format_ident!("{}_chan", id);
        syn::parse_quote! {
            #ident.sender().sink()
        }
    } else {
        expr
    }
}

fn input_rx(expr: syn::Expr) -> syn::Expr {
    if let Some(id) = get_ident(&expr, "rx_") {
        let ident = ident_to_path_expr(id);
        syn::parse_quote! {
            #ident.clone()
        }
    } else {
        expr
    }
}

fn rewrite_attribute_token_for_struct_builder(attribute: AttributeToken) -> AttributeToken {
    use AttributeToken::*;
    match attribute {
        CastType(expr) => CastType(expr),
        PostBuild(expr) => PostBuild(expr),
        CaptureView(expr) => CaptureView(output_chan(expr)),
        Xmlns(expr) => Xmlns(input_rx(expr)),
        Style(expr) => Style(input_rx(expr)),
        StyleSingle(name, expr) => StyleSingle(name, input_rx(expr)),
        On(name, expr) => On(name, output_chan(expr)),
        Window(name, expr) => Window(name, output_chan(expr)),
        Document(name, expr) => Document(name, output_chan(expr)),
        BooleanSingle(name, expr) => BooleanSingle(name, input_rx(expr)),
        BooleanTrue(expr) => BooleanTrue(expr),
        PatchChildren(expr) => PatchChildren(input_rx(expr)),
        Attrib(name, expr) => Attrib(name, input_rx(expr)),
        Other(name, expr) => Other(name, input_rx(expr)),
    }
}

fn rewrite_view_token_for_struct_builder(view_token: &ViewToken) -> ViewToken {
    let view_token: ViewToken = view_token.clone();
    match view_token {
        ViewToken::Element {
            name,
            name_span,
            attributes,
            children,
        } => {
            let mut attributes: Vec<AttributeToken> = attributes
                .into_iter()
                .map(rewrite_attribute_token_for_struct_builder)
                .collect::<Vec<_>>();
            attributes.push(AttributeToken::CastType(parse_quote! {T}));
            let children = children
                .iter()
                .map(rewrite_view_token_for_struct_builder)
                .collect::<Vec<_>>();
            ViewToken::Element {
                name,
                name_span,
                attributes,
                children,
            }
        }
        ViewToken::Text(expr) => ViewToken::Text(expr),
        ViewToken::Block(expr) => ViewToken::Block(input_rx(expr)),
    }
}

fn make_struct_builder(view_token: &ViewToken) -> Result<proc_macro2::TokenStream, Error> {
    let view_token = rewrite_view_token_for_struct_builder(view_token);
    node_to_builder_token_stream(&view_token)
}

///// Convert a single RSX node into a single custom struct view and implementations.
//fn node_to_relay(node: Node) -> Result<proc_macro2::TokenStream, Error> {
//    let name_span = node.name_span().unwrap_or(Span::call_site());
//    let relay = NamedRelay::try_from(node)?;
//
//    let struct_def = relay.struct_def();
//    let builder = make_struct_builder(child)?;
//    let struct_fields = fields.iter().map(StructViewField::to_field);
//    let chans = fields.iter().map(StructViewField::to_chan);
//    let just_field_names = fields.iter().map(StructViewField::to_field_name_only);
//    let (impl_fns, errs) = partition_unzip(fields.iter(), StructViewField::to_impl_fns);
//    if let Some(err) = combine_errors(errs) {
//        return Err(err);
//    }
//    let impl_fns = impl_fns.into_iter().flatten();
//    let name = syn::Ident::new(&name, Span::call_site());
//    let token = quote! {
//        #struct_def
//
//
//    };
//    Ok(token)
//}

//#[proc_macro]
///// Uses an html description to define a struct that you can use to communicate
///// asyncronously with a view from a logic loop, greatly reducing the amount
///// of boilerplate required to wire a view.
//pub fn relay(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
//    let tstream = match syn_rsx::parse(input) {
//        Ok(parsed) => {
//            let (tokens, errs) = partition_unzip(parsed.into_iter(), node_to_relay);
//            if let Some(error) = combine_errors(errs) {
//                error.to_compile_error().into()
//            } else {
//                match tokens.len() {
//                    0 => quote! {},
//                    1 => {
//                        let ts = &tokens[0];
//                        quote! { #ts }
//                    }
//                    _ => quote! { #(#tokens)* },
//                }
//            }
//        }
//        Err(error) => error.to_compile_error(),
//    };
//    proc_macro::TokenStream::from(tstream)
//}

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
