//! Contains parsing an RSX node into various data types.
use std::convert::TryFrom;

use proc_macro2::Span;
use quote::{format_ident, quote};
use syn::Error;
use syn_rsx::{Node, NodeType};

fn under_to_dash(s: &str) -> String {
    s.replace("_", "-")
}

#[derive(Clone, Debug)]
/// An enumeration of all supported attribute types.
pub enum AttributeToken {
    CastType(syn::Expr),
    PostBuild(syn::Expr),
    CaptureView(syn::Expr),
    Xmlns(syn::Expr),
    Style(syn::Expr),
    StyleSingle(String, syn::Expr),
    On(String, syn::Expr),
    Window(String, syn::Expr),
    Document(String, syn::Expr),
    BooleanSingle(String, syn::Expr),
    BooleanTrue(String),
    PatchChildren(syn::Expr),
    Attrib(String, syn::Expr),
    Other(String, syn::Expr),
}

impl TryFrom<syn_rsx::Node> for AttributeToken {
    type Error = syn::Error;

    fn try_from(node: syn_rsx::Node) -> Result<Self, Self::Error> {
        let span = node.name_span().unwrap_or(Span::call_site());
        if let Some(key) = node.name_as_string() {
            if let Some(expr) = node.value {
                match key.split(':').collect::<Vec<_>>().as_slice() {
                    ["cast", "type"] => Ok(AttributeToken::CastType(expr)),
                    ["post", "build"] => Ok(AttributeToken::PostBuild(expr)),
                    ["capture", "view"] => Ok(AttributeToken::CaptureView(expr)),
                    ["xmlns"] => Ok(AttributeToken::Xmlns(expr)),
                    ["style"] => Ok(AttributeToken::Style(expr)),
                    ["style", name] => {
                        let name = under_to_dash(name);
                        Ok(AttributeToken::StyleSingle(name, expr))
                    }
                    ["on", event] => Ok(AttributeToken::On(event.to_string(), expr)),
                    ["window", event] => Ok(AttributeToken::Window(event.to_string(), expr)),
                    ["document", event] => Ok(AttributeToken::Document(event.to_string(), expr)),
                    ["boolean", name] => {
                        let name = under_to_dash(name);
                        Ok(AttributeToken::BooleanSingle(name, expr))
                    }
                    ["patch", "children"] => Ok(AttributeToken::PatchChildren(expr)),
                    [attribute_name] => {
                        let name = under_to_dash(attribute_name);
                        Ok(AttributeToken::Attrib(name, expr))
                    }
                    keys => {
                        let name = under_to_dash(&keys.join(":"));
                        Ok(AttributeToken::Other(name, expr))
                    }
                }
            } else {
                let name = under_to_dash(&key);
                Ok(AttributeToken::BooleanTrue(name))
            }
        } else {
            Err(Error::new(span, "dom attribute is missing a name"))
        }
    }
}

impl AttributeToken {
    /// Attempt to create a token stream representing one link in a `ViewBuilder` chain.
    pub fn try_builder_token_stream(
        self: &AttributeToken,
    ) -> Result<proc_macro2::TokenStream, Error> {
        use AttributeToken::*;
        match self {
            CastType(_) => Ok(quote! {}), // handled by a preprocessor
            PostBuild(expr) => Ok(quote! {
                .with_post_build(#expr)
            }),
            CaptureView(expr) => Ok(quote! {
                .with_capture_view(#expr)
            }),
            Xmlns(expr) => Ok(quote! {
                .with_namespace(#expr)
            }),
            Style(expr) => Ok(quote! {
                .with_style_stream(#expr)
            }),
            StyleSingle(name, expr) => Ok(quote! {
                .with_single_style_stream(#name, #expr)
            }),
            On(name, expr) => Ok(quote! {
                .with_event(#name, #expr)
            }),
            Window(name, expr) => Ok(quote! {
                .with_window_event(#name, #expr)
            }),
            Document(name, expr) => Ok(quote! {
                .with_document_event(#name, #expr)
            }),
            BooleanSingle(name, expr) => Ok(quote! {
                .with_single_bool_attrib_stream(#name, #expr)
            }),
            PatchChildren(expr) => Ok(quote! {
                .with_child_stream(#expr)
            }),
            Attrib(name, expr) => Ok(quote! {
                .with_single_attrib_stream(#name, #expr)
            }),
            Other(name, expr) => Ok(quote! {
                .with_attrib_stream(#name, #expr)
            }),
            BooleanTrue(expr) => Ok(quote! {
                .with_single_bool_attrib_stream(#expr, true)
            }),
        }
    }
}

#[derive(Clone, Debug)]
/// An enumeration of all supported nodes types.
pub enum ViewToken {
    Element {
        name: String,
        name_span: proc_macro2::Span,
        attributes: Vec<AttributeToken>,
        children: Vec<ViewToken>,
    },
    Text(syn::Expr),
    Block(syn::Expr),
}

impl TryFrom<Node> for ViewToken {
    type Error = Error;

    fn try_from(node: Node) -> Result<Self, Self::Error> {
        let name_span = node.name_span().unwrap_or(Span::call_site());
        match &node.node_type {
            NodeType::Element => match node.name_as_string() {
                Some(tag) => {
                    let name = tag;

                    let mut attributes = vec![];
                    for attribute in node.attributes.into_iter() {
                        let token = AttributeToken::try_from(attribute)?;
                        attributes.push(token);
                    }

                    let mut children = vec![];
                    for child in node.children.into_iter() {
                        let token = ViewToken::try_from(child)?;
                        children.push(token);
                    }

                    Ok(ViewToken::Element {
                        name,
                        name_span,
                        attributes,
                        children,
                    })
                }
                None => Err(Error::new(
                    node.name_span().unwrap_or_else(|| Span::call_site()),
                    "View node is missing a name.",
                )),
            },
            NodeType::Text => {
                if let Some(val) = node.value {
                    Ok(ViewToken::Text(val))
                } else {
                    Err(Error::new(
                        node.name_span().unwrap_or(Span::call_site()),
                        "Text node is missing a value.",
                    ))
                }
            }
            NodeType::Block => {
                if let Some(val) = node.value {
                    Ok(ViewToken::Block(val))
                } else {
                    Err(Error::new(
                        node.name_span().unwrap_or(Span::call_site()),
                        "Block node is missing a value.",
                    ))
                }
            }
            _ => Err(Error::new(
                node.name_span().unwrap_or_else(|| Span::call_site()),
                "View node is missing a name.",
            )),
        }
    }
}

pub fn get_ident(expr: &syn::Expr, prefix: &str) -> Option<syn::Ident> {
    if let syn::Expr::Path(path) = expr {
        let id = path.path.get_ident()?;
        Some(syn::Ident::new(&format!("{}{}", prefix, id), id.span()))
    } else {
        None
    }
}

pub struct RelayInput {
    pub msg_type: syn::Expr,
    pub name: syn::Ident,
}

impl RelayInput {
    /// The name of this input as it appears in a relay struct field definition.
    pub fn struct_field_name(&self) -> syn::Ident {
        format_ident!("tx_{}", self.name)
    }
}

pub struct RelayOutput {
    pub msg_type: syn::Expr,
    pub name: syn::Ident,
}

impl RelayOutput {
    pub fn struct_field_name(&self) -> syn::Ident {
        format_ident!("{}_chan", self.name)
    }
}

pub enum RelayAttribute {
    Input(RelayInput),
    Output(RelayOutput),
}

impl TryFrom<Node> for RelayAttribute {
    type Error = syn::Error;

    fn try_from(node: Node) -> Result<Self, Self::Error> {
        let span = node.name_span().unwrap_or(Span::call_site());
        if let Some(key) = node.name_as_string() {
            if let Some(expr) = node.value {
                match key.split(':').collect::<Vec<_>>().as_slice() {
                    ["input", name] => Ok(RelayAttribute::Input(RelayInput {
                        name: syn::Ident::new(name, span),
                        msg_type: expr,
                    })),
                    ["output", name] => Ok(RelayAttribute::Output(RelayOutput {
                        name: syn::Ident::new(name, span),
                        msg_type: expr,
                    })),
                    _ => Err(Error::new(
                        span,
                        &format!("Unsupported relay attribute: {}", key),
                    )),
                }
            } else {
                Err(Error::new(
                    span,
                    &format!("Relay attribute must have a value: {}", key),
                ))
            }
        } else {
            Err(Error::new(span, "Relay attribute must have a key"))
        }
    }
}

impl RelayAttribute {
    /// Attempt to create a token that defines a single field in a named
    /// relay struct definition.
    pub fn struct_field_def(&self) -> proc_macro2::TokenStream {
        match self {
            RelayAttribute::Input(input) => {
                let tx_input = input.struct_field_name();
                let msg_type = &input.msg_type;
                quote! { #tx_input: mogwai::channel::broadcast::Sender<#msg_type>}
            }
            RelayAttribute::Output(output) => {
                let chan = output.struct_field_name();
                let msg_type = &output.msg_type;
                quote! { #chan: mogwai::channel::broadcast::Sender<#msg_type>}
            }
        }
    }
}

/// ```ignore
/// <MyViewRelay
///  input:set_text = String
///  output:get_click = DomEvent
///  />
/// ```
///
/// ```ignore
/// <MyViewRelay
///  input:set_text = broadcast::Channel<String>
///  output:get_click = broadcast::Channel<DomEvent> >
///
///     <div on:click = get_click.sender()>
///         {("Hello", set_text.receiver())}
///     </div>
///
/// </MyViewRelay>
/// ```
pub struct NamedRelay {
    pub name: String,
    pub attributes: Vec<RelayAttribute>,
    pub views: Vec<ViewToken>,
}

impl TryFrom<Node> for NamedRelay {
    type Error = syn::Error;

    fn try_from(node: Node) -> Result<Self, Self::Error> {
        let name_span = node.name_span().unwrap_or(Span::call_site());
        match &node.node_type {
            NodeType::Element => match node.name_as_string() {
                Some(tag) => {
                    let name = tag;

                    let mut attributes = vec![];
                    for attribute in node.attributes.into_iter() {
                        let token = RelayAttribute::try_from(attribute)?;
                        attributes.push(token);
                    }

                    let mut views = vec![];
                    for child in node.children.into_iter() {
                        let token = ViewToken::try_from(child)?;
                        views.push(token);
                    }

                    Ok(NamedRelay {
                        name,
                        attributes,
                        views,
                    })
                }
                None => Err(Error::new(
                    node.name_span().unwrap_or_else(|| Span::call_site()),
                    "Relay is missing a name.",
                )),
            },
            _ => Err(Error::new(
                name_span,
                "The top level node of a relay must be a named element.",
            )),
        }
    }
}

impl NamedRelay {
    /// Produces a struct definition.
    pub fn struct_def(&self) -> proc_macro2::TokenStream {
        let NamedRelay {
            name,
            attributes,
            views,
        } = self;
        let struct_field_defs = attributes.iter().map(RelayAttribute::struct_field_def);

        quote! {
            #[derive(Clone)]
            struct #name<T: mogwai::event::Eventable> {
                inner: mogwai::prelude::Either<mogwai::channel::broadcast::Receiver<T>, T>,
                #(#struct_field_defs,)*
            }
        }
    }

    ///// Produces an implementation definition.
    //pub fn implementation(&self) -> proc_macro2::TokenStream {
    //    let NamedRelay {
    //        name,
    //        attributes,
    //        views,
    //    } = self;

    //    quote! {
    //        impl<T> #name<T>
    //        where
    //            T: mogwai::event::Eventable + mogwai::target::Sendable + Clone + Unpin,
    //            <T as mogwai::event::Eventable>::Event: Clone + mogwai::target::Sendable + Unpin,
    //        {
    //            pub fn new() -> (#name<T>, ViewBuilder<T>) {
    //                use mogwai::futures::IntoSenderSink;

    //                let (tx_t, rx_t) = mogwai::channel::broadcast::bounded::<T>(1);
    //                #(#chans)*

    //                let builder = #builder;

    //                let view = #name {
    //                    inner: mogwai::prelude::Either::Left(rx_t),
    //                    #(#just_field_names,)*
    //                };

    //                (view, builder)
    //            }

    //            pub async fn get_inner(&mut self) -> Result<T, ()> {
    //                use mogwai::futures::StreamExt;
    //                let t = match self.inner.clone() {
    //                    mogwai::prelude::Either::Left(mut rx) => {
    //                        let t = rx.next().await.ok_or(())?;
    //                        self.inner = mogwai::prelude::Either::Right(t.clone());
    //                        t
    //                    }
    //                    mogwai::prelude::Either::Right(t) => t,
    //                };

    //                Ok(t)
    //            }

    //            #(#impl_fns)*
    //        }
    //    }
    //}
}
